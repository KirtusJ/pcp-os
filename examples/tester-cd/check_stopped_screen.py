#!/usr/bin/env python3
"""What the kernel shows on a machine it cannot switch off, and what it says it did not test (issue 309).

Under an emulator the kernel ends by asking the emulator to power off. It knows it is emulated by the
processor's hypervisor flag; QEMU can withhold that flag (`-cpu max,-hypervisor`), and the kernel then
behaves as on a real machine: it uses no emulator port and ends on a stopped screen with the processor
halted. This check boots kernels that way, with no disk and no network, and reads the screen with the
kernel's own font. It shows what the kernel does when it believes it is on hardware. It is not a run on
hardware.

Predicates, fixed before the first run.
 H1 no keyboard controller (`-machine pc,i8042=off`), at 320 by 200 and at 1920 by 1080: the first row that
    holds anything is "PCP OS has stopped."; a row begins "Verified: all " and the screen's text holds
    "self-tests it ran passed."; the text holds "No keyboard controller answered. This kernel drives PS/2 keyboards
    only." and "This kernel cannot switch the computer off. Hold its power button until it goes
    dark."; no cell is anything but a character; the emulator is still running.
 H5 on every stopped screen the count in "Verified: all <n> self-tests it ran passed." is the number of
    passes in the whole serial log, which includes the check made after the session (A3 found it one short).
 H6 on every stopped screen the reason shown for having no keyboard is the one the same boot's serial line
    "keyboard: ..." calls for, shown once, and neither of the kernel's other two reasons; with a keyboard
    in use, none of the three. The check proves this reading on made-up rows before any guest is started:
    a second reason added, the reason of another outcome, and a reason shown with a keyboard in use must
    each be refused.
 H7 (with kernels built with `keyboard-busy-demo` and `keyboard-silent-demo`, which end keyboard start-up as
    a controller that did not take its set-up and as a keyboard that did not acknowledge, touching no
    port) at 800 by 600 and 320 by 200: the stopped screen meets H5 and H6 with that outcome's sentence.
 H2 a keyboard, at 800 by 600: the desktop appears; F10 then Enter gives a screen that meets H1 except that
    it does not hold the sentence about a keyboard; the emulator is still running.
 H3 the kernel built with `no-key-replay-demo`, at 800 by 600: F5 holds "Not run at start: the six tests
    that press keys, because this keyboard controller does not replay keys for a test."; after Esc, F10 and
    Enter the stopped screen holds "Not run: the six tests that press keys, because this keyboard
    controller does not replay keys for a test."
 H4 the default kernel's F5 at 800 by 600 holds no "Not run".

 H8 the serial record and visible skipped-test paragraph agree with the fixture. The compact stopped
    screen may omit that paragraph for space. If it is shown, the complete reason must agree.
 H9 every stopped screen is whole sentences of the kernel's and nothing else: the title, then at most one
    each of the verdict, the skipped-tests paragraph, a keyboard reason and "Nothing is running and nothing
    more will be written.", in that order, then what the person must do. A part of a sentence is refused
    (Director 2 found that H8 took a paragraph cut to "Not", "Not ru", "the six tests that press" or "there
    is no keyboard" on a small screen for one left out). Proved on made-up rows before any guest is started.

Usage: check_stopped_screen.py <default kernel.elf32> <no-key-replay-demo kernel.elf32> <new evidence folder>
                               [--busy <kernel.elf32>] [--silent <kernel.elf32>]
"""
import argparse
import json
import re
from pathlib import Path
import runpy
import socket
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
CHECK = runpy.run_path(str(HERE / "check.py"))
PINS = runpy.run_path(str(HERE / "check_compact_pins.py"))
MEASURE = HERE.parents[1] / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
STOPPED = "PCP OS has stopped."
NO_KEYBOARD = "No keyboard controller answered. This kernel drives PS/2 keyboards only."
# What the kernel's serial line "keyboard: ..." says became of the keyboard, and the one sentence the
# stopped screen must then show (none when the keyboard is in use). The kernel has three reasons for
# having no keyboard; a screen must show the one its own boot gave and neither of the others.
READY = "a PS/2 controller answered"
BUSY = "a controller is there but did not take its set-up in time; it is not used"
SILENT = "the controller is set up but the keyboard did not acknowledge; it is not used"
REASONS = {
    READY: None,
    "no controller": NO_KEYBOARD,
    BUSY: "The keyboard controller did not take its set-up in time; the keyboard is not used.",
    SILENT: "The keyboard did not acknowledge, so it is not used.",
}
POWER = "This kernel cannot switch the computer off. Hold its power button until it goes dark."
NO_REPLAY = "this keyboard controller does not replay keys for a test"
NO_CONTROLLER = "there is no keyboard controller"
SETUP_REFUSED = "the keyboard controller did not take its set-up"
NO_ACK = "the keyboard did not acknowledge"
KEYBOARD_FOR_SKIP = {None: READY, NO_REPLAY: READY, NO_CONTROLLER: "no controller", SETUP_REFUSED: BUSY, NO_ACK: SILENT}
SKIP_RECORD = "the self-tests that press keys were not run: "
SIX_KEYS = "the six tests that press keys, because "
WHY = SIX_KEYS + NO_REPLAY + "."


