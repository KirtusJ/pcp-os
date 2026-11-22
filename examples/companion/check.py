#!/usr/bin/env python3
"""A program and its companion run side by side and exchange bytes; each keeps its own registers.

A language model will run as a program of its own beside the one a person uses. The kernel ran one
loaded program at a time. With the `companion` feature a program named on the loader's line by its
digest is started beside another named the same way, with the console and one end of a channel
(dev/talk) and nothing else; the two run in turn; when the foreground program ends the kernel ends
the companion.

Three builds of probe/ are given: talk-front, talk-back, and a talk-back that leaves early.

Predicates, fixed before the first run.
 P1 together: 100 exchanges of 1,000 bytes complete with no byte wrong (the companion reads at most 7
    bytes at a time, so each exchange is many short reads and writes).
 P2 a channel written to without reading says so (-11) after taking exactly 65,536 bytes, and all
    of them come back once it is read.
 P3 each program, where the processor has 256-bit registers, kept its own pattern through every
    switch: both report 0 times that the register held something else, and both used the registers.
 P4 told to finish, the companion ends by itself and the front's next read answers 0.
 P5 with the companion that leaves early: the front's read answers 0 once what was written is read,
    and its write then answers -32; the front carries on and ends by itself.
 P6 without the words that name a companion the same front program is refused the channel (-13).
 P7 with the words but on a line whose worker digest matches no loaded file, likewise refused.
 P8 every run ends with status 33, 71 self-tests pass, none fails, the kernel does not stop, and
    every frame is given back.

Usage: check.py <kernel.elf32 built with --features companion> <talk-front> <talk-back> <leaving talk-back>
This is an emulator, and two small test programs: nothing here is a model or a person's program.
"""
import argparse
import hashlib
import re
import subprocess
import sys
from pathlib import Path


def boot(qemu, kernel, files, append):
    done = subprocess.run([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "256M", "-kernel", kernel.name, "-initrd", ",".join(str(f) for f in files),
                           "-append", append, "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                          stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=kernel.parent, timeout=600)
    return done.returncode, done.stdout.decode(errors="replace").splitlines()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("front", type=Path)
    parser.add_argument("back", type=Path)
    parser.add_argument("leaving", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    kernel, front, back, leaving = (p.resolve() for p in (args.kernel, args.front, args.back, args.leaving))
    if front.name != "talk-front" or back.name != "talk-back" or leaving.name != "talk-back":
        sys.exit("the programs must be in files named talk-front, talk-back and (in another folder) talk-back")
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    problems = []

    def common(name, status, lines):
        passes = sum(1 for line in lines if line.startswith("  pass "))
        if status != 33 or passes != 71 or any(line.startswith("  FAIL") or "KERNEL STOPPED" in line for line in lines):
            problems.append("%s: status %r, %d passes, or a failure" % (name, status, passes))
        frames = [m for m in (re.search(r"frames: (\d+) of (\d+) free, (\d+) handed out and (\d+) given back", line) for line in lines) if m]
        if len(frames) != 1 or frames[0].group(1) != frames[0].group(2) or frames[0].group(3) != frames[0].group(4):
            problems.append("%s: not every frame was given back" % name)

    def said(lines, who):
        own = [line[len(who):] for line in lines if line.startswith(who)]
        return own + [line.split(" wrote: ", 1)[1][len(who):] for line in lines if line.startswith("companion ") and " wrote: " + who in line]

    def require(name, got, wanted):
        for words in wanted:
            if words not in got:
                problems.append("%s: no line %r in %r" % (name, words, got))

    words = "auto=talk-front then=off worker_for=%s worker_image=%s"
    status, lines = boot(args.qemu, kernel, [front, back], words % (digest(front), digest(back)))
    common("together", status, lines)
    require("together, front", said(lines, "front: "), ["opening the channel answered 0", "exchanges completed 100", "bytes that came back wrong 0",
            "the full channel answered -11", "bytes it took before that 65536", "bytes that came back after it 65536", "telling it to finish answered 1",
            "the read after that answered 0", "wide registers used 1", "times its register held something else 0"])
    require("together, back", said(lines, "back: "), ["opening the channel answered 0", "told to finish after 165536", "bytes answered 165536",
            "wide registers used 1", "times its register held something else 0"])
    if not any(line.startswith("companion talk-back: it exited with 0") for line in lines):
        problems.append("together: the companion did not end by itself when told to finish")
    print("together: " + "; ".join(said(lines, "front: ")))

    status, lines = boot(args.qemu, kernel, [front, leaving], words % (digest(front), digest(leaving)))
    common("companion leaves", status, lines)
    require("companion leaves, front", said(lines, "front: "), ["the other end was gone in round 1", "that read answered 0", "a write to it then answered -32",
            "exchanges completed 1", "bytes that came back wrong 0", "times its register held something else 0"])
    print("companion leaves: " + "; ".join(said(lines, "front: ")))

    for name, append in (("no companion named", "auto=talk-front then=off"),
                         ("a worker that is not loaded", words % (digest(front), "0" * 64))):
        status, lines = boot(args.qemu, kernel, [front, back], append)
        common(name, status, lines)
        if said(lines, "front: ") != ["opening the channel answered -13"] or any("companion" in line for line in lines if not line.startswith("  ")):
            problems.append("%s: the channel was not refused, or a companion was started: %r" % (name, said(lines, "front: ")))
        print("%s: %s" % (name, "; ".join(said(lines, "front: "))))

    if problems:
        for problem in problems:
            print("FAIL " + problem, file=sys.stderr)
        sys.exit(1)
    print("companion: two programs side by side exchange bytes exactly, each keeps its registers, and neither outlives the run")


if __name__ == "__main__":
    main()
