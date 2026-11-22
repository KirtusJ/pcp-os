#!/usr/bin/env python3
"""The kernel asks before it makes a file a program proposed (kernel feature `proposals`; tailorbird contract).

A test program (`examples/proposals/guest`) that is named by its digest on the kernel's command line may
propose one new text file. The kernel stops it, shows the proposal on its own screen and takes the answer
from the keyboard itself. This check boots the kernel under an emulator, gives the answers by keys, and
reads the kernel's serial log and its screen (with the kernel's own font).

Predicates, fixed before the first run.
 A1 allowed (Down is not needed: one page; F12, then Enter): the screen held "PCP OS ASKS YOU", the name
    and the first line of the content before any key; the log has exactly one "CREATED after the person
    allowed it" naming the file, its size and the SHA-256 of the content; the program's first write is
    answered with the record's length and outcome 1; a read too small for the receipt is answered -22
    and loses nothing; the same proposal again is answered outcome 1 with no second CREATED; the same
    numbers with other content are answered -16 and the receipt still says outcome 1 of the first; a
    new proposal for the taken name is answered -17 and outcome 3; the two proposals the screen could
    not show exactly, and two whose names reach for the kernel's devices or out of their folder, are
    answered -22 and the last receipt stays as it was. The file is in a project's folder inside the
    folder Taylor builds into, and the screen names folder and file apart.
 A2 in every case the program could not make a file by itself: its direct attempt is answered -13.
 D1 refused with Esc: the first proposal is answered outcome 2 and the log says "DENIED by the person;
    nothing made" for it; no CREATED anywhere. (The program's later proposal for the same name, still
    free, is a new question; the check answers that one with Esc too, so there are two DENIED lines.)
 D2 Enter alone (never F12): the same as D1. The answer is no unless F12 came first.
 D3 F12 and then a key that is not Enter: the same as D1.
 H1 Enter held down since before the question appeared does not allow: with Enter down, F12 is pressed
    and two seconds pass with nothing created or refused; after Enter is let go and pressed again, the file
    is created. The answer needs a fresh press.
 F1 F12 pressed while the program runs and nothing waits: the kernel's screen says "No file is waiting
    for your approval."; a key returns; the program goes on and its later proposal is asked as usual.
 S1 on a display too small to show the file whole and legibly (320 by 200: ten rows of thirty-two) the
    person is not asked: the kernel says the display is too small, the log says "REFUSED: it could
    not be shown whole on this display", the receipt's outcome is 6, and nothing is created whatever is pressed
    (F12 and Enter are sent). At 640 by 480 the question is asked and the file's first line is on it.
 K1 (in the allowed case) reads that fail take nothing: a read too small and a read of nothing are -22,
    a read into memory that is not the program's is -14, and the read straight after, with no proposal
    in between, gives all 100 bytes: outcome 1, attempt 7, proposal 1, lasting 0, and the content's own
    SHA-256, the kernel's number 1 for the run, and the SHA-256 of the whole request as the check
    computes it itself (lengths, name, content). The same numbers with another name are refused -16
    like another content. The sixty-fifth proposal of the run, and it exactly, is not taken (-28);
    none of the flood was asked about or made; and after it the first proposal of all is still answered
    from its receipt, outcome 1, with no second CREATED.
 L1 (at 800 by 600) a file of a thousand lines each ending in a space is 3,000 bytes and 1,001
    lines, within what a proposal may be, but on the screen each line needs a second row saying it ends
    in a space: more than sixty-four pages. The kernel takes the proposal (the write is answered with
    its length), does not ask, says on its screen that the file is too long to show, and the receipt's
    outcome is 6; no file long.txt is made. (Director 2: the first count was of the file's own lines.)
 N1 without `propose_image` on the command line the program is refused the proposal device (-13) and
    nobody is asked: no screen, no CREATED, no DENIED.
 Every case: the emulator ends with status 33 (the kernel's own checks passed and it powered off).

Usage: check.py <kernel.elf32 built with proposals> <proposal-probe ELF> <new evidence folder>
This is an emulator; the keys are sent by the emulator, not pressed by a person.
"""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
CHECK = runpy.run_path(str(HERE.parent / "tester-cd/check.py"))
PINS = runpy.run_path(str(HERE.parent / "tester-cd/check_compact_pins.py"))
NAME = "taylor/first-project/note.txt"
CONTENT = b"Hello.\nThis file was made by PCP OS after you allowed it.\n"
RECORD = 32 + len(NAME) + len(CONTENT)