NOTHING = "Nothing is running and nothing more will be written."


def text_of(rows):
    return " ".join(" ".join(rows).split())


def whole_problems(rows):
    """H9: the screen is whole sentences the kernel has, in the kernel's order, and nothing else."""
    one_of = lambda texts: "(?:%s)" % "|".join(re.escape(text) for text in texts)
    verdict = r"(?:Verified: all \d+ self-tests it ran passed\.|NOT VERIFIED: \d+ of \d+ self-tests failed\.)"
    skipped = re.escape("Not run: " + SIX_KEYS) + one_of(reason for reason in KEYBOARD_FOR_SKIP if reason) + r"\."
    reason = one_of(reason for reason in REASONS.values() if reason)
    whole = "%s(?: %s)?(?: %s)?(?: %s)?(?: %s)? %s" % (re.escape(STOPPED), verdict, skipped, reason, re.escape(NOTHING), re.escape(POWER))
    return [] if re.fullmatch(whole, text_of(rows)) else ["the screen is not whole sentences of the kernel's in their order"]


def skip_problems(serial, rows, expected, *, f5=False, compact=False):
    """Check the exact reason against the fixture, the same boot's serial record and the decoded pixels."""
    lines = serial.splitlines()
    outcomes = [line[len("keyboard: "):] for line in lines if line.startswith("keyboard: ")]
    if outcomes != [KEYBOARD_FOR_SKIP[expected]]:
        return ["keyboard initialization %r disagrees with the fixture for skipped reason %r" % (outcomes, expected)]
    reasons = [line[len(SKIP_RECORD):] for line in lines if line.startswith(SKIP_RECORD)]
    if reasons != ([expected] if expected is not None else []):
        return ["serial skipped-test records %r disagree with the fixture %r" % (reasons, expected)]
    text = text_of(rows)
    if not f5 and whole_problems(rows):
        return whole_problems(rows)
    heading = "Not run at start:" if f5 else "Not run:"
    # A smaller stopped screen can sacrifice the entire skipped-tests paragraph. A visible one must
    # still be complete. F5 and roomier stopped screens carry it whenever the serial record does.
    if compact and not f5 and "Not run" not in text and SIX_KEYS not in text and not any(why + "." in text for why in reasons):
        return []
    if text.count("Not run") != len(reasons) or text.count(SIX_KEYS) != len(reasons):
        return ["wrong number of visible skipped-test explanations for serial %r" % reasons]
    if reasons and (heading + " " + SIX_KEYS + reasons[0] + ".") not in text:
        return ["the visible skipped-test explanation differs from the same boot's serial record"]
    return []


