#!/usr/bin/env python3
"""A guest built with the processor's floating point computes what the software build computes, and
two such programs at once do not disturb one another.

The stock bare target (`x86_64-unknown-none`) does floating point in software, which is many times
slower (Director C3 found this on the model worker). The same source built for the tier-1 target
`x86_64-unknown-linux-gnu`, with no C library and the guest linker script, uses SSE2: that target's
own `core` and `alloc` are built for it, so nothing about the bare target's ABI is changed. This
check shows on the kernel, in an emulator, that such a program is right.

Predicates, fixed before the first run.
 H1 the hardware build contains scalar SSE arithmetic (addss, mulss, addsd or mulsd) and the
    software build contains none.
 H2 alone on the kernel, the software build and the hardware build print the same bits for the f32
    sum, the f64 sum and the sum over numbers kept on a heap; each says no later round differed.
 H3 the red zone (the 128 bytes below the stack pointer, which code for the Linux target may use):
    the hardware build fills four places in it and finds them unchanged in every one of
    400,000,000 turns of a loop, while the kernel's clock interrupts it. The control build, whose
    loop makes a call, finds them changed in every turn.
 H4 two hardware-float programs at once, one as the other's companion, on a processor with 256-bit
    registers ("max") and on one with SSE2 only ("qemu64"): the front program prints exactly what
    it printed alone; the back program's sums (f32, f64, heap) are the bits the software build gives
    for the same series; the back program reports at least eight rounds and none of them wrong.
 H5 every boot ends with status 33, 71 self-tests passed and every frame given back.

Usage: check.py <kernel.elf32 with companion> <soft> <hard> <hard, BACK=1> <hard, CLOBBER=1> <soft, BACK_SERIES=1>
       (each a float-probe built as probe/README says)
This is an emulator. Times here say nothing about a real machine.
"""
import argparse
import hashlib
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

TURNS = 400_000_000


