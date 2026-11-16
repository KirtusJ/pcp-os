#!/usr/bin/env python3
"""Independent public native PCP OS register screen optical witness.

Run real QEMU guests at 800x600 and 1024x768, decode exactly 64 visible
SHA-256 nibbles with the committed kernel font, and apply the independently
developed #232 P6 checker. This deliberately DOES NOT authenticate the
decoded digest against an independently verified same-boot ledger.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import time

HERE = Path(__file__).resolve()
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / "checker/os-docs/display-acceptance/register"))
sys.path.insert(0, str(ROOT / "os-docs/display-acceptance"))
import check_register as reg
from check_display import Monitor, wait_serial

GROUND = bytes.fromhex("10283f")
TEAL = bytes.fromhex("41bad7")
TEXT = bytes.fromhex("f2f7fc")
SOFT = bytes.fromhex("9fb4c8")
ALPHABET = "0123456789abcdef"


def raster(text, font, fg):
    palette = [
        bytes((bg * (255 - a) + ink * a + 127) // 255 for ink, bg in zip(fg, GROUND))
        for a in range(256)
    ]
    return [
        b"".join(
            palette[alpha]
            for char in text
            for alpha in font[ord(char) - 32][10*y:10*y+10]
        )
        for y in range(20)
    ]


def exact_cells(size, pixels, x, y, text, font, fg):
    width, height = size
    if x + 10 * len(text) > width or y + 20 > height:
        return False
    rows = raster(text, font, fg)
    return all(
        pixels[(yy*width+x)*3:(yy*width+x+len(text)*10)*3] == row
        for yy, row in ((y+k, rows[k]) for k in range(20))
    )


def nibble_lookup(font):
    out = {}
    for char in ALPHABET:
        rows = raster(char, font, TEAL)
        pixels = b"".join(rows)
        if pixels in out:
            raise ValueError("two hexadecimal font glyphs are pixel-identical")
        out[pixels] = char
    if len(out) != 16:
        raise ValueError("incomplete hexadecimal glyph alphabet")
    return out


def decode_head(size, pixels, font, lut):
    width, height = size
    if not exact_cells(size, pixels, 20, 40, "HEAD ", font, TEAL):
        raise ValueError("HEAD prefix not displayed in claimed location")
    chars = []
    for i in range(64):
        x = 20 + (5 + i) * 10
        cell = b"".join(
            pixels[((40+y)*width+x)*3:((40+y)*width+x+10)*3]
            for y in range(20)
        )
        if cell not in lut:
            raise ValueError("head cell %d does not match any actual hex glyph" % i)
        chars.append(lut[cell])
    return "".join(chars)


def visit(kernel, measured, root, width, height, vram, font, timeout):
    root.mkdir(parents=True, exist_ok=False)
    serial = root / "serial.log"
    report = {
        "claimed_size": [width, height], "vram_mib": vram,
        "kernel_sha256": hashlib.sha256(kernel.read_bytes()).hexdigest(),
        "measured_kernel": measured, "frames": {},
        "head_source": "read from native pixels; independent same-boot ledger readback NOT supplied",
        "ledger_head_independently_authenticated": False,
    }
    cmd = [
        "qemu-system-x86_64", "-machine", "q35", "-accel", "tcg",
        "-icount", "shift=3", "-cpu", "max", "-m", "128M", "-nic", "none",
        "-display", "none", "-vga", "none", "-device", "VGA,vgamem_mb=%d" % vram,
        "-monitor", "none", "-qmp", "stdio", "-serial", "file:"+str(serial),
        "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-kernel", str(kernel), "-append", "image="+measured,
    ]
    report["qemu_args"] = cmd
    lut = nibble_lookup(font)
    with (root / "qemu.stderr").open("wb") as errors:
        guest = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=errors)
        monitor = Monitor(guest, timeout)
        def screenshot(name):
            path = root / (name + ".ppm")
            monitor.execute("screendump", filename=str(path))
            width, height, pixels = reg.read_ppm(path)
            return path, ((width, height), pixels)
        try:
            greeting = monitor.receive(time.monotonic() + timeout)
            if "QMP" not in greeting:
                raise RuntimeError("missing native QEMU QMP handshake")
            monitor.execute("qmp_capabilities")
            wait_serial(guest, serial, ("display: Bochs, %dx%d" % (width, height)).encode(), timeout)

            end = time.monotonic() + timeout
            while time.monotonic() < end:
                path, (size, pixels) = screenshot("launcher")
                if (size == (width, height)
                        and exact_cells(size, pixels, 190, 40, "PCP OS", font, TEXT)
                        and exact_cells(size, pixels, 20, height-40, "Up/Down choose", font, TEXT)
                        and exact_cells(size, pixels, 20, height-20, "Ctrl+Alt+Delete", font, SOFT)):
                    report["frames"]["launcher"] = hashlib.sha256(path.read_bytes()).hexdigest()
                    initial = pixels
                    break
                time.sleep(.25)
            else:
                raise TimeoutError("native PCP OS launcher pixels missing")

            monitor.key("r")
            end = time.monotonic() + timeout
            failure = ""
            while time.monotonic() < end:
                path, (size, pixels) = screenshot("register")
                try:
                    if size != (width, height):
                        raise ValueError("register image dimensions differ")
                    head = decode_head(size, pixels, font, lut)
                    actual = reg.check_scanout(path, head, font)
                    if not actual["accepted"]:
                        raise ValueError("raster mismatch " + str(actual))
                    break
                except ValueError as exc:
                    failure = str(exc)
                    time.sleep(.25)
            else:
                raise TimeoutError("native register head absent: " + failure)

            report["optically_decoded_head"] = head
            report["frames"]["register"] = hashlib.sha256(path.read_bytes()).hexdigest()
            report["full_head_pixels"] = actual
            changed = head[:-1] + ("0" if head[-1] != "0" else "1")
            adverse = reg.check_scanout(path, changed, font)
            if adverse["accepted"] or adverse["mismatching_pixels"] == 0:
                raise RuntimeError("changed last hex digit falsely accepted")
            report["changed_tail_negative"] = adverse

            monitor.key("up")
            time.sleep(.30)
            path, (size, pixels) = screenshot("register_up")
            if decode_head(size, pixels, font, lut) != head:
                raise ValueError("head changed during Up")
            report["frames"]["register_up"] = hashlib.sha256(path.read_bytes()).hexdigest()
            monitor.key("down")
            time.sleep(.30)
            path, (size, pixels) = screenshot("register_down")
            if decode_head(size, pixels, font, lut) != head:
                raise ValueError("head changed during Down")
            report["frames"]["register_down"] = hashlib.sha256(path.read_bytes()).hexdigest()
            report["up_down_changed_frame"] = report["frames"]["register_up"] != report["frames"]["register"]

            monitor.key("esc")
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                path, (size, pixels) = screenshot("returned")
                if size == (width, height) and pixels == initial:
                    report["frames"]["returned"] = hashlib.sha256(path.read_bytes()).hexdigest()
                    break
                time.sleep(.25)
            else:
                raise TimeoutError("Esc did not restore the native launcher byte-for-byte")

            monitor.key("p")
            report["qemu_exit"] = guest.wait(timeout=timeout)
            raw = serial.read_bytes()
            report["serial_sha256"] = hashlib.sha256(raw).hexdigest()
            if report["qemu_exit"] != 33 or b"ALL SELF-TESTS PASS" not in raw or b"  FAIL" in raw:
                raise RuntimeError("guest shutdown, self-tests or serial failed")
            report["passed"] = True
        except Exception as exc:
            report["passed"] = False
            report["error"] = str(exc)
            raise
        finally:
            (root / "result.json").write_text(json.dumps(report, indent=2, sort_keys=True)+"\n")
            monitor.selector.close()
            if guest.poll() is None:
                guest.kill()
            guest.wait()
            guest.stdin.close()
            guest.stdout.close()
    return report


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--kernel", type=Path, required=True)
    p.add_argument("--measurement", required=True)
    p.add_argument("--font", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--timeout", type=int, default=150)
    args = p.parse_args()
    kernel = args.kernel.resolve()
    font = reg.glyphs_from_file(args.font)
    for w, h, vram in [(800, 600, 4), (1024, 768, 16)]:
        report = visit(kernel, args.measurement, args.output.resolve() / ("%dx%d" % (w,h)),
                       w,h,vram,font,args.timeout)
        print(json.dumps(report, sort_keys=True), flush=True)
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
