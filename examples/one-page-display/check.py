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
 O3 the program presents its first frame, and the screen after it, and again after two letters are typed, is
    not one colour. With --before the two screens are byte for byte the same as this kernel's: what is shown
    is the same picture, and only where it was drawn has changed.

What this does not show: the repaint itself. An emulator writes a framebuffer at the speed of memory, so the
wipe the owner saw cannot be seen here, with either kernel. That the wipe is gone needs the laptop.

Usage: check.py <kernel.elf32> <drawing guest ELF> <new output directory> [--before <kernel.elf32>]
Needs qemu-system-x86_64, OVMF at /usr/share/OVMF, grub-mkstandalone with the x86_64-efi modules, and mtools.
The guest is started as "tailorbird" (auto=tailorbird); any program that asks to draw through the kernel will do.
"""
import argparse
import json
import os
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
        for letter in ("h", "i"):
            call("send-key", keys=[{"type": "qcode", "data": letter}])
            time.sleep(0.6)
        time.sleep(1.5)
        call("screendump", filename=str(out / "typed.ppm"))
    finally:
        vm.kill()
        vm.wait()
    return presented, serial.read_text(errors="replace").splitlines(), (out / "first.ppm").read_bytes(), (out / "typed.ppm").read_bytes()


def one_colour(picture):
    body = picture.split(b"\n", 3)[3]
    return len({body[i:i + 3] for i in range(0, len(body), 3 * 997)}) < 2


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

    presented, lines, first, typed = run(args.kernel.resolve(), args.guest.resolve(), efi, out / "kernel")
    display = [line for line in lines if line.startswith("display:")]
    check("O1 the loader's display is one page of 1920 by 1080, mapped whole", any(line.startswith("display: Loader, 1920x1080, 1 page(s), 8100 KiB") for line in display), repr(display[:1]))
    check("O2 the kernel says a program that asks draws to memory of its own", OWN in display, repr(display[1:2]))
    check("O3 the program presented, and both screens hold a picture", presented and not one_colour(first) and not one_colour(typed) and first != typed)
    if args.before:
        was, old_lines, old_first, old_typed = run(args.before.resolve(), args.guest.resolve(), efi, out / "before")
        check("O2 control: the kernel as it was does not say so", OWN not in old_lines)
        check("O3 control: the kernel as it was shows the same two pictures, byte for byte", was and old_first == first and old_typed == typed)
    print("%s: %d failed" % ("FAILED" if failed else "ALL PASS", failed))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
