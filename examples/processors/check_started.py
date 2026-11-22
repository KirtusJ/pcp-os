#!/usr/bin/env python3
"""The kernel starts the machine's other processors, and its one lock lets one into the kernel at a
time (feature `processors`).

Stages 2 and 3 of using every processor (issue 396). No other processor runs a program yet. What is
checked is that each one the firmware lists is brought to the same state as the first, on tables and
stacks of its own, and left idling until it is sent a message; that with all of them going in and
out of the kernel at once a count they all add to comes out exact, and does not when the lock is
not waited for; that a kernel built without the feature starts none; and that a start that
is not answered ends, is said, and leaves nothing behind. The kernel's own word is not taken for the
processors' state: it is read from the emulator, which is outside the kernel.

Predicates, fixed before the first run. "Sound" means status 33 with no FAIL line, and 74 passes
where there is one processor (the 71 of a kernel without the feature, and three this feature adds)
or 75 where there are more (the count of S9 as well).
 S1 with 1, 2, 4 and 8 processors on a "pc" machine with 256 MiB, and 4 on a "q35" machine with
    6 GiB, the kernel says all the others started and are parked, the interrupt controller reached
    through memory, N threads on N cores; and the boot is sound.
 S2 with 4 processors that are 2 cores of 2 threads, it says 4 threads on 2 cores; sound.
 S3 with 2 processors present and room for 4, the firmware's 2 absent ones are counted as not to be
    used and are not started: 1 of 1; sound.
 S4 the emulator's own account, 4 processors, once the kernel has stopped: every processor is in
    ring 0 and each other one is halted (the first asks the emulator to shut down and may be read
    before or after its own halt); each other one has interrupts on (it waits for a message), is in 64-bit mode, and has the first one's
    CR0, CR4, CR3 and interrupt table; all four segment tables are at different addresses, and each
    task register points into its own processor's segment table page.
 S8 in every boot of S1 to S3 that starts another processor, the kernel says that every one it
    started came up holding other memory types than the first and was given the first's (this
    emulator returns a restarted processor with those registers cleared; a real processor keeps
    them, and the kernel then writes nothing). The check that all hold the same afterwards passes.
 S11 (C3's control) on a machine of four whose processors have the page-attribute table and no memory-range
    registers (cpu "max,mtrr=off,pat=on"), the only difference a started processor can have from the first is
    the table the kernel changed. The kernel says each of the three was given the first's table with its ranges
    untouched, says nothing of memory types, and the boot is sound with an exact count.
 S9 in every boot of S1 to S3 with more than one processor, the kernel says the count all
    processors added to together is exactly processors x 20000, and its check passes.
 S10 the control, 4 processors: the kernel built so that the lock is not waited for during that
    count says a count below 80000, that one check FAILs and no other does, status 35.
 S5 the kernel built without the feature, 4 processors: it says nothing of other processors, passes
    71, status 33; and by the emulator's account the other three are not on the kernel's map (paging
    is off in their CR0): the kernel never started them.
 S6 the extended-mode demonstration, 4 processors: where the emulator offers the extended mode, all
    three start with the controller reached in it, and the boot is sound. Where the emulator does
    not offer it (it says so when asked for it), this predicate is NOT RUN and that is printed; it
    is then neither passed nor failed.
 S7 the silent demonstration, 4 processors: 3 of 4 started; the check that every one answered FAILs
    and no other does, status 35; the check of the map still passes; and the boot takes less than
    60 seconds more than S1's 4-processor boot. (How many checks pass is not fixed here: a kernel
    that failed a check does not go on to the tests that need the keyboard.)

The first run of this file (record pr4-started-20261122T141701Z, kept) failed three predicates as
then written, and each was the predicate's error or the emulator's limit, not the kernel's: S5
required CR0 as at reset, but this BIOS itself leaves the others in protected mode; S6 could not be
run because this emulator does not offer the extended mode, and the kernel correctly stayed in the
ordinary one; S7 required 72 passes, but a kernel with a failed check stops before the keyboard tests.
The second run (pr6-started-20261122T142009Z, kept) failed S2 to S4 for a reason that was the
kernel's to deal with: it had begun to compare each processor's memory types with the first's, and
in this emulator they differ. The kernel now gives them the first's, and S8 was added. The third
run (pr7-started-20261122T142600Z, kept) failed S4 because this file read the registers three
seconds after the kernel's last line and the first processor had not yet halted; it now waits
until the emulator says the machine has shut down. The fourth (pr10-started-20261122T142905Z, kept)
failed S4 again on the first processor only: it asks for the shutdown and can be read before its
own halt, so S4 no longer requires the first to be halted, only the others.

Usage: check_started.py <kernel with processors> <kernel without> <extended demo> <silent demo> <unlocked demo> [--qemu ...]
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


def machine(qemu, kernel, kind, memory, smp, cpu="max"):
    return [qemu, "-machine", kind, "-accel", "tcg", "-cpu", cpu, "-smp", smp, "-m", memory, "-kernel", str(kernel), "-append", "then=off",
            "-display", "none", "-nic", "none", "-no-reboot"]


def boot(qemu, kernel, kind, memory, smp, cpu="max"):
    began = time.monotonic()
    done = subprocess.run(machine(qemu, kernel, kind, memory, smp, cpu) + ["-serial", "stdio", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"], capture_output=True, timeout=900)
    lines = done.stdout.decode(errors="replace").splitlines()
    return {"status": done.returncode, "passes": sum(1 for line in lines if line.startswith("  pass ")), "fails": [line for line in lines if line.startswith("  FAIL ")],
            "said": [line for line in lines if line.startswith("other processors: ") and "started and parked" in line],
            "count": [line for line in lines if line.startswith("other processors: all ")],
            "given": [line for line in lines if line.startswith("other processors: ") and "memory types" in line],
            "table": [line for line in lines if line.startswith("other processors: ") and "page-attribute table" in line], "lines": lines, "took": time.monotonic() - began}


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
            # The kernel has stopped when the emulator says the machine shut down; until then the
            # first processor may still be at work, and its registers are not yet the ones to read.
            status = ""
            until = time.monotonic() + 120
            while "shutdown" not in status and time.monotonic() < until:
                time.sleep(1)
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
    for name in ("kernel", "plain", "extended", "silent", "unlocked"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    problems = []

    def sound(label, run, passes=74, status=33):
        if run["status"] != status or run["passes"] != passes or (status == 33 and run["fails"]):
            problems.append("%s: status %r, %d passes, FAIL lines %r" % (label, run["status"], run["passes"], run["fails"]))

    def line(parked, listed, reached, threads, cores):
        return ["other processors: %d of %d started and parked, the interrupt controller reached %s; %d thread(s) on %d core(s) in all; none runs any work yet" % (parked, listed, reached, threads, cores)]

    def counted(label, run, processors):
        want = ["other processors: all %d processors added to one count together, each giving the kernel's lock up and taking it again 20000 times: %d of %d"
                % (processors, processors * 20000, processors * 20000)] if processors > 1 else []
        if run["count"] != want:
            problems.append("S9 (%s): the kernel said %r" % (label, run["count"]))

    def given(label, run, parked):
        want = ["other processors: %d came up holding other memory types than the first and were given the first's" % parked] if parked else []
        if run["given"] != want:
            problems.append("S8 (%s): the kernel said %r" % (label, run["given"]))

    four = None
    for kind, memory, counts in (("pc", "256M", (1, 2, 4, 8)), ("q35", "6G", (4,))):
        for count in counts:
            run = boot(args.qemu, args.kernel, kind, memory, str(count))
            label = "S1 %s, %s, %d" % (kind, memory, count)
            sound(label, run, passes=75 if count > 1 else 74)
            given(label, run, count - 1)
            counted(label, run, count)
            if run["said"] != line(count - 1, count - 1, "through memory", count, count):
                problems.append("%s: the kernel said %r" % (label, run["said"]))
            else:
                print("%s: %s" % (label, run["said"][0]))
            if kind == "pc" and count == 4:
                four = run["took"]
    run = boot(args.qemu, args.kernel, "pc", "256M", "4", cpu="max,mtrr=off,pat=on")
    sound("S11", run, passes=75)
    counted("S11", run, 4)
    want = ["other processors: 3 came up with another page-attribute table than the first and were given the first's; their memory ranges agreed and were not touched"]
    if run["table"] != want or run["given"]:
        problems.append("S11: the kernel said %r and %r" % (run["table"], run["given"]))
    else:
        print("S11: %s" % run["table"][0])
    run = boot(args.qemu, args.kernel, "pc", "256M", "4,cores=2,threads=2")
    sound("S2", run, passes=75)
    given("S2", run, 3)
    counted("S2", run, 4)
    if run["said"] != line(3, 3, "through memory", 4, 2):
        problems.append("S2: the kernel said %r" % run["said"])
    else:
        print("S2: %s" % run["said"][0])
    run = boot(args.qemu, args.kernel, "pc", "256M", "2,maxcpus=4")
    sound("S3", run, passes=75)
    given("S3", run, 1)
    counted("S3", run, 2)
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
            if cpu["ring"] != "0" or (n and cpu["halted"] != "1"):
                bad.append("processor %d is not in ring 0, or is one of the others and not halted" % n)
            if int(cpu["tr"], 16) & ~0xfff != int(cpu["gdt"], 16) or int(cpu["gdt"], 16) & 0xfff:
                bad.append("processor %d's task register is not in its own segment table page" % n)
            if n and (not int(cpu["flags"], 16) & 0x200 or cpu["cs"] != "CS64" or any(cpu[r] != first[r] for r in ("cr0", "cr3", "cr4", "idt"))):
                bad.append("processor %d differs from the first: %r against %r" % (n, cpu, first))
        if len({cpu["gdt"] for cpu in cpus}) != 4:
            bad.append("two processors share a segment table")
        if int(first["cr0"], 16) & 0x80010001 != 0x80010001:
            bad.append("the first processor has paging or write protection off")
    if bad:
        problems.extend("S4: " + text for text in bad)
    else:
        print("S4: by the emulator's account (%s) all four are in ring 0, the other three halted with interrupts on in 64-bit mode with CR0 %s, CR4 %s, CR3 %s, segment tables at %s"
              % (status.strip().splitlines()[-2].strip() if len(status.strip().splitlines()) > 1 else "?", first["cr0"], first["cr4"], first["cr3"], ", ".join(cpu["gdt"] for cpu in cpus)))

    run = boot(args.qemu, args.plain, "pc", "256M", "4")
    sound("S5", run, passes=71)
    status, cpus = registers(args.qemu, args.plain, "4")
    if run["said"] or len(cpus) != 4 or any(cpu["cr0"] is None or int(cpu["cr0"], 16) & 0x80000000 for cpu in cpus[1:]):
        problems.append("S5: said %r; the others' CR0: %r" % (run["said"], [cpu["cr0"] for cpu in cpus[1:]]))
    else:
        print("S5: without the feature nothing is said of other processors, and their CR0 is %s: paging off, never on the kernel's map" % ", ".join(cpu["cr0"] for cpu in cpus[1:]))

    offered = subprocess.run([args.qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max,+x2apic", "-display", "none", "-nic", "none", "-S", "-monitor", "stdio"], input=b"quit\n", capture_output=True, timeout=60)
    if b"x2apic" in offered.stderr:
        print("S6: NOT RUN: this emulator does not offer the extended mode (%s)" % offered.stderr.decode(errors="replace").strip().splitlines()[0])
    else:
        run = boot(args.qemu, args.extended, "pc", "256M", "4")
        sound("S6", run, passes=75)
        if run["said"] != line(3, 3, "in its extended mode", 4, 4):
            problems.append("S6: the kernel said %r" % run["said"])
        else:
            print("S6: %s" % run["said"][0])

    run = boot(args.qemu, args.silent, "pc", "256M", "4")
    answered = "  FAIL every other processor that was tried answered, as itself, able to do what the first can and holding the same memory types, and is parked"
    mapped = "  pass after starting them the kernel's map is as it was, with at most the one page of the interrupt controller's registers added, never executable"
    if run["status"] != 35 or run["said"] != line(3, 4, "through memory", 4, 4) or run["fails"] != [answered] or mapped not in run["lines"] or four is None or run["took"] > four + 60:
        problems.append("S7: status %r, said %r, FAIL lines %r, took %.1f s against %r" % (run["status"], run["said"], run["fails"], run["took"], four))
    else:
        print("S7: %s; the one check fails and the map check passes; %.1f s against %.1f s" % (run["said"][0], run["took"], four))

    run = boot(args.qemu, args.unlocked, "pc", "256M", "4")
    exact = "  FAIL every processor answers a message, and with all of them in and out of the kernel at once its one lock lets in one at a time: the count is exact"
    seen = re.fullmatch(r".*: (\d+) of 80000", run["count"][0]) if len(run["count"]) == 1 else None
    if run["status"] != 35 or run["fails"] != [exact] or not seen or int(seen.group(1)) >= 80000:
        problems.append("S10: status %r, FAIL lines %r, said %r" % (run["status"], run["fails"], run["count"]))
    else:
        print("S10: without waiting for the lock the count was %s of 80000, and that one check failed" % seen.group(1))
    if not any(text.startswith("S9") for text in problems):
        print("S9: in every boot with more than one processor the count was exact")
    if not any(text.startswith("S8") for text in problems):
        print("S8: every processor started in S1 to S3 was given the first's memory types, and said so")
    if problems:
        sys.exit("\n".join(problems))
    print("other processors: started and parked as the predicates require, on every machine tried")


if __name__ == "__main__":
    main()
