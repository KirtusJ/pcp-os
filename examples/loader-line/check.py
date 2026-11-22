#!/usr/bin/env python3
"""The kernel takes the loader's line whole up to 1,023 bytes, and no further.

The limit was 255 bytes. A line names things by SHA-256 (the kernel's own measurement, a nominated
program, a model's descriptor), and three of those with their names do not fit in 255. A line that
does not fit is not taken whole, and then nothing it names is acted on, so the limit decides what
can be asked for at all.

The loader's line is the kernel's file name, a space, and what is appended. The word "then=off"
(end by itself when there is nothing to run) is placed last, after padding words, so the kernel
ends by itself only if it read that far.

Predicates, fixed before the first run.
 T1 with the line 200, 255, 256, 600 and exactly 1,023 bytes long the kernel reads the last word and
    ends by itself with status 33, every self-test passing.
 T2 with the line 1,024 and 1,200 bytes long the last word is beyond what is taken: the kernel does
    not end by itself within the time allowed (it is stopped by this check), and no self-test fails.
 T3 in every case the line the kernel prints begins with the line given, as far as it took it.

Usage: check.py <kernel.elf32> [--qemu qemu-system-x86_64]
This is an emulator's loader. A real loader may limit the line itself.
"""
import argparse
import subprocess
import sys
from pathlib import Path


def run(qemu, kernel, total, seconds):
    last = "then=off"
    room = total - len(kernel.name) - 1 - len(last) - 1
    words, n = [], 0
    while sum(len(w) + 1 for w in words) < room:
        words.append("p%03d=xxxxxxxx" % n)
        n += 1
    padding = " ".join(words)[:room].rstrip()
    padding = padding + "x" * (room - len(padding))
    append = padding + " " + last
    line = kernel.name + " " + append
    assert len(line) == total, (len(line), total)
    vm = subprocess.Popen([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "256M", "-kernel", kernel.name, "-append", append,
                           "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                          stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=kernel.parent)
    try:
        out, _ = vm.communicate(timeout=seconds)
        status = vm.returncode
    except subprocess.TimeoutExpired:
        vm.kill()
        out, _ = vm.communicate()
        status = None
    return status, out.decode(errors="replace").splitlines(), line


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    kernel = args.kernel.resolve()
    problems = []
    for total, read in ((200, True), (255, True), (256, True), (600, True), (1023, True), (1024, False), (1200, False)):
        status, lines, line = run(args.qemu, kernel, total, 120 if read else 45)
        printed = [l for l in lines if l.startswith("command line: ")]
        taken = printed[0][len("command line: "):] if printed else ""
        if not taken or not line.startswith(taken) or len(taken) != min(total, 1023):
            problems.append("%d bytes: the kernel printed %d bytes of the line, or other bytes" % (total, len(taken)))
        if any(l.startswith("  FAIL") for l in lines):
            problems.append("%d bytes: a self-test failed" % total)
        if read and status != 33:
            problems.append("%d bytes: the last word was not acted on (status %r)" % (total, status))
        if not read and status is not None:
            problems.append("%d bytes: the kernel ended by itself (status %r) on a word beyond what it takes" % (total, status))
        print("%d bytes: took %d, %s" % (total, len(taken), "ended by itself with %r" % status if status is not None else "did not act on the last word"))
    if problems:
        for problem in problems:
            print("FAIL " + problem, file=sys.stderr)
        sys.exit(1)
    print("loader line: taken whole to 1,023 bytes; a longer one is not acted on past that")


if __name__ == "__main__":
    main()
