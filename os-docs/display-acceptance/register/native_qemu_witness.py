#!/usr/bin/env python3
"""Capture real PCP OS register frames on model-free QEMU and retain adverse controls.

This is an optical observation, NOT independent verification of the ledger head.
The decoded glyphs may only become release evidence when the same boot's
register bytes/head and kernel identity are independently authenticated.
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
sys.path.insert(0, str(HERE.parents[1]))
sys.path.insert(0, str(HERE.parents[4] / "checker/os-docs/display-acceptance/register"))
from check_display import Monitor, wait_serial  # existing QMP driver
from check_register import check_scanout, glyphs_from_file, read_ppm

GROUND = bytes.fromhex("10283f")
TEAL = bytes.fromhex("41bad7")
TEXT = bytes.fromhex("f2f7fc")
HEX = set("0123456789abcdef")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def cell_bytes(glyphs, ch, fg):
    mask = glyphs[ord(ch) - 32]
    colors = [bytes((back * (255 - a) + fore * a + 127) // 255
                    for fore, back in zip(fg, GROUND)) for a in range(256)]
    return b"".join(colors[a] for a in mask)


def pixels_for_cell(rgb, width, left, top):
    return b"".join(rgb[3 * ((top + y) * width + left):
                        3 * ((top + y) * width + left + 10)]
                    for y in range(20))


def label_present(size, rgb, glyphs, label, col, row, fg):
    width, height = size
    x, y = col * 10, row * 20
    if x + 10 * len(label) > width or y + 20 > height:
        return False
    return all(pixels_for_cell(rgb, width, x + n * 10, y) ==
               cell_bytes(glyphs, ch, fg)
               for n, ch in enumerate(label))


def decode_head(size, rgb, glyphs):
    width, height = size
    if width < 710 or height < 60:
        return None
    alphabet = " HEAD0123456789abcdef"
    templates = {cell_bytes(glyphs, ch, TEAL): ch for ch in alphabet}
    if len(templates) != len(set(alphabet)):
        raise ValueError("ambiguous font glyph templates")
    observed = []
    for n in range(69):
        cell = pixels_for_cell(rgb, width, 20 + 10 * n, 40)
        ch = templates.get(cell)
        if ch is None:
            return None
        observed.append(ch)
    text = "".join(observed)
    if not text.startswith("HEAD ") or len(text) != 69 or any(ch not in HEX for ch in text[5:]):
        return None
    return text[5:]


def take(monitor, out, name):
    path = out / (name + ".ppm")
    monitor.execute("screendump", filename=str(path))
    width, height, rgb = read_ppm(path)
    return path, (width, height), rgb


def poll_frame(monitor, out, name, predicate, *, seconds=75):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        path, size, rgb = take(monitor, out, name)
        answer = predicate(size, rgb)
        if answer:
            return path, size, rgb, answer
        time.sleep(.2)
    raise TimeoutError(f"{name}: expected native framebuffer paint absent")


def run(args, report):
    out = args.output.resolve()
    serial = out / "serial.log"
    memory = 4 if args.width == 800 else 8
    command = [
        "qemu-system-x86_64", "-machine", "q35", "-accel", "tcg",
        "-icount", "shift=3,align=off,sleep=off", "-cpu", "max",
        "-m", "128M", "-nic", "none", "-display", "none",
        "-vga", "none", "-device", f"VGA,vgamem_mb={memory}",
        "-serial", f"file:{serial}", "-monitor", "none",
        "-qmp", "stdio", "-no-reboot",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=4",
        "-kernel", str(args.kernel.resolve()),
        "-append", f"image={args.measurement} then=menu",
    ]
    report["qemu_command"] = command
    glyphs = glyphs_from_file(args.font)
    with (out / "qemu.stderr").open("wb") as errors:
        process = subprocess.Popen(command, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=errors)
        monitor = Monitor(process, 100)
        try:
            greeting = monitor.receive(time.monotonic() + monitor.timeout)
            if "QMP" not in greeting:
                raise RuntimeError("QMP greeting missing")
            monitor.execute("qmp_capabilities")
            wait_serial(process, serial, b"ALL SELF-TESTS PASS", 100)
            menu, size, before, _ = poll_frame(
                monitor, out, "launcher",
                lambda s, p: label_present(s, p, glyphs, "PCP OS", 19, 2, TEXT)
            )
            report["observed_mode"] = list(size)
            if size != (args.width, args.height):
                raise ValueError(f"native mode {size} != requested {args.width}x{args.height}")
            report["launcher_sha256"] = sha(menu)

            monitor.key("r")
            reg, _, original, head = poll_frame(
                monitor, out, "register",
                lambda s, p: decode_head(s, p, glyphs)
            )
            report["observed_head_from_pixels_only"] = head
            report["register_sha256"] = sha(reg)
            pixel = check_scanout(reg, head, glyphs)
            other = head[:-1] + ("0" if head[-1] != "0" else "1")
            adverse = check_scanout(reg, other, glyphs)
            report["observed_glyph_pixel_match"] = pixel["accepted"]
            report["changed_tail_rejected"] = not adverse["accepted"]
            report["changed_tail_mismatch_pixels"] = adverse["mismatching_pixels"]
            if not pixel["accepted"] or adverse["accepted"]:
                raise ValueError("pixel comparator failed its genuine-frame adverse control")

            monitor.key("up")
            time.sleep(.4)
            up, up_size, up_pixels = take(monitor, out, "up")
            monitor.key("down")
            time.sleep(.4)
            down, down_size, down_pixels = take(monitor, out, "down")
            report["up_changed_register_pixels"] = up_pixels != original
            report["up_full_head_preserved"] = decode_head(up_size, up_pixels, glyphs) == head
            report["down_full_head_preserved"] = decode_head(down_size, down_pixels, glyphs) == head
            report["down_restored_register_pixels"] = down_pixels == original
            if not (report["up_full_head_preserved"] and report["down_full_head_preserved"]
                    and report["down_restored_register_pixels"]):
                raise ValueError("Up/Down lost head or failed to restore register frame")

            monitor.key("esc")
            returned, returned_size, returned_pixels, _ = poll_frame(
                monitor, out, "returned",
                lambda s, p: label_present(s, p, glyphs, "PCP OS", 19, 2, TEXT)
            )
            report["escape_restored_launcher_pixels"] = returned_pixels == before
            report["returned_sha256"] = sha(returned)
            if returned_size != size or not report["escape_restored_launcher_pixels"]:
                raise ValueError("Esc failed to restore the original launcher frame")

            monitor.key("p")
            report["qemu_exit"] = process.wait(timeout=35)
            raw = serial.read_bytes()
            report["serial_sha256"] = hashlib.sha256(raw).hexdigest()
            report["serial_size"] = len(raw)
            report["measured_kernel_match"] = (
                args.measurement.encode() + b" (measured; equal" in raw)
            report["self_tests_passed"] = (
                b"ALL SELF-TESTS PASS" in raw and b"  FAIL" not in raw
            )
            if report["qemu_exit"] != 33 or not report["self_tests_passed"] or not report["measured_kernel_match"]:
                raise RuntimeError("guest exit, kernel measurement or self-tests failed")
        finally:
            monitor.selector.close()
            if process.poll() is None:
                process.kill()
            process.wait()
            process.stdin.close()
            process.stdout.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--kernel", type=Path, required=True)
    ap.add_argument("--font", type=Path, required=True)
    ap.add_argument("--measurement", required=True)
    ap.add_argument("--width", type=int, choices=(800, 1024), required=True)
    ap.add_argument("--height", type=int, choices=(600, 768), required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    if (args.width, args.height) not in ((800, 600), (1024, 768)):
        ap.error("only 800x600 and 1024x768 are paired")
    args.output.mkdir(parents=True, exist_ok=False)
    report = {
        "candidate_kernel_sha256": sha(args.kernel),
        "kernel_font_sha256": sha(args.font),
        "requested_mode": [args.width, args.height],
        "scope": "actual native QEMU pixels and navigation; digest decoded from pixels only",
        "independent_same_boot_ledger_head_verified": False,
    }
    try:
        run(args, report)
        report["passed_native_visual_probe"] = True
    except (RuntimeError, OSError, ValueError, TimeoutError, subprocess.TimeoutExpired) as exc:
        report["passed_native_visual_probe"] = False
        report["error"] = str(exc)
    (args.output / "result.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["passed_native_visual_probe"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
