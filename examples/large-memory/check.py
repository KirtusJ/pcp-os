#!/usr/bin/env python3
"""One program can be given the machine's memory above four gibibytes, and it all comes back.

The kernel used only the memory below one gibibyte: a program got 960 MiB whatever the machine had,
which a large language model cannot be loaded into. A program's own pages are now taken from the
usable memory above four gibibytes where there is any (page tables, and frames a device is told the
address of, stay below one gibibyte), and from the pool below as before when there is none.

The probe (probe/, built for x86_64-unknown-none) maps 64 MiB at a time and writes to every page.

Predicates, fixed before the first run.
 L1 with 1,024 and 2,048 MiB of machine memory (none above four gibibytes) the program gets 960 MiB
    and is then refused with error 12, as before this change, and no line about high memory appears.
 L2 with 8,192 MiB the program gets all of the memory above four gibibytes, then the 960 MiB below:
    6,080 MiB on machine "pc" (5,120 MiB above, 1,310,720 frames) and 7,104 MiB on "q35" (6,144 MiB
    above, 1,572,864 frames). The kernel's line for high memory shows that many frames, all free at
    the end, and as many given back as handed out.
 L4 every page handed to the program reads zero at both ends before the program writes to it.
 L6 every page is written with a value of its own at both ends, and after the last piece has been
    written every page of every piece is read again: all of them hold what was written (no two pages
    are one piece of memory, and none lost its contents).
 L5 (with --demo-kernel, built with the high-memory-demo feature, at 8,192 MiB) what a device would be
    given, a run of frames and a single frame, lies below one gibibyte although memory above four is
    free; a program's frame lies above four gibibytes, takes a write, is given back, and reads zero
    when taken again.
 L3 at every size the emulator ends with status 33, 71 self-tests pass and none fails, and the pool
    below one gibibyte has every frame free at the end.

Usage: check.py <kernel.elf32> <memory-probe> [--qemu qemu-system-x86_64]
This is an emulator. It says nothing of a real machine's memory map, of memory between one and four
gibibytes (not used), or of speed.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

# What a machine of 8,192 MiB has above four gibibytes is the emulated machine's own layout, not the
# kernel's: "pc" keeps 3 GiB below and puts 5 GiB above; "q35" keeps 2 GiB below and puts 6 GiB above
# (B3 ran this on q35 and found 7,104 MiB where this check had only "pc"'s figure).
MACHINES = {"pc": (6080, 1310720), "q35": (7104, 1572864)}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("probe", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--demo-kernel", type=Path)
    parser.add_argument("--machine", choices=sorted(MACHINES), default="pc")
    args = parser.parse_args()
    kernel, probe = args.kernel.resolve(), args.probe.resolve()
    if probe.name != "memory-probe":
        sys.exit("the program must be in a file named memory-probe")
    problems = []
    big, frames = MACHINES[args.machine]
    for mebibytes, want, high in ((1024, 960, None), (2048, 960, None), (8192, big, frames)):
        done = subprocess.run(
            [args.qemu, "-machine", args.machine, "-accel", "tcg", "-cpu", "max", "-m", "%dM" % mebibytes, "-kernel", kernel.name, "-initrd", str(probe),
             "-append", "auto=memory-probe then=off", "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot",
             "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=kernel.parent, timeout=900)
        lines = done.stdout.decode(errors="replace").splitlines()
        said = [line for line in lines if line.startswith("memory: ") and "so far" not in line]
        if said != ["memory: refused with 12", "memory: pages that did not read zero 0", "memory: pages read again after all were written %d" % (want * 256),
                    "memory: of those, pages that did not hold what was written 0", "memory: mebibytes mapped and touched %d" % want]:
            problems.append("%d MiB: the program said %r, not refused with 12 after %d MiB" % (mebibytes, said, want))
        passes = sum(1 for line in lines if line.startswith("  pass "))
        if done.returncode != 33 or passes != 71 or any(line.startswith("  FAIL") for line in lines):
            problems.append("%d MiB: status %r with %d passes, or a failed self-test" % (mebibytes, done.returncode, passes))
        low = [re.search(r"frames: (\d+) of (\d+) free, (\d+) handed out and (\d+) given back", line) for line in lines]
        low = [m for m in low if m]
        if len(low) != 1 or low[0].group(1) != low[0].group(2) or low[0].group(3) != low[0].group(4):
            problems.append("%d MiB: the pool below one gibibyte did not get every frame back" % mebibytes)
        above = [re.search(r"memory above four gibibytes: (\d+) of (\d+) frames free, (\d+) handed out and (\d+) given back", line) for line in lines]
        above = [tuple(int(n) for n in m.groups()) for m in above if m]
        if high is None and above:
            problems.append("%d MiB: high memory was reported on a machine that has none: %r" % (mebibytes, above))
        if high is not None and above != [(high, high, high, high)]:
            problems.append("%d MiB: high memory reported as %r, not %d frames all handed out once and all returned" % (mebibytes, above, high))
        print("%s, %d MiB: %s%s" % (args.machine, mebibytes, "; ".join(said), "; high frames %r" % (above,) if above else ""))
    if args.demo_kernel:
        demo = args.demo_kernel.resolve()
        done = subprocess.run([args.qemu, "-machine", args.machine, "-accel", "tcg", "-cpu", "max", "-m", "8192M", "-kernel", demo.name, "-append", "then=off",
                               "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                              stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=demo.parent, timeout=600)
        lines = done.stdout.decode(errors="replace").splitlines()
        said = [line for line in lines if line.startswith("high memory demo: ")]
        want = "high memory demo: run below one gibibyte 1, single frame below one gibibyte 1, program frame above four gibibytes 1, written 1, all given back 1, zeroed when taken again 1"
        if said != [want] or done.returncode != 33 or any(line.startswith("  FAIL") for line in lines):
            problems.append("the demonstration kernel said %r (status %r)" % (said, done.returncode))
        print("demo: %s" % "; ".join(said))
    else:
        print("L5 not run: no --demo-kernel given")
    if problems:
        for problem in problems:
            print("FAIL " + problem, file=sys.stderr)
        sys.exit(1)
    print("large memory: a program gets the memory above four gibibytes and the 960 MiB below; all of it comes back")


if __name__ == "__main__":
    main()