def stopped_problems(rows, keyboard, passed=None, outcome=None):
    """`passed` is the number of "  pass " lines in the whole serial log, when it is known."""
    problems = []
    held = [row for row in rows if row.strip()]
    text = text_of(rows)
    if not held or held[0].strip() != STOPPED:
        problems.append("the first row is not %r" % STOPPED)
    if not any(row.startswith("  Verified: all ") for row in rows) or "self-tests it ran passed." not in text:
        problems.append("the verification sentence is not whole")
    if passed is not None and "Verified: all %d self-tests it ran passed." % passed not in text:
        problems.append("the count on the screen is not the %d passes of the serial log" % passed)
    if POWER not in text:
        problems.append("what the person must do is not whole")
    if (NO_KEYBOARD in text) == keyboard:
        problems.append("the sentence about a keyboard is %s" % ("there with a keyboard" if keyboard else "missing"))
    # The reason shown is the one this boot's serial log gave, and no other (Director 2's finding: the
    # first form looked only for the one sentence and would have passed a screen that also showed another).
    if outcome is not None:
        if outcome not in REASONS:
            problems.append("the serial log's keyboard outcome %r is not one the kernel has" % outcome)
        else:
            shown = [reason for reason in REASONS.values() if reason and reason in text]
            wanted = [REASONS[outcome]] if REASONS[outcome] else []
            if shown != wanted or (wanted and text.count(wanted[0]) != 1):
                problems.append("the keyboard reasons shown are %r; this boot's serial log calls for %r" % (shown, wanted))
    if any("?" in row for row in rows):
        problems.append("a cell is no character of the kernel's font")
    return problems + whole_problems(rows)


