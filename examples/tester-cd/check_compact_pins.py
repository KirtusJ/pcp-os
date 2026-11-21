#!/usr/bin/env python3
"""The small-display desktop with more pinned programs than it has rows.

The tester CD carries one or two programs, so its own check cannot fill the small-display desktop. This
check takes the kernel and Surfbird out of a built tester CD, verifies the kernel against the receipt, and
boots that kernel directly with the same program under fourteen names, at 640 by 480 and at 320 by 200.
It pins every one, then requires, with the first pin chosen and with the last:

- the list follows the choice, and row 4 says which places are shown of how many;
- the chosen program's whole name, and its size and the beginning of its digest, are on the screen;
- the row of the keys is intact, and nothing else is on it or on the row above it at 640 by 480;
- with two widgets turned on, their lines are shown too and the identity is still whole.

A last boot at 640 by 480 has one program under a name of forty characters, which the kernel refuses to
admit as a task (its self-tests say so for 33 bytes). Enter on its pin must leave, on the desktop, the
words that it could not be started, with its identity still whole (A3's finding: the refusal was silent).

A boot at 320 by 200 has the same program under two names of 64 characters that differ only at
character 31 (B3's case: beginning, ending, size and digest all equal). Each is pinned and chosen, and the
parts of its name are turned with Right: joined, they must be the whole name the loader gave, and the two
pins must differ in exactly the part that holds character 31.

It also proves its own reading: three altered copies of a good screen must be refused (the identity's
size-and-digest row replaced by the keys, which is the defect B3 found; the wrong program named; the keys
row overwritten).

Usage: check_compact_pins.py <built tester CD folder> <new evidence folder> [--qemu qemu-system-x86_64]
Predicates were fixed before the first run. The guest is given no disk and no network.
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
PROGRAMS = ["program-%02d.elf" % n for n in range(1, 15)]
KEYS_WIDE = "Up/Down choose   Enter open   F1 keys   F10 off"
KEYS_NARROW = "Enter open  F1 keys  F10 off"


def read(frame, glyphs):
    """Every text row of a frame of any size, by the kernel's own font (check.py refuses frames under 640 by 480)."""
    magic, size, depth, pixels = frame.split(b"\n", 3)
    width, height = (int(n) for n in size.split())
    if magic != b"P6" or depth != b"255" or len(pixels) != width * height * 3:
        return None
    rows = []
    for row in range(height // 20):
        text = ""
        for column in range(width // 10):
            cell = [pixels[at:at + 3] for y in range(20) for x in range(10)
                    for at in (((row * 20 + y) * width + column * 10 + x) * 3,)]
            ground = max(set(cell), key=cell.count)
            text += glyphs.get(tuple(pixel != ground for pixel in cell), "?")
        rows.append(text.rstrip())
    return rows


def home_problems(rows, chosen, places, facts, wide, widgets=()):
    """What is wrong with a small-display desktop on which pin number `chosen` (from 1) is chosen."""
    if rows is None:
        return ["not a whole frame"]
    problems = []
    name = PROGRAMS[chosen - 1]
    keys = len(rows) - 2
    if rows[keys].strip() != (KEYS_WIDE if wide else KEYS_NARROW):
        problems.append("the row of the keys reads %r" % rows[keys])
    listed = [row.strip() for row in rows[5:keys] if row.strip().startswith(("PINNED ", "Programs"))]
    if "PINNED " + name not in listed:
        problems.append("the chosen pin is not in the list shown")
    if wide:
        note = re.fullmatch(r"  Places (\d+) to (\d+) of (\d+)(?: \| .*)?", rows[4])
        if not note or int(note[3]) != places or not int(note[1]) <= chosen + 1 <= int(note[2]):
            problems.append("row 4 does not say which places are shown, with the chosen one among them: %r" % rows[4])
        elif len(listed) != int(note[2]) - int(note[1]) + 1:
            problems.append("the list has %d rows and row 4 says %s to %s" % (len(listed), note[1], note[2]))
    else:
        # A row of 28 characters says it shortly: which place is chosen of how many.
        if rows[4].strip() != "Place %d/%d" % (chosen + 1, places) or len(listed) != 1:
            problems.append("row 4 should read 'Place %d/%d' over one row of the list: %r, %r" % (chosen + 1, places, rows[4], listed))
    below = [row.strip() for row in rows[5 + len(listed):keys]]
    text = [row for row in below if row]
    wanted = (["Enter starts"] if wide else []) + [name, facts if wide else facts[:len(facts)]] + list(widgets)
    if wide and text != wanted:
        problems.append("below the list: %r, expected %r" % (text, wanted))
    if not wide:
        # At 320 by 200 a row holds 28 characters: the size and digest are cut and say so with "..".
        # At 320 by 200 a row holds 28 characters: the size, then as many of the digest's first sixteen
        # digits as fit (at least eight), then "..".
        size, digest = facts.split("   ")[0], facts.split("SHA-256 ")[1][:16]
        short = re.fullmatch(re.escape(size) + r" sha ([0-9a-f]{8,16})\.\.", text[1]) if len(text) == 2 else None
        if len(text) != 2 or text[0] != name or not (text[1] == facts or (short and digest.startswith(short[1]))):
            problems.append("below the list: %r, expected the name and the size and digest, cut visibly if cut" % text)
    if wide and rows[keys - 1].strip():
        problems.append("the row above the keys is not blank: %r" % rows[keys - 1])
    return problems


def boot(qemu, kernel, folder, measurement, mode, out, glyphs, timeout, programs=None):
    programs = programs or PROGRAMS
    out.mkdir(parents=True)
    serial, sock = out / "serial.txt", out / "qmp.sock"
    cmd = [qemu, "-machine", "pc", "-accel", "tcg", "-cpu", "qemu64", "-m", "128M", "-kernel", str(kernel),
           "-initrd", ",".join(programs), "-append", "image=%s desktop=%s" % (measurement, mode), "-vga", "std", "-nic", "none",
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
            path, end = out / (name + ".ppm"), time.monotonic() + timeout
            rows = None
            while time.monotonic() < end:
                time.sleep(0.4)
                rpc("screendump", filename=str(path))
                rows = read(path.read_bytes(), glyphs)
                if rows and accept(rows):
                    (out / (name + ".txt")).write_text("\n".join(rows) + "\n")
                    return rows
            (out / (name + ".txt")).write_text("\n".join(rows or []) + "\n")
            raise TimeoutError("%s at %s did not appear; last rows are in %s.txt" % (name, mode, name))

        end = time.monotonic() + timeout
        while "desktop: " not in (serial.read_text(errors="replace") if serial.exists() else ""):
            if time.monotonic() > end:
                raise TimeoutError("the kernel did not reach its desktop at " + mode)
            time.sleep(0.5)
        log = serial.read_text(errors="replace")
        if measurement + " (measured; equal to the value the loader named)" not in log:
            raise RuntimeError("the running kernel did not measure as the receipt says")
        screen("home-empty", lambda rows: any("Programs" in row for row in rows[5:]))
        key("ret")
        for _ in programs:
            key("ctrl", "p")
            key("down")
        key("esc")
        key("down")
        return rpc, key, screen, guest, stream
    except BaseException:
        guest.kill()
        raise


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
    surf = files["surf.elf"]
    for name in PROGRAMS:
        (folder / name).write_bytes(surf)
    size = "%d KiB" % ((len(surf) + 1023) >> 10)
    facts = "%s   SHA-256 %s..." % (size, hashlib.sha256(surf).hexdigest()[:16])
    glyphs = CHECK["kernel_glyphs"]()
    places = len(PROGRAMS) + 1
    problems, results = [], {}
    for mode, wide in (("640x480", True), ("320x200", False)):
        out = evidence / mode
        rpc, key, screen, guest, stream = boot(args.qemu, folder / "kernel.elf32", folder, receipt["kernel_measurement"], mode, out, glyphs, args.timeout)
        try:
            first = screen("first-pin", lambda rows: not home_problems(rows, 1, places, facts, wide))
            for _ in PROGRAMS[1:]:
                key("down")
            last = screen("last-pin", lambda rows: not home_problems(rows, len(PROGRAMS), places, facts, wide))
            results[mode] = {"first_pin_row_4": first[4].strip(), "last_pin_row_4": last[4].strip()}
            if wide:
                # The controls: each altered copy of the good screen must be refused.
                keys = len(last) - 2
                at = max(i for i, row in enumerate(last[:keys]) if row.strip())
                forged = {"the size and digest replaced by the keys": last[:at] + [last[keys]] + last[at + 1:],
                          "another program named": [row.replace(PROGRAMS[-1], PROGRAMS[0]) if not row.strip().startswith("PINNED") else row for row in last],
                          "the keys overwritten": last[:keys] + ["  " + facts] + last[keys + 1:]}
                for what, rows in forged.items():
                    if not home_problems(rows, len(PROGRAMS), places, facts, wide):
                        problems.append("control accepted: " + what)
                results[mode]["controls_refused"] = sorted(forged)
                # Two widgets on: This machine and Session. Their lines are shown and the identity stays whole.
                key("f2")
                for _ in range(3):
                    key("down")
                key("ret")
                key("down")
                key("ret")
                key("esc")
                with_widgets = screen("last-pin-widgets", lambda rows: any(row.strip().startswith("memory ") for row in rows))
                lines = [row.strip() for row in with_widgets if row.strip().startswith(("memory ", "Session files", "VM files"))]
                found = home_problems(with_widgets, len(PROGRAMS), places, facts, wide, widgets=lines)
                if len(lines) != 2 or found:
                    problems.append("%s with widgets: %s" % (mode, found or "expected two widget lines, saw %r" % lines))
                results[mode]["widget_lines"] = lines
            key("f10")
            screen("power-question", lambda rows: any("POWER OFF" in row for row in rows))
            key("ret")
            results[mode]["exit_code"] = guest.wait(timeout=40)
        except Exception as error:  # every failure is kept, with what was on the screen
            problems.append("%s: %s" % (mode, error))
        finally:
            if guest.poll() is None:
                guest.kill()
            stream.close()
        log = (out / "serial.txt").read_text(errors="replace")
        if "ALL SELF-TESTS PASS." not in log:
            problems.append(mode + ": the kernel did not finish with all its self-tests passing")
    # The refused program: one pin, chosen, Enter; the desktop must say what happened.
    long_name = "a-name-of-forty-characters-in-all-xxxx.elf"
    (folder / long_name).write_bytes(surf)
    out = evidence / "refused-640x480"
    rpc, key, screen, guest, stream = boot(args.qemu, folder / "kernel.elf32", folder, receipt["kernel_measurement"], "640x480", out, glyphs, args.timeout, programs=[long_name])
    try:
        refusal = "Not started: the program could not be admitted as a task"

        def identity(rows, caption):
            text = [row.strip() for row in rows]
            return caption in text and long_name in text and facts in text and rows[len(rows) - 2].strip() == KEYS_WIDE
        before = screen("before-enter", lambda rows: identity(rows, "Enter starts"))
        if any("could not be started" in row for row in before):
            problems.append("refused: the desktop spoke of a start before Enter")
        key("ret")
        # The card stops saying 'Enter starts' and says why not, and the message is there in full as well.
        after = screen("after-enter", lambda rows: identity(rows, refusal) and not any(row.strip() == "Enter starts" for row in rows)
                       and "could not be started: the program could not be admitted as a task" in "".join(row[2:] for row in rows))
        results["refused-640x480"] = {"said": [row.strip() for row in after if row not in before and row.strip()]}
        key("f10")
        screen("power-question", lambda rows: any("POWER OFF" in row for row in rows))
        key("ret")
        results["refused-640x480"]["exit_code"] = guest.wait(timeout=40)
    except Exception as error:
        problems.append("refused-640x480: %s" % error)
    finally:
        if guest.poll() is None:
            guest.kill()
        stream.close()
    if long_name + " could not be started: the program could not be admitted as a task" not in (out / "serial.txt").read_text(errors="replace"):
        problems.append("refused-640x480: the serial log does not record the refusal")
    # Two names that differ in the middle only, on the smallest display: every character must be readable.
    aliases = ["a" * 30 + letter + "z" * 29 + ".elf" for letter in "AB"]
    for name in aliases:
        (folder / name).write_bytes(surf)
    # Ten short-named programs go before them, so that the place has two digits and row 4 is as full as it
    # gets (A3's case: 'Place 15/15 | name 1/3 <- ->' lost its right arrow when a count was added).
    fillers = PROGRAMS[:10]
    out = evidence / "aliases-320x200"
    rpc, key, screen, guest, stream = boot(args.qemu, folder / "kernel.elf32", folder, receipt["kernel_measurement"], "320x200", out, glyphs, args.timeout, programs=fillers + aliases)
    for _ in fillers:
        key("down")
    try:
        read_parts = []
        for pin in (1, 2):
            if pin == 2:
                key("down")
            parts = []
            for part in (1, 2, 3):
                if part > 1:
                    key("right")
                rows = screen("pin%d-part%d" % (pin, part), lambda rows, pin=pin, part=part: rows[4].strip() == "Place %d/13 | name %d/3 <- ->" % (pin + 11, part))
                body = [row.strip() for row in rows[5:len(rows) - 2] if row.strip()]
                if len(body) != 3 or not body[0].startswith("PINNED aaaa") or not re.fullmatch(r"\d+ KiB sha [0-9a-f]{8,16}\.\.", body[2]):
                    raise RuntimeError("pin %d part %d: unexpected rows %r" % (pin, part, body))
                parts.append(body[1])
            if "".join(parts) != aliases[pin - 1]:
                problems.append("aliases-320x200: pin %d's parts join to %r, not the loader's name" % (pin, "".join(parts)))
            read_parts.append(parts)
        differing = [i + 1 for i in range(3) if read_parts[0][i] != read_parts[1][i]]
        if differing != [2]:
            problems.append("aliases-320x200: the two names should differ in part 2 only; they differ in %r" % differing)
        results["aliases-320x200"] = {"parts": read_parts, "differ_in_part": differing}
        # With the name in parts and a two-digit place, row 4 is full. Two widgets are turned on that have
        # no room: how many lines are not shown must still be said (B3's and A3's finding), on the display's
        # last row, and row 4 must keep both arrows.
        key("f2")
        for _ in range(3):
            key("down")
        key("ret")
        key("down")
        key("ret")
        key("esc")
        hidden = screen("widgets-hidden", lambda rows: rows[-1].strip() == "2 widget line(s) not shown")
        if not re.fullmatch(r"  Place 13/13 [|] name [1-3]/3 <- ->", hidden[4]):
            problems.append("aliases-320x200: with widgets hidden, row 4 reads %r" % hidden[4])
        results["aliases-320x200"]["hidden_widgets_row"] = hidden[-1].strip()
        key("f10")
        screen("power-question", lambda rows: any("POWER OFF" in row for row in rows))
        key("ret")
        results["aliases-320x200"]["exit_code"] = guest.wait(timeout=40)
    except Exception as error:
        problems.append("aliases-320x200: %s" % error)
    finally:
        if guest.poll() is None:
            guest.kill()
        stream.close()
    # A name of 255 characters, the longest a file can have, as the fifteenth pin on the smallest display:
    # ten parts at place 16 of 16 (A3's and B3's case). Row 4 must keep both arrows on every part, and the
    # parts must join to the whole name. The kernel's register refuses a name this long as an artifact, so
    # this guest ends with one failed self-test of its own; that is not asserted either way here.
    long255 = "".join("%s%02d" % (chr(97 + i % 26), i) for i in range(84))[:251] + ".elf"
    (folder / long255).write_bytes(surf)
    out = evidence / "ten-320"
    rpc, key, screen, guest, stream = boot(args.qemu, folder / "kernel.elf32", folder, receipt["kernel_measurement"], "320x200", out, glyphs, args.timeout, programs=PROGRAMS[:14] + [long255])
    try:
        for _ in range(14):
            key("down")
        parts = []
        for part in range(1, 11):
            if part > 1:
                key("right")
            rows = screen("part%02d" % part, lambda rows, part=part: rows[4].strip() == "16/16 | name %d/10 <- ->" % part)
            body = [row.strip() for row in rows[5:len(rows) - 2] if row.strip()]
            parts.append(body[1] if len(body) == 3 else "")
        if "".join(parts) != long255:
            problems.append("ten-parts-320x200: the parts join to %r" % "".join(parts))
        results["ten-parts-320x200"] = {"row_4_first": "16/16 | name 1/10 <- ->", "row_4_last": "16/16 | name 10/10 <- ->", "joined_equals_name": "".join(parts) == long255}
        key("f10")
        screen("power-question", lambda rows: any("POWER OFF" in row for row in rows))
        key("ret")
        results["ten-parts-320x200"]["exit_code"] = guest.wait(timeout=40)
    except Exception as error:
        problems.append("ten-parts-320x200: %s" % error)
    finally:
        if guest.poll() is None:
            guest.kill()
        stream.close()
    summary = {"kernel_measurement": receipt["kernel_measurement"], "programs": len(PROGRAMS), "results": results, "problems": problems, "passed": not problems}
    (evidence / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for mode in results:
        print("%-8s %s" % (mode, results[mode]))
    for problem in problems:
        print("PROBLEM", problem)
    print("passed" if not problems else "FAILED")
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
