#!/usr/bin/env python3
"""Every place the kernel writes to a processor port, checked against a reviewed list.

A write to a port reaches hardware directly. On a machine the kernel was not written for, what answers at
a port is not known, so each write must be one a person has read and placed: in a function that runs only
after its device was found, only under an emulator, or on a port every PC has. This program reads the
kernel's source, lists every call of a port-writing function as (file, function, port as written), and
compares the list with tests/port_writes.txt. It fails if a write is in the source and not in the list,
or in the list and not in the source, and if the instruction itself appears anywhere but in the
port-writing functions. It reads source text: it does not prove what value a computed port has, and it
does not run the kernel.

Usage: python3 tests/port_writes.py          compare, exit 1 on any difference
       python3 tests/port_writes.py --list   print the list as found in the source
"""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE.parent / "src"
WRITERS = ("outb", "out8", "out16", "out32", "port_out8", "port_out16", "port_out32")
CALL = re.compile(r"(?<![\w.])(%s)\(" % "|".join(WRITERS))
FUNCTION = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?(?:extern\s+\"C\"\s+)?fn\s+(\w+)")


def first_argument(text, start):
    """The first argument of a call whose opening bracket is just before `start`, as written."""
    depth, out = 0, ""
    for ch in text[start:]:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            if depth == 0:
                break
            depth -= 1
        elif ch == "," and depth == 0:
            break
        out += ch
    return " ".join(out.split())


def found():
    writes, instructions = set(), []
    for path in sorted(SRC.rglob("*.rs")):
        name = path.relative_to(SRC).as_posix()
        # Comments are removed first, keeping every line, so that a call split over lines is read whole.
        text = "\n".join(line.split("//", 1)[0] for line in path.read_text().splitlines())
        starts = [(m.start(), m.group(1)) for m in re.finditer(FUNCTION.pattern, text, re.MULTILINE)]

        def function_at(position):
            inside = [fn for at, fn in starts if at <= position]
            return inside[-1] if inside else "(no function)"

        for m in re.finditer(r"asm!\(\s*\"(?:rep\s+)?outs?[bwd]?\b", text):
            instructions.append((name, function_at(m.start()), text.count("\n", 0, m.start()) + 1))
        for call in CALL.finditer(text):
            function = function_at(call.start())
            if function not in WRITERS:
                writes.add((name, function, first_argument(text, call.end())))
    return writes, instructions


def reviewed():
    entries, problems = {}, []
    for number, line in enumerate((HERE / "port_writes.txt").read_text().splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        parts = [p.strip() for p in line.split("|")]
        if len(parts) != 4 or not all(parts):
            problems.append("port_writes.txt line %d is not 'file | function | port | when it runs'" % number)
            continue
        entries[tuple(parts[:3])] = parts[3]
    return entries, problems


def main():
    writes, instructions = found()
    if sys.argv[1:] == ["--list"]:
        for entry in sorted(writes):
            print(" | ".join(entry))
        return 0
    entries, problems = reviewed()
    for name, function, number in instructions:
        if function not in WRITERS:
            problems.append("%s:%d: a port-write instruction outside the port-writing functions (in %s)" % (name, number, function))
    if len(instructions) != 3:
        problems.append("expected the port-write instruction in exactly 3 functions, found %d" % len(instructions))
    for entry in sorted(writes - set(entries)):
        problems.append("not reviewed: " + " | ".join(entry))
    for entry in sorted(set(entries) - writes):
        problems.append("reviewed but no longer in the source: " + " | ".join(entry))
    for problem in problems:
        print(problem)
    print("%d port writes in the source, %d reviewed, %d problem(s)" % (len(writes), len(entries), len(problems)))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