class Guest:
    def __init__(self, qemu, kernel, mode, machine, out, glyphs):
        out.mkdir(parents=True)
        self.out, self.glyphs = out, glyphs
        self.serial, sock = out / "serial.txt", out / "qmp.sock"
        digest = subprocess.check_output([sys.executable, str(MEASURE), str(kernel)], text=True).strip()
        self.process = subprocess.Popen(
            [qemu, "-machine", machine, "-accel", "tcg", "-cpu", "max,-hypervisor", "-m", "128M", "-kernel", str(kernel),
             "-append", "image=%s desktop=%s" % (digest, mode), "-vga", "std", "-nic", "none", "-display", "none", "-monitor", "none",
             "-qmp", "unix:%s,server=on,wait=off" % sock, "-serial", "file:%s" % self.serial, "-no-reboot"],
            stdout=subprocess.DEVNULL, stderr=(out / "qemu-stderr.txt").open("wb"))
        end = time.monotonic() + 20
        while not sock.exists():
            if time.monotonic() > end or self.process.poll() is not None:
                raise RuntimeError("the emulator did not start")
            time.sleep(0.1)
        link = socket.socket(socket.AF_UNIX)
        link.connect(str(sock))
        self.stream = link.makefile("rw")
        self.stream.readline()
        self.rpc("qmp_capabilities")

    def rpc(self, command, **arguments):
        self.stream.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
        self.stream.flush()
        while True:
            reply = json.loads(self.stream.readline())
            if "return" in reply or "error" in reply:
                if "error" in reply:
                    raise RuntimeError("%s: %s" % (command, reply["error"]))
                return reply["return"]

    def key(self, *names):
        self.rpc("send-key", keys=[{"type": "qcode", "data": n} for n in names], **{"hold-time": 80})
        time.sleep(0.9)

    def screen(self, name, accept, timeout=120):
        """Two successive captures that are the same bytes and are accepted."""
        path, end, rows, last = self.out / (name + ".ppm"), time.monotonic() + timeout, None, None
        while time.monotonic() < end:
            time.sleep(0.5)
            self.rpc("screendump", filename=str(path))
            frame = path.read_bytes()
            rows = PINS["read"](frame, self.glyphs)
            if rows and frame == last and accept(rows):
                (self.out / (name + ".txt")).write_text("\n".join(rows) + "\n")
                return rows
            last = frame
        (self.out / (name + ".txt")).write_text("\n".join(rows or []) + "\n")
        raise TimeoutError("%s was not accepted; the last rows are in %s.txt" % (name, name))

    def outcome(self):
        """What the same boot's serial log says became of the keyboard: the text after "keyboard: "."""
        found = [line[len("keyboard: "):] for line in self.serial.read_text(errors="replace").splitlines() if line.startswith("keyboard: ")]
        return found[0] if len(found) == 1 else "%d keyboard lines" % len(found)

    def passed(self):
        return sum(line.startswith("  pass ") for line in self.serial.read_text(errors="replace").splitlines())

    def running(self):
        return self.process.poll() is None and self.rpc("query-status")["status"] == "running"

    def end(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("default", type=Path)
    parser.add_argument("demonstration", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--busy", type=Path, help="a kernel built with keyboard-busy-demo")
    parser.add_argument("--silent", type=Path, help="a kernel built with keyboard-silent-demo")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    glyphs = CHECK["kernel_glyphs"]()
    problems, results = [], {}
    # The controls for H6, on made-up rows: only the reading is being proved here, not the kernel.
    good = ["  " + STOPPED, "  Verified: all 66 self-tests it ran passed.", "  " + NO_KEYBOARD, "  " + POWER]
    busy, silent = REASONS[BUSY], REASONS[SILENT]
    controls = {"an intact screen": (good, False, "no controller", False),
                "a second reason added": (good[:3] + ["  " + busy] + good[3:], False, "no controller", True),
                "the reason of another outcome": (good[:2] + ["  " + silent] + good[3:], False, "no controller", True),
                "the right sentence for the wrong outcome": (good, False, "the controller is set up but the keyboard did not acknowledge; it is not used", True),
                "a reason shown with a keyboard in use": (good[:2] + ["  " + busy] + good[3:], True, "a PS/2 controller answered", True)}
    for what, (rows, keyboard, outcome, must_refuse) in controls.items():
        if bool(stopped_problems(rows, keyboard, 66, outcome)) != must_refuse:
            problems.append("control: %s was %s" % (what, "accepted" if must_refuse else "refused"))
    results["reason_controls"] = sorted(controls)
    # The controls for H9, on made-up rows: a small screen with one row to spare and part of the paragraph in it.
    record = "keyboard: no controller\n" + SKIP_RECORD + NO_CONTROLLER + "\n"
    paragraph = "Not run: " + SIX_KEYS + NO_CONTROLLER + "."
    parts = {"nothing": (None, False), "the whole paragraph": (paragraph, False)}
    parts.update({cut: (cut, True) for cut in ("Not", "Not ru", "Not r", "Not run:", "the six tests that press", "there is no keyboard", paragraph[:-1])})
    for what, (extra, must_refuse) in parts.items():
        rows = good[:2] + (["  " + extra] if extra else []) + good[2:]
        refused = bool(skip_problems(record, rows, NO_CONTROLLER, compact=True)) and bool(stopped_problems(rows, False, 66, "no controller"))
        accepted = not skip_problems(record, rows, NO_CONTROLLER, compact=True) and not stopped_problems(rows, False, 66, "no controller")
        if not (refused if must_refuse else accepted):
            problems.append("control: a small screen with %s of the skipped-tests paragraph was %s" % (what, "accepted" if must_refuse else "refused"))
    results["paragraph_controls"] = sorted(parts)

    def trial(name, kernel, mode, machine, steps, expected_skip=None):
        guest = None
        try:
            guest = Guest(args.qemu, kernel, mode, machine, evidence / name, glyphs)
            for problem in steps(guest):
                problems.append("%s: %s" % (name, problem))
            for problem in skip_problems(guest.serial.read_text(), results[name], expected_skip, compact=mode == "320x200"):
                problems.append("%s H8: %s" % (name, problem))
            if not guest.running():
                problems.append("%s: the emulator is not still running on the stopped screen" % name)
        except Exception as error:  # every failure is kept, with what was on the screen
            problems.append("%s: %s: %s" % (name, type(error).__name__, error))
        finally:
            if guest:
                guest.end()

    def no_keyboard(guest):
        rows = guest.screen("stopped", lambda rows: any(STOPPED in row for row in rows))
        results[guest.out.name] = [row for row in rows if row.strip()]
        return ["H1: " + p for p in stopped_problems(rows, False, guest.passed(), guest.outcome())]

    def no_keyboard_other(guest):
        # No keyboard is in use and the reason is not that none answered: H6 says which sentence, from the
        # serial line; the sentence for "no controller" must not be there, so `keyboard` is given as true
        # for that one test and the rest is as on any stopped screen.
        rows = guest.screen("stopped", lambda rows: any(STOPPED in row for row in rows))
        results[guest.out.name] = [row for row in rows if row.strip()]
        outcome = guest.outcome()
        found = ["H7: " + p for p in stopped_problems(rows, True, guest.passed(), outcome)]
        if REASONS.get(outcome) in (None, NO_KEYBOARD):
            found.append("H7: the serial log's keyboard outcome is %r, which is not one of the two refusals" % outcome)
        return found

    def with_keyboard(guest):
        guest.screen("home", lambda rows: any("Programs" in row for row in rows))
        guest.key("f5")
        f5 = guest.screen("f5", lambda rows: any("VERIFICATION" in row for row in rows))
        found = ["H4: the default kernel's F5 says tests were not run"] if "Not run" in text_of(f5) else []
        found.extend("H8 F5: " + p for p in skip_problems(guest.serial.read_text(), f5, None, f5=True))
        guest.key("esc")
        guest.key("f10")
        guest.screen("power", lambda rows: any("POWER OFF" in row for row in rows))
        guest.key("ret")
        rows = guest.screen("stopped", lambda rows: any(STOPPED in row for row in rows))
        results[guest.out.name] = [row for row in rows if row.strip()]
        return found + ["H2: " + p for p in stopped_problems(rows, True, guest.passed(), guest.outcome())]

    def without_replay(guest):
        guest.screen("home", lambda rows: any("Programs" in row for row in rows))
        guest.key("f5")
        f5 = guest.screen("f5", lambda rows: any("VERIFICATION" in row for row in rows))
        found = [] if "Not run at start: " + WHY in text_of(f5) else ["H3: F5 does not say which tests were not run and why"]
        found.extend("H8 F5: " + p for p in skip_problems(guest.serial.read_text(), f5, NO_REPLAY, f5=True))
        guest.key("esc")
        guest.key("f10")
        guest.screen("power", lambda rows: any("POWER OFF" in row for row in rows))
        guest.key("ret")
        rows = guest.screen("stopped", lambda rows: any(STOPPED in row for row in rows))
        results[guest.out.name] = [row for row in rows if row.strip()]
        if "Not run: " + WHY not in text_of(rows):
            found.append("H3: the stopped screen does not say which tests were not run and why")
        return found + ["H3: " + p for p in stopped_problems(rows, True, guest.passed(), guest.outcome())]

    trial("nokbd-320", args.default, "320x200", "pc,i8042=off", no_keyboard, NO_CONTROLLER)
    trial("nokbd-1080", args.default, "1920x1080", "pc,i8042=off", no_keyboard, NO_CONTROLLER)
    trial("kbd-800", args.default, "800x600", "pc", with_keyboard)
    trial("noreplay-800", args.demonstration, "800x600", "pc", without_replay, NO_REPLAY)
    # H7: the kernel's other two reasons, each in a guest of its own.
    if args.busy:
        trial("busy-800", args.busy, "800x600", "pc", no_keyboard_other, SETUP_REFUSED)
        trial("busy-320", args.busy, "320x200", "pc", no_keyboard_other, SETUP_REFUSED)
    if args.silent:
        trial("silent-800", args.silent, "800x600", "pc", no_keyboard_other, NO_ACK)
        trial("silent-320", args.silent, "320x200", "pc", no_keyboard_other, NO_ACK)
    (evidence / "result.json").write_text(json.dumps({"problems": problems, "results": results}, indent=2) + "\n")
    for problem in problems:
        print("FAILED", problem)
    if problems:
        sys.exit(1)
    print("stopped screen: whole with and without a keyboard at three sizes; the tests not run are named on F5 and on it")


if __name__ == "__main__":
    main()
