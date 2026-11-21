#!/usr/bin/env python3
"""Check Home, F5 and Esc on one compact QEMU guest and retain its original frames."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import subprocess


optical = runpy.run_path(str(Path(__file__).with_name('check.py')))
MEASURE = Path(__file__).resolve().parents[2] / 'PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py'


def check(kernel, out, geometry):
    out.mkdir(parents=True, exist_ok=False)
    kernel_bytes = kernel.read_bytes()
    measurement = runpy.run_path(str(MEASURE))['measure'](kernel_bytes)
    width, height = map(int, geometry.split('x'))
    serial = out / 'serial.log'
    cmd = ['qemu-system-x86_64', '-machine', 'pc', '-cpu', 'max', '-accel', 'tcg',
           '-m', '256M', '-smp', '1', '-kernel', str(kernel),
           '-append', f'desktop={geometry} image={measurement}', '-vga', 'std', '-display', 'none',
           '-nic', 'none', '-monitor', 'none', '-qmp', 'stdio', '-serial', 'file:' + str(serial),
           '-device', 'isa-debug-exit,iobase=0xf4,iosize=4']
    with (out / 'stderr.log').open('wb') as err:
        vm = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
        try:
            monitor = optical['Monitor'](vm)
            monitor.call('qmp_capabilities')
            screens = optical['DesktopScreens'](monitor, out, 40)
            rows = screens.wait('home', lambda view: 'PCP OS | DESKTOP' in view[1]
                                and view[5].strip() == 'Programs' and 'F10 off' in view[-2])
            home = (out / 'home.ppm').read_bytes()
            if not home.startswith(f'P6\n{width} {height}\n255\n'.encode()):
                raise RuntimeError('guest did not use requested compact geometry')
            if 'Verified at start:' not in rows[3] or screens.verification_screen(rows):
                raise RuntimeError('the unchanged compact Home failed the no-F5 control')

            screens.verification('f5')
            screens.wait_frame('restored', lambda view, frame: frame == home)
            verification = (out / 'f5-verification.ppm').read_bytes()
            # Erase a whole title or verdict, using the adjacent blank row from this same frame.
            head, size, depth, pixels = verification.split(b'\n', 3)
            header = b'\n'.join((head, size, depth, b''))
            row_size = width * 3 * 20
            for row in (1, 7):
                start = row * row_size
                erased = pixels[:start] + pixels[start+row_size:start+2*row_size] + pixels[start+row_size:]
                if screens.verification_screen(screens.read(header + erased, screens.glyphs)):
                    raise RuntimeError(f'F5 accepted an erased row {row}')

            monitor.key('f10')
            screens.wait('power', lambda view: view[1].strip() == 'POWER OFF'
                         and 'Enter switch off    Esc go back' in view[-2])
            monitor.key('ret')
            status = vm.wait(timeout=40)
        finally:
            if vm.poll() is None:
                vm.kill()
                vm.wait()
    log = serial.read_text()
    if (status != 33 or (measurement + ' (measured; equal to the value the loader named)') not in log
            or 'ALL SELF-TESTS PASS.' not in log):
        raise RuntimeError('guest did not finish with its measured kernel and passing self-tests')
    result = {'geometry': geometry, 'argv': cmd, 'exit': status,
              'kernel_sha256': hashlib.sha256(kernel_bytes).hexdigest(), 'measurement': measurement,
              'frames': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.glob('*.ppm'))}}
    (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('kernel', type=Path)
    parser.add_argument('evidence', type=Path)
    parser.add_argument('--geometry', choices=('800x600', '1024x768'), default='800x600')
    args = parser.parse_args()
    check(args.kernel.resolve(), args.evidence.resolve(), args.geometry)
