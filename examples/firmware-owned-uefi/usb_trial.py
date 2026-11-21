"""Boot the full-heap candidate from a read-only USB volume in disposable Q35/OVMF guests."""
import argparse
import json
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import tempfile
import time

from image import digest, make
from usb_observer import capture, compare, create
from refusal_screen import matches as matches_refusal

HERE = Path(__file__).resolve().parent
OPTICAL = runpy.run_path(str(HERE.parent / 'optical-disk-check/check.py'))


def usb_command(qemu, code, template, volume, out, ram, video='std'):
    """Create the ordinary writable fixtures and attach the read-only removable volume."""
    serial, trace = out / 'serial.txt', out / 'firmware.txt'
    graphics = {
        'std': ['-vga', 'std'],
        'virtio-vga': ['-vga', 'none', '-device', 'virtio-vga'],
        'ramfb': ['-vga', 'none', '-device', 'ramfb'],
    }[video]
    args = [qemu, '-machine', 'q35', '-accel', 'tcg', '-cpu', 'max', '-m', str(ram), '-smp', '1',
            '-S', '-no-shutdown', '-no-reboot', '-nic', 'none', '-display', 'none', *graphics,
            '-audiodev', 'none,id=snd', '-device', 'AC97,audiodev=snd', '-serial', 'file:' + str(serial),
            '-monitor', 'none', '-qmp', 'stdio', '-debugcon', 'file:' + str(trace),
            '-global', 'isa-debugcon.iobase=0xe9', '-boot', 'strict=on',
            '-drive', f'if=pflash,unit=0,format=raw,readonly=on,file={code}',
            '-device', 'qemu-xhci,id=xhci', '-drive', f'if=none,id=stick,format=raw,readonly=on,file={volume}',
            '-device', 'usb-storage,bus=xhci.0,id=usb,drive=stick,removable=on,bootindex=1']
    return args + create(out, template, OPTICAL['make_gpt_disk'])


def loader_picture(startup, steps, home):
    """Read the loader display from the kernel, the firmware trace and the actual Home frame."""
    display = re.search(r'^display: Loader, (\d+)x(\d+), 1 page\(s\), '
                        r'(\d+) KiB of its memory mapped at (0x[0-9a-f]+)$', startup, re.M)
    fields = dict(line.split('=', 1) for line in steps
                  if line.startswith(('console_framebuffer=', 'console_pitch=')))
    if not display or 'console_framebuffer' not in fields or 'console_pitch' not in fields:
        raise ValueError('missing kernel loader display or firmware framebuffer geometry')
    width, height, mapped_kib = map(int, display.groups()[:3])
    address = int(fields['console_framebuffer'], 16)
    pitch = int(fields['console_pitch'], 16)
    picture_end = address + pitch * height
    first_page = address // 4096
    end_page = (picture_end + 4095) // 4096
    if int(display[4], 16) != address or pitch < width * 4 or mapped_kib != (end_page - first_page) * 4:
        raise ValueError('kernel framebuffer address or mapped size disagrees with the firmware picture')
    if home.read_bytes().split(b'\n', 2)[1] != f'{width} {height}'.encode():
        raise ValueError('visible Home and loader display dimensions differ')
    return display[0], address


def trial(qemu, code, template, volume, image, out, ram, video='std'):
    out.mkdir()
    serial, trace = out / 'serial.txt', out / 'firmware.txt'
    args = usb_command(qemu, code, template, volume, out, ram, video)
    receipt = {'video': video, 'ram_mib': ram, 'argv': args, 'passed': False, 'snapshots': []}
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
                if video in ('virtio-vga', 'ramfb'):
                    receipt['loader_display'], receipt['framebuffer_address'] = loader_picture(
                        startup, steps, out / 'home.ppm')
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
    return receipt


