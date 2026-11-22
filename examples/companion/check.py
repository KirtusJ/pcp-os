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
    of them come back once it is read. The companion is first asked to hold and says it has, so that
    nothing is taken from the channel while it is filled, however the two programs are scheduled;
    while it holds it writes 65,537 bytes, the last of which waits for room, and the front reads
    exactly those.
 P2b a write through a descriptor opened for reading only is refused (-9), and a write from an address
    that is not the program's is refused (-14); neither puts a byte in the channel (the exchanges
    of P1 follow them and would come back wrong).
 P3 each program, where the processor has 256-bit registers, kept its own pattern through every
    switch: both report 0 times that the register held something else, and both used the registers.
 P4 told to finish, the companion ends by itself and the front's next read answers 0.
 P5 with the companion that leaves early: the front's read answers 0 once what was written is read,
    and its write then answers -32; the front carries on and ends by itself.
 P6 without the words that name a companion the same front program is refused the channel (-13).
 P7 with the words but on a line whose worker digest matches no loaded file, likewise refused.
 P9 a companion that computes for ever and makes no call does not keep the other program from
    running: a front that sleeps twenty milliseconds a hundred times finishes, and the kernel then
    ends the companion.
 P10 the person's Ctrl+Alt+Delete, pressed while the front sleeps and that companion computes, ends
    both: the front is reported STOPPED by that key, the companion as ended by it together with the
    front, and every frame comes back.
 P8 every run ends with status 33, 71 self-tests pass, none fails, the kernel does not stop, and
    every frame is given back.

Usage: check.py <kernel.elf32 built with --features companion> <talk-front> <talk-back> <leaving talk-back>
       [--never-yields <talk-back built with SPIN=1> --alone <talk-front built with ALONE=1> --waits <talk-front built with WAIT=1>]
