#!/usr/bin/env python3
"""Exercise a PCP OS optical image with a writable, disposable IDE or NVMe disk.

Requires Python 3, qemu-system-x86_64 with SeaBIOS.
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
import uuid
import zlib

PARTITION_GUID = uuid.UUID('32802589-a96c-4a24-8fa4-de349d70cb05')
MARKER = b"PCP OS disk-preservation test. Keep this file across cold boots.\r\n"
KERNELS = {
    'published': '0232c02a8946d66216da0ac65953a246ce3d41d7a966ae31aa83c3335b3259e3',
    'keyboard-01': 'a9be6b1599bb10eead1344ae4a2ca53ec978074ae9aef1d30e8da10f04d044f3',
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_fat16(disk, sector_bytes, first, volume_sectors, fat_sectors):
    """Write the FAT16 volume and return the byte offset of KEEP.TXT."""
    start = first * sector_bytes
    root = start + (1 + 2 * fat_sectors) * sector_bytes
    data = root + 512 * 32  # 512 directory entries
    disk[start:data] = bytes(data - start)
    disk[start:start + 11] = b'\xeb\x3c\x90PCPTEST '
    struct.pack_into('<HBHBHHBHHHII', disk, start + 11,
                     sector_bytes, 1, 1, 2, 512, volume_sectors, 0xf8, fat_sectors, 63, 255, first, 0)
    struct.pack_into('<BBBI11s8s', disk, start + 36, 0x80, 0, 0x29,
                     0x42534431, b'PCP-DISK   ', b'FAT16   ')
    disk[start + 510:start + 512] = b'\x55\xaa'
    for sector in (1, 1 + fat_sectors):
        struct.pack_into('<HHH', disk, start + sector * sector_bytes, 0xfff8, 0xffff, 0xffff)
    disk[root:root + 11] = b'KEEP    TXT'
    disk[root + 11] = 0x20
    struct.pack_into('<HI', disk, root + 26, 2, len(MARKER))
    disk[data:data + len(MARKER)] = MARKER
    return data


def make_disk():
    """Build the original 512-byte MBR fixture."""
    first, volume = 2048, 16384
    disk = bytearray(b'\xa5' * ((first + volume) * 512))
    disk[:first * 512] = bytes(first * 512)
    struct.pack_into('<B3sB3sII', disk, 446, 0, b'\xfe\xff\xff', 6,
                     b'\xfe\xff\xff', first, volume)
    disk[510:512] = b'\x55\xaa'
    marker = write_fat16(disk, 512, first, volume, 64)
    return bytes(disk), marker


def make_gpt_disk(sector_bytes):
    """Place a FAT16 partition between independent primary and backup GPTs."""
    first = 1048576 // sector_bytes
    volume, fat = (16384, 64) if sector_bytes == 512 else (8192, 4)
    end = first + volume  # exclusive partition end, in logical sectors
    sectors = end + first
    disk = bytearray(sectors * sector_bytes)
    disk[first * sector_bytes:end * sector_bytes] = b'\xa5' * (volume * sector_bytes)
    struct.pack_into('<B3sB3sII', disk, 446, 0, b'\x00\x02\x00', 0xee, b'\xff' * 3, 1, sectors - 1)
    disk[510:512] = b'\x55\xaa'

    entries = bytearray(128 * 128)
    table_sectors = len(entries) // sector_bytes
    basic_data = uuid.UUID('ebd0a0a2-b9e5-4433-87c0-68b6b72699c7').bytes_le
    disk_guid = uuid.UUID('5f21da79-2bac-480a-829f-ba245a8a4529').bytes_le
    struct.pack_into('<16s16sQQQ72s', entries, 0, basic_data, PARTITION_GUID.bytes_le,
                     first, end - 1, 0, 'PCP disk check'.encode('utf-16-le'))
    last = sectors - 1
    for here, peer, table in ((1, last, 2), (last, 1, last - table_sectors)):
        header = bytearray(92)
        struct.pack_into('<8sIIIIQQQQ16sQIII', header, 0, b'EFI PART', 0x10000, len(header),
                         0, 0, here, peer, 2 + table_sectors, last - table_sectors - 1,
                         disk_guid, table, 128, 128, zlib.crc32(entries))
        struct.pack_into('<I', header, 16, zlib.crc32(header))
        disk[here * sector_bytes:here * sector_bytes + len(header)] = header
        disk[table * sector_bytes:table * sector_bytes + len(entries)] = entries
    marker = write_fat16(disk, sector_bytes, first, volume, fat)
    return bytes(disk), marker


def check_gpt_fixture(path, sector_bytes):
    # partx reports start in 512-byte units even for a 4096-byte logical sector.
    row = subprocess.check_output(['partx', '--show', '--noheadings', '--bytes',
                                   '--sector-size', str(sector_bytes), '--output',
                                   'START,SIZE,UUID', str(path)], text=True).split()
    size = 8388608 if sector_bytes == 512 else 33554432
    if row != ['2048', str(size), str(PARTITION_GUID)]:
        raise ValueError(f'Unexpected partition from partx: {row}')
    filesystem = subprocess.check_output(['blkid', '-p', '-O', '1048576', '-s', 'TYPE',
                                          '-o', 'value', str(path)], text=True).strip()
    if filesystem != 'vfat':
        raise ValueError(f'Unexpected filesystem from blkid: {filesystem}')


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


# Exact RGB regions from the kernel's 1024x768 framebuffer. The launcher approval
# row also appears in the independently recorded Windows/macOS optical boots.
VERIFIED = (180, 98, 642, 120, '100fc821e22cf5bda99d4f96cf1d4d442deb15cd3f3a2869cc30e5e7cc33971c')
LAUNCHER_KEYS = (12, 714, 420, 744, '216cbff2e9413ccca07850193195f1f3e25391f775343c85f66f33dbb8c5f80b')
REGISTER_KEYS = (12, 714, 420, 744, '123e5fedc122fb7016cce09b51260948f16476b2be1807fdf1d6993e489dbae8')


def matches_frame(raw, regions):
    header = b'P6\n1024 768\n255\n'
    if not raw.startswith(header) or len(raw) != len(header) + 1024 * 768 * 3:
        return False
    pixels = memoryview(raw)[len(header):]
    for left, top, right, bottom, expected in regions:
        h = hashlib.sha256()
        for y in range(top, bottom):
            h.update(pixels[(1024 * y + left) * 3:(1024 * y + right) * 3])
        if h.hexdigest() != expected:
            return False
    return True


def frame(monitor, directory, name, timeout, register=False):
    capture = directory / (name + '.ppm')
    regions = (REGISTER_KEYS,) if register else (VERIFIED, LAUNCHER_KEYS)

    def inspect():
        monitor.call('screendump', filename=str(capture))
        return matches_frame(capture.read_bytes(), regions)

    wait_for(inspect, f'waiting for {name} framebuffer', timeout)


def exercise(q, evidence, timeout, result, kernel):
    serial = evidence / 'serial.log'

    def sees(text, count=1):
        wait_for(lambda: serial.read_text(errors='replace').count(text) >= count, text, timeout)

    q.call('cont')
    sees(kernel + ' (measured; equal to the value the loader named)')
    frame(q, evidence, 'launcher', timeout)
    if sum(line.startswith('  pass ') for line in serial.read_text().splitlines()) != 71:
        raise ValueError('expected 71 serial startup passes')
    result['at_launcher'] = q.disk_stats()
    q.key('ret')
    sees('SURFBIRD media ready')
    q.key('spc')
    sees('SURFBIRD media drained')
    sees('surf.elf ended DONE: exit 0')
    frame(q, evidence, 'after-playback', timeout)
    q.key('ret')
    sees('SURFBIRD media ready', 2)
    q.key('ctrl', 'alt', 'delete')
    sees('surf.elf ended STOPPED')
    frame(q, evidence, 'after-stop', timeout)
    q.key('r')
    frame(q, evidence, 'register', timeout, register=True)
    q.key('esc')
    frame(q, evidence, 'returned', timeout)
    result['before_poweroff'] = q.disk_stats()
    q.key('p')
    sees("the kernel's register of its own start, verification and stop verifies and replays without a problem")
    sees('ALL SELF-TESTS PASS.')
    wait_for(lambda: q.call('query-status')['status'] == 'shutdown', 'guest shutdown', timeout)
    if not any(e['event'] == 'SHUTDOWN' and e.get('data', {}).get('guest') is True for e in q.events):
        raise RuntimeError('QEMU did not observe guest-requested shutdown')


def run(args, root, baseline, marker_offset, machine, control):
    case = machine + ('-write-control' if control else '-session')
    evidence = args.output / case
    evidence.mkdir()
    disk = root / (case + '.raw')
    disk.write_bytes(baseline)
    sector_bytes = args.nvme_sector_size or 512
    if args.nvme_sector_size:
        check_gpt_fixture(disk, sector_bytes)
        attachment_args = ['-drive', 'id=sentinel,if=none,format=raw,snapshot=off,file=' + str(disk),
                           '-device', f'nvme,id=nvme0,serial=PCPTEST,drive=sentinel,'
                                      f'logical_block_size={sector_bytes},physical_block_size={sector_bytes}']
    else:
        attachment_args = ['-drive', 'id=sentinel,if=ide,index=0,format=raw,snapshot=off,file=' + str(disk)]
    serial = evidence / 'serial.log'
    profile = (['-cpu', 'qemu64', '-m', '64M'] if args.tester_profile else
               ['-cpu', 'max', '-m', '256M', '-icount', 'shift=3,align=off,sleep=off'])
    cmd = [args.qemu, '-machine', machine, '-accel', 'tcg', *profile,
           '-smp', '1', '-nic', 'none', '-display', 'none', '-qmp', 'stdio',
           '-audiodev', 'none,id=snd',
           '-device', 'AC97,audiodev=snd', '-serial', 'file:' + str(serial),
           '-boot', 'order=d,strict=on', '-no-shutdown', '-S',
           *attachment_args,
           '-cdrom', str(args.iso), '-trace', 'enable=blk_co_pwritev',
           '-trace', 'enable=ide_sector_write']
    if args.firmware:
        cmd += ['-L', str(args.firmware)]
    result = {'case': case, 'iso_sha256': args.sha256, 'before_sha256': digest(baseline),
              'qemu': args.qemu_version, 'argv': cmd, 'logical_sector_bytes': sector_bytes}
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
                command = f'qemu-io sentinel "write -q -P 0x5a {marker_offset} {sector_bytes}"'
                result['write_response'] = q.call('human-monitor-command', **{'command-line': command})
            else:
                exercise(q, evidence, args.timeout, result, KERNELS[args.kernel])
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
        expected[marker_offset:marker_offset + sector_bytes] = b'\x5a' * sector_bytes
        result['matched_expectation'] = actual == expected and actual != baseline
    else:
        # Require positive read activity as well as the expected writes on the named disk.
        end = result['at_end']
        no_writes = (0, 0, 0, 0, 0)
        expected = (1, sector_bytes, sector_bytes, 0, 0) if args.expect_writeback else no_writes
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
    p.add_argument('--kernel', choices=KERNELS, default='published', help='expected measured kernel')
    p.add_argument('--tester-profile', action='store_true',
                   help='use qemu64, 64 MiB and the ordinary QEMU clock')
    p.add_argument('--nvme-sector-size', choices=(512, 4096), type=int,
                   help='attach a GPT/FAT16 NVMe disk to Q35; requires partx and blkid')
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
    baseline, marker_offset = make_gpt_disk(args.nvme_sector_size) if args.nvme_sector_size else make_disk()
    machines = ('q35',) if args.nvme_sector_size else ('pc', 'q35')
    checks = []
    with tempfile.TemporaryDirectory(prefix='pcpos-disk-') as temp:
        for machine in machines:
            for control in (True, False):
                try:
                    checks.append(run(args, Path(temp), baseline, marker_offset, machine, control))
                except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
                    print(f"{machine} control={control}: {error}", flush=True)
                    checks.append(False)
    return 0 if all(checks) else 1


if __name__ == '__main__':
    raise SystemExit(main())
