"""Boot the full-heap candidate from a read-only USB volume in disposable Q35/OVMF guests."""
import argparse
import json
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import tempfile

from image import digest, make
from usb_observer import capture, compare, create

HERE = Path(__file__).resolve().parent
OPTICAL = runpy.run_path(str(HERE.parent / 'optical-disk-check/check.py'))


def usb_command(qemu, code, template, volume, out, ram):
    """Create the ordinary writable fixtures and attach the read-only removable volume."""
    serial, trace = out / 'serial.txt', out / 'firmware.txt'
    args = [qemu, '-machine', 'q35', '-accel', 'tcg', '-cpu', 'max', '-m', str(ram), '-smp', '1',
            '-S', '-no-shutdown', '-no-reboot', '-nic', 'none', '-display', 'none', '-vga', 'std',
            '-audiodev', 'none,id=snd', '-device', 'AC97,audiodev=snd', '-serial', 'file:' + str(serial),
            '-monitor', 'none', '-qmp', 'stdio', '-debugcon', 'file:' + str(trace),
            '-global', 'isa-debugcon.iobase=0xe9', '-boot', 'strict=on',
            '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={code}',
            '-device', 'qemu-xhci,id=xhci', '-drive', f'if=none,id=stick,format=raw,readonly=on,file={volume}',
            '-device', 'usb-storage,bus=xhci.0,id=usb,drive=stick,removable=on,bootindex=1']
    return args + create(out, template, OPTICAL['make_gpt_disk'])


