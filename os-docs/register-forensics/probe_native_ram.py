#!/usr/bin/env python3
"""Experimental same-paused-instant native QEMU register RAM candidate vs P6.

This is Director 11's candidate-oracle lane. It never changes a live register.
D10 owns kernel drawing, D2 owns optical acceptance. This script only creates
a bounded, disposable model-free guest and reads its paused physical RAM.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import mmap
import os
from pathlib import Path
import selectors
import subprocess
import sys
import time

from register_ram_oracle import select_candidate


def source_checker(path: Path):
    spec = importlib.util.spec_from_file_location('pinned_d2_check_register', str(path))
    if spec is None or spec.loader is None:
        raise ValueError('cannot load pinned optical checker')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class QMP:
    def __init__(self, child, timeout):
        self.child = child
        self.timeout = timeout
        self.select = selectors.DefaultSelector()
        self.select.register(child.stdout, selectors.EVENT_READ)
        self.buffer = bytearray()
        self.events = []
        self.commands = []
        self.next_id = 0

    def read(self, deadline):
        while b'\n' not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.select.select(remaining):
                raise TimeoutError('QMP response deadline exceeded')
            raw = os.read(self.child.stdout.fileno(), 65536)
            if not raw:
                raise RuntimeError('QEMU stdout closed: %s' % self.child.poll())
            self.buffer.extend(raw)
        row, _, rest = self.buffer.partition(b'\n')
        self.buffer = rest
        return json.loads(row)

    def greeting(self):
        row = self.read(time.monotonic() + self.timeout)
        if 'QMP' not in row:
            raise RuntimeError('expected QMP greeting, got %s' % str(row)[:100])
        return row

    def command(self, name, **arguments):
        self.next_id += 1
        mid = 'd11-%03d' % self.next_id
        command = {'execute': name, 'arguments': arguments, 'id': mid}
        self.commands.append({'action': name, 'args': arguments, 'id': mid})
        self.child.stdin.write((json.dumps(command, sort_keys=True) + '\n').encode())
        self.child.stdin.flush()
        deadline = time.monotonic() + self.timeout
        while True:
            row = self.read(deadline)
            if 'event' in row:
                self.events.append(row)
                continue
            if row.get('id') != mid:
                raise RuntimeError('QMP response identity mismatch: ' + str(row)[:120])
            if 'error' in row:
                raise RuntimeError('%s: %s' % (name, row['error']))
            if 'return' not in row:
                raise RuntimeError('missing QMP return object')
            return row['return']

    def key(self, name):
        return self.command('send-key', keys=[{'type':'qcode', 'data':name}], **{'hold-time':70})

    def close(self):
        self.select.close()


GROUND = bytes.fromhex('10283f')
TEAL = bytes.fromhex('41bad7')
SOFT = bytes.fromhex('9fb4c8')
TEXT = bytes.fromhex('f2f7fc')
PANEL = bytes.fromhex('18364f')


def raster(text, glyphs, fg, bg):
    palette = [bytes((b * (255-a) + f * a + 127)//255
                     for f, b in zip(fg, bg)) for a in range(256)]
    return [b''.join(palette[a] for char in text
                     for a in glyphs[ord(char)-32][10*y:10*y+10])
            for y in range(20)]


def text_at(size, pixels, col, row, text, glyphs, fg=TEAL, bg=GROUND):
    width, height = size
    x, y = col*10, row*20
    if x < 0 or y < 0 or x + len(text)*10 > width or y + 20 > height:
        return False
    expected = raster(text, glyphs, fg, bg)
    return all(pixels[((y+dy)*width+x)*3:((y+dy)*width+x+len(text)*10)*3] == want
               for dy, want in enumerate(expected))


def head_region(size, pixels):
    width, height = size
    if width < 710 or height < 60:
        return None
    return b''.join(pixels[((40+y)*width+20)*3:((40+y)*width+710)*3]
                    for y in range(20))


def wait_serial(child, serial, text, timeout):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if text in (serial.read_bytes() if serial.exists() else b''):
            return
        if child.poll() is not None:
            raise RuntimeError('guest exited before serial evidence: %s' % child.returncode)
        time.sleep(.2)
    raise TimeoutError('missing serial: ' + repr(text))


def screenshot(qmp, checker, path):
    qmp.command('screendump', filename=str(path))
    width, height, pixels = checker.read_ppm(path)
    return (width,height), pixels


def wait_screen(qmp, checker, serial, child, path, width, height, glyphs, timeout):
    wait_serial(child, serial, ('display: Bochs, %dx%d' % (width,height)).encode(), timeout)
    # Confirm complete launcher, including both footer rows, before sending R.
    stop = time.monotonic() + timeout
    while time.monotonic() < stop:
        size, pixels = screenshot(qmp, checker, path)
        rows = height//20
        if (size == (width,height)
            and text_at(size,pixels,19,2,'PCP OS',glyphs,TEXT,GROUND)
            and text_at(size,pixels,2,rows-2,'Up/Down choose',glyphs,TEXT,PANEL)
            and text_at(size,pixels,2,rows-1,'Ctrl+Alt+Delete',glyphs,SOFT,PANEL)):
            break
        time.sleep(.3)
    else:
        raise TimeoutError('fully painted launcher not ready')
    qmp.key('r')
    # Require stable full 69-cell row in two separate actual screendumps.
    last = None
    stop = time.monotonic() + timeout
    while time.monotonic() < stop:
        size, pixels = screenshot(qmp, checker, path)
        if size != (width,height) or not text_at(size,pixels,2,2,'HEAD ',glyphs):
            last = None
            time.sleep(.3)
            continue
        region = head_region(size,pixels)
        # The rightmost digest cell must be painted; two stable frames reject
        # the most obvious mid-draw race but do not prove zero subsequent changes.
        final = b''.join(pixels[((40+y)*width+700)*3:((40+y)*width+710)*3] for y in range(20))
        if region is not None and final != GROUND * 200 and region == last:
            return
        last = region
        time.sleep(.5)
    raise TimeoutError('full stable head row not available')


def run(args):
    checker = source_checker(args.checker)
    glyphs = checker.glyphs_from_file(args.font)
    args.output.mkdir(parents=True, exist_ok=False)
    width, height = args.mode
    vram = 4 if width == 800 else 16
    serial = args.output / 'serial.log'
    errors = args.output / 'qemu.stderr'
    live_ppm = args.output / 'capture-probe.ppm'
    paused_ppm = args.output / 'register-paused.ppm'
    # Volatile file MUST NOT be uploaded as an artifact. Discard in all paths.
    raw_ram = args.output / 'volatile-register-ram.bin'
    report = {'kind': 'disposable-qemu-guest-ram-candidate-and-p6',
              'kernel_sha256': hashlib.sha256(args.kernel.read_bytes()).hexdigest(),
              'measured_image': args.measurement,
              'resolution':[width,height], 'vram_mib':vram,
              'live_record_pointer_authenticated':False,
              'invalid_live_register_refusal_tested':False,
              'host_enforced_acr_or_persistent_worker_observed':False,
              'candidate_oracle_not_promotion_criterion':True}
    command = [
        'qemu-system-x86_64', '-machine','q35','-accel','tcg','-icount','shift=3',
        '-cpu','max','-m','128M','-nic','none',
        '-display','none','-vga','none','-device','VGA,vgamem_mb=%d'%vram,
        '-monitor','none','-qmp','stdio','-serial','file:'+str(serial),
        '-no-reboot','-device','isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-kernel',str(args.kernel),'-append','image='+args.measurement
    ]
    report['qemu_argv']=command
    monitor = None
    guest = None
    try:
        with errors.open('wb') as err:
            guest = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
            monitor = QMP(guest,args.timeout)
            report['qmp_greeting']=monitor.greeting()
            monitor.command('qmp_capabilities')
            wait_screen(monitor,checker,serial,guest,live_ppm,width,height,glyphs,args.timeout)
            # Stop and query QEMU's observed state before BOTH captures.
            monitor.command('stop')
            qstatus = monitor.command('query-status')
            report['paused_query_status']=qstatus
            if qstatus.get('running') is not False or qstatus.get('status') != 'paused':
                raise RuntimeError('QMP did not confirm paused guest: '+str(qstatus))
            size, pixels = screenshot(monitor,checker,paused_ppm)
            if size!=(width,height):
                raise RuntimeError('paused framebuffer geometry differs')
            report['paused_p6_sha256']=hashlib.sha256(paused_ppm.read_bytes()).hexdigest()
            monitor.command('pmemsave',val=0,size=128*1024*1024,filename=str(raw_ram))
            if raw_ram.stat().st_size != 128*1024*1024:
                raise RuntimeError('QMP raw guest RAM dump has wrong size')
            with raw_ram.open('rb') as fp:
                raw_hash=hashlib.file_digest(fp,'sha256').hexdigest()
                fp.seek(0)
                with mmap.mmap(fp.fileno(),0,access=mmap.ACCESS_READ) as mem:
                    selected=select_candidate(mem,minimum_events=1)
            report['paused_ram_sha256']=raw_hash
            report['memory_candidate']=selected
            if not selected['found']:
                raise RuntimeError('RAM candidate unavailable: '+str(selected))
            head = selected['independent_head']
            count = selected['longest_events']
            # The visible count and full head must match the *memory-selected*
            # values, NOT self-decoded values from the P6.
            title='THE KERNEL\'S REGISTER: %d events, chain intact'%count
            title_match=text_at(size,pixels,2,1,title,glyphs)
            optical=checker.check_scanout(paused_ppm,head,glyphs)
            changed=head[:-1]+('0' if head[-1]!='0' else '1')
            adverse=checker.check_scanout(paused_ppm,changed,glyphs)
            report['title_raster_matches_independent_count']=title_match
            report['full_head_matches_independent_ram_head']=optical
            report['changed_last_nibble_negative']=adverse
            report['candidate_accepted']=bool(title_match and optical['accepted']
                                               and not adverse['accepted']
                                               and adverse['mismatching_pixels']>0)
            if not report['candidate_accepted']:
                raise RuntimeError('native RAM candidate and paused pixels do not match')
            monitor.command('cont')
            monitor.key('p')
            code = guest.wait(timeout=args.timeout)
            raw=serial.read_bytes()
            report['qemu_exit']=code
            report['serial_sha256']=hashlib.sha256(raw).hexdigest()
            if code != 33 or b'ALL SELF-TESTS PASS' not in raw or b'  FAIL' in raw:
                raise RuntimeError('native self-tests or shutdown failed')
            report['passed']=True
            return report
    except Exception as exc:
        report['passed']=False
        report['error']=str(exc)
        raise
    finally:
        if raw_ram.exists():
            raw_ram.unlink()  # untrusted secret-bearing RAM never uploaded
        if live_ppm.exists():
            live_ppm.unlink() # only paused proof retained
        if monitor:
            report['qmp_commands']=monitor.commands
            report['qmp_events']=monitor.events
            monitor.close()
        if guest:
            if guest.poll() is None:
                guest.kill()
            guest.wait()
            guest.stdin.close()
            guest.stdout.close()
        (args.output/'result.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')


def parse_args():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--kernel',type=Path,required=True)
    ap.add_argument('--measurement',required=True)
    ap.add_argument('--font',type=Path,required=True)
    ap.add_argument('--checker',type=Path,required=True,help='independently owned optical checker from pinned D2 source')
    ap.add_argument('--output',type=Path,required=True)
    ap.add_argument('--mode',choices=['800x600','1024x768'],default='800x600')
    ap.add_argument('--timeout',type=int,default=300)
    args=ap.parse_args()
    args.mode=tuple(map(int,args.mode.split('x')))
    args.output=args.output.resolve()
    args.kernel=args.kernel.resolve()
    args.font=args.font.resolve()
    args.checker=args.checker.resolve()
    if len(args.measurement)!=64 or any(x not in '0123456789abcdef' for x in args.measurement):
        ap.error('measurement must be 64 lowercase SHA-256 digits')
    return args


if __name__=='__main__':
    try:
        observation=run(parse_args())
        print(json.dumps(observation,sort_keys=True))
    except Exception as exc:
        print(json.dumps({'passed':False,'error':str(exc)},sort_keys=True),file=sys.stderr)
        raise
