#!/usr/bin/env python3
"""Settings and the small-display list of programs on displays with few rows (issue 314).

Two defects were seen in guests by A3 and B3 and predicted at further heights by Director 2: in Settings a
widget's row lay under the keys' line or below the screen, so Enter changed a setting with nothing shown;
in the list of programs the line that says what a program may use was drawn over the sentence about
verification (ten rows) or over the chosen program (twelve rows).

This check takes the kernel and Surfbird out of a built tester CD, verifies the kernel against the
receipt, and boots that kernel directly, with no disk and no network, with the same program under two
names (alpha.elf and bravo.elf). The predicates below were fixed before the first run.

Settings, at 320 by 200, 220, 240, 260 and 280 and, as the roomy control, at 640 by 480. With each of the
five choices chosen in turn, on a frame of the guest:
 S1 each of the five choices is on a row of its own, as its state mark and its whole label and nothing else;
 S2 the last row that holds anything is the keys' line, whole, and it is none of those five rows;
 S3 exactly one row has the ground of a chosen row, and it is the row of the choice that is chosen;
 S4 after Enter, the state marks read are those before with exactly this choice's change (a widget turned
    over; of the two forms, the chosen one on and the other off), and the frame's bytes differ unless the
    choice was already on (the first form at the start);
 S5 after every choice has been set and the widgets set back, Esc gives a desktop whose rows equal those
    before Settings was opened; and with the widget Session then turned on alone, Esc gives a desktop
    whose rows differ from those.

The list of programs, at 320 by 200, 220, 240 and 260 and, as the roomy control, at 640 by 480:
 P1 a row begins with the verification sentence in one of the kernel's wordings, with the number of
    self-tests the serial log reports;
 P2 exactly one row has the ground of a chosen row and, after its stripe, it reads as the chosen program's
    name and nothing else;
 P3 a row begins "Access:" and the keys' line or lines are whole on the last rows that hold anything;
 P4 the rows of P1, P2, P3 are different rows;
 P5 Down moves the choice from alpha.elf to bravo.elf and P1 to P4 hold again.

The check proves its own reading on a good frame of each kind: for Settings, the keys' line written over
a widget's row, and a state mark changed; for the list, the verification row emptied, and the chosen row
emptied. Each must be refused.

Usage: check_small_screens.py <built tester CD folder> <new evidence folder> [--qemu qemu-system-x86_64]
Every failure is kept in result.json with the rows that were on the screen.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import runpy
import socket
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
CHECK = runpy.run_path(str(HERE / "check.py"))
BUILD = runpy.run_path(str(HERE / "build.py"))
PINS = runpy.run_path(str(HERE / "check_compact_pins.py"))
PROGRAMS = ["alpha.elf", "bravo.elf"]
CHOICES = ["A bar across the screen", "A dock in the middle", "Recent activity", "This machine", "Session"]
SETTINGS_KEYS = {True: "Up/Down choose    Enter set    Esc back", False: "Up/Down  Enter set  Esc back"}
LIST_KEYS = {True: ["Up/Down choose   Enter open   D details   R register   P off"],
             False: ["Up/Down choose   Enter open", "D details  R register  P off"]}
MARK = re.compile(r"^  (\(o\)|\( \)|\[on\]|\[  \]) {2,3}(.+)$")


def grounds(frame):
    """The ground colour of the sixth cell of every text row, and the colour most rows have."""
    _, size, _, pixels = frame.split(b"\n", 3)
    width, height = (int(n) for n in size.split())
    found = []
    for row in range(height // 20):
        cell = [pixels[at:at + 3] for y in range(20) for x in range(10) for at in (((row * 20 + y) * width + 50 + x) * 3,)]
        found.append(max(set(cell), key=cell.count))
    return found, max(set(found), key=found.count)


def chosen_rows(frame):
    found, usual = grounds(frame)
    return [i for i, ground in enumerate(found) if ground != usual]


def settings_states(rows):
    """{label: mark} for every row that is a mark and a label and nothing else."""
    states = {}
    for row in rows:
        match = MARK.match(row)
        if match and match[2] in CHOICES and row == "  %s%s%s" % (match[1], " " * (6 - len(match[1])), match[2]):
            states.setdefault(match[2], []).append(match[1])
    return states


def settings_problems(rows, frame, chosen, roomy):
    problems = []
    states = settings_states(rows)
    where = {label: [i for i, row in enumerate(rows) if MARK.match(row) and MARK.match(row)[2] == label] for label in CHOICES}
    for label in CHOICES:
        if len(states.get(label, [])) != 1 or len(where[label]) != 1:
            problems.append("S1: %r is not on exactly one row of its own" % label)
    held = [i for i, row in enumerate(rows) if row.strip()]
    if not held or rows[held[-1]] != "  " + SETTINGS_KEYS[roomy] or any(held[-1] in at for at in where.values()):
        problems.append("S2: the last row is not the keys' line alone")
    lit = chosen_rows(frame)
    if len(lit) != 1 or where[CHOICES[chosen]] != lit:
        problems.append("S3: chosen rows %r, expected the row of %r %r" % (lit, CHOICES[chosen], where[CHOICES[chosen]]))
    return problems


def expected_after(before, chosen):
    after = dict(before)
    if chosen < 2:
        after[CHOICES[0]], after[CHOICES[1]] = ("(o)", "( )") if chosen == 0 else ("( )", "(o)")
    else:
        # A state that could not be read before stays unread here, and S1 has said so.
        if CHOICES[chosen] in before:
            after[CHOICES[chosen]] = "[  ]" if before[CHOICES[chosen]] == "[on]" else "[on]"
    return after


def list_problems(rows, frame, name, roomy, passed):
    problems = []
    verified = [i for i, row in enumerate(rows) if re.match(r"^  Verified(?: at start)?: all %d\b" % passed, row)]
    if len(verified) != 1:
        problems.append("P1: the verification sentence is on %d rows" % len(verified))
    lit = chosen_rows(frame)
    # The chosen row begins with a coloured stripe in its third cell, which is no character; the name follows it.
    if len(lit) != 1 or rows[lit[0]][:2] != "  " or rows[lit[0]][3:] != name:
        problems.append("P2: chosen rows %r read %r, expected %r" % (lit, [rows[i][3:] for i in lit], name))
    access = [i for i, row in enumerate(rows) if row.startswith("  Access:")]
    held = [i for i, row in enumerate(rows) if row.strip()]
    keys = LIST_KEYS[roomy]
    if not access or [rows[i] for i in held[-len(keys):]] != ["  " + line for line in keys]:
        problems.append("P3: no Access row, or the keys are not whole on the last rows")
    key_rows = held[-len(keys):]
    if len(set(verified + lit + access + key_rows)) != len(verified) + len(lit) + len(access) + len(key_rows):
        problems.append("P4: two of the rows are the same row")
    return problems


def boot(qemu, kernel, folder, measurement, mode, out, glyphs, timeout):
    out.mkdir(parents=True)
    serial, sock = out / "serial.txt", out / "qmp.sock"
    cmd = [qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "qemu64", "-m", "128M", "-kernel", str(kernel),
           "-initrd", ",".join(PROGRAMS), "-append", "image=%s desktop=%s" % (measurement, mode), "-vga", "std", "-nic", "none",
           "-display", "none", "-monitor", "none", "-qmp", "unix:%s,server=on,wait=off" % sock, "-serial", "file:%s" % serial, "-no-reboot"]
    guest = subprocess.Popen(cmd, cwd=folder, stdout=subprocess.DEVNULL, stderr=(out / "qemu-stderr.txt").open("wb"))
    try:
        end = time.monotonic() + 20
        while not sock.exists():
            if time.monotonic() > end or guest.poll() is not None:
                raise RuntimeError("the emulator did not start")
            time.sleep(0.1)
        link = socket.socket(socket.AF_UNIX)
        link.connect(str(sock))
        stream = link.makefile("rw")

        def rpc(command, **arguments):
            stream.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if "return" in reply or "error" in reply:
                    if "error" in reply:
                        raise RuntimeError("%s: %s" % (command, reply["error"]))
                    return reply

        stream.readline()
        rpc("qmp_capabilities")

        def key(*names):
            rpc("send-key", keys=[{"type": "qcode", "data": n} for n in names], **{"hold-time": 80})
            time.sleep(0.9)

        def screen(name, accept):
            """Two successive captures that are the same bytes and are accepted: (rows, frame)."""
            path, end = out / (name + ".ppm"), time.monotonic() + timeout
            rows, last = None, None
            while time.monotonic() < end:
                time.sleep(0.4)
                rpc("screendump", filename=str(path))
                frame = path.read_bytes()
                rows = PINS["read"](frame, glyphs)
                if rows and frame == last and accept(rows, frame):
                    (out / (name + ".txt")).write_text("\n".join(rows) + "\n")
                    return rows, frame
                last = frame
            (out / (name + ".txt")).write_text("\n".join(rows or []) + "\n")
            raise TimeoutError("%s at %s was not accepted; the last rows are in %s.txt" % (name, mode, name))

        end = time.monotonic() + timeout
        while "desktop: " not in (serial.read_text(errors="replace") if serial.exists() else ""):
            if time.monotonic() > end:
                raise TimeoutError("the kernel did not reach its desktop at " + mode)
            time.sleep(0.5)
        log = serial.read_text(errors="replace")
        if measurement + " (measured; equal to the value the loader named)" not in log:
            raise RuntimeError("the running kernel did not measure as the receipt says")
        passed = sum(line.startswith("  pass ") for line in log.split("desktop: ")[0].splitlines())
        return key, screen, guest, passed
    except BaseException:
        guest.kill()
        raise


def settings_trial(key, screen, roomy, problems, results, mode):
    home, _ = screen("home", lambda rows, frame: any("Programs" in row for row in rows))
    key("f2")
    rows, frame = screen("settings-0", lambda rows, frame: any("SETTINGS" in row for row in rows))
    states = {label: marks[0] for label, marks in settings_states(rows).items() if len(marks) == 1}
    seen = []
    for chosen in range(5):
        if chosen:
            key("down")
            rows, frame = screen("settings-%d" % chosen, lambda rows, frame: True)
        found = settings_problems(rows, frame, chosen, roomy)
        key("ret")
        after_rows, after_frame = screen("settings-%d-set" % chosen, lambda rows, frame: True)
        found += settings_problems(after_rows, after_frame, chosen, roomy)
        now = {label: marks[0] for label, marks in settings_states(after_rows).items() if len(marks) == 1}
        want = expected_after(states, chosen)
        if now != want:
            found.append("S4: states read %r, expected %r" % (now, want))
        if (after_frame == frame) != (want == states):
            found.append("S4: the frame %s although the state %s" % ("did not change" if after_frame == frame else "changed", "changed" if want != states else "did not"))
        problems += ["%s settings choice %d: %s" % (mode, chosen, p) for p in found]
        seen.append({"choice": CHOICES[chosen], "before": states.get(CHOICES[chosen]), "after": now.get(CHOICES[chosen])})
        states, rows, frame = now, after_rows, after_frame
    good_rows, good_frame = rows, frame
    # Set everything back: the three widgets off (the choice is on Session), the bar chosen again.
    key("ret")
    key("up")
    key("ret")
    key("up")
    key("ret")
    key("up")
    key("up")
    key("ret")
    key("esc")
    back, _ = screen("home-after", lambda rows, frame: any("Programs" in row for row in rows))
    if back != home:
        problems.append("%s S5: the desktop after setting everything back differs from the one before" % mode)
    key("f2")
    for _ in range(4):
        key("down")
    key("ret")
    key("esc")
    with_session, _ = screen("home-session-on", lambda rows, frame: any("Programs" in row for row in rows))
    if with_session == home:
        problems.append("%s S5: the desktop with the widget Session on equals the one with it off" % mode)
    results["settings"] = seen
    # The controls, on the good frame with the last choice chosen and on.
    at = next((i for i, row in enumerate(good_rows) if row.endswith("  Session")), None)
    if at is None:
        problems.append("%s: no good Settings frame to make the controls from" % mode)
        return
    forged = {"the keys' line over a widget's row": good_rows[:at] + ["  " + SETTINGS_KEYS[roomy]] + good_rows[at + 1:],
              "a state mark changed": [row.replace("[on]  Session", "[  ]  Session") for row in good_rows]}
    if not settings_problems(forged["the keys' line over a widget's row"], good_frame, 4, roomy):
        problems.append("%s control accepted: the keys' line over a widget's row" % mode)
    if settings_states(forged["a state mark changed"]) == settings_states(good_rows):
        problems.append("%s control accepted: a state mark changed" % mode)
    results["settings_controls_refused"] = sorted(forged)


def list_trial(key, screen, roomy, passed, problems, results, mode):
    key("ret")
    seen = {}
    for name in PROGRAMS:
        if name != PROGRAMS[0]:
            key("down")
        rows, frame = screen("programs-" + name, lambda rows, frame: any(row.startswith("  Access:") for row in rows) or True)
        found = list_problems(rows, frame, name, roomy, passed)
        problems += ["%s programs with %s chosen: %s" % (mode, name, p) for p in found]
        seen[name] = [row for row in rows if row.strip()]
    results["programs"] = seen
    lit = chosen_rows(frame)
    verified = [i for i, row in enumerate(rows) if row.startswith("  Verified")]
    for what, gone in (("the verification row emptied", verified[:1]), ("the chosen row emptied", lit[:1])):
        forged = [("" if i in gone else row) for i, row in enumerate(rows)]
        if not gone or not list_problems(forged, frame, PROGRAMS[-1], roomy, passed):
            problems.append("%s control accepted: %s" % (mode, what))
    results["programs_controls_refused"] = ["the chosen row emptied", "the verification row emptied"]
    key("esc")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("built", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--timeout", type=float, default=90)
    args = parser.parse_args()
    receipt = json.loads((args.built / "receipt.json").read_text())
    image = (args.built / "pcp-os-tester.iso").read_bytes()
    if hashlib.sha256(image).hexdigest() != receipt["iso_sha256"]:
        sys.exit("the ISO is not the one its receipt describes")
    files = BUILD["read_back"](image)
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    folder = evidence / "boot"
    folder.mkdir()
    (folder / "kernel.elf32").write_bytes(files["kernel.elf32"])
    for name in PROGRAMS:
        (folder / name).write_bytes(files["surf.elf"])
    glyphs = CHECK["kernel_glyphs"]()
    problems, results = [], {}
    for mode, settings, programs in (("320x200", True, True), ("320x220", True, True), ("320x240", True, True), ("320x260", True, True),
                                     ("320x280", True, False), ("640x480", True, True)):
        roomy = mode == "640x480"
        results[mode] = {}
        try:
            key, screen, guest, passed = boot(args.qemu, folder / "kernel.elf32", folder, receipt["kernel_measurement"], mode, evidence / mode, glyphs, args.timeout)
        except Exception as error:
            problems.append("%s: %s: %s" % (mode, type(error).__name__, error))
            continue
        try:
            if settings:
                # Settings is roomy from fourteen rows and forty-one columns; 320 across is never roomy.
                settings_trial(key, screen, roomy, problems, results[mode], mode)
            if programs:
                list_trial(key, screen, roomy, passed, problems, results[mode], mode)
            key("f10")
            screen("power-question", lambda rows, frame: any("POWER OFF" in row for row in rows))
            key("ret")
            results[mode]["exit_code"] = guest.wait(timeout=40)
            if results[mode]["exit_code"] != 0:
                problems.append("%s: the guest ended with %r" % (mode, results[mode]["exit_code"]))
        except Exception as error:  # every failure is kept, with what was on the screen
            problems.append("%s: %s: %s" % (mode, type(error).__name__, error))
        finally:
            if guest.poll() is None:
                guest.kill()
    (evidence / "result.json").write_text(json.dumps({"problems": problems, "results": results}, indent=2) + "\n")
    for problem in problems:
        print("FAILED", problem)
    if problems:
        sys.exit(1)
    print("small screens: Settings at six sizes and the list of programs at five passed; controls refused")


if __name__ == "__main__":
    main()