def trial(qemu, code, template, volume, image, out, ram):
    out.mkdir()
    serial, trace = out / 'serial.txt', out / 'firmware.txt'
    args = usb_command(qemu, code, template, volume, out, ram)
    receipt = {'ram_mib': ram, 'argv': args, 'passed': False, 'snapshots': []}
    snapshots = receipt['snapshots']
    try:
        with (out / 'stderr.txt').open('wb') as err, subprocess.Popen(
                args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as guest:
            try:
                monitor = OPTICAL['Monitor'](guest)
                rpc = monitor.call
                rpc('qmp_capabilities')
                qblocks = rpc('query-block')
                receipt['blocks'] = qblocks
                media = next(x for x in qblocks if x.get('device') == 'stick')
                if not media['removable'] or not media['inserted']['ro'] or Path(media['inserted']['file']) != volume:
                    raise ValueError('expected the read-only removable USB fixture')
                snapshots.append(capture(rpc, out, 'before'))
                rpc('cont')
                OPTICAL['wait_for'](lambda: 'owned-handoff' in trace.read_text(errors='replace'),
                                    'waiting for firmware-owned handoff', 50)
                steps = trace.read_text().splitlines()
                expected = ['removable-vm', 'allocation=0000000000000000', 'owned-before-write',
                            'content-verified', 'exit_status=0000000000000000', 'owned-handoff']
                if [s for s in steps if s in expected] != expected:
                    raise ValueError('missing ownership or content read-back before transfer')
                screens = OPTICAL['DesktopScreens'](monitor, out, 60)
                screens.home('home')
                rpc('stop')
                startup = serial.read_text(errors='replace')
                boot_device = re.search(r'BdsDxe: starting Boot[0-9A-Fa-f]{4} [^\n]*USB[^\n]*', startup)
                if not boot_device or sum(line.startswith('  pass ') for line in startup.splitlines()) != OPTICAL['SCREEN']['STARTUP_CHECKS']:
                    raise ValueError('missing OVMF USB boot device or startup self-tests')
                receipt['firmware_boot_device'] = boot_device[0].strip()
                snapshots.append(capture(rpc, out, 'home'))
                rpc('cont')
                screens.verification('home')
                if (out / 'home-after-verification.ppm').read_bytes() != (out / 'home.ppm').read_bytes():
                    raise ValueError('Esc from Verification did not restore the original Home frame')
                monitor.key('f10')
                screens.wait('power', lambda v: v[1] == '  POWER OFF' and v[3] == '  Switch PCP OS off?'
                             and v[5] == '  The register is closed and verified first. Files of this session that were'
                             and v[6] == '  not saved by a program are lost.'
                             and v[-2] == '  Enter switch off    Esc go back')
                monitor.key('ret')
                OPTICAL['wait_for'](lambda: rpc('query-status')['status'] == 'shutdown', 'guest power off', 35)
                snapshots.append(capture(rpc, out, 'shutdown'))
                receipt['shutdown'] = next(e['data'] for e in monitor.events if e['event'] == 'SHUTDOWN')
                rpc('quit')
                receipt['exit'] = guest.wait(timeout=8)
            finally:
                if guest.poll() is None:
                    guest.kill()
                    guest.wait()
        log = serial.read_text(errors='replace')
        receipt['self_tests'] = sum(s.startswith('  pass ') for s in log.splitlines())
        receipt['intervals'] = [compare(out, a, b) for a, b in zip(snapshots, snapshots[1:])]
        receipt['usb_sha256_after'] = digest(volume)
        receipt['nvme_sha256_after_quit'] = digest(out / 'internal.raw')
        receipt['vars_sha256_after_quit'] = digest(out / 'vars.fd')
        if (receipt['nvme_sha256_after_quit'] != snapshots[-1]['nvme_sha256']
                or receipt['vars_sha256_after_quit'] != snapshots[-1]['vars_sha256']):
            raise ValueError('backing files changed between guest shutdown and QEMU exit')
        if (image['measurement'] + ' (measured; equal to the value the loader named)' not in log
                or receipt['self_tests'] != OPTICAL['SCREEN']['STARTUP_CHECKS'] + 1 or 'ALL SELF-TESTS PASS.' not in log
                or receipt['exit'] != 0 or receipt['shutdown'] != {'guest': True, 'reason': 'guest-shutdown'}
                or receipt['usb_sha256_after'] != image['usb_sha256']):
            raise ValueError('boot measurement, final self-tests, medium or shutdown disagreed')
        for interval in receipt['intervals']:
            disk = interval['counter_deltas']['sentinel']
            if not interval['nvme_bytes_preserved'] or any(disk[k] for k in
                    ('wr_operations', 'wr_bytes', 'invalid_wr_operations', 'failed_wr_operations', 'unmap_operations',
                     'failed_unmap_operations', 'invalid_unmap_operations')):
                raise ValueError('internal disk changed or saw a write or discard request')
        if receipt['intervals'][-1]['vars_bytes_changed'] or receipt['intervals'][-1]['counter_deltas']['varstore']['wr_operations']:
            raise ValueError('firmware variables were written after Home')
        receipt['passed'] = True
    finally:
        (out / 'result.json').write_text(json.dumps(receipt, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('distribution_iso', type=Path)
    parser.add_argument('relocated_kernel', type=Path)
    parser.add_argument('evidence', type=Path)
    parser.add_argument('--qemu', default='qemu-system-x86_64')
    parser.add_argument('--ovmf', type=Path, default=Path('/usr/share/OVMF'))
    args = parser.parse_args()
    out = args.evidence.resolve()
    out.mkdir(parents=True, exist_ok=False)
    code, template = (args.ovmf.resolve() / n for n in ('OVMF_CODE_4M.fd', 'OVMF_VARS_4M.fd'))
    measure = HERE.parents[1] / 'PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py'
    with tempfile.TemporaryDirectory() as folder:
        work = Path(folder)
        image = make(args.distribution_iso.resolve(), args.relocated_kernel.resolve(), measure, work, HERE, usb_vm=True)
        image['ovmf_code_sha256'], image['ovmf_vars_sha256'] = digest(code), digest(template)
        image['experiment_commit'] = subprocess.check_output(['git', '-C', str(HERE), 'rev-parse', 'HEAD'], text=True).strip()
        image['experiment_tree'] = subprocess.check_output(['git', '-C', str(HERE), 'rev-parse', 'HEAD^{tree}'], text=True).strip()
        image['experiment_dirty'] = bool(subprocess.check_output(
            ['git', '-C', str(HERE), 'status', '--porcelain', '--', '.'], text=True))
        image['qemu_version'] = subprocess.check_output([args.qemu, '--version'], text=True).splitlines()[0]
        (out / 'image.json').write_text(json.dumps(image, indent=2) + '\n')
        volume = out / 'usb.img'
        shutil.copyfile(work / 'usb.img', volume)
        for ram in (128, 256):
            trial(args.qemu, code, template, volume, image, out / str(ram), ram)


if __name__ == '__main__':
    main()