Without the three optional programs P9 and P10 are reported as not run.
This is an emulator, and two small test programs: nothing here is a model or a person's program.
"""
import argparse
import hashlib
import json
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def boot(qemu, kernel, files, append):
    # A run that passes takes a few seconds. One that has not ended in two minutes has two programs
    # waiting on each other, and is reported with what was said until then.
    try:
        done = subprocess.run([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "256M", "-kernel", kernel.name, "-initrd", ",".join(str(f) for f in files),
                               "-append", append, "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                              stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=kernel.parent, timeout=120)
    except subprocess.TimeoutExpired as late:
        return None, (late.stdout or b"").decode(errors="replace").splitlines()
    return done.returncode, done.stdout.decode(errors="replace").splitlines()


def stopped(qemu, kernel, files, append):
    """Boot, wait until the front sleeps and the companion computes, press Ctrl+Alt+Delete, and wait for the end."""
    folder = Path(tempfile.mkdtemp(prefix="q"))
    serial, sock = folder / "serial.txt", folder / "s"
    vm = subprocess.Popen([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "256M", "-kernel", kernel.name, "-initrd", ",".join(str(f) for f in files),
                           "-append", append, "-display", "none", "-nic", "none", "-serial", "file:%s" % serial, "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
                           "-qmp", "unix:%s,server=on,wait=off" % sock], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, cwd=kernel.parent)
    try:
        link = None
        for _ in range(100):
            try:
                link = socket.socket(socket.AF_UNIX)
                link.connect(str(sock))
                break
            except OSError:
                time.sleep(0.1)
        stream = link.makefile("rw")
        stream.readline()

        def rpc(command, **arguments):
            stream.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
            stream.flush()
            while True:
                answer = json.loads(stream.readline())
                if "return" in answer or "error" in answer:
                    return answer

        rpc("qmp_capabilities")
        log = lambda: serial.read_text(errors="replace") if serial.exists() else ""
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline and not ("front: waiting to be stopped" in log() and "back: computing for ever" in log()):
            time.sleep(0.2)
        if time.monotonic() < deadline:
            rpc("send-key", keys=[{"type": "qcode", "data": key} for key in ("ctrl", "alt", "delete")])
        try:
            status = vm.wait(timeout=60)
        except subprocess.TimeoutExpired:
            status = None
        return status, log().splitlines()
    finally:
        if vm.poll() is None:
            vm.kill()
            vm.wait()
        shutil.rmtree(folder, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("front", type=Path)
    parser.add_argument("back", type=Path)
    parser.add_argument("leaving", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--never-yields", type=Path)
    parser.add_argument("--alone", type=Path)
    parser.add_argument("--waits", type=Path)
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
    require("together, front", said(lines, "front: "), ["opening the channel answered 0", "a write through a descriptor opened to read answered -9",
            "a write from no address answered -14", "exchanges completed 100", "bytes that came back wrong 0",
            "asking it to hold answered 254", "the full channel answered -11", "bytes it took before that 65536",
            "bytes the companion wrote while it held 65537", "of those, bytes that were not what it wrote 0", "bytes that came back after it 65536", "telling it to finish answered 1",
            "the read after that answered 0", "wide registers used 1", "times its register held something else 0"])
    require("together, back", said(lines, "back: "), ["opening the channel answered 0", "held until it had written 65537", "told to finish after 165536", "bytes answered 165536",
            "wide registers used 1", "times its register held something else 0"])
    if not any(line.startswith("companion talk-back: it exited with 0") for line in lines):
        problems.append("together: the companion did not end by itself when told to finish")
    print("together: " + "; ".join(said(lines, "front: ")))

    status, lines = boot(args.qemu, kernel, [front, leaving], words % (digest(front), digest(leaving)))
    common("companion leaves", status, lines)
    require("companion leaves, front", said(lines, "front: "), ["the other end was gone in round 1", "a read of it then answered 0", "a write to it then answered -32",
            "exchanges completed 1", "bytes that came back wrong 0", "times its register held something else 0"])
    print("companion leaves: " + "; ".join(said(lines, "front: ")))

    for name, append in (("no companion named", "auto=talk-front then=off"),
                         ("a worker that is not loaded", words % (digest(front), "0" * 64))):
        status, lines = boot(args.qemu, kernel, [front, back], append)
        common(name, status, lines)
        if said(lines, "front: ") != ["opening the channel answered -13"] or any("companion" in line for line in lines if not line.startswith("  ")):
            problems.append("%s: the channel was not refused, or a companion was started: %r" % (name, said(lines, "front: ")))
        print("%s: %s" % (name, "; ".join(said(lines, "front: "))))

    if args.never_yields and args.alone and args.waits:
        spins, alone, waits = (p.resolve() for p in (args.never_yields, args.alone, args.waits))
        status, lines = boot(args.qemu, kernel, [alone, spins], words % (digest(alone), digest(spins)))
        common("a companion that never yields", status, lines)
        require("a companion that never yields, front", said(lines, "front: "), ["opening the channel answered 0", "went on by itself for periods of sleep 100"])
        require("a companion that never yields, back", said(lines, "back: "), ["computing for ever and making no call"])
        if not any(line.startswith("companion talk-back: the kernel ended it when the program it was beside ended") for line in lines):
            problems.append("P9: the kernel did not end the companion that never yields when the front ended")
        print("a companion that never yields: " + "; ".join(said(lines, "front: ")))

        status, lines = stopped(args.qemu, kernel, [waits, spins], words % (digest(waits), digest(spins)))
        common("stopped by the person", status, lines)
        if not any(line.startswith("talk-front ended STOPPED: stopped by the person with Ctrl+Alt+Delete") for line in lines) \
                or not any(line.startswith("companion talk-back: the person's Ctrl+Alt+Delete ended it together with the program it was beside") for line in lines):
            problems.append("P10: the Stop key did not end both, or the kernel did not say so: %r" % [line[:90] for line in lines if "talk-" in line][-4:])
        print("stopped by the person: " + "; ".join(line[:100] for line in lines if line.startswith(("talk-front ended", "companion talk-back:"))))
    else:
        print("P9 and P10 not run: --never-yields, --alone and --waits were not all given")

    if problems:
        for problem in problems:
            print("FAIL " + problem, file=sys.stderr)
        sys.exit(1)
    print("companion: two programs side by side exchange bytes exactly, each keeps its registers, and neither outlives the run")


if __name__ == "__main__":
    main()
