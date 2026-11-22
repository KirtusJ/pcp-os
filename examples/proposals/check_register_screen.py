#!/usr/bin/env python3
"""The kernel's register screen (F4) says which file was made: its path, size, task and whole digest.

The register records every file the kernel makes. Its screen showed, for such an event, only that
an artifact was added: two different files, or the same name with other bytes, looked the same
(Director 2). The screen now puts, under the event, the file's whole path, its size with what made
it and for which task, and all sixty-four digits of the digest of its bytes.

Predicates, fixed before the first run.
 R1 a file is proposed and allowed (F12, then Enter). The program ends and the desktop comes back.
    F4 then shows, somewhere in the register, exactly these three lines, each on a row of its own, in this order and next to
    one another: `file <the whole path>`; `<size> bytes, PRODUCED, for task <the id of the task
    the kernel started for that program in this boot>`; `sha256 <the SHA-256 of the bytes
    proposed>`. The task's id is read from the same screen (the row of the task's start).
 R2 the same file refused with Esc: the register screen shows no line beginning `file `.
 R3 the comparison refuses, before anything is booted, a screen with another digest, another size,
    another task, another path, the digest cut short, the three lines out of order or apart, and
    a screen with none of them.
 R5 the whole register is read in both boots: the screen opens at its end, is taken up until it
    no longer moves, and every row is read on the way down, a row met on two
    screenfuls being counted once (a made-up register longer than the screen, with its file only on
    the first screenful, is read whole before anything is booted); the events read are numbered from 0
    with none missing, and are as many as the screen's title says. (A3 found that an earlier
    version read only the last screenful, so its "no file anywhere" looked at one screen.)
 R4 both boots end with status 33 after F10 and Enter, and 71 self-tests pass.

Usage: check_register_screen.py <kernel.elf32 built with proposals> <proposal-probe ELF> <new evidence folder>
This is an emulator; the keys are sent by the emulator, not pressed by a person.
"""
import argparse
import hashlib
import runpy
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = runpy.run_path(str(HERE / "check.py"), run_name="proposals_check")
Guest, CHECK, NAME, CONTENT = BASE["Guest"], BASE["CHECK"], BASE["NAME"], BASE["CONTENT"]


def identity(path, content, task):
    return ["file %s" % path, "%d bytes, PRODUCED, for task %s" % (len(content), task), "sha256 %s" % hashlib.sha256(content).hexdigest()]


def shows(rows, wanted):
    """Whether the rows hold the three lines, whole, in order, one under the other, exactly once."""
    seen = [row.strip() for row in rows]
    places = [i for i in range(len(seen) - 2) if seen[i:i + 3] == wanted]
    return len(places) == 1 and sum(1 for row in seen if row.startswith("file ")) == 1


def body(shown):
    """The rows of one screenful that belong to the register: not the title, the row of headings
    (both stay at the top of every screenful), the two rows at the foot, or empty rows."""
    return [row for row in shown[:-2] if row.strip() and "REGISTER" not in row and not row.strip().startswith("seq  time")]


def gather(pages):
    """Every row of the register, from screenfuls taken from the top down. A screenful repeats rows
    of the one before it; the longest run that ends the rows read and begins the new ones is met once."""
    rows = []
    for shown in pages:
        fresh = body(shown)
        overlap = next((n for n in range(min(len(fresh), len(rows)), 0, -1) if rows[-n:] == fresh[:n]), 0)
        rows += fresh[overlap:]
    return rows


