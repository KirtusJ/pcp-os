#!/usr/bin/env python3
"""The kernel starts the machine's other processors and parks them (feature `processors`).

Stage 2 of using every processor (issue 396). No other processor runs any work yet. What is checked
is that each one the firmware lists is brought to the same state as the first, on tables and stacks
of its own, and halted; that a kernel built without the feature starts none; and that a start that
is not answered ends, is said, and leaves nothing behind. The kernel's own word is not taken for the
processors' state: it is read from the emulator, which is outside the kernel.

Predicates, fixed before the first run. "Sound" means status 33 and 73 passes (the 71 of a kernel
without the feature, and the two this feature adds), with no FAIL line.
 S1 with 1, 2, 4 and 8 processors on a "pc" machine with 256 MiB, and 4 on a "q35" machine with
    6 GiB, the kernel says all the others started and are parked, the interrupt controller reached
    through memory, N threads on N cores; and the boot is sound.
 S2 with 4 processors that are 2 cores of 2 threads, it says 4 threads on 2 cores; sound.
 S3 with 2 processors present and room for 4, the firmware's 2 absent ones are counted as not to be
    used and are not started: 1 of 1; sound.
 S4 the emulator's own account, 4 processors, once the kernel has stopped: every processor is
    halted in ring 0; each other one has interrupts off, is in 64-bit mode, and has the first one's
    CR0, CR4, CR3 and interrupt table; all four segment tables are at different addresses, and each
    task register points into its own processor's segment table page.
 S5 the kernel built without the feature, 4 processors: it says nothing of other processors, passes
    71, status 33; and by the emulator's account the other three never left their reset state (CR0
    has paging and protection off).
 S6 the extended-mode demonstration, 4 processors: all three start, the controller reached in its
    extended mode; sound.
 S7 the silent demonstration, 4 processors: 3 of 4 started; the check that every one answered FAILs
    and nothing else does (72 passes, status 35); the check of the map still passes; and the boot
    takes less than 60 seconds more than S1's 4-processor boot.

Usage: check_started.py <kernel with processors> <kernel without> <extended demo> <silent demo> [--qemu ...]
This is an emulator with SeaBIOS. Nothing here is a run on a real machine or under UEFI firmware.
"""
import argparse
import re
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def machine(qemu, kernel, kind, memory, smp):
    return [qemu, "-machine", kind, "-accel", "tcg", "-cpu", "max", "-smp", smp, "-m", memory, "-kernel", str(kernel), "-append", "then=off",
            "-display", "none", "-nic", "none", "-no-reboot"]


