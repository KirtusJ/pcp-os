#!/usr/bin/env python3
"""The kernel on a machine with no serial port (issue 309).

The kernel writes its report to the first serial port. Most laptops have none. It used to set the port
up and write every byte without knowing a port was there, waiting up to a million polls for each byte
if the address read as never ready. It now looks first: a line-status register that reads all ones is
no port, and a port must keep two values in its scratch register before anything else is written.

Predicates, fixed before the first run. The same kernel is booted twice in QEMU with `then=off`:
 N1 with a serial port: exit code 33, the log says ALL SELF-TESTS PASS and has no "  FAIL" line;
 N2 with no serial port (`-serial none`): exit code 33 inside the time limit, in no more than three
    times the seconds the first boot took plus ten.
The second boot shows that the kernel verifies itself and ends without a port to report to, and does not
crawl. It cannot show the report, which is written nowhere: on such a machine the screen is the report.

Usage: no_serial.py <kernel.elf32> [--qemu qemu-system-x86_64]
"""
import argparse
from pathlib import Path
import subprocess
import sys
import time


def boot(qemu, kernel, serial, timeout):
    measure = Path(__file__).resolve().parents[1] / "tools" / "measure.py"
    digest = subprocess.check_output([sys.executable, str(measure), str(kernel)], text=True).strip()
    began = time.monotonic()
    result = subprocess.run([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "128M", "-smp", "1", "-nic", "none",
                             "-display", "none", "-monitor", "none", "-serial", serial, "-no-reboot",
                             "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-kernel", str(kernel),
                             "-append", "image=%s then=off" % digest], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=timeout)
    return result.returncode, result.stdout.decode(errors="replace"), time.monotonic() - began


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    problems = []
    code, log, took = boot(args.qemu, args.kernel, "stdio", args.timeout)
    if code != 33 or "ALL SELF-TESTS PASS" not in log or any(line.startswith("  FAIL") for line in log.splitlines()):
        problems.append("N1: with a serial port the kernel ended with %d" % code)
    try:
        silent_code, silent_log, silent_took = boot(args.qemu, args.kernel, "none", min(args.timeout, took * 3 + 10))
        if silent_code != 33:
            problems.append("N2: with no serial port the kernel ended with %d" % silent_code)
    except subprocess.TimeoutExpired:
        problems.append("N2: with no serial port the kernel did not end in three times %.1f s plus ten" % took)
        silent_took = None
    for problem in problems:
        print("FAILED", problem)
    if problems:
        sys.exit(1)
    print("no serial port: the kernel verified itself and ended in %.1f s without one (%.1f s with one)" % (silent_took, took))


if __name__ == "__main__":
    main()