class Guest:
    # (file, digest of the probe, digest of the file) when every run of the probe is to have a
    # companion beside it; None otherwise.
    companion = None

    def __init__(self, qemu, kernel, probe, out, append, also=None):
        out.mkdir(parents=True)
        if Guest.companion and "auto=proposal-probe" in append:
            beside, front, back = Guest.companion
            also = "%s,%s" % (also, beside) if also else str(beside)
            append += " worker_for=%s worker_image=%s" % (front, back)
        # The socket is kept in a short directory of its own: a socket's path may hold 107 bytes, and
        # the evidence directory's can be longer than that.
        self.out, self.serial, sock = out, out / "serial.txt", Path(tempfile.mkdtemp(prefix="q")) / "s"
        self.socket_at = sock
        self.process = subprocess.Popen(
            [qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "128M", "-kernel", str(kernel),
             "-initrd", "%s proposal-probe" % probe + (",%s" % also if also else ""), "-append", append, "-vga", "std", "-display", "none", "-nic", "none",
             "-serial", "file:%s" % self.serial, "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
             "-qmp", "unix:%s,server=on,wait=off" % sock], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            try:
                self.link = socket.socket(socket.AF_UNIX)
                self.link.connect(str(sock))
                break
            except OSError:
                time.sleep(0.1)
        self.file = self.link.makefile("rw")
        self.file.readline()
        self.rpc("qmp_capabilities")

    def rpc(self, command, **arguments):
        # A key sent after the guest has powered off has nowhere to go; that is not an error of the check.
        try:
            self.file.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
            self.file.flush()
            while True:
                line = self.file.readline()
                if not line:
                    return {}
                answer = json.loads(line)
                if "return" in answer or "error" in answer:
                    return answer
        except (OSError, ValueError):
            return {}

    def log(self):
        return self.serial.read_text(errors="replace") if self.serial.exists() else ""

    def until(self, words, seconds=120):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if words in self.log():
                return True
            if self.process.poll() is not None:
                return words in self.log()
            time.sleep(0.2)
        return False

    def key(self, name):
        self.rpc("send-key", keys=[{"type": "qcode", "data": name}])
        time.sleep(0.6)

    def screen(self, name, glyphs):
        picture = self.out / (name + ".ppm")
        self.rpc("screendump", filename=str(picture))
        time.sleep(0.3)
        return PINS["read"](picture.read_bytes(), glyphs) or []

    def end(self, seconds=120):
        for stream in (self.file, self.link):
            try:
                stream.close()
            except OSError:
                pass
        try:
            return self.process.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            return None
        finally:
            shutil.rmtree(self.socket_at.parent, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("probe", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    # Two builds of examples/proposals/filler, each in a file named store-filler: one leaving the store
    # one byte short of room for the note (FILL = the store's budget less 57), one leaving exactly room
    # (less 58). With them the full-store cases run; without them they are reported as not run.
    parser.add_argument("--filler-over", type=Path)
    parser.add_argument("--filler-exact", type=Path)
    # A third build, in a file named store-after (FILL=57 FILE=after.bin): after the refusal for want of
    # one byte it writes the 57 bytes that are left, as an ordinary program, and reads them back.
    parser.add_argument("--filler-after", type=Path)
    # A kernel built with the companion feature as well, and a program that waits on the channel
    # (examples/companion/probe built with BACK=1): every run of the probe then has it beside it, and
    # each case must come out the same, the companion having been started.
    parser.add_argument("--companion", type=Path)
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    glyphs = CHECK["kernel_glyphs"]()
    digest = hashlib.sha256(args.probe.read_bytes()).hexdigest()
    content = hashlib.sha256(CONTENT).hexdigest()
    request = hashlib.sha256(len(NAME).to_bytes(4, "little") + len(CONTENT).to_bytes(4, "little") + NAME.encode() + CONTENT).hexdigest()
    granted = "auto=proposal-probe then=off propose_image=%s" % digest
    if args.companion:
        Guest.companion = (args.companion.resolve(), digest, hashlib.sha256(args.companion.read_bytes()).hexdigest())
    problems, results = [], {}

    def case(name, append, keys, asked, idle=None):
        guest = Guest(args.qemu, args.kernel, args.probe, evidence / name, append)
        found = []
        try:
            if not guest.until("probe: opening the proposal device answered"):
                found.append("the program did not reach the proposal device")
            if idle:
                found += idle(guest)
            rows = []
            if asked:
                guest.until("probe: asking")
                time.sleep(2.0)
                rows = guest.screen("question", glyphs)
                text = " ".join(" ".join(rows).split())
                for words in ("PCP OS ASKS YOU", "Folder: taylor/first-project", "File: note.txt", "Hello.", "Nothing has been made yet."):
                    if words not in text:
                        found.append("the kernel's screen does not hold %r" % words)
            for key in keys:
                if key == "wait":
                    time.sleep(2.5)
                elif key in ("ret-down", "ret-up"):
                    guest.rpc("input-send-event", events=[{"type": "key", "data": {"down": key == "ret-down", "key": {"type": "qcode", "data": "ret"}}}])
                    time.sleep(0.6)
                elif key == "nothing-yet":
                    if "CREATED" in guest.log() or "DENIED" in guest.log():
                        found.append("H1: an answer was taken while Enter was still held from before F12")
                else:
                    guest.key(key)
            # A screen that only tells the person something waits for any key: press one now and then
            # until the program has finished. (A space refuses a question, so it allows nothing.)
            for _ in range(12):
                if guest.until("probe: done", seconds=4):
                    break
                guest.key("spc")
            status = guest.end()
            if status != 33:
                found.append("the emulator ended with status %r, not 33" % status)
            log = guest.log()
            results[name] = {"status": status, "probe": [line for line in log.splitlines() if line.startswith(("probe:", "proposal "))], "screen": rows}
            return found, log
        finally:
            if guest.process.poll() is None:
                guest.process.kill()

    def answers(log):
        return [line.split()[-1] for line in log.splitlines() if line.startswith(("probe: write answered", "probe: receipt bytes", "probe: outcome"))]

    found, log = case("allowed", granted, ["f12", "ret"], True)
    created = [line for line in log.splitlines() if "CREATED after the person allowed it" in line]
    if len(created) != 1 or NAME not in created[0] or "(%d bytes, SHA-256 %s)" % (len(CONTENT), content) not in created[0]:
        found.append("A1: not exactly one CREATED line naming the file, its size and digest: %r" % created)
    want = [str(RECORD), "100", "1", str(RECORD), "100", "1", "-16", "100", "1", "-16", "100", "1", "-17", "100", "3", "-22", "100", "3",
            "-22", "100", "3", "-22", "100", "3", "-22", "100", "3", str(RECORD), "100", "1"]
    for words in ("probe: a read too small answered -22", "probe: a read of nothing answered -22", "probe: a read into no memory answered -14",
                  "probe: the read straight after answered 100", "probe: its outcome 1", "probe: its attempt 7", "probe: its proposal 1",
                  "probe: its lasting 0", "probe: its run 1", "probe: its request digest " + request,
                  "probe: its content digest " + content, "probe: answering -28", "probe: after the flood"):
        if words not in log:
            found.append("A1: the log does not hold %r" % words)
    # Three receipts are kept before the flood, for (7,1) created, (7,2) name taken and (7,7) too long to
    # show: a proposal refused unseen or as a conflict keeps none. The flood's numbers 100 to 160 fill
    # the other sixty-one places, so 161 is the first the kernel does not take (C3 worked this out; a range would have passed a
    # smaller limit).
    took = [line for line in log.splitlines() if line.startswith("probe: the kernel took no more proposals at number ")]
    if took != ["probe: the kernel took no more proposals at number 161"]:
        found.append("A1: the kernel did not stop taking proposals exactly at number 161: %r" % took)
    # (On this case's large display the long file fits in sixty-four pages, is asked about like any
    # other, and is refused by the space the check presses. L1 is checked at 800 by 600 below.)
    if answers(log) != want:
        found.append("A1: the program was answered %r, not %r" % (answers(log), want))
    if "probe: making a file directly answered -13" not in log:
        found.append("A2: the program's own attempt to make a file was not refused with -13")
    # R1: started with another build folder, the same program can build only there: its proposal under
    # taylor/ is refused unseen, nobody is asked, and its list names the other folder and holds nothing.
    other, other_log = case("another-folder", granted + " build_folder=workshop/kit", [], False)
    for words in ("probe: list folder workshop/kit\n", "probe: list count 0", "probe: list complete 1", "probe: opening the note to read answered -2"):
        if words not in other_log:
            other.append("R1: the log does not hold %r" % words)
    if answers(other_log)[:2] != ["-22", "0"] or any(line.startswith("proposal ") for line in other_log.splitlines()):
        other.append("R1: a proposal outside the folder of this run was taken or put to the person: %r" % answers(other_log)[:3])
    found += other
    # R2, R3: a folder name the kernel does not accept, or two of them, and the program that would
    # propose is not started at all: no other folder is used in its place, and the kernel says why.
    # R4: a loader line too long to be taken whole may have lost the word naming the proposing program,
    # so no program is started from it (it must not run as an ordinary program with the store).
    for name, extra, why, said_words in (
            ("bad-folder", " build_folder=Dev/x", "build_folder=Dev/x is not a name a build folder may have; no build folder is set", "its projects folder setting is not a valid name"),
            ("two-folders", " build_folder=one build_folder=two", "build_folder=one is given more than once; no build folder is set", "its projects folder setting is not a valid name"),
            ("cut-line", " " + " ".join("pad%02d=xxxxxxxx" % n for n in range(20)), "the loader's line was not taken whole, so no build folder is set", "the loader's line was cut, so what it may use is not known"),
            # R5: the same with the word naming the program beyond the cut, where the kernel never sees it.
            ("cut-nomination", None, "the loader's line was not taken whole, so no build folder is set", "the loader's line was cut, so what it may use is not known")):
        line = granted + extra if extra is not None else "auto=proposal-probe then=off " + " ".join("pad%02d=xxxxxxxx" % n for n in range(20)) + " propose_image=%s" % digest
        guest = Guest(args.qemu, args.kernel, args.probe, evidence / name, line)
        try:
            said = guest.until("proposal-probe was not started: " + said_words, seconds=60)
            status = guest.end()
            folder_log = guest.log()
            results[name] = {"status": status, "probe": [line for line in folder_log.splitlines() if line.startswith(("probe:", "proposal "))]}
            if not said or why not in folder_log:
                found.append("%s: the kernel did not say the program was not started, or why" % name)
            if "probe:" in folder_log or any(line.startswith("proposal ") for line in folder_log.splitlines()):
                found.append("%s: the program ran, or a proposal was taken" % name)
            if status != 33:
                found.append("%s: the emulator ended with status %r, not 33" % (name, status))
        finally:
            if guest.process.poll() is None:
                guest.process.kill()
    # F1, F2: an ordinary program with the store fills it first; then the proposing program is started
    # from the launcher and its note is allowed. With one byte too little room nothing is made and the
    # receipt says no room (5); with exactly enough the note is made.
    full_run = []
    # The store's budget in the kernel as built without the smaller demonstration heap (regular.rs).
    # The fillers must have written exactly these totals: a filler built for any other size is the
    # wrong setup, and the case fails instead of passing for another reason.
    budget = 4 << 20
    for name, filler, outcome, made, fill in (("store-one-byte-short", args.filler_over, "5", 0, budget - len(CONTENT) + 1),
                                              ("store-exactly-enough", args.filler_exact, "1", 1, budget - len(CONTENT))):
        if filler is None:
            continue
        full_run.append(name)
        after = args.filler_after if not made else None
        guest = Guest(args.qemu, args.kernel, args.probe, evidence / name, "auto=store-filler then=menu propose_image=%s" % digest,
                      also="%s,%s" % (filler, after) if after else filler)
        try:
            filled = guest.until("filler: of ", seconds=120)
            time.sleep(3)
            guest.key("ret")
            time.sleep(1.5)
            guest.key("ret")
            asked = guest.until("probe: asking", seconds=40)
            time.sleep(2.0)
            guest.key("f12")
            guest.key("ret")
            answered = guest.until("probe: outcome ", seconds=20)
            time.sleep(1.0)
            if after:
                # The refusal must leave the store as it was: the proposing program is let finish (a
                # space refuses its later questions), and then an ordinary program writes exactly the
                # bytes that are left and reads them back.
                for _ in range(12):
                    if guest.until("probe: done", seconds=4):
                        break
                    guest.key("spc")
                time.sleep(3)
                guest.key("down")
                time.sleep(0.5)
                guest.key("down")
                time.sleep(0.5)
                guest.key("ret")
                guest.until("starting store-after", seconds=20)
                time.sleep(4)
                full_run.append("an ordinary write into the room left after the refusal")
            full_log = guest.log()
            results[name] = {"probe": [line for line in full_log.splitlines() if line.startswith(("probe:", "proposal ", "filler:"))][:12],
                             "filler_sha256": hashlib.sha256(filler.read_bytes()).hexdigest(),
                             "after_sha256": hashlib.sha256(after.read_bytes()).hexdigest() if after else None, "filled_bytes": fill}
            wrote = [line for line in full_log.splitlines() if line.startswith("filler: ")]
            if not filled or wrote[:4] != ["filler: wrote %d" % fill, "filler: of %d" % fill, "filler: read back %d" % fill, "filler: read back the same 1"]:
                found.append("%s: the filler did not write and read back exactly %d bytes: %r" % (name, fill, wrote[:4]))
            left = budget - fill
            if after and wrote[4:] != ["filler: wrote %d" % left, "filler: of %d" % left, "filler: read back %d" % left, "filler: read back the same 1"]:
                found.append("%s: after the refusal an ordinary program did not write and read back the %d bytes left: %r" % (name, left, wrote[4:]))
            if not asked or not answered or answers(full_log)[:3] != [str(RECORD), "100", outcome]:
                found.append("%s: the first proposal was answered %r, not outcome %s" % (name, answers(full_log)[:3], outcome))
            if len([line for line in full_log.splitlines() if "CREATED after the person allowed it" in line]) != made:
                found.append("%s: the note was made %s" % (name, "although there was no room" if not made else "no times, or more than once"))
            if "KERNEL STOPPED" in full_log:
                found.append("%s: the kernel stopped" % name)
        finally:
            if guest.process.poll() is None:
                guest.process.kill()
    # B1: afterwards the program sees its build folder and nothing else, and can change nothing by itself.
    listed = [line for line in log.splitlines() if line.startswith("probe: listed ")]
    if listed != ["probe: listed %s of bytes %d" % (NAME, len(CONTENT))]:
        found.append("B1: the list does not hold exactly the one file made, with its size: %r" % listed)
    size = 24 + len("taylor") + 8 + len(NAME)
    snapshot = (struct.pack("<IIIIQ", 1, 1, 1, 6, 1) + b"taylor" + struct.pack("<HHI", len(NAME), 0, len(CONTENT)) + NAME.encode())
    for words in ("probe: a file outside the build folder was answered -22", "probe: opening the list to write answered -13",
                  "probe: the list ended with 0", "probe: the list is bytes %d" % size, "probe: list version 1", "probe: list count 1",
                  "probe: list complete 1", "probe: list run 1", "probe: list folder taylor\n", "probe: list bytes left over 0",
                  "probe: writing to the list answered -9", "probe: opening the note to read answered 0",
                  "probe: the note read back bytes %d" % len(CONTENT), "probe: the note read back the same 1",
                  "probe: writing to the note answered -9", "probe: opening the note to write answered -13",
                  "probe: opening the note to empty it answered -13", "probe: opening the list to read and make answered -13",
                  "probe: opening the list to read and empty answered -13", "probe: opening the note to read and make answered -13",
                  "probe: opening the note to read and empty answered -13", "probe: opening the note to read after all that answered 0", "probe: opening the note to make it anew answered -13",
                  "probe: opening the list to make it anew answered -13", "probe: the note read after all that ended with 0",
                  "probe: the note after all that is the same 1", "probe: closing it answered 0",
                  "probe: list hex " + snapshot.hex() + "\n", "probe: removing the note answered -13",
                  "probe: renaming the note answered -13", "probe: a name outside the folder answered -2",
                  "probe: a name beside the folder answered -2"):
        if words not in log:
            found.append("B1: the log does not hold %r" % words)
    if "elsewhere" in " ".join(line for line in log.splitlines() if line.startswith("proposal ")):
        found.append("B1: a file outside the build folder was put to the person")
    problems += ["allowed: " + p for p in found]

    for name, keys in (("refused-esc", ["esc", "wait", "esc"]), ("refused-enter-alone", ["ret", "wait", "esc"]),
                       ("refused-f12-then-other", ["f12", "spc", "wait", "esc"])):
        found, log = case(name, granted, keys, True)
        denied = [line for line in log.splitlines() if "DENIED by the person; nothing made" in line]
        if len(denied) != 2 or "CREATED" in log:
            found.append("not two DENIED (the case's and the later question's) and no CREATED: %r" % [line for line in log.splitlines() if line.startswith("proposal ")])
        if answers(log)[:3] != [str(RECORD), "100", "2"]:
            found.append("the first proposal was answered %r, not outcome 2" % answers(log)[:3])
        if "probe: making a file directly answered -13" not in log:
            found.append("A2: the program's own attempt to make a file was not refused with -13")
        problems += [name + ": " + p for p in found]

    def enter_held(guest):
        guest.until("probe: idle")
        guest.rpc("input-send-event", events=[{"type": "key", "data": {"down": True, "key": {"type": "qcode", "data": "ret"}}}])
        return []

    found, log = case("held-enter", granted, ["f12", "wait", "nothing-yet", "ret-up", "ret"], True, idle=enter_held)
    if len([line for line in log.splitlines() if "CREATED after the person allowed it" in line]) != 1 or "note.txt" not in "".join(
            line for line in log.splitlines() if "CREATED" in line):
        found.append("H1: after Enter was let go and pressed again the note was not created exactly once")
    problems += ["held-enter: " + p for p in found]

    def nothing_waiting(guest):
        guest.until("probe: idle")
        time.sleep(0.5)
        guest.key("f12")
        time.sleep(1.0)
        text = " ".join(" ".join(guest.screen("nothing-waiting", glyphs)).split())
        guest.key("spc")
        return [] if "No file is waiting for your approval." in text else ["F1: the kernel's screen did not say that nothing waits: %r" % text[:200]]

    found, log = case("f12-with-nothing", granted, ["f12", "ret"], True, idle=nothing_waiting)
    if len([line for line in log.splitlines() if "CREATED after the person allowed it" in line]) != 1:
        found.append("F1: after the notice the program's proposal was not asked and created as usual")
    problems += ["f12-with-nothing: " + p for p in found]

    guest = Guest(args.qemu, args.kernel, args.probe, evidence / "too-small", granted + " desktop=320x200")
    found = []
    try:
        guest.until("probe: asking")
        time.sleep(2.0)
        text = " ".join(" ".join(guest.screen("too-small", glyphs)).split())
        if "too small to show a file" not in text:
            found.append("S1: the small display does not say it is too small: %r" % text[:160])
        for key in ("f12", "ret", "wait", "esc", "wait", "esc"):
            time.sleep(2.5) if key == "wait" else guest.key(key)
        guest.until("probe: done")
        status, log = guest.end(), guest.log()
        if status != 33 or "CREATED" in log or "it could not be shown whole on this display" not in log or answers(log)[:3] != [str(RECORD), "100", "6"]:
            found.append("S1: status %r, answers %r, or something was created" % (status, answers(log)[:3]))
        results["too-small"] = {"status": status, "probe": [line for line in log.splitlines() if line.startswith(("probe:", "proposal "))]}
    finally:
        if guest.process.poll() is None:
            guest.process.kill()
    problems += ["too-small: " + p for p in found]

    found, log = case("long-at-800x600", granted + " desktop=800x600", ["f12", "ret"], True)
    for words in ("probe: long write answered 3061", "probe: long receipt bytes 100", "probe: long outcome 6",
                  "long.txt (3000 bytes", "it could not be shown whole on this display"):
        if words not in log:
            found.append("L1: the log does not hold %r" % words)
    if any("long.txt" in line and "CREATED" in line for line in log.splitlines()) or any("long.txt" in line and "DENIED" in line for line in log.splitlines()):
        found.append("L1: the long file was made, or the person was asked about it")
    problems += ["long-at-800x600: " + p for p in found]

    found, log = case("at-640x480", granted + " desktop=640x480", ["f12", "ret"], True)
    if len([line for line in log.splitlines() if "CREATED after the person allowed it" in line]) != 1:
        found.append("S1: at 640 by 480 the file was not asked about and created")
    problems += ["at-640x480: " + p for p in found]

    found, log = case("not-granted", "auto=proposal-probe then=off", [], False)
    if "probe: opening the proposal device answered -13" not in log or "CREATED" in log or "DENIED" in log:
        found.append("N1: without the grant the device was not refused with -13, or somebody was asked")
    problems += ["not-granted: " + p for p in found]

    (evidence / "result.json").write_text(json.dumps({"probe_sha256": digest, "problems": problems, "results": results}, indent=2) + "\n")
    if problems:
        sys.exit("\n".join(problems))
    print("proposals: allowed once by F12 then Enter and made by the kernel; refused three ways; a held Enter does not allow; F12 with nothing waiting says so; refused unseen; not offered without the grant")
    print("full-store cases run: %s" % (", ".join(full_run) if full_run else "none (no --filler-over / --filler-exact given)"))


if __name__ == "__main__":
    main()