def boot(qemu, kernel, kind, memory, smp):
    began = time.monotonic()
    done = subprocess.run(machine(qemu, kernel, kind, memory, smp) + ["-serial", "stdio", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"], capture_output=True, timeout=900)
    lines = done.stdout.decode(errors="replace").splitlines()
    return {"status": done.returncode, "passes": sum(1 for line in lines if line.startswith("  pass ")), "fails": [line for line in lines if line.startswith("  FAIL ")],
            "said": [line for line in lines if line.startswith("other processors: ")], "lines": lines, "took": time.monotonic() - began}


def registers(qemu, kernel, smp):
    """Every processor's registers as the emulator gives them once the kernel has stopped."""
    with tempfile.TemporaryDirectory() as folder:
        path = str(Path(folder) / "monitor")
        log = Path(folder) / "serial"
        running = subprocess.Popen(machine(qemu, kernel, "pc", "256M", smp) + ["-serial", "file:%s" % log, "-no-shutdown", "-monitor", "unix:%s,server,nowait" % path],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            until = time.monotonic() + 600
            while time.monotonic() < until and not (log.exists() and b"SELF-TEST" in log.read_bytes()):
                time.sleep(1)
            time.sleep(3)
            link = socket.socket(socket.AF_UNIX)
            link.connect(path)
            link.settimeout(5)

            def ask(command):
                link.sendall((command + "\n").encode())
                text = b""
                try:
                    while not text.rstrip().endswith(b"(qemu)") or command.encode() not in text:
                        text += link.recv(1 << 16)
                except socket.timeout:
                    pass
                return text.decode(errors="replace")
            ask("info version")
            status = ask("info status")
            text = ask("info registers -a")
        finally:
            running.kill()
            running.wait()
    found = []
    for block in text.split("CPU#")[1:]:
        one = {}
        for name, pattern in (("rip", r"RIP=([0-9a-f]+)"), ("flags", r"RFL=([0-9a-f]+)"), ("ring", r"CPL=(\d)"), ("halted", r"HLT=(\d)"), ("cs", r"CS =[0-9a-f]+ [0-9a-f]+ [0-9a-f]+ [0-9a-f]+ DPL=\d (\S+)"),
                              ("tr", r"TR =[0-9a-f]+ ([0-9a-f]+)"), ("gdt", r"GDT= +([0-9a-f]+)"), ("idt", r"IDT= +([0-9a-f]+)"), ("cr0", r"CR0=([0-9a-f]+)"), ("cr3", r"CR3=([0-9a-f]+)"),
                              ("cr4", r"CR4=([0-9a-f]+)")):
            hit = re.search(pattern, block)
            one[name] = hit.group(1) if hit else None
        found.append(one)
    return status, found


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("kernel", "plain", "extended", "silent"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    problems = []

    def sound(label, run, passes=73, status=33):
        if run["status"] != status or run["passes"] != passes or (status == 33 and run["fails"]):
            problems.append("%s: status %r, %d passes, FAIL lines %r" % (label, run["status"], run["passes"], run["fails"]))

    def line(parked, listed, reached, threads, cores):
        return ["other processors: %d of %d started and parked, the interrupt controller reached %s; %d thread(s) on %d core(s) in all; none runs any work yet" % (parked, listed, reached, threads, cores)]

    four = None
    for kind, memory, counts in (("pc", "256M", (1, 2, 4, 8)), ("q35", "6G", (4,))):
        for count in counts:
            run = boot(args.qemu, args.kernel, kind, memory, str(count))
            label = "S1 %s, %s, %d" % (kind, memory, count)
            sound(label, run)
            if run["said"] != line(count - 1, count - 1, "through memory", count, count):
                problems.append("%s: the kernel said %r" % (label, run["said"]))
            else:
                print("%s: %s" % (label, run["said"][0]))
            if kind == "pc" and count == 4:
                four = run["took"]
    run = boot(args.qemu, args.kernel, "pc", "256M", "4,cores=2,threads=2")
    sound("S2", run)
    if run["said"] != line(3, 3, "through memory", 4, 2):
        problems.append("S2: the kernel said %r" % run["said"])
    else:
        print("S2: %s" % run["said"][0])
    run = boot(args.qemu, args.kernel, "pc", "256M", "2,maxcpus=4")
    sound("S3", run)
    listed = [text for text in run["lines"] if text.startswith("processors: the firmware lists")]
    if run["said"] != line(1, 1, "through memory", 2, 2) or len(listed) != 1 or not listed[0].startswith("processors: the firmware lists 2 usable") or "2 not to be used" not in listed[0]:
        problems.append("S3: the kernel said %r and %r" % (listed, run["said"]))
    else:
        print("S3: %s | %s" % (listed[0], run["said"][0]))

    status, cpus = registers(args.qemu, args.kernel, "4")
    first = cpus[0] if cpus else {}
    bad = []
    if len(cpus) != 4 or any(value is None for cpu in cpus for value in cpu.values()):
        bad.append("the emulator did not give four processors' registers: %r" % cpus)
    else:
        for n, cpu in enumerate(cpus):
            if cpu["halted"] != "1" or cpu["ring"] != "0":
                bad.append("processor %d is not halted in ring 0" % n)
            if int(cpu["tr"], 16) & ~0xfff != int(cpu["gdt"], 16) or int(cpu["gdt"], 16) & 0xfff:
                bad.append("processor %d's task register is not in its own segment table page" % n)
            if n and (int(cpu["flags"], 16) & 0x200 or cpu["cs"] != "CS64" or any(cpu[r] != first[r] for r in ("cr0", "cr3", "cr4", "idt"))):
                bad.append("processor %d differs from the first: %r against %r" % (n, cpu, first))
        if len({cpu["gdt"] for cpu in cpus}) != 4:
            bad.append("two processors share a segment table")
        if int(first["cr0"], 16) & 0x80010001 != 0x80010001:
            bad.append("the first processor has paging or write protection off")
    if bad:
        problems.extend("S4: " + text for text in bad)
    else:
        print("S4: by the emulator's account (%s) all four are halted in ring 0, the other three in 64-bit mode with CR0 %s, CR4 %s, CR3 %s, segment tables at %s"
              % (status.strip().splitlines()[-2].strip() if len(status.strip().splitlines()) > 1 else "?", first["cr0"], first["cr4"], first["cr3"], ", ".join(cpu["gdt"] for cpu in cpus)))

    run = boot(args.qemu, args.plain, "pc", "256M", "4")
    sound("S5", run, passes=71)
    status, cpus = registers(args.qemu, args.plain, "4")
    if run["said"] or len(cpus) != 4 or any(cpu["cr0"] is None or int(cpu["cr0"], 16) & 0x80000001 for cpu in cpus[1:]):
        problems.append("S5: said %r; the others' CR0: %r" % (run["said"], [cpu["cr0"] for cpu in cpus[1:]]))
    else:
        print("S5: without the feature nothing is said of other processors, and their CR0 is %s: never started" % ", ".join(cpu["cr0"] for cpu in cpus[1:]))

    run = boot(args.qemu, args.extended, "pc", "256M", "4")
    sound("S6", run)
    if run["said"] != line(3, 3, "in its extended mode", 4, 4):
        problems.append("S6: the kernel said %r" % run["said"])
    else:
        print("S6: %s" % run["said"][0])

    run = boot(args.qemu, args.silent, "pc", "256M", "4")
    sound("S7", run, passes=72, status=35)
    answered = "  FAIL every other processor that was tried answered, as itself and able to do what the first can, and is parked"
    mapped = "  pass after starting them the kernel's map is as it was, with at most the one page of the interrupt controller's registers added, never executable"
    if run["said"] != line(3, 4, "through memory", 4, 4) or run["fails"] != [answered] or mapped not in run["lines"] or four is None or run["took"] > four + 60:
        problems.append("S7: said %r, FAIL lines %r, took %.1f s against %r" % (run["said"], run["fails"], run["took"], four))
    else:
        print("S7: %s; the one check fails and the map check passes; %.1f s against %.1f s" % (run["said"][0], run["took"], four))

    if problems:
        sys.exit("\n".join(problems))
    print("other processors: started and parked as the predicates require, on every machine tried")


if __name__ == "__main__":
    main()
