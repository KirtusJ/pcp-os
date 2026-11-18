#!/usr/bin/env python3
"""Native PCP OS 640x480 optical rejection witness (test-only kernel preference).

The workflow temporarily changes one Bochs display-preference line in an
isolated checkout. It does NOT test the unmodified/released kernel image.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

import native_guest as n


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--kernel", required=True, type=Path)
    p.add_argument("--measurement", required=True)
    p.add_argument("--font", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    p.add_argument("--timeout", type=int, default=150)
    a = p.parse_args()
    root = a.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    serial = root / "serial.log"
    w, h = 640, 480
    font = n.reg.glyphs_from_file(a.font)
    lut = n.nibble_lookup(font)
    cmd = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg",
           "-icount", "shift=3", "-cpu", "max", "-m", "128M", "-nic", "none",
           "-display", "none", "-vga", "none", "-device", "VGA,vgamem_mb=4",
           "-monitor", "none", "-qmp", "stdio", "-serial", "file:" + str(serial),
           "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
           "-kernel", str(a.kernel.resolve()), "-append", "image=" + a.measurement]
    report = {"mode": [w, h],
              "scope": "native 640x480 under temporary test-only Bochs mode preference",
              "unmodified_product_kernel": False,
              "kernel_sha256": sha(a.kernel), "measured_kernel": a.measurement,
              "qemu_args": cmd, "frames": {}, "passed": False}
    with (root / "qemu.stderr").open("wb") as errors:
        guest = subprocess.Popen(cmd, stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=errors)
        mon = n.Monitor(guest, a.timeout)

        def capture(name):
            path = root / (name + ".ppm")
            mon.execute("screendump", filename=str(path))
            width, height, rgb = n.reg.read_ppm(path)
            if (width, height) != (w, h):
                raise ValueError("unexpected QEMU scanout mode %dx%d" % (width, height))
            report["frames"][name] = sha(path)
            return rgb

        try:
            if "QMP" not in mon.receive(time.monotonic() + a.timeout):
                raise RuntimeError("QMP handshake absent")
            mon.execute("qmp_capabilities")
            n.wait_serial(guest, serial, b"display: Bochs, 640x480", a.timeout)
            deadline = time.monotonic() + a.timeout
            while time.monotonic() < deadline:
                launcher = capture("launcher")
                if (n.exact_cells((w, h), launcher, 190, 40,
                                  "PCP OS", font, n.TEXT)
                        and n.exact_cells((w, h), launcher, 20, (h//20-2)*20,
                                          "Up/Down choose", font, n.TEXT, n.PANEL)):
                    break
                time.sleep(.25)
            else:
                raise TimeoutError("native 640 launcher did not appear")

            mon.key("r")
            deadline = time.monotonic() + a.timeout
            while time.monotonic() < deadline:
                frame = capture("register")
                if (n.exact_cells((w, h), frame, 20, 40, "HEAD ", font, n.TEAL)
                        and n.exact_cells((w, h), frame, 20, 20,
                                          "THE KERNEL'S REGISTER: 41 events, chain intact",
                                          font, n.TEAL)):
                    break
                time.sleep(.25)
            else:
                raise TimeoutError("native 640 register header missing")

            # x=20 is column 2; five 'HEAD ' glyphs precede SHA-256.
            # Full HEAD ends at x=710, beyond this 640-pixel scanout.
            start_x = 20 + 5 * 10
            visible_count = (w-start_x)//10
            if visible_count != 57:
                raise AssertionError("incorrect 640-width geometry")
            shown = []
            for i in range(visible_count):
                x = start_x + i * 10
                actual = b"".join(
                    frame[((40+y)*w+x)*3:((40+y)*w+x+10)*3]
                    for y in range(20))
                if actual not in lut:
                    raise ValueError("visible nibble %d not in actual font" % i)
                shown.append(lut[actual])
            try:
                n.decode_head((w, h), frame, font, lut)
            except ValueError:
                report["full_64_checker_rejected"] = True
            else:
                raise AssertionError("64-nibble checker accepted cropped frame")

            # Positive control: a different visible nibble changes real pixels,
            # whereas the final seven nibble cells are outside the framebuffer.
            last_cell_x = start_x + (visible_count-1)*10
            actual = b"".join(
                frame[((40+y)*w+last_cell_x)*3:((40+y)*w+last_cell_x+10)*3]
                for y in range(20))
            replacement = next(c for c in n.ALPHABET if c != shown[-1])
            negative = b"".join(n.raster(replacement, font, n.TEAL))
            changed = sum(actual[i:i+3] != negative[i:i+3]
                          for i in range(0, len(actual), 3))
            if changed <= 0:
                raise AssertionError("changed visible nibble falsely matches")

            report.update({"visible_head_prefix": "".join(shown),
                           "visible_head_nibbles": visible_count,
                           "clipped_head_nibbles": 64-visible_count,
                           "changed_visible_nibble_diff_pixels": changed,
                           "last_hex_cell_start_x": start_x + 63*10,
                           "last_hex_cell_end_x_exclusive": start_x + 64*10,
                           "full_64_visible": False})
            mon.key("esc")
            deadline = time.monotonic() + a.timeout
            while time.monotonic() < deadline:
                returned = capture("returned")
                if returned == launcher:
                    break
                time.sleep(.25)
            else:
                raise TimeoutError("Esc did not restore full launcher pixels")
            mon.key("p")
            report["qemu_exit"] = guest.wait(timeout=a.timeout)
            raw = serial.read_bytes()
            report["serial_sha256"] = hashlib.sha256(raw).hexdigest()
            if (report["qemu_exit"] != 33
                    or b"ALL SELF-TESTS PASS" not in raw
                    or b"  FAIL" in raw
                    or (a.measurement + " (measured; equal").encode() not in raw):
                raise RuntimeError("native guest shutdown/self-test/measurement failed")
            report["passed"] = True
            print(json.dumps(report, sort_keys=True), flush=True)
        except Exception as exc:
            report["error"] = str(exc)
            raise
        finally:
            (root / "result.json").write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n")
            mon.selector.close()
            if guest.poll() is None:
                guest.kill()
            guest.wait()
            guest.stdin.close()
            guest.stdout.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
