#!/usr/bin/env python3
"""The kernel reads from the firmware's tables how many processors the machine has.

Stage 1 of using every processor (issue 396). Nothing is started yet: the kernel reports what the
firmware lists and goes on running on one. What is checked here is that it reads real firmware's
tables rightly, wherever they lie, and refuses a wrong address without harm.

Predicates, fixed before the first run.
 C1 with 1, 2, 4 and 8 emulated processors on a "pc" machine with 256 MiB, the kernel says the
    firmware lists exactly that many usable, found where a BIOS puts the root pointer.
 C2 the same with the tables above the first gibibyte: a "q35" machine with 6 GiB and 4 processors,
    and a "pc" machine with 2 GiB and 3.
 C3 a loader's `acpi_rsdp=` naming an address in memory given as free, one beyond what can be read,
    one not in the one form taken (0x and sixteen lowercase digits), and the word given twice each
    give "not known" with the reason, and nothing else changes. Naming the address the firmware's
    own root pointer is at (found by a first boot that looks for it) gives the same count.
 C4 every boot passes all 71 self-tests and ends with status 33: the wider map at start (four
    gibibytes where it was one) costs nothing the self-tests can see.

Usage: check.py <kernel.elf32> [--qemu qemu-system-x86_64]
This is an emulator. Its firmware is SeaBIOS; a UEFI machine's loader must name the address.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path


said_at = []


def boot(qemu, kernel, machine, memory, processors, more=""):
    done = subprocess.run([qemu, "-machine", machine, "-accel", "tcg", "-cpu", "max", "-smp", str(processors), "-m", memory, "-kernel", str(kernel),
                           "-append", ("then=off " + more).strip(), "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot",
                           "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"], capture_output=True, timeout=600)
    lines = done.stdout.decode(errors="replace").splitlines()
    said = [line for line in lines if line.startswith("processors: ") and "root pointer is at" not in line]
    said_at.clear()
    said_at.extend(line.rsplit(" ", 1)[1] for line in lines if line.startswith("processors: the root pointer is at "))
    passes = sum(1 for line in lines if line.startswith("  pass "))
    return done.returncode, passes, said


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    problems = []

    def sound(name, status, passes):
        if status != 33 or passes != 71:
            problems.append("C4 %s: status %r and %d passes, not 33 and 71" % (name, status, passes))

    for label, machine, memory, counts in (("C1", "pc", "256M", (1, 2, 4, 8)), ("C2", "q35", "6G", (4,)), ("C2", "pc", "2G", (3,))):
        for count in counts:
            name = "%s, %s, %d processor(s)" % (machine, memory, count)
            status, passes, said = boot(args.qemu, args.kernel, machine, memory, count)
            sound(name, status, passes)
            want = "processors: the firmware lists %d usable (found where a BIOS puts it); 0 more than this kernel would start, 0 not to be used; this kernel runs on one" % count
            if said != [want]:
                problems.append("%s %s: the kernel said %r" % (label, name, said))
            else:
                print("%s: %d listed" % (name, count))
    free = "a table lies outside what can be read at start, or in memory given as free"
    form = "the loader's line is cut short, or its acpi_rsdp= is not the one form taken"
    for word, reason in (("acpi_rsdp=0x0000000000100000", free), ("acpi_rsdp=0x0000001ffffffff0", free), ("acpi_rsdp=0x100000", form), ("acpi_rsdp=0x00000000000F5000", form),
                         ("acpi_rsdp=0x00000000000f5000 acpi_rsdp=0x00000000000f5000", form)):
        status, passes, said = boot(args.qemu, args.kernel, "pc", "256M", 2, word)
        sound(word, status, passes)
        if said != ["processors: not known (%s); this kernel runs on one" % reason]:
            problems.append("C3 %s: the kernel said %r" % (word, said))
        else:
            print("%s: refused, %s" % (word, reason))
    # The firmware's own root pointer, as a first boot that looks for it reports it, then named.
    boot(args.qemu, args.kernel, "pc", "256M", 2)
    if len(said_at) != 1 or not re.fullmatch(r"0x[0-9a-f]{16}", said_at[0]):
        problems.append("C3: the kernel did not say where the root pointer is: %r" % said_at)
    else:
        word = "acpi_rsdp=" + said_at[0]
        status, passes, said = boot(args.qemu, args.kernel, "pc", "256M", 2, word)
        sound(word, status, passes)
        if said != ["processors: the firmware lists 2 usable (at the address the loader named); 0 more than this kernel would start, 0 not to be used; this kernel runs on one"]:
            problems.append("C3 %s: the kernel said %r" % (word, said))
        else:
            print("%s: 2 listed at the address named" % word)
    if problems:
        sys.exit("\n".join(problems))
    print("processors: the firmware's count is read rightly on every machine tried, and a wrong address is refused")


if __name__ == "__main__":
    main()
