#!/usr/bin/env python3
"""The kernel verifies itself with any amount of memory (issue 309; found by B3 at 2,048 MiB).

With two gibibytes or more the kernel's frame memory runs up to the one-gibibyte line. A start-up check
asked that the last page below that line be unmapped, which is right only on a smaller machine: the
check failed, the kernel went to SAFE and offered no programs. A laptop has more memory than that.

Predicates, fixed before the first run.
 M1 at each of 128, 1024, 2048, 4096 and 8192 MiB the emulator ends with status 33 (every self-test
    passed), the serial log has no line beginning "  FAIL", and it holds "ALL SELF-TESTS PASS.".
 M2 the number of lines beginning "  pass " is the same at every size: no check is skipped or added
    by the amount of memory.

Usage: high_memory.py <kernel.elf32> [--qemu qemu-system-x86_64]
This is an emulator. It says nothing of a real machine's memory map.
"""
import argparse
import subprocess
import sys

SIZES = (128, 1024, 2048, 4096, 8192)


def boot(qemu, kernel, mebibytes):
    done = subprocess.run(
        [qemu, "-cpu", "max", "-kernel", kernel, "-m", "%dM" % mebibytes, "-display", "none", "-serial", "stdio",
         "-no-reboot", "-append", "then=off", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=300)
    lines = done.stdout.decode(errors="replace").splitlines()
    return done.returncode, lines


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    problems, passes = [], {}
    for size in SIZES:
        status, lines = boot(args.qemu, args.kernel, size)
        failed = [line for line in lines if line.startswith("  FAIL")]
        passes[size] = sum(line.startswith("  pass ") for line in lines)
        if status != 33 or failed or not any(line.startswith("ALL SELF-TESTS PASS.") for line in lines):
            problems.append("M1 at %d MiB: status %d, %d failed%s" % (size, status, len(failed), ": " + failed[0].strip() if failed else ""))
    if len(set(passes.values())) != 1 or 0 in passes.values():
        problems.append("M2: the number of checks passed differs by memory: %r" % passes)
    if problems:
        sys.exit("\n".join(problems))
    print("high memory: %d checks passed at each of %s MiB" % (passes[SIZES[0]], ", ".join(str(s) for s in SIZES)))


if __name__ == "__main__":
    main()
