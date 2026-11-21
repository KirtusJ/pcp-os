#!/usr/bin/env python3
"""Exercise a published PCP OS optical image with a writable, disposable IDE disk.

Requires Python 3, qemu-system-x86_64 with SeaBIOS, and tesseract.
The only disk this program writes is a new ordinary file in a temporary directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import struct
import subprocess
import tempfile
import time

SECTOR = 512
FIRST = 2048
VOLUME = 16384
MARKER = b"PCP OS disk-preservation test. Keep this file across cold boots.\r\n"
FAT_SECTORS = 64
ROOT_SECTORS = 32
DATA_SECTOR = 1 + 2 * FAT_SECTORS + ROOT_SECTORS
BLOCK_OFFSET = (FIRST + DATA_SECTOR) * SECTOR
KERNEL = '0232c02a8946d66216da0ac65953a246ce3d41d7a966ae31aa83c3335b3259e3'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def make_disk():
    """Build an MBR with a FAT16 primary partition containing KEEP.TXT."""
    disk = bytearray(b'\xa5' * ((FIRST + VOLUME) * SECTOR))
    disk[:FIRST * SECTOR] = bytes(FIRST * SECTOR)
    struct.pack_into('<B3sB3sII', disk, 446, 0, b'\xfe\xff\xff', 6,
                     b'\xfe\xff\xff', FIRST, VOLUME)
    disk[510:512] = b'\x55\xaa'
    start = FIRST * SECTOR
    disk[start:start + DATA_SECTOR * SECTOR] = bytes(DATA_SECTOR * SECTOR)
    disk[start:start + 11] = b'\xeb\x3c\x90PCPTEST '
    struct.pack_into('<HBHBHHBHHHII', disk, start + 11,
                     SECTOR, 1, 1, 2, 512, VOLUME, 0xf8, FAT_SECTORS, 63, 255, FIRST, 0)
    struct.pack_into('<BBB I 11s 8s', disk, start + 36, 0x80, 0, 0x29,
                     0x42534431, b'PCP-DISK   ', b'FAT16   ')
    disk[start + 510:start + 512] = b'\x55\xaa'
    for sector in (1, 1 + FAT_SECTORS):
        struct.pack_into('<HHH', disk, start + sector * SECTOR, 0xfff8, 0xffff, 0xffff)
    root = start + (1 + 2 * FAT_SECTORS) * SECTOR
    disk[root:root + 11] = b'KEEP    TXT'
    disk[root + 11] = 0x20
    struct.pack_into('<HI', disk, root + 26, 2, len(MARKER))
    data = start + DATA_SECTOR * SECTOR
    disk[data:data + len(MARKER)] = MARKER
    return bytes(disk)


class Monitor:
    def __init__(self, process):
        self.process = process
        self.pending = b''
        self.events = []
        if 'QMP' not in self.read():
            raise RuntimeError('missing QMP greeting')

    def read(self):
        while b'\n' not in self.pending:
            if not select.select([self.process.stdout], [], [], 8)[0]:
                raise TimeoutError("QMP did not respond")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError("QEMU closed QMP")
            self.pending += chunk
        line, self.pending = self.pending.split(b'\n', 1)
        return json.loads(line)

    def call(self, name, **kwargs):
        p = self.process
        msg = json.dumps({'execute': name, 'arguments': kwargs}).encode()
        p.stdin.write(msg + b'\n')
        p.stdin.flush()
        while True:
            reply = self.read()
            if 'event' in reply:
                self.events.append(reply)
            elif 'return' in reply:
                return reply['return']
            else:
                raise RuntimeError(f"{name}: {reply}")

    def key(self, *keys):
        events = [{'type': 'qcode', 'data': k} for k in keys]
        self.call('send-key', keys=events, **{'hold-time': 70})

    def disk_stats(self):
        rows = self.call('query-blockstats')
        row = next(d['stats'] for d in rows if d.get('device') == 'sentinel')
        return {k: row[k] for k in ('wr_operations', 'wr_bytes', 'failed_wr_operations',
                                    'invalid_wr_operations', 'rd_operations',
                                    'rd_bytes', 'wr_highest_offset')}


def wait_for(test, description, timeout):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        result = test()
        if result:
            return result
        time.sleep(.25)
    raise TimeoutError(description)


def frame(monitor, directory, name, title, timeout):
    capture = directory / (name + '.ppm')
    def inspect():
        monitor.call('screendump', filename=str(capture))
        ocr = subprocess.run(['tesseract', str(capture), 'stdout'], capture_output=True,
                             check=True, text=True, timeout=10).stdout
        return ocr if title.lower() in ocr.lower() else None
    text = wait_for(inspect, f"waiting for {name} screen containing {title!r}", timeout)
    (directory / (name + '.txt')).write_text(text)
    return text


def exercise(q, evidence, timeout, result):
    serial = evidence / 'serial.log'

    def sees(text, count=1):
        wait_for(lambda: serial.read_text(errors='replace').count(text) >= count, text, timeout)

    q.call('cont')
    launcher = frame(q, evidence, 'launcher', 'FILES FROM THE LOADER', timeout)
    if 'all 71 self-tests passed' not in launcher.lower():
        raise ValueError(f'unexpected startup screen: {launcher}')
    sees(KERNEL + ' (measured; equal to the value the loader named)')
    result['at_launcher'] = q.disk_stats()
    q.key('ret')
    sees('SURFBIRD media ready')
    q.key('spc')
    sees('SURFBIRD media drained')
    sees('surf.elf ended DONE: exit 0')
    frame(q, evidence, 'after-playback', 'FILES FROM THE LOADER', timeout)
    q.key('ret')
    sees('SURFBIRD media ready', 2)
    q.key('ctrl', 'alt', 'delete')
    sees('surf.elf ended STOPPED')
    frame(q, evidence, 'after-stop', 'FILES FROM THE LOADER', timeout)
    q.key('r')
    frame(q, evidence, 'register', 'Esc back', timeout)
    q.key('esc')
    frame(q, evidence, 'returned', 'FILES FROM THE LOADER', timeout)
    result['before_poweroff'] = q.disk_stats()
    q.key('p')
    sees("the kernel's register of its own start, verification and stop verifies and replays without a problem")
    sees('ALL SELF-TESTS PASS.')
    wait_for(lambda: q.call('query-status')['status'] == 'shutdown', 'guest shutdown', timeout)
    if not any(e['event'] == 'SHUTDOWN' and e.get('data', {}).get('guest') is True for e in q.events):
        raise RuntimeError('QEMU did not observe guest-requested shutdown')


def run(args, root, baseline, machine, control):
    case = machine + ('-write-control' if control else '-session')
    evidence = args.output / case
    evidence.mkdir()
    disk = root / (case + '.raw')
    disk.write_bytes(baseline)
    serial = evidence / 'serial.log'
    cmd = [args.qemu, '-machine', machine, '-accel', 'tcg', '-cpu', 'max', '-m', '256M',
           '-smp', '1', '-nic', 'none', '-display', 'none', '-qmp', 'stdio',
           '-icount', 'shift=3,align=off,sleep=off', '-audiodev', 'none,id=snd',
           '-device', 'AC97,audiodev=snd', '-serial', 'file:' + str(serial),
           '-boot', 'order=d,strict=on', '-no-shutdown', '-S',
           '-drive', 'id=sentinel,if=ide,index=0,format=raw,snapshot=off,file=' + str(disk),
           '-cdrom', str(args.iso), '-trace', 'enable=blk_co_pwritev',
           '-trace', 'enable=ide_sector_write']
    if args.firmware:
        cmd += ['-L', str(args.firmware)]
    result = {'case': case, 'iso_sha256': args.sha256, 'before_sha256': digest(baseline),
              'qemu': args.qemu_version, 'argv': cmd}
    with (evidence / 'qemu.stderr').open('wb') as err:
        vm = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
        try:
            q = Monitor(vm)
            q.call('qmp_capabilities')
            attachment = next(d['inserted'] for d in q.call('query-block')
                              if d.get('device') == 'sentinel')
            if attachment['ro'] or attachment['drv'] != 'raw' or Path(attachment['file']) != disk:
                raise ValueError(f'unexpected disk attachment: {attachment}')
            result['before_cpu_start'] = q.disk_stats()
            if control:
                command = f'qemu-io sentinel "write -q -P 0x5a {BLOCK_OFFSET} 512"'
                result['write_response'] = q.call('human-monitor-command', **{'command-line': command})
            else:
                exercise(q, evidence, args.timeout, result)
            result['at_end'] = q.disk_stats()
            result['events'] = q.events
            q.call('quit')
            if vm.wait(timeout=8):
                raise RuntimeError('QEMU exit status was nonzero')
        finally:
            if vm.poll() is None:
                vm.kill()
                vm.wait()

    actual = disk.read_bytes()
    result['after_sha256'] = digest(actual)
    result['disk_preserved'] = actual == baseline
    if control:
        expected = bytearray(baseline)
        expected[BLOCK_OFFSET:BLOCK_OFFSET + SECTOR] = b'\x5a' * SECTOR
        result['matched_expectation'] = actual == expected and actual != baseline
    else:
        # Require positive read activity as well as the expected writes on the named disk.
        end = result['at_end']
        no_writes = (0, 0, 0, 0, 0)
        expected = (1, 512, 512, 0, 0) if args.expect_writeback else no_writes
        fields = ('wr_operations', 'wr_bytes', 'wr_highest_offset',
                  'failed_wr_operations', 'invalid_wr_operations')
        def writes(stage):
            return tuple(result[stage][k] for k in fields)
        result['zero_guest_writes'] = writes('at_end') == no_writes
        result['matched_expectation'] = (result['disk_preserved'] and end['rd_bytes'] > 0 and
            writes('before_cpu_start') == no_writes and
            all(writes(stage) == expected for stage in ('at_launcher', 'before_poweroff', 'at_end')))
    (evidence / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(f"{case}: matched_expectation={result['matched_expectation']}, "
          f"disk_preserved={result['disk_preserved']}, guest write operations={result['at_end']['wr_operations']}",
          flush=True)
    return result['matched_expectation']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('iso', type=Path)
    p.add_argument('output', type=Path, help='new evidence directory')
    p.add_argument('--sha256', required=True, help='expected SHA-256 obtained from a trusted reference')
    p.add_argument('--qemu', default='qemu-system-x86_64')
    p.add_argument('--firmware', type=Path)
    p.add_argument('--timeout', type=float, default=35)
    p.add_argument('--expect-writeback', action='store_true',
                   help='old-loader control: require one successful LBA-zero sector write')
    args = p.parse_args()
    args.qemu_version = subprocess.check_output([args.qemu, '--version'], text=True).splitlines()[0]
    args.iso = args.iso.resolve(strict=True)
    if not args.iso.is_file() or digest(args.iso.read_bytes()) != args.sha256:
        p.error("ISO must be a regular file matching the expected SHA-256")
    args.output = args.output.resolve()
    args.output.mkdir()
    baseline = make_disk()
    checks = []
    with tempfile.TemporaryDirectory(prefix='pcpos-disk-') as temp:
        for machine in ('pc', 'q35'):
            for control in (True, False):
                try:
                    checks.append(run(args, Path(temp), baseline, machine, control))
                except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
                    print(f"{machine} control={control}: {error}", flush=True)
                    checks.append(False)
    return 0 if all(checks) else 1


if __name__ == '__main__':
    raise SystemExit(main())
