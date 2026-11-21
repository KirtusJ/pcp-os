#!/usr/bin/env python3
"""Exercise compact Home, F5 and Esc on the single-app CD and its measured kernel."""
import argparse
import json
from pathlib import Path
import runpy
import subprocess
import tempfile


optical = runpy.run_path(str(Path(__file__).with_name('check.py')))
REPO = Path(__file__).resolve().parents[2]
STARTUP_CHECKS = optical['SCREEN']['STARTUP_CHECKS']
digest = optical['digest']


def check_screens(monitor, out, geometry):
    width, height = map(int, geometry.split('x'))
    screens = optical['DesktopScreens'](monitor, out, 40)
    startup = f'  Verified at start: all {STARTUP_CHECKS} self-tests passed.'
    rows = screens.wait('home', lambda view: view[1][7:] == 'PCP OS | DESKTOP'
                        and view[3] == startup and view[5] == '   Programs'
                        and view[-2] == '  Up/Down choose   Enter open   F1 keys   F10 off')
    home = (out / 'home.ppm').read_bytes()
    if not home.startswith(f'P6\n{width} {height}\n255\n'.encode()):
        raise RuntimeError('guest did not use requested compact geometry')
    if screens.verification_screen(rows):
        raise RuntimeError('the unchanged compact Home passed the no-F5 control')

    screens.verification('f5')
    screens.wait_frame('restored', lambda view, frame: frame == home)
    verification = (out / 'f5-verification.ppm').read_bytes()
    # Copy a blank row over the title, then the verdict, on the real F5 capture.
    magic, size, depth, pixels = verification.split(b'\n', 3)
    header = b'\n'.join((magic, size, depth, b''))
    row_size = width * 3 * 20
    for row in (1, 7):
        start = row * row_size
        end = start + row_size
        erased = pixels[:start] + pixels[end:end + row_size] + pixels[end:]
        if screens.verification_screen(screens.read(header + erased, screens.glyphs)):
            raise RuntimeError(f'F5 accepted an erased row {row}')

    monitor.key('f10')
    screens.wait('power', lambda view: view[1] == '  POWER OFF' and view[3] == '  Switch PCP OS off?'
                 and view[-2] == '  Enter switch off    Esc go back')
    monitor.key('ret')


def check(qemu, out, geometry, iso, kernel, receipt):
    out.mkdir()
    optical_boot = geometry == '800x600'
    serial = out / 'serial.log'
    cmd = [qemu, '-machine', 'pc', '-cpu', 'qemu64', '-accel', 'tcg', '-m', '64M',
           '-vga', 'std', '-nic', 'none', '-audiodev', 'none,id=snd', '-device', 'AC97,audiodev=snd',
           '-display', 'none', '-monitor', 'none', '-qmp', 'stdio', '-serial', 'file:' + str(serial)]
    measurement = receipt['kernel_measurement']
    if optical_boot:
        cmd += ['-boot', 'order=d,strict=on', '-cdrom', str(iso), '-global', 'VGA.vgamem_mb=4']
    else:
        cmd += ['-kernel', str(kernel), '-append', f'desktop={geometry} image={measurement}']
    result = {'geometry': geometry, 'boot': 'optical' if optical_boot else 'direct-kernel',
              'argv': cmd, 'iso_sha256': receipt['iso_sha256'], 'kernel_sha256': receipt['on_disc']['kernel.elf32']['sha256'],
              'kernel_measurement': measurement, 'source_commit': receipt['source_commit'], 'passed': False}
    with (out / 'stderr.log').open('wb') as err:
        vm = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
        try:
            monitor = optical['Monitor'](vm)
            monitor.call('qmp_capabilities')
            media = [r for r in monitor.call('query-block') if 'inserted' in r]
            result['inserted_media'] = media
            if optical_boot:
                if (len(media) != 1 or not media[0]['removable'] or not media[0]['inserted']['ro']
                        or Path(media[0]['inserted']['file']) != iso):
                    raise RuntimeError('expected the exact tester ISO as the only medium, in a read-only CD drive')
            elif media:
                raise RuntimeError('unexpected medium in the direct-kernel guest')
            optical['wait_for'](lambda: f'desktop: {geometry} at 1x UI scale' in serial.read_text(),
                                'waiting for the requested geometry on serial', 60)
            log = serial.read_text()
            result['startup_checks'] = sum(line.startswith('  pass ') for line in log.splitlines())
            if (result['startup_checks'] != STARTUP_CHECKS or any(line.lower().startswith('  fail') for line in log.splitlines())
                    or measurement + ' (measured; equal to the value the loader named)' not in log):
                raise RuntimeError('guest did not start with its measured kernel and passing self-tests')

            check_screens(monitor, out, geometry)
            while not any(event['event'] == 'SHUTDOWN' for event in monitor.events):
                monitor.events.append(monitor.read())
            result['shutdown'] = next(event['data'] for event in monitor.events if event['event'] == 'SHUTDOWN')
            result['exit'] = vm.wait(timeout=40)
            log = serial.read_text()
            result['final_checks'] = sum(line.startswith('  pass ') for line in log.splitlines())
            if (result['exit'] != 0 or result['shutdown'] != {'guest': True, 'reason': 'guest-shutdown'}
                    or result['final_checks'] != STARTUP_CHECKS + 1 or 'ALL SELF-TESTS PASS.' not in log):
                raise RuntimeError('guest did not finish its self-tests and power off normally')
            result['passed'] = True
        finally:
            if vm.poll() is None:
                vm.kill()
                vm.wait()
            result['frames'] = {p.name: digest(p.read_bytes()) for p in sorted(out.glob('*.ppm'))}
            (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('built', type=Path, help='tester CD build directory with its receipt and ISO')
    parser.add_argument('evidence', type=Path, help='new evidence directory')
    parser.add_argument('--qemu', default='qemu-system-x86_64')
    args = parser.parse_args()
    receipt = json.loads((args.built / 'receipt.json').read_text())
    iso = (args.built / 'pcp-os-tester.iso').resolve(strict=True)
    raw = iso.read_bytes()
    if digest(raw) != receipt['iso_sha256']:
        parser.error('the ISO does not match its build receipt')
    builder = runpy.run_path(str(REPO / 'examples/tester-cd/build.py'))
    kernel = builder['read_back'](raw)['kernel.elf32']
    measure = runpy.run_path(str(REPO / 'PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py'))['measure']
    if digest(kernel) != receipt['on_disc']['kernel.elf32']['sha256'] or measure(kernel) != receipt['kernel_measurement']:
        parser.error('the on-disc kernel does not match its receipt digest and measurement')
    out = args.evidence.resolve()
    out.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory() as temp:
        extracted = Path(temp) / 'kernel.elf32'
        extracted.write_bytes(kernel)
        for geometry in ('800x600', '1024x768'):
            check(args.qemu, out / geometry, geometry, iso, extracted, receipt)


if __name__ == '__main__':
    main()
