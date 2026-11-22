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
    person is not asked: the kernel says the display is too small, the log says "REFUSED: this display
    is too small to show it", the receipt's outcome is 6, and nothing is created whatever is pressed
    (F12 and Enter are sent). At 640 by 480 the question is asked and the file's first line is on it.
 K1 (in the allowed case) reads that fail take nothing: a read too small and a read of nothing are -22,
    a read into memory that is not the program's is -14, and the read straight after, with no proposal
    in between, gives all 100 bytes: outcome 1, attempt 7, proposal 1, lasting 0, and the content's own
    SHA-256. The same numbers with another name are refused -16 like another content. After sixty-four
    proposals in one run the kernel takes no more (-28); none was asked about or made.
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
import socket
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
CHECK = runpy.run_path(str(HERE.parent / "tester-cd/check.py"))
PINS = runpy.run_path(str(HERE.parent / "tester-cd/check_compact_pins.py"))
NAME = "taylor/first-project/note.txt"
CONTENT = b"Hello.\nThis file was made by PCP OS after you allowed it.\n"
RECORD = 32 + len(NAME) + len(CONTENT)


class Guest:
    def __init__(self, qemu, kernel, probe, out, append):
        out.mkdir(parents=True)
        self.out, self.serial, sock = out, out / "serial.txt", out / "q.sock"
        self.process = subprocess.Popen(
            [qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "128M", "-kernel", str(kernel),
             "-initrd", "%s proposal-probe" % probe, "-append", append, "-vga", "std", "-display", "none", "-nic", "none",
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


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("probe", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    glyphs = CHECK["kernel_glyphs"]()
    digest = hashlib.sha256(args.probe.read_bytes()).hexdigest()
    content = hashlib.sha256(CONTENT).hexdigest()
    granted = "auto=proposal-probe then=off propose_image=%s" % digest
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
            guest.until("probe: done")
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
            "-22", "100", "3", "-22", "100", "3", "-22", "100", "3"]
    for words in ("probe: a read too small answered -22", "probe: a read of nothing answered -22", "probe: a read into no memory answered -14",
                  "probe: the read straight after answered 100", "probe: its outcome 1", "probe: its attempt 7", "probe: its proposal 1",
                  "probe: its lasting 0", "probe: its content digest " + content, "probe: answering -28"):
        if words not in log:
            found.append("A1: the log does not hold %r" % words)
    # In this case seven receipts are kept before the flood (five named above and two more refusals
    # that had a name to keep), so the sixty-fourth is reached partway through it.
    took = [line for line in log.splitlines() if line.startswith("probe: the kernel took no more proposals at number ")]
    if len(took) != 1 or not 100 < int(took[0].split()[-1]) < 170:
        found.append("A1: the kernel did not stop taking proposals within the flood: %r" % took)
    if answers(log) != want:
        found.append("A1: the program was answered %r, not %r" % (answers(log), want))
    if "probe: making a file directly answered -13" not in log:
        found.append("A2: the program's own attempt to make a file was not refused with -13")
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
    if len([line for line in log.splitlines() if "CREATED after the person allowed it" in line]) != 1 or "DENIED" in log:
        found.append("H1: after Enter was let go and pressed again the file was not created exactly once")
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
        if status != 33 or "CREATED" in log or "this display is too small to show it" not in log or answers(log)[:3] != [str(RECORD), "100", "6"]:
            found.append("S1: status %r, answers %r, or something was created" % (status, answers(log)[:3]))
        results["too-small"] = {"status": status, "probe": [line for line in log.splitlines() if line.startswith(("probe:", "proposal "))]}
    finally:
        if guest.process.poll() is None:
            guest.process.kill()
    problems += ["too-small: " + p for p in found]

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


if __name__ == "__main__":
    main()