def allocation_refusal_trial(qemu, code, template, volume, image, out):
    """Refused fixed allocation at 64 MiB, using only the firmware USB keyboard."""
    out.mkdir()
    args = usb_command(qemu, code, template, volume, out, 64)
    args[args.index('-machine') + 1] = 'q35,i8042=off'
    args.extend(['-device', 'usb-kbd,id=external-keyboard,bus=xhci.0'])
    serial, trace = out / 'serial.txt', out / 'firmware.txt'
    first = 'Press F10 to request power off.'
    confirm = 'Switch off? Enter switches off. Esc cancels.'
    receipt = {'argv': args, 'passed': False, 'keys': []}
    with (out / 'stderr.txt').open('w') as err, subprocess.Popen(
            args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as vm:
        try:
            monitor = OPTICAL['Monitor'](vm)
            call = monitor.call
            call('qmp_capabilities')
            before = capture(call, out, 'before')
            call('cont')

            def screen(name, prompt, occurrence):
                OPTICAL['wait_for'](lambda: serial.read_text(errors='replace').count(prompt) >= occurrence,
                                    name, 45)
                if call('query-status')['status'] != 'running':
                    raise ValueError('firmware stopped before confirmation')
                frame = out / (name + '.ppm')
                def visible():
                    call('screendump', filename=str(frame))
                    return matches_refusal(frame.read_bytes(), prompt)
                OPTICAL['wait_for'](visible, 'complete text on ' + name, 10)

            def key(name):
                monitor.key(name)
                receipt['keys'].append(name)
                time.sleep(.2)

            screen('refused', first, 1)
            for name in ('ret', 'q', 'f10'):
                key(name)
            screen('confirm', confirm, 1)
            for name in ('y', 'esc'):
                key(name)
            screen('cancelled', first, 2)
            for name in ('ret', 'f10'):
                key(name)
            screen('confirm-again', confirm, 2)
            key('ret')
            OPTICAL['wait_for'](lambda: call('query-status')['status'] == 'shutdown', 'confirmed power off', 30)
            after = capture(call, out, 'shutdown')
            steps = trace.read_text().splitlines()
            allocation = next(line for line in steps if line.startswith('allocation='))
            guard = ['handoff-refused', 'refusal_watchdog_status=0000000000000000', 'refusal-power-confirmed']
            if ([line for line in steps if line in guard] != guard or int(allocation.split('=')[1], 16) == 0 or
                    any(line in steps for line in ('owned-before-write', 'owned-handoff'))):
                raise ValueError('unexpected allocation or handoff trace')
            text = serial.read_text(errors='replace')
            if ('PCP OS could not start.' not in text or
                    'Firmware could not reserve the memory needed by the kernel.' not in text or
                    'You can switch off the computer with its power button.' not in text or
                    text.count(first) != 2 or text.count(confirm) != 2 or '  pass ' in text):
                raise ValueError('incomplete refusal or unexpected kernel output')
            receipt.update(snapshots=[before, after], interval=compare(out, before, after),
                           events=monitor.events.copy(), usb_sha256=digest(volume))
            if ([e['data'] for e in receipt['events'] if e['event'] == 'SHUTDOWN'] !=
                    [{'guest': True, 'reason': 'guest-shutdown'}] or receipt['usb_sha256'] != image['usb_sha256'] or
                    not receipt['interval']['nvme_bytes_preserved'] or
                    any(receipt['interval']['counter_deltas']['sentinel'].values())):
                raise ValueError('shutdown, media, or internal block requests disagreed')
            call('quit')
            receipt['passed'] = vm.wait(timeout=10) == 0
            if not receipt['passed']:
                raise ValueError('QEMU did not exit normally')
        finally:
            if vm.poll() is None:
                vm.kill()
                vm.wait()
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
        trial(args.qemu, code, template, volume, image, out / "virtio-vga", 128, "virtio-vga")
        addresses = []
        for ram in (128, 256):
            receipt = trial(args.qemu, code, template, volume, image, out / f'ramfb-{ram}', ram, 'ramfb')
            addresses.append(receipt['framebuffer_address'])
        if addresses[0] == addresses[1]:
            raise ValueError('RAMFB stayed at the same address across the two guest memory sizes')
        allocation_refusal_trial(args.qemu, code, template, volume, image, out / 'allocation-refusal')


if __name__ == '__main__':
    main()