def boot(qemu, kernel, files, append, cpu):
    with tempfile.TemporaryDirectory() as folder:
        names = []
        for i, f in enumerate(files):
            # The files are all called float-probe; the kernel starts one by name.
            name = Path(folder) / ("float-probe" if i == 0 else "float-beside")
            shutil.copyfile(f, name)
            names.append(name.name)
        shutil.copyfile(kernel, Path(folder) / "kernel.elf32")
        done = subprocess.run([qemu, "-machine", "pc", "-accel", "tcg", "-cpu", cpu, "-m", "256M", "-kernel", "kernel.elf32", "-initrd", ",".join(names),
                               "-append", append, "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                              cwd=folder, capture_output=True, timeout=1500)
        return done.returncode, done.stdout.decode(errors="replace").splitlines()


def said(lines, who):
    """What a program printed, as {words: [values]}. A companion's lines reach the console as it
    writes them and again, after it has ended, in the kernel's account of what it wrote; only the
    kernel's account is read for it, so each of its lines counts once."""
    out = {}
    for line in lines:
        beside = " wrote: " in line
        if beside != (who == "back"):
            continue
        match = re.fullmatch(r"%s: (.*?) ?([0-9a-f]{16})?" % who, line.split(" wrote: ", 1)[-1].strip())
        if match:
            out.setdefault(match.group(1), []).append(match.group(2))
    return out


def sound(name, status, lines, problems):
    passes = sum(1 for line in lines if line.startswith("  pass "))
    frames = [m for line in lines for m in [re.search(r"frames: (\d+) of (\d+) free", line)] if m]
    if status != 33 or passes != 71 or any(line.startswith("  FAIL") for line in lines):
        problems.append("H5 %s: status %r and %d passes, not 33 and 71" % (name, status, passes))
    if not frames or any(m.group(1) != m.group(2) for m in frames):
        problems.append("H5 %s: not every frame was given back" % name)


def selftest():
    lines = ["front: f32 000000004e96410f", "back: f64 41bad70ae9999ab5", "companion float-beside wrote: back: f64 41bad70ae9999ab5", "front: done", "front: f32x 0000000000000000"]
    assert said(lines, "front") == {"f32": ["000000004e96410f"], "done": [None], "f32x": ["0000000000000000"]}
    assert said(lines, "back") == {"f64": ["41bad70ae9999ab5"]}
    problems = []
    sound("x", 33, ["  pass a"] * 71 + ["kernel heap: 1 bytes; frames: 5 of 5 free, 1 handed out"], problems)
    assert problems == []
    sound("x", 33, ["  pass a"] * 71 + ["frames: 4 of 5 free"], problems)
    sound("x", 1, ["  pass a"] * 71 + ["frames: 5 of 5 free"], problems)
    assert len(problems) == 2


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("kernel", "soft", "hard", "back", "clobber", "series"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    if sys.flags.optimize:
        sys.exit("run without -O: the check's own tests are asserts")
    selftest()
    problems = []

    # H1
    counts = {}
    for name in ("soft", "hard"):
        code = subprocess.run(["objdump", "-d", "--no-show-raw-insn", str(getattr(args, name))], capture_output=True, text=True, check=True).stdout
        counts[name] = len(re.findall(r"\b(?:addss|mulss|addsd|mulsd)\b", code))
    if counts["soft"] != 0 or counts["hard"] == 0:
        problems.append("H1: scalar SSE arithmetic instructions: software build %(soft)d, hardware build %(hard)d" % counts)

    # H2, H3
    alone = {}
    for name in ("soft", "hard", "clobber", "series"):
        status, lines = boot(args.qemu, args.kernel, [getattr(args, name)], "auto=float-probe then=off", "max")
        sound(name + " alone", status, lines, problems)
        alone[name] = said(lines, "front")
        if alone[name].get("done") != [None] or alone[name].get("rounds wrong") != ["%016x" % 0]:
            problems.append("H2 %s: it did not run to its end with no round wrong: %r" % (name, alone[name]))
    for words in ("f32", "f64", "heap"):
        if alone["soft"].get(words) != alone["hard"].get(words) or not alone["hard"].get(words) or len(alone["hard"][words]) != 1:
            problems.append("H2: %s is %r in software and %r in hardware" % (words, alone["soft"].get(words), alone["hard"].get(words)))
    if alone["hard"].get("red zone wrong") != ["%016x" % 0] or alone["soft"].get("red zone wrong") != ["%016x" % 0]:
        problems.append("H3: the red zone did not keep what was put in it: %r" % alone["hard"].get("red zone wrong"))
    if alone["clobber"].get("red zone wrong") != ["%016x" % TURNS]:
        problems.append("H3: the control with a call in its loop reported %r changed turns, not all %d" % (alone["clobber"].get("red zone wrong"), TURNS))

    # H4
    digest = lambda f: hashlib.sha256(f.read_bytes()).hexdigest()
    for cpu in ("max", "qemu64"):
        status, lines = boot(args.qemu, args.kernel, [args.hard, args.back], "auto=float-probe then=off worker_for=%s worker_image=%s" % (digest(args.hard), digest(args.back)), cpu)
        sound("together on " + cpu, status, lines, problems)
        front, back = said(lines, "front"), said(lines, "back")
        if front != alone["hard"]:
            problems.append("H4 %s: beside another program the front one printed %r, not what it printed alone %r" % (cpu, front, alone["hard"]))
        for words in ("f32", "f64", "heap"):
            if back.get(words) != alone["series"].get(words) or not back.get(words):
                problems.append("H4 %s: the back program's %s is %r; software gives %r for that series" % (cpu, words, back.get(words), alone["series"].get(words)))
        rounds = [int(v, 16) for v in back.get("rounds so far, and of them wrong", [])]
        if not rounds or max(r >> 32 for r in rounds) < 8 or any(r & 0xffff_ffff for r in rounds):
            problems.append("H4 %s: the back program reported %r (rounds in the high half, wrong ones in the low)" % (cpu, ["%016x" % r for r in rounds]))
        else:
            print("together on %s: the back program did %d rounds beside the front one, none wrong" % (cpu, max(r >> 32 for r in rounds)))

    if problems:
        sys.exit("\n".join(problems))
    print("hard float: %d SSE arithmetic instructions against none; the same bits as software for f32 %s, f64 %s and the heap sum %s; the red zone kept; two programs at once on both processors" % (
        counts["hard"], alone["hard"]["f32"][0], alone["hard"]["f64"][0], alone["hard"]["heap"][0]))


if __name__ == "__main__":
    main()
