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
    "self-tests it ran passed."; the text holds "No keyboard this kernel can use answered: it drives PS/2
    keyboards only." and "This kernel cannot switch the computer off. Hold its power button until it goes
    dark."; no cell is anything but a character; the emulator is still running.
 H5 on every stopped screen the count in "Verified: all <n> self-tests it ran passed." is the number of
    passes in the whole serial log, which includes the check made after the session (A3 found it one short).
 H2 a keyboard, at 800 by 600: the desktop appears; F10 then Enter gives a screen that meets H1 except that
    it does not hold the sentence about a keyboard; the emulator is still running.
 H3 the kernel built with `no-key-replay-demo`, at 800 by 600: F5 holds "Not run at start: the six tests
    that press keys, because this keyboard controller does not replay keys for a test."; after Esc, F10 and
    Enter the stopped screen holds "Not run: the six tests that press keys, because this keyboard
    controller does not replay keys for a test."
 H4 the default kernel's F5 at 800 by 600 holds no "Not run".

Usage: check_stopped_screen.py <default kernel.elf32> <no-key-replay-demo kernel.elf32> <new evidence folder>
"""
import argparse
import json
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
NO_KEYBOARD = "No keyboard this kernel can use answered: it drives PS/2 keyboards only."
POWER = "This kernel cannot switch the computer off. Hold its power button until it goes dark."
WHY = "the six tests that press keys, because this keyboard controller does not replay keys for a test."


def text_of(rows):
    return " ".join(" ".join(rows).split())


def stopped_problems(rows, keyboard, passed=None):
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
    if any("?" in row for row in rows):
        problems.append("a cell is no character of the kernel's font")
    return problems


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
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    glyphs = CHECK["kernel_glyphs"]()
    problems, results = [], {}

    def trial(name, kernel, mode, machine, steps):
        guest = None
        try:
            guest = Guest(args.qemu, kernel, mode, machine, evidence / name, glyphs)
            for problem in steps(guest):
                problems.append("%s: %s" % (name, problem))
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
        return ["H1: " + p for p in stopped_problems(rows, False, guest.passed())]

    def with_keyboard(guest):
        guest.screen("home", lambda rows: any("Programs" in row for row in rows))
        guest.key("f5")
        f5 = guest.screen("f5", lambda rows: any("VERIFICATION" in row for row in rows))
        found = ["H4: the default kernel's F5 says tests were not run"] if "Not run" in text_of(f5) else []
        guest.key("esc")
        guest.key("f10")
        guest.screen("power", lambda rows: any("POWER OFF" in row for row in rows))
        guest.key("ret")
        rows = guest.screen("stopped", lambda rows: any(STOPPED in row for row in rows))
        results[guest.out.name] = [row for row in rows if row.strip()]
        return found + ["H2: " + p for p in stopped_problems(rows, True, guest.passed())]

    def without_replay(guest):
        guest.screen("home", lambda rows: any("Programs" in row for row in rows))
        guest.key("f5")
        f5 = guest.screen("f5", lambda rows: any("VERIFICATION" in row for row in rows))
        found = [] if "Not run at start: " + WHY in text_of(f5) else ["H3: F5 does not say which tests were not run and why"]
        guest.key("esc")
        guest.key("f10")
        guest.screen("power", lambda rows: any("POWER OFF" in row for row in rows))
        guest.key("ret")
        rows = guest.screen("stopped", lambda rows: any(STOPPED in row for row in rows))
        results[guest.out.name] = [row for row in rows if row.strip()]
        if "Not run: " + WHY not in text_of(rows):
            found.append("H3: the stopped screen does not say which tests were not run and why")
        return found + ["H3: " + p for p in stopped_problems(rows, True, guest.passed())]

    trial("nokbd-320", args.default, "320x200", "pc,i8042=off", no_keyboard)
    trial("nokbd-1080", args.default, "1920x1080", "pc,i8042=off", no_keyboard)
    trial("kbd-800", args.default, "800x600", "pc", with_keyboard)
    trial("noreplay-800", args.demonstration, "800x600", "pc", without_replay)
    (evidence / "result.json").write_text(json.dumps({"problems": problems, "results": results}, indent=2) + "\n")
    for problem in problems:
        print("FAILED", problem)
    if problems:
        sys.exit(1)
    print("stopped screen: whole with and without a keyboard at three sizes; the tests not run are named on F5 and on it")


if __name__ == "__main__":
    main()
