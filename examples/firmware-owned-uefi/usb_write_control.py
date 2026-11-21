"""Boot a separate QEMU/OVMF diagnostic USB and validate both writable disposable fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from image import digest
from usb_observer import capture, compare
from usb_trial import OPTICAL, usb_command


HERE = Path(__file__).resolve().parent
VARIABLE = '86a1bff3-4b7c-4860-99da-1056de89abaa/PcpUsbObserverControl'
VALUE = b'vm-scratch-marker-v1'


def require(ok, message):
    if not ok:
        raise ValueError(message)


def build_usb(out, prefix):
    lib, inc = prefix / 'lib', prefix / 'include/efi'
    def run(*args):
        subprocess.run(args, cwd=out, check=True)
    run('gcc', '-Wall', '-Wextra', '-Werror', '-fpic', '-fshort-wchar', '-fno-stack-protector',
        '-mno-red-zone', '-DEFI_FUNCTION_WRAPPER', '-I' + str(inc), '-I' + str(inc / 'x86_64'),
        '-c', HERE / 'usb_write_control.c', '-o', 'control.o')
    run('ld', '-nostdlib', '-znocombreloc', '-T', lib / 'elf_x86_64_efi.lds', '-shared', '-Bsymbolic',
        lib / 'crt0-efi-x86_64.o', 'control.o', '-L' + str(lib), '-lefi', '-lgnuefi', '-o', 'control.so')
    sections = ['.text', '.rodata', '.sdata', '.data', '.dynamic', '.dynsym', '.rel', '.rela', '.reloc']
    run('objcopy', *[arg for section in sections for arg in ('-j', section)], '--target=efi-app-x86_64',
        'control.so', 'BOOTX64.EFI')
    usb = out / 'positive.raw'
    with usb.open('xb') as disk:
        disk.truncate(8 << 20)
    run('mformat', '-i', usb, '::')
    run('mmd', '-i', usb, '::/EFI', '::/EFI/BOOT')
    run('mcopy', '-i', usb, 'BOOTX64.EFI', '::/EFI/BOOT/BOOTX64.EFI')
    return usb


def wait_marker(path, expected):
    def reached():
        text = path.read_text(errors='replace')
        if 'failed' in text:
            raise RuntimeError(text)
        return expected in text.splitlines()
    OPTICAL['wait_for'](reached, expected, 45)


def check_changes(stages, changes, baseline, marker_at, marker, disk):
    boot, control, shutdown = changes
    require(boot['nvme_bytes_preserved'] and shutdown['nvme_bytes_preserved'], 'NVMe changed outside control')
    stats = control['counter_deltas']['sentinel']
    require(stats['rd_operations'] > 0 and stats['rd_bytes'] > 0 and stats['wr_operations'] > 0
            and stats['wr_bytes'] >= 512 and not stats['failed_wr_operations'] and not stats['invalid_wr_operations'],
            'the known file write was not visible in the named NVMe backend')
    require(len(disk) == len(baseline) and disk[marker_at:marker_at + len(marker)] == b'p' + marker[1:],
            'unexpected file data after the control')
    directory_at = baseline.index(b'KEEP    TXT')
    # FAT16 permits updating access date and last-write time/date on the existing directory entry.
    allowed = {marker_at} | {directory_at + n for n in (18, 19, 22, 23, 24, 25)}
    changed = [i for i, (a, b) in enumerate(zip(baseline, disk)) if a != b]
    require(marker_at in changed and set(changed) <= allowed, 'unexpected change outside the file and its timestamps')
    require(control['variables_added'] == [VARIABLE] and not control['variables_removed']
            and not control['variables_modified'] and control['vars_bytes_changed'] > 0,
            'firmware store did not record the one isolated diagnostic variable')
    expected = {'attributes': 7, 'bytes': len(VALUE), 'sha256': hashlib.sha256(VALUE).hexdigest()}
    require(stages[2]['variables'][VARIABLE] == expected and shutdown['vars_bytes_changed'] == 0,
            'diagnostic firmware variable differs or changed on shutdown')
    return changed


def trial(args, out, usb):
    code = args.ovmf / 'OVMF_CODE_4M.fd'
    template = args.ovmf / 'OVMF_VARS_4M.fd'
    cmd = usb_command(args.qemu, code, template, usb, out, 256)
    debug = out / 'firmware.txt'
    baseline, marker_at = OPTICAL['make_gpt_disk'](512)
    (out / 'qemu-command.json').write_text(json.dumps(cmd, indent=2) + '\n')
    with (out / 'stderr.txt').open('wb') as err, subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as guest:
        try:
            q = OPTICAL['Monitor'](guest)
            q.call('qmp_capabilities')
            require(q.call('qom-get', path='/machine/peripheral/internal', property='serial') == 'PCP-USB-INTERNAL',
                    'unexpected NVMe serial')
            stages = [capture(q.call, out, 'prelaunch')]
            q.call('cont')
            for name, marker in (('control-ready', 'CONTROL ready'), ('control-written', 'CONTROL verified')):
                wait_marker(debug, marker)
                q.call('stop')
                stages.append(capture(q.call, out, name))
                q.call('cont')
                q.key('ret')
            OPTICAL['wait_for'](lambda: q.call('query-status')['status'] == 'shutdown', 'guest shutdown', 10)
            stages.append(capture(q.call, out, 'guest-shutdown'))
            require(any(e['event'] == 'SHUTDOWN' and e['data'] == {'guest': True, 'reason': 'guest-shutdown'}
                        for e in q.events), 'guest-requested shutdown absent')
            q.call('quit')
            require(guest.wait(timeout=8) == 0, 'QEMU exited unsuccessfully')
        finally:
            if guest.poll() is None:
                guest.kill()
                guest.wait()
    changes = [compare(out, a, b) for a, b in zip(stages, stages[1:])]
    offsets = check_changes(stages, changes, baseline, marker_at, OPTICAL['MARKER'], (out / 'internal.raw').read_bytes())
    receipt = {'passed': True, 'stages': stages, 'changes': changes, 'events': q.events,
               'changed_disk_offsets': offsets, 'fixture_sha256': hashlib.sha256(baseline).hexdigest(),
               'usb_sha256': digest(usb), 'efi_sha256': digest(usb.parent / 'BOOTX64.EFI'),
               'ovmf_code_sha256': digest(code), 'ovmf_template_sha256': digest(template)}
    (out / 'result.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(f"Positive control passed: {len(offsets)} changed NVMe bytes; {changes[1]['vars_bytes_changed']} changed VARS bytes")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('evidence', type=Path, help='new directory for this isolated control')
    p.add_argument('--qemu', default='qemu-system-x86_64')
    p.add_argument('--ovmf', type=Path, default=Path('/usr/share/OVMF'))
    p.add_argument('--efi-prefix', type=Path, default=Path('/usr'), help='GNU-EFI include and library prefix')
    args = p.parse_args()
    args.ovmf = args.ovmf.resolve()
    out = args.evidence.resolve()
    out.mkdir(parents=True, exist_ok=False)
    # Keep the separate write-capable control image out of both the product and the observations.
    with tempfile.TemporaryDirectory() as tmp:
        trial(args, out, build_usb(Path(tmp), args.efi_prefix.resolve()))


if __name__ == '__main__':
    main()
