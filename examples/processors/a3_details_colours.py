"""Compare each displayed glyph with the kernel's font, including its colour."""
import ast
import hashlib
import json
import re
import runpy
import shutil
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
FONT = ROOT / "PCP_OS_Rust/crates/pcpos-kernel/src"
MONITOR = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))["Monitor"]
PALETTE = {"soft": 0x9FB4C8, "amber": 0xE3A24B, "teal": 0x41BAD7, "text": 0xF2F7FC, "good": 0x8FE0C2}


def rgb(colour):
    return np.array([(colour >> shift) & 255 for shift in (16, 8, 0)], dtype=np.int32)


def font_table(scale):
    if scale == 2:
        glyphs = np.frombuffer((FONT / "font-2x.bin").read_bytes(), dtype=np.uint8).reshape(95, 40, 20)
    else:
        source = (FONT / "font.rs").read_text().split(" = [", 1)[1].split("];", 1)[0]
        glyphs = np.array(ast.literal_eval("[" + source + "]")).reshape(95, 20, 10)
    alpha = glyphs.astype(np.int32)[..., None]
    lookup = {}
    for colour, value in PALETTE.items():
        pixels = ((rgb(0x10283F) * (255 - alpha) + rgb(value) * alpha + 127) // 255).astype(np.uint8)
        for i, glyph in enumerate(pixels):
            lookup[glyph.tobytes()] = (chr(32 + i), colour) if i else (" ", "ground")
    return lookup


def capture(q, out, label, scale, table):
    path = out / "screen.ppm"
    q.call("screendump", filename=str(path))
    with Image.open(path) as image:
        assert image.size == (1920, 1080), image.size
        image.save(out / (label + ".png"))
        pixels = np.array(image)
    rows, inks = [], []
    h, w = 20 * scale, 10 * scale
    for y in range(0, pixels.shape[0] - h + 1, h):
        cells = [table.get(pixels[y:y+h, x:x+w].tobytes(), ("?", None))
                 for x in range(0, pixels.shape[1] - w + 1, w)]
        rows.append("".join(char for char, colour in cells).rstrip())
        inks.append([colour for char, colour in cells])
    (out / (label + ".txt")).write_text("\n".join(rows) + "\n")
    return rows, inks


def locate(phrase, rows, inks, expected):
    for first in range(len(rows)):
        parts, colours = [], []
        for last in range(first, min(first + 4, len(rows))):
            row = rows[last]
            words = row.strip()
            if not words:
                break
            parts.append(words)
            colours.extend(inks[last][index] for index, char in enumerate(row) if char != " ")
            whole = " ".join(parts)
            if whole == phrase:
                assert set(colours) == {expected}, (phrase, set(colours))
                return {"text": phrase, "rows": [first, last], "colour": expected,
                        "rgb": "#%06x" % PALETTE[expected]}
            if not phrase.startswith(whole + " "):
                break
    return None


def main():
    case, medium, directory = sys.argv[1:]
    assert case in ("pat", "no-pat")
    out = Path(directory).resolve()
    out.mkdir(parents=True, exist_ok=True)
    image = Path(medium).resolve()
    firmware = Path("/usr/share/OVMF")
    shutil.copyfile(firmware / "OVMF_VARS_4M.fd", out / "vars.fd")
    serial = out / "serial.log"
    cpu = "max" if case == "pat" else "max,-pat"
    command = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", cpu,
               "-m", "256M", "-vga", "none", "-device", "virtio-vga", "-display", "none", "-nic", "none",
               "-no-reboot", "-qmp", "stdio", "-serial", "file:" + str(serial),
               "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-drive", "if=pflash,unit=0,format=raw,readonly=on,file=" + str(firmware / "OVMF_CODE_4M.fd"),
               "-drive", "if=pflash,unit=1,format=raw,file=" + str(out / "vars.fd"),
               "-device", "qemu-xhci,id=xhci", "-drive", "if=none,id=stick,readonly=on,format=raw,file=" + str(image),
               "-device", "usb-storage,bus=xhci.0,drive=stick,bootindex=1"]
    with (out / "stderr.log").open("w") as err, subprocess.Popen(command, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=err) as guest:
        try:
            q = MONITOR(guest)
            q.call("qmp_capabilities")
            deadline = time.monotonic() + 150
            while time.monotonic() < deadline and guest.poll() is None:
                log = serial.read_text(errors="replace") if serial.exists() else ""
                match = re.search(r"desktop: 1920x1080 at ([12])x UI scale", log)
                if match:
                    break
                time.sleep(.2)
            else:
                raise RuntimeError("Desktop did not start")
            assert "display: Loader, 1920x1080, 1 page(s)" in log
            scale = int(match[1])
            table = font_table(scale)
            patterns = (r"^display memory: [0-9]+ of the [0-9]+ pages ", r"^display: a program that asks ")
            targets = []
            for pattern in patterns:
                matches = [row for row in log.splitlines() if re.match(pattern, row)]
                assert len(matches) == 1, matches
                targets.append(matches[0])
            assert ("2025 of the 2025" if case == "pat" else "0 of the 2025") in targets[0]
            assert "draws to memory of its own" in targets[1]
            time.sleep(1)
            q.key("spc")
            time.sleep(1)
            q.key("ctrl", "d")
            time.sleep(1)
            found = {}
            for i in range(5):
                label = "details-" + str(i)
                rows, inks = capture(q, out, label, scale, table)
                for j, phrase in enumerate(targets):
                    expected = "amber" if j == 0 and case == "no-pat" else "soft"
                    record = locate(phrase, rows, inks, expected)
                    if record:
                        found[j] = dict(record, capture=label + ".png")
                if len(found) == 2:
                    break
                q.key("down")
                time.sleep(.8)
            assert len(found) == 2, (targets, found, rows)
            for keys in [("esc",), ("esc",), ("f10",), ("ret",)]:
                q.key(*keys)
                time.sleep(.7)
            status = guest.wait(timeout=18)
            log = serial.read_text(errors="replace")
            assert status == 33 and "ALL SELF-TESTS PASS" in log, (status, log[-900:])
            result = {"cpu": cpu, "geometry": [1920, 1080], "page_count": 1, "ui_scale": scale,
                      "checks_passed": sum(row.startswith("pass ") for row in log.splitlines()),
                      "exit": status, "observations": [found[0], found[1]]}
            (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
            print(json.dumps(result), flush=True)
        finally:
            if guest.poll() is None:
                guest.kill()
            guest.wait()


if __name__ == "__main__":
    main()
