#!/usr/bin/env python3
"""On a display with one page, is a picture seen while it is being made? A program that stops half way says.

C3's probe (pull request 428), written again in this repository as probe/. The program shows one picture, all
of one colour; draws the top half of a second in another colour and waits for a key; draws the bottom half and
waits again; and only then asks for the second to be shown. The screen is read at each wait. Because the
program waits for as long as it takes to read the screen, the answer does not depend on how fast an emulator
writes a framebuffer, which is what the still pictures of check.py could not get round.

Booted as the owner's laptop boots: UEFI firmware, GRUB, a framebuffer the loader set at 1920 by 1080 and no
display adapter the kernel drives.

Predicates, fixed before the first run. "All of a colour" means every one of the 2,073,600 pixels of a screen
file whose header says 1920 by 1080 and whose body is exactly that long.
 P1 the kernel reports "display: Loader, 1920x1080, 1 page(s)", and the program says it was told to draw to
    memory of its own.
 P2 at the first wait (top half of the second picture drawn, not shown) the screen is all of the first colour.
 P3 at the second wait (all of the second picture drawn, not shown) the screen is all of the first colour.
 P4 after the program asks, the screen is all of the second colour.
 P5 the monitor accepted each of the two keys that moved the program on, and the program said each step in order.
 With --before (a kernel from before the correction), the control, which must show the fault:
 P6 the program says it draws on the display itself; at the first wait the top 540 rows are all of the second
    colour and the bottom 540 all of the first; at the second wait the screen is all of the second colour,
    before the program has asked for anything to be shown.

What this does not show: how long a picture takes to reach a real panel, or anything on the laptop.

Usage: check_pause.py <kernel.elf32> <probe ELF> <new output directory> [--before <kernel.elf32>]
The probe is built from probe/ with: cargo build --release --target x86_64-unknown-none
Needs qemu-system-x86_64, OVMF at /usr/share/OVMF, grub-mkstandalone with the x86_64-efi modules, and mtools.
"""
import argparse
import json
import runpy
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SHARED = runpy.run_path(str(HERE / "check.py"))
FIRST, SECOND = bytes((0x20, 0x60, 0x80)), bytes((0xE0, 0xA0, 0x30))
STEPS = ["probe: first picture shown", "probe: top half of the second drawn, not shown", "probe: all of the second drawn, not shown", "probe: second picture shown"]


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

    def said(words, within):
        until = time.time() + within
        while time.time() < until:
            if serial.exists() and words in serial.read_text(errors="replace"):
                return True
            time.sleep(0.3)
        return False

    keys, shots = [], {}
    try:
        vm.stdout.readline()
        call("qmp_capabilities")
        for step, name, key in ((STEPS[1], "first-wait", True), (STEPS[2], "second-wait", True), (STEPS[3], "shown", False)):
            if not said(step, 180 if not shots else 30):
                break
            time.sleep(1.5)  # the screen is read well after the program began to wait
            call("screendump", filename=str(out / (name + ".ppm")))
            shots[name] = (out / (name + ".ppm")).read_bytes()
            if key:
                keys.append("return" in call("send-key", keys=[{"type": "qcode", "data": "spc"}]))
    finally:
        vm.kill()
        vm.wait()
    return serial.read_text(errors="replace").splitlines(), keys, shots


def all_of(shot, colour, rows=(0, 1080)):
    """Whether rows [from, to) of a whole 1920x1080 screen file are every pixel `colour`."""
    read = SHARED["picture"](shot) if shot else None
    if read is None or read[:2] != (1920, 1080):
        return False
    return read[2][rows[0] * 5760:rows[1] * 5760] == colour * (1920 * (rows[1] - rows[0]))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("guest", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--before", type=Path)
    args = parser.parse_args()
    out = args.out
    out.mkdir(parents=True)
    (out / "grub.cfg").write_text(SHARED["GRUB"])
    efi = out / "BOOTX64.EFI"
    subprocess.run(["grub-mkstandalone", "-O", "x86_64-efi", "-o", str(efi), "--modules=multiboot all_video gfxterm part_gpt part_msdos fat search normal efi_gop",
                    "boot/grub/grub.cfg=%s" % (out / "grub.cfg")], check=True, stderr=subprocess.DEVNULL)
    failed = 0

    def check(name, passed, detail=""):
        nonlocal failed
        failed += not passed
        print(("PASS " if passed else "FAIL ") + name + ((" | " + detail) if detail else ""))

    lines, keys, shots = run(args.kernel.resolve(), args.guest.resolve(), efi, out / "kernel")
    probe = [line for line in lines if line.startswith("probe:")]
    check("P1 one page of 1920 by 1080, and the program is told to draw to memory of its own",
          any(line.startswith("display: Loader, 1920x1080, 1 page(s)") for line in lines) and probe[:1] == ["probe: told to draw to memory of its own"], repr(probe[:1]))
    check("P2 at the first wait the screen is all of the first colour", all_of(shots.get("first-wait"), FIRST))
    check("P3 at the second wait the screen is all of the first colour", all_of(shots.get("second-wait"), FIRST))
    check("P4 once the program asks, the screen is all of the second colour", all_of(shots.get("shown"), SECOND))
    check("P5 both keys were accepted and the program said each step in order", keys == [True, True] and probe[1:] == STEPS, repr((keys, probe)))
    if args.before:
        lines, keys, shots = run(args.before.resolve(), args.guest.resolve(), efi, out / "before")
        probe = [line for line in lines if line.startswith("probe:")]
        check("P6 control: the kernel as it was shows the half-made picture, and the whole one before it is asked for",
              probe[:1] == ["probe: draws on the display itself"] and keys == [True, True]
              and all_of(shots.get("first-wait"), SECOND, (0, 540)) and all_of(shots.get("first-wait"), FIRST, (540, 1080)) and all_of(shots.get("second-wait"), SECOND),
              repr((probe, keys)))
    print("%s: %d failed" % ("FAILED" if failed else "ALL PASS", failed))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
