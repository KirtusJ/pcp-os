#!/usr/bin/env python3
"""The kernel on a keyboard controller that does not replay keys (issue 309).

The self-tests of keys, sleep and stopping a program press keys by asking the keyboard controller to
replay them (its command 0xD2). An emulated controller does. A laptop's is a program in its embedded
controller and need not; a kernel that assumed it did would wait for a key that never comes, or call a
healthy machine unverified. The kernel therefore asks first, with one byte, and where the answer is no it
does not run those tests and says so.

No emulator here lacks the command, so the second kernel is built with the feature `no-key-replay-demo`,
which makes the question answer no without touching a port. This shows what the kernel does with that
answer; it does not show what a real controller answers.

Predicates, fixed before the first run. Each kernel is booted in QEMU with `then=off`, no disk, no network:
 R1 the default kernel ends with QEMU's exit code 33, says ALL SELF-TESTS PASS, has no line beginning
    "  FAIL" and no line saying that tests were not run;
 R2 the demonstration kernel ends with exit code 33 inside the time limit, says ALL SELF-TESTS PASS, has
    no line beginning "  FAIL", and has the line "the self-tests that press keys were not run: this keyboard
    controller does not replay keys for a test";
 R3 the demonstration kernel passes exactly six fewer self-tests than the default kernel: the six that
    press keys, and no other (B3 found that the first form of this left out seven that press none).

Usage: replay_probe.py <default kernel.elf32> <demonstration kernel.elf32> [--qemu qemu-system-x86_64]
"""
import argparse
from pathlib import Path
import subprocess
import sys

NOT_RUN = "the self-tests that press keys were not run: this keyboard controller does not replay keys for a test"


def boot(qemu, kernel, timeout):
    measure = Path(__file__).resolve().parents[1] / "tools" / "measure.py"
    digest = subprocess.check_output([sys.executable, str(measure), str(kernel)], text=True).strip()
    result = subprocess.run([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "128M", "-smp", "1", "-nic", "none",
                             "-display", "none", "-monitor", "none", "-serial", "stdio", "-no-reboot",
                             "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-kernel", str(kernel),
                             "-append", "image=%s then=off" % digest], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=timeout)
    log = result.stdout.decode(errors="replace")
    return result.returncode, log, sum(line.startswith("  pass ") for line in log.splitlines())


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("default", type=Path)
    parser.add_argument("demonstration", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    problems = []
    code, log, passed = boot(args.qemu, args.default, args.timeout)
    if code != 33 or "ALL SELF-TESTS PASS" not in log or any(line.startswith("  FAIL") for line in log.splitlines()) or "were not run" in log:
        problems.append("R1: the default kernel ended with %d after %d passes" % (code, passed))
    try:
        demo_code, demo_log, demo_passed = boot(args.qemu, args.demonstration, args.timeout)
    except subprocess.TimeoutExpired:
        problems.append("R2: the demonstration kernel did not end inside %d s" % args.timeout)
        demo_code, demo_log, demo_passed = None, "", 0
    if demo_code != 33 or "ALL SELF-TESTS PASS" not in demo_log or any(line.startswith("  FAIL") for line in demo_log.splitlines()) or NOT_RUN not in demo_log.splitlines():
        problems.append("R2: the demonstration kernel ended with %r after %d passes" % (demo_code, demo_passed))
    if demo_passed != passed - 6:
        problems.append("R3: %d passes without replay against %d with it" % (demo_passed, passed))
    for problem in problems:
        print("FAILED", problem)
    if problems:
        sys.exit(1)
    print("replay probe: %d self-tests with replay, %d without, and the kernel says which were not run" % (passed, demo_passed))


if __name__ == "__main__":
    main()
