#!/usr/bin/env python3
"""The kernel on a keyboard controller that does not replay keys (issue 309).

Six self-tests ask the keyboard controller to replay keystrokes using command 0xD2. The QEMU
controller supports it. When the probe reports that replay is unavailable on a verified kernel,
the kernel skips those six, explains why, and runs the seven other checks in the same group.

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
from collections import Counter
from pathlib import Path
import subprocess
import sys

NOT_RUN = "the self-tests that press keys were not run: this keyboard controller does not replay keys for a test"

# The two builds differ only in whether the controller replays scripted keys.
# Use the full pass labels so an unrelated pass cannot replace a missing check.
KEY_CHECKS = (
    "the stop key ends a program that never yields, long before its budget",
    "a program the person stopped is STOPPED in the kernel's task table, as it is in what the kernel observed of it",
    "a program waiting in a read of the keyboard is woken by each key's interrupt and reads the keys in order: it idled until the first and exited 0 on the third",
    "the stop key ends a program that is asleep (for the longest sleep accepted: its deadline does not overflow) at once, and all its memory comes back",
    "the stop key ends a program that waits in a read of the keyboard; it had read Ctrl and Alt and was never handed Delete; all its memory comes back",
    "a stop key pressed before a program is started does not stop it",
)
REPLAY_INDEPENDENT = {
    "rejected batch returns the frames of every member": 3,
    "a program that sleeps 50 ms is woken at its millisecond: the processor idled 51 timer periods, no fewer and no more, and the sleep answered 0": 1,
    "a program under a name of 32 bytes is admitted and runs to its end": 1,
    "a program under a name of 33 bytes is refused admission as a task": 1,
    "a program that cannot be admitted as a task leaves no memory behind: as many frames are free after as before": 1,
}


def pass_inventory(log):
    return Counter(line.removeprefix("  pass ") for line in log.splitlines()
                   if line.startswith("  pass "))


def coverage_problems(normal, limited):
    problems = []
    for name, expected in REPLAY_INDEPENDENT.items():
        if normal[name] != expected or limited[name] != expected:
            problems.append(f"R3: expected {expected} of {name!r} in each run, "
                            f"saw {normal[name]} with replay and {limited[name]} without")
    for name in KEY_CHECKS:
        if limited[name]:
            problems.append(f"R3: the run without replay reported a key-dependent pass: {name!r}")
    expected_normal = limited + Counter(KEY_CHECKS)
    for label, difference in (("missing with replay", expected_normal - normal),
                              ("extra with replay", normal - expected_normal)):
        if difference:
            problems.append(f"R3: {label}: {dict(difference)}")
    return problems


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
    problems.extend(coverage_problems(pass_inventory(log), pass_inventory(demo_log)))
    for problem in problems:
        print("FAILED", problem)
    if problems:
        sys.exit(1)
    print("replay probe: %d self-tests with replay, %d without, and the kernel says which were not run" % (passed, demo_passed))


if __name__ == "__main__":
    main()
