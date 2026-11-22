#!/usr/bin/env python3
"""A program's 256-bit vector registers are its own (the kernel keeps them, and clears them between programs).

A language model needs the wide vector instructions to answer at a usable speed. Before this the kernel
turned on and kept only the 128-bit registers: a wider instruction ended the program. Where the
processor has them, the 256-bit registers are now on, saved and loaded with the rest of a program's
state, and a program starts with them empty.

Two builds of probe/ are given, in files named vector-leave and vector-look. In one boot the first runs,
fills a register and ends leaving it full; then the second is started from the launcher.

Predicates, fixed before the first run.
 V1 on a processor with the registers (cpu "max" and "Haswell"): each program finds the register empty
    when it starts ("found clean 1"), reads back what it set ("set 1") and still has it after sleeping
    while the kernel ran ("kept 1"). The second finds it empty although the first left it full.
 V2 on a processor without them (cpu "qemu64"): each program says "vector: none" and uses none.
 V3 every boot ends with status 33 after the kernel's own shutdown, with no line beginning "  FAIL".

Usage: check.py <kernel.elf32> <vector-leave> <vector-look> [--qemu qemu-system-x86_64]
This is an emulator. It says nothing of a real processor.
"""
import argparse
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def boot(qemu, kernel, leave, look, cpu, folder):
    serial = Path(folder) / ("serial-%s.txt" % cpu)
    vm = subprocess.Popen(
        [qemu, "-machine", "pc", "-accel", "tcg", "-cpu", cpu, "-m", "256M", "-kernel", kernel.name,
         "-initrd", "%s,%s" % (look, leave), "-append", "auto=vector-leave then=menu", "-vga", "std", "-display", "none",
         "-nic", "none", "-serial", "file:%s" % serial, "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=kernel.parent, text=True)

    def call(command, **arguments):
        vm.stdin.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
        vm.stdin.flush()
        while True:
            reply = json.loads(vm.stdout.readline())
            if "return" in reply or "error" in reply:
                return reply

    def key(name):
        call("send-key", keys=[{"type": "qcode", "data": name}])
        time.sleep(0.4)

    def until(words, seconds):
        deadline = time.time() + seconds
        while time.time() < deadline:
            if serial.exists() and words in serial.read_text(errors="replace"):
                return True
            time.sleep(0.2)
        return False

    try:
        vm.stdout.readline()
        call("qmp_capabilities")
        first = until("vector-leave ended", 90)
        time.sleep(3)
        key("ret")
        time.sleep(1.5)
        key("ret")
        second = until("vector-look ended", 30)
        time.sleep(1)
        key("esc")
        time.sleep(1)
        key("f10")
        time.sleep(1)
        key("ret")
        try:
            status = vm.wait(timeout=30)
        except subprocess.TimeoutExpired:
            status = None
        return first, second, status, serial.read_text(errors="replace").splitlines()
    finally:
        if vm.poll() is None:
            vm.kill()
            vm.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("leave", type=Path)
    parser.add_argument("look", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    kernel, leave, look = args.kernel.resolve(), args.leave.resolve(), args.look.resolve()
    if leave.name != "vector-leave" or look.name != "vector-look":
        sys.exit("the two programs must be in files named vector-leave and vector-look")
    wide = ["vector: found clean 1", "vector: set 1", "vector: kept 1"]
    problems = []
    with tempfile.TemporaryDirectory() as folder:
        for cpu, want in (("max", wide + ["vector: left full"] + wide + ["vector: done"]), ("Haswell", wide + ["vector: left full"] + wide + ["vector: done"]),
                          ("qemu64", ["vector: none", "vector: none"])):
            first, second, status, lines = boot(args.qemu, kernel, leave, look, cpu, folder)
            said = [line for line in lines if line.startswith("vector: ")]
            if not first or not second:
                problems.append("%s: a program did not run to its end (first %s, second %s)" % (cpu, first, second))
            if said != want:
                problems.append("%s: the programs said %r, not %r" % (cpu, said, want))
            if status != 33 or any(line.startswith("  FAIL") for line in lines):
                problems.append("%s: the kernel ended with status %r, or a self-test failed" % (cpu, status))
            print("%s: %s" % (cpu, "; ".join(said)))
    if problems:
        for problem in problems:
            print("FAIL " + problem, file=sys.stderr)
        sys.exit(1)
    print("vector state: kept for each program and empty at its start where the processor has it; not used where it has not")


if __name__ == "__main__":
    main()
