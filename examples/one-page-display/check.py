#!/usr/bin/env python3
"""On a display with one page, a program that asks draws off the screen and the kernel shows each finished picture.

The owner's HP EliteBook 840 G7 showed every repaint being made: the screen wiped to the ground colour and
redrawn piece by piece. Its firmware framebuffer is one page of 1920 by 1080, which is exactly 2,025 pages of
memory, so the kernel mapped the screen itself into the program. This boots the kernel as that machine does,
through UEFI firmware and GRUB with a framebuffer the loader set and no display adapter the kernel drives
(QEMU's virtio-vga under OVMF), and starts a program that draws.

Predicates, fixed before the first run.
 O1 the kernel reports "display: Loader, 1920x1080, 1 page(s)" and 8100 KiB mapped: the laptop's case, whole pages.
 O2 the kernel says "display: a program that asks draws to memory of its own, and the kernel shows each
    finished picture". With --before (the kernel as it was) that line is absent: the control.
 O3 the program presents its first frame. Each of four screens (after the first frame; after a wait with no
    key; after "h"; after "i") is a whole 1920 by 1080 picture (the file's header and length say so) with more
    than one colour counted over every pixel. With --before the first and the two typed screens are byte for
    byte the same as this kernel's: what is shown is the same picture, and only where it was drawn has changed.
 O4 the monitor accepts both keys; the screen left alone for as long as a key is given is byte for byte
    unchanged; and each key then changes some rows and not all of them. So a change is a key's, one at a time.

Whether a picture is seen while it is being made is asked directly by check_pause.py beside this file.

What O3 and O4 do not show: what the picture is of. Nothing here reads a letter or a title, so a screen that
answered each key by changing one pixel would pass. What Tailorbird draws is read in its own checks.

What this does not show: the repaint itself. An emulator writes a framebuffer at the speed of memory, so the
wipe the owner saw cannot be seen here, with either kernel. That the wipe is gone needs the laptop.

Usage: check.py <kernel.elf32> <drawing guest ELF> <new output directory> [--before <kernel.elf32>]
Needs qemu-system-x86_64, OVMF at /usr/share/OVMF, grub-mkstandalone with the x86_64-efi modules, and mtools.
The guest is started as "tailorbird" (auto=tailorbird); any program that asks to draw through the kernel will do.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

GRUB = """set timeout=0
insmod all_video
insmod gfxterm
insmod multiboot
insmod part_gpt
insmod fat
set gfxmode=1920x1080x32
set gfxpayload=keep
terminal_output gfxterm
search --no-floppy --file /kernel.elf32 --set=root
multiboot /kernel.elf32 kernel auto=tailorbird then=menu
module /tailorbird tailorbird
boot
"""
OWN = "display: a program that asks draws to memory of its own, and the kernel shows each finished picture"


def run(kernel, guest, efi, out):
    out.mkdir(parents=True)
    image, serial = out / "esp.img", out / "serial.txt"
    subprocess.run(["truncate", "-s", "64M", str(image)], check=True)
    subprocess.run(["mformat", "-i", str(image), "-F", "::"], check=True)
    subprocess.run(["mmd", "-i", str(image), "::EFI", "::EFI/BOOT"], check=True)
    for source, name in ((efi, "::EFI/BOOT/BOOTX64.EFI"), (kernel, "::kernel.elf32"), (guest, "::tailorbird")):
        subprocess.run(["mcopy", "-i", str(image), str(source), name], check=True)
    shutil.copy("/usr/share/OVMF/OVMF_VARS_4M.fd", out / "vars.fd")
    vm = subprocess.Popen(
        ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "512M", "-vga", "none", "-device", "virtio-vga",
         "-nic", "none", "-display", "none", "-serial", "file:%s" % serial, "-no-reboot",
         "-drive", "if=pflash,unit=0,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd",
         "-drive", "if=pflash,unit=1,format=raw,file=%s" % (out / "vars.fd"), "-drive", "format=raw,file=%s" % image, "-qmp", "stdio"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)

    def call(command, **arguments):
        vm.stdin.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
        vm.stdin.flush()
        while True:
            reply = json.loads(vm.stdout.readline())
            if "return" in reply or "error" in reply:
                return reply

    try:
        vm.stdout.readline()
        call("qmp_capabilities")
        deadline = time.time() + 180
        presented = False
        while time.time() < deadline and not presented:
            presented = serial.exists() and "first frame presented" in serial.read_text(errors="replace")
            time.sleep(0.5)
        time.sleep(2)
        call("screendump", filename=str(out / "first.ppm"))
        time.sleep(2.1)
        call("screendump", filename=str(out / "idle.ppm"))
        sent = []
        for letter, name in (("h", "typed-h.ppm"), ("i", "typed.ppm")):
            sent.append("return" in call("send-key", keys=[{"type": "qcode", "data": letter}]))
            time.sleep(2.1)
            call("screendump", filename=str(out / name))
    finally:
        vm.kill()
        vm.wait()
    shots = [(out / name).read_bytes() for name in ("first.ppm", "idle.ppm", "typed-h.ppm", "typed.ppm")]
    return presented, serial.read_text(errors="replace").splitlines(), all(sent) and len(sent) == 2, shots


def picture(data):
    """The size and pixels of a binary PPM as QEMU writes it, or None when it is not exactly one."""
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header:
        return None
    width, height = int(header.group(1)), int(header.group(2))
    body = data[header.end():]
    return (width, height, body) if len(body) == width * height * 3 else None


def colours(body):
    """How many different colours there are, over every pixel."""
    return len({body[i:i + 3] for i in range(0, len(body), 3)})


def rows_changed(first, second, width):
    """The rows in which two pictures of one size differ."""
    row = width * 3
    return [y for y in range(len(first) // row) if first[y * row:(y + 1) * row] != second[y * row:(y + 1) * row]]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("guest", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--before", type=Path)
    args = parser.parse_args()
    out = args.out
    out.mkdir(parents=True)
    (out / "grub.cfg").write_text(GRUB)
    efi = out / "BOOTX64.EFI"
    subprocess.run(["grub-mkstandalone", "-O", "x86_64-efi", "-o", str(efi), "--modules=multiboot all_video gfxterm part_gpt part_msdos fat search normal efi_gop",
                    "boot/grub/grub.cfg=%s" % (out / "grub.cfg")], check=True, stderr=subprocess.DEVNULL)
    failed = 0

    def check(name, passed, detail=""):
        nonlocal failed
        failed += not passed
        print(("PASS " if passed else "FAIL ") + name + ((" | " + detail) if detail else ""))

    presented, lines, sent, (first, idle, after_h, typed) = run(args.kernel.resolve(), args.guest.resolve(), efi, out / "kernel")
    display = [line for line in lines if line.startswith("display:")]
    check("O1 the loader's display is one page of 1920 by 1080, mapped whole", any(line.startswith("display: Loader, 1920x1080, 1 page(s), 8100 KiB") for line in display), repr(display[:1]))
    check("O2 the kernel says a program that asks draws to memory of its own", OWN in display, repr(display[1:2]))
    shots = [picture(shot) for shot in (first, idle, after_h, typed)]
    whole = all(shot is not None and shot[:2] == (1920, 1080) for shot in shots)
    kinds = [colours(shot[2]) for shot in shots] if whole else []
    check("O3 the program presented, and all four screens are whole 1920x1080 pictures of more than one colour",
          presented and whole and min(kinds) >= 2, "colours %s" % kinds)
    by_h = rows_changed(shots[1][2], shots[2][2], 1920) if whole else []
    by_i = rows_changed(shots[2][2], shots[3][2], 1920) if whole else []
    check("O4 left alone the screen does not change; each accepted key changes it, in part",
          whole and sent and first == idle and 0 < len(by_h) < 1080 and 0 < len(by_i) < 1080,
          "keys accepted %s, idle the same %s, rows changed by h %d, by i %d" % (sent, first == idle, len(by_h), len(by_i)))
    if args.before:
        was, old_lines, _, (old_first, _, old_h, old_typed) = run(args.before.resolve(), args.guest.resolve(), efi, out / "before")
        check("O2 control: the kernel as it was does not say so", OWN not in old_lines)
        check("O3 control: the kernel as it was shows the same three pictures, byte for byte", was and old_first == first and old_h == after_h and old_typed == typed)
    print("%s: %d failed" % ("FAILED" if failed else "ALL PASS", failed))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
