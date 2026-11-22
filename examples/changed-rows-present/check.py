#!/usr/bin/env python3
"""Does the kernel write to a one-page display only the rows a program's new picture changed?

On a display with one page a program draws in memory of its own and the kernel shows each finished
picture (examples/one-page-display). Showing it used to write every row every time: on the owner's
HP EliteBook 840 G7 that was about a second for each picture, and typed letters waited behind it.
The kernel now keeps, in frames of the pool, what it last put on the display for the program, and
writes only where the new picture differs.

This boots as that laptop does (OVMF, GRUB, a framebuffer the loader set at 1920 by 1080, no display
adapter the kernel drives) with a kernel built with `proposals,present-rows-demo`, which says on the
serial line how many rows each picture wrote. The program is Tailorbird, started by the loader's line.

Predicates, fixed before the first run. "A picture" is one line "present: N of 1080 rows written".
 R1 the first picture writes every row: 1080 of 1080.
 R2 a letter typed into the still screen gives at least one further picture, and every picture
    between the first and the next step writes at least 1 row and fewer than 270 (a quarter of the
    screen: the letter appears in the field at the foot of the window, and nothing above it moves).
 R3 after the kernel itself has drawn on the display while the program holds it (F12 with nothing
    waiting brings the kernel's own "No file is waiting for your approval." screen; a key returns),
    the program's next picture writes every row again: 1080 of 1080.
 R4 and the letter after that is again a small picture: at least 1 row, fewer than 270.
 R5 the screen after each step (first picture, first letter, second letter, the letter after the
    kernel drew, one more letter) is byte for byte the screen at the same step under the control
    kernel, which is this repository's kernel before the change, built with `proposals`.
 R6 after the program is left (Esc) and the machine is switched off (F10, Enter), the kernel's own
    count says every frame of the pool is free and as many were given back as were handed out.
 R7 with the loader's line carrying present_keep=refuse (the demo's stand-in for a pool that cannot
    spare the frames), the kernel says so, every picture writes 1080 of 1080, none fails, the five
    screens are again byte for byte the control's, and R6 holds.

What this does not show: time. An emulator writes a framebuffer at the speed of memory, so how much
faster a typed letter reaches the laptop's panel is not measured here; only how many rows are written.

Usage: check.py <kernel with proposals,present-rows-demo> <control kernel with proposals> <Tailorbird ELF> <new directory>
Needs qemu-system-x86_64, OVMF at /usr/share/OVMF, grub-mkstandalone with the x86_64-efi modules, and mtools.
A failed predicate is printed and the run exits 1 after all boots; serial logs and screens are kept either way.
"""
import hashlib
import json
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
multiboot /kernel.elf32 kernel auto=tailorbird then=menu propose_image=%s%s
module /tailorbird tailorbird
boot
"""
STEPS = ("first", "letter-1", "letter-2", "after-kernel-drew", "letter-4")
PRESENT = re.compile(r"^present: (\d+) of (\d+) rows written$")
CENSUS = re.compile(r"frames: (\d+) of (\d+) free, (\d+) handed out and (\d+) given back")


def boot(kernel, guest, out, extra=""):
    """One boot through the five steps and a shutdown. Returns (serial lines seen at each step, the five screens, all serial lines)."""
    out.mkdir(parents=True)
    (out / "grub.cfg").write_text(GRUB % (hashlib.sha256(guest.read_bytes()).hexdigest(), extra))
    efi = out / "BOOTX64.EFI"
    subprocess.run(["grub-mkstandalone", "-O", "x86_64-efi", "-o", str(efi), "--modules=multiboot all_video gfxterm part_gpt part_msdos fat search normal efi_gop",
                    "boot/grub/grub.cfg=%s" % (out / "grub.cfg")], check=True, stderr=subprocess.DEVNULL)
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

    def key(*names):
        call("send-key", keys=[{"type": "qcode", "data": name} for name in names])

    def lines():
        return serial.read_text(errors="replace").splitlines() if serial.exists() else []

    def settle(tag, seconds=2.5):
        """Wait until the serial log has been still for a moment, take the screen, and say how many lines there are."""
        time.sleep(seconds)
        count = len(lines())
        while True:
            time.sleep(1.0)
            if len(lines()) == count:
                break
            count = len(lines())
        call("screendump", filename=str(out / (tag + ".ppm")))
        return count

    marks, screens = [], {}
    try:
        vm.stdout.readline()
        call("qmp_capabilities")
        deadline = time.time() + 240
        while time.time() < deadline and not any("first frame presented" in line for line in lines()):
            time.sleep(0.5)
        marks.append(settle("first"))
        key("h")
        marks.append(settle("letter-1"))
        key("i")
        marks.append(settle("letter-2"))
        key("f12")                      # the kernel's own screen, over the program
        time.sleep(2.0)
        call("screendump", filename=str(out / "kernel-screen.ppm"))
        key("spc")                      # any key returns to the program
        time.sleep(1.5)
        key("j")
        marks.append(settle("after-kernel-drew"))
        key("k")
        marks.append(settle("letter-4"))
        key("esc")
        time.sleep(3.0)
        key("f10")
        time.sleep(1.5)
        key("ret")
        try:
            vm.wait(timeout=30)
        except subprocess.TimeoutExpired:
            pass
    finally:
        vm.kill()
        vm.wait()
    for tag in STEPS + ("kernel-screen",):
        screens[tag] = (out / (tag + ".ppm")).read_bytes() if (out / (tag + ".ppm")).exists() else b""
    return marks, screens, lines()


def pictures(lines, start, end):
    """The (rows written, rows of the display) of each picture said between two places in the log."""
    return [tuple(int(n) for n in m.groups()) for m in (PRESENT.match(line) for line in lines[start:end]) if m]


def census_whole(lines):
    found = [CENSUS.search(line) for line in lines]
    found = [tuple(int(n) for n in m.groups()) for m in found if m]
    return bool(found) and found[-1][0] == found[-1][1] and found[-1][2] == found[-1][3], (found[-1] if found else None)


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    kernel, control, guest, out = (Path(a).resolve() for a in sys.argv[1:])
    out.mkdir(parents=True, exist_ok=False)
    failed = []

    def predicate(name, holds, detail=""):
        print(("PASS " if holds else "FAIL ") + name + ((" | " + detail) if detail else ""))
        if not holds:
            failed.append(name)

    marks, screens, lines = boot(kernel, guest, out / "kernel")
    small = lambda group: bool(group) and all(1 <= rows < 270 and of == 1080 for rows, of in group)
    first = pictures(lines, 0, marks[0])
    one, two = pictures(lines, marks[0], marks[1]), pictures(lines, marks[1], marks[2])
    after, last = pictures(lines, marks[2], marks[3]), pictures(lines, marks[3], marks[4])
    predicate("R1 the first picture writes every row", first[:1] == [(1080, 1080)], repr(first))
    predicate("R2 a typed letter is a small picture", small(one) and small(two), "%r then %r" % (one, two))
    predicate("R3 after the kernel drew, the next picture writes every row", after[:1] == [(1080, 1080)], repr(after))
    predicate("R4 and the letter after that is small again", small(last), repr(last))

    _, control_screens, control_lines = boot(control, guest, out / "control")
    same = {tag: screens[tag] == control_screens[tag] and len(screens[tag]) > 1920 * 1080 * 3 for tag in STEPS}
    kernel_drew = screens["kernel-screen"] != screens["letter-2"] and len(screens["kernel-screen"]) > 1920 * 1080 * 3
    predicate("R5 every screen is the control's, byte for byte", all(same.values()) and kernel_drew and not pictures(control_lines, 0, None),
              "%r; the kernel's own screen differed from the program's: %s" % (same, kernel_drew))
    whole, census = census_whole(lines)
    predicate("R6 at shutdown every frame is back in the pool", whole, repr(census))

    r_marks, r_screens, r_lines = boot(kernel, guest, out / "refused", " present_keep=refuse")
    every = pictures(r_lines, 0, None)
    said = any("the frames for a kept picture are refused" in line for line in r_lines)
    r_same = all(r_screens[tag] == control_screens[tag] for tag in STEPS)
    r_whole, r_census = census_whole(r_lines)
    predicate("R7 where the frames are refused every row is written and nothing fails",
              said and len(every) >= 5 and all(p == (1080, 1080) for p in every) and r_same and r_whole,
              "said %s, %d pictures, all whole %s, screens the control's %s, pool %r" % (said, len(every), all(p == (1080, 1080) for p in every), r_same, r_census))

    if failed:
        sys.exit("failed: " + ", ".join(failed))
    print("all seven hold")


if __name__ == "__main__":
    main()