def selftest():
    # A register longer than the screen, with the file only on the first screenful: it is found, once.
    head, foot = ["  THE KERNEL'S REGISTER: 9 events", "  seq  time  actor  kind"], ["", "  Esc back"]
    whole = ["  %d    2026-11-22T10:00:00Z  kernel    E" % n for n in range(9)]
    whole[2:2] = ["      file a/b.txt", "      1 bytes, PRODUCED, for task T1", "      sha256 " + "1" * 64]
    for step in (1, 3, 5):
        pages = [head + whole[at:at + 5] + foot for at in range(0, len(whole) - 4, step)]
        if pages[-1] != head + whole[-5:] + foot:
            pages.append(head + whole[-5:] + foot)
        assert gather(pages) == whole, step
    assert gather([head + whole[-5:] + foot]) == whole[-5:]

    good = identity(NAME, CONTENT, "T15")
    around = ["42 TASK_STARTED", "43   2026-11-22T10:00:00Z  kernel    ARTIFACT_ADDED"]
    assert shows(around + ["      " + line for line in good] + ["44 TASK_ENDED"], good)
    other = lambda i, text: around + ["      " + (text if n == i else line) for n, line in enumerate(good)]
    for what, rows in {
            "another digest": other(2, "sha256 " + "0" * 64),
            "the digest cut short": other(2, good[2][:-2] + ".."),
            "another size": other(1, good[1].replace("%d bytes" % len(CONTENT), "999 bytes")),
            "another task": other(1, good[1].replace("T15", "T14")),
            "another path": other(0, good[0] + ".old"),
            "out of order": around + ["      " + good[i] for i in (0, 2, 1)],
            "apart": around + ["      " + good[0], "44 TASK_ENDED", "      " + good[1], "      " + good[2]],
            "twice": around + ["      " + line for line in good + good],
            "a second file beside it": around + ["      " + line for line in good] + ["      file taylor/first-project/other.txt"],
            "none": around}.items():
        assert not shows(rows, good), what


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("probe", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    selftest()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    glyphs = CHECK["kernel_glyphs"]()
    digest = hashlib.sha256(args.probe.read_bytes()).hexdigest()
    problems = []

    def register_rows(name, keys):
        guest = Guest(args.qemu, args.kernel, args.probe, evidence / name, "auto=proposal-probe then=menu propose_image=%s" % digest)
        rows = []
        try:
            if not guest.until("probe: asking"):
                problems.append("%s: the program did not ask" % name)
            time.sleep(2.0)
            for key in keys:
                guest.key(key)
            # The probe goes on to other proposals; a space refuses each, and allows nothing.
            for _ in range(12):
                if guest.until("probe: done", seconds=4):
                    break
                guest.key("spc")
            time.sleep(2.0)
            guest.key("f4")
            time.sleep(1.0)
            # The screen opens at the end of the register (A3: scrolling only down read just the last
            # screenful). Go up until it no longer moves, then read every row on the way down.
            last = None
            for page in range(80):
                shown = guest.screen("register-up-%02d" % page, glyphs)
                if not any("REGISTER" in row for row in shown):
                    problems.append("%s: F4 did not show the register: %r" % (name, [row.strip() for row in shown if row.strip()][:3]))
                    break
                if shown == last:
                    break
                last = shown
                guest.key("up")
                time.sleep(0.3)
            title = next((row for row in (last or []) if "REGISTER" in row), "")
            last = None
            pages = []
            for page in range(80):
                shown = guest.screen("register-%02d" % page, glyphs)
                if shown == last:
                    break
                pages.append(shown)
                last = shown
                guest.key("down")
                time.sleep(0.3)
            rows = gather(pages)
            # Every event was read: they are numbered from 0 with none missing, as many as the title says.
            numbers = [int(row.split()[0]) for row in rows if len(row.split()) > 2 and row.split()[0].isdigit() and row.split()[1][:2] == "20" and row.split()[1].endswith("Z")]
            said = [int(word) for word, after in zip(title.split(), title.split()[1:]) if after.startswith("events") and word.isdigit()]
            if numbers != list(range(len(numbers))) or said != [len(numbers)]:
                problems.append("%s: the register screen was not read whole: events %r..%r in order %r, title says %r" % (
                    name, numbers[:1], numbers[-1:], numbers == list(range(len(numbers))), said))
            guest.key("esc")
            time.sleep(0.5)
            guest.key("f10")
            time.sleep(1.0)
            guest.key("ret")
            status = guest.end()
            passes = sum(1 for line in guest.log().splitlines() if line.startswith("  pass "))
            if status != 33 or passes != 71:
                problems.append("%s: status %r and %d passes, not 33 and 71" % (name, status, passes))
        finally:
            guest.end(seconds=5)
        (evidence / (name + "-register.txt")).write_text("\n".join(rows) + "\n")
        return rows

    rows = register_rows("allowed", ["f12", "ret"])
    started = [row.split() for row in rows if "TASK_STARTED" in row and "proposal-probe" in row]
    task = next((word for row in started for i, word in enumerate(row) if i and row[i - 1] == "id"), None)
    if len(started) != 1 or task is None:
        problems.append("R1: the register screen does not show the one start of the probe's task with its id: %r" % started)
    elif not shows(rows, identity(NAME, CONTENT, task)):
        problems.append("R1: the register screen does not show exactly the file made: %r" % [row.strip() for row in rows if row.strip().startswith(("file ", "sha256 ")) or " bytes, " in row])
    else:
        print("allowed: " + "; ".join(identity(NAME, CONTENT, task)))

    rows = register_rows("refused", ["esc"])
    if any(row.strip().startswith("file ") for row in rows) or not any("TASK_STARTED" in row for row in rows):
        problems.append("R2: after a refusal the register screen shows a file, or was not read: %r" % [row.strip() for row in rows if row.strip().startswith("file ")])
    else:
        print("refused: no file on the register screen")

    if problems:
        sys.exit("\n".join(problems))
    print("register screen: a file the kernel made is shown with its whole path, size, task and digest; a refused file is not there")


if __name__ == "__main__":
    main()
