#!/usr/bin/env python3
"""Boot a tester CD made by build.py, with only that CD attached, and check one whole session.

    python3 examples/tester-cd/check.py <output directory of build.py> <new evidence directory>

For each of QEMU's `pc` and `q35` machines, on the ordinary clock and on the instruction-counted one: the
kernel must measure as the value in the receipt, pass every startup self-test, start the Surfbird demo from
the keyboard and see it end DONE, start it again and stop it with the kernel's Stop key, show the register,
and power off when asked. The guest has no disk and no network device. The register screen is read cell by
cell with the kernel's own font and must show an intact chain and the two demo tasks ending DONE and STOPPED.
After two Escape presses the launcher must still respond and reopen the same register. The Surfbird screen
is kept as evidence. Every other assertion is on the
serial log, QEMU's own events and its exit status.

This is one emulator on one host. It is not a test of a physical machine, of UEFI firmware or of any other
hypervisor.
"""
import argparse
import ast
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import time


REPO = Path(__file__).resolve().parents[2]
STARTUP_CHECKS = 71  # the number of self-tests kernel 0.3.1 reports before the launcher; change it with the kernel


def kernel_glyphs():
    """The kernel's 10 by 20 font, from its source, as the set of inked pixels of each character."""
    source = (REPO / "PCP_OS_Rust" / "crates" / "pcpos-kernel" / "src" / "font.rs").read_text()
    table = ast.literal_eval("[" + source.split(" = [", 1)[1].split("];", 1)[0] + "]")
    return {tuple(alpha > 0 for alpha in glyph): chr(32 + i) for i, glyph in enumerate(table)}


def read_screen(ppm, glyphs):
    """Every text row of a kernel screen of any size, read cell by cell against the kernel's own font, on
    the 10 by 20 grid that starts at the top-left corner. A cell that is no character of the font reads as '?'."""
    try:
        magic, size, depth, pixels = ppm.split(b"\n", 3)
        width, height = (int(n) for n in size.split())
    except ValueError:
        return None
    if magic != b"P6" or depth != b"255" or len(pixels) != width * height * 3 or width < 640 or height < 480:
        return None
    rows = []
    for row in range(height // 20):
        text = ""
        for column in range(width // 10):
            cell = [pixels[at:at + 3] for y in range(20) for x in range(10)
                    for at in (((row * 20 + y) * width + column * 10 + x) * 3,)]
            # The ground is taken cell by cell (its commonest colour), so that text on a bar or a panel of
            # another colour than the screen's is read too. Where the ground under a cell is shaded and
            # not flat, the cell matches no character and reads as '?'.
            ground = collections.Counter(cell).most_common(1)[0][0]
            text += glyphs.get(tuple(pixel != ground for pixel in cell), "?")
        rows.append(text.rstrip())
    return rows


def register_problems(rows):
    """What the register screen must show after one played and one stopped demo."""
    if rows is None:
        return ["the register screen was not a whole frame"]
    problems = []
    if any("?" in row for row in rows):
        problems.append("the register screen holds a cell that is no character of the kernel's font")
    head = next((row for row in rows if "THE KERNEL'S REGISTER:" in row), "")
    if "chain intact" not in head:
        problems.append("the register heading does not say the chain is intact")
    started = [re.search(r"id (T\d+)", row).group(1) for row in rows if "TASK_STARTED" in row and "label surf.elf" in row and re.search(r"id (T\d+)", row)]
    ended = {m.group(1): m.group(2) for m in (re.search(r"TASK_ENDED\s+id (T\d+); outcome (\w+)", row) for row in rows) if m}
    if len(started) != 2:
        problems.append("expected two starts of surf.elf on the register screen, read %d" % len(started))
    elif [ended.get(task) for task in started] != ["DONE", "STOPPED"]:
        problems.append("expected the two surf.elf tasks to end DONE then STOPPED, read %s" % [ended.get(task) for task in started])
    return problems


def check_desk_label(ppm, label):
    # The selected queue row uses the guest's lowercase bitmap font over green.
    header = b"P6\n800 480\n255\n"
    if not ppm.startswith(header) or len(ppm) != len(header) + 800 * 480 * 3:
        raise ValueError("unexpected queue desk display mode")
    source = (REPO / "store/apps/parrots-all-the-way-down/guest/src/screen.rs").read_text()
    table = ast.literal_eval(source.split("const LOWER:", 1)[1].split("= ", 1)[1].split("];", 1)[0] + "]")
    pixels = memoryview(ppm)[len(header):]
    for index, ch in enumerate(label):
        for y, bits in enumerate(table[ord(ch) - ord("a")]):
            for x in range(6):
                at = 3 * ((160 + y) * 800 + 73 + index * 6 + x)
                ink = x < 5 and bits & (16 >> x)
                if pixels[at:at + 3] != (b"\xff\xfc\xf0" if ink else b"\x21\x66\x47"):
                    raise ValueError("the selected queue row did not display " + label)


def exercise_queue(key, wait, screen, out):
    key("down")
    time.sleep(.2)
    key("ret")
    wait("PARROTS QUEUE total=0 active=0 selected=0 state=NONE store=New queue")
    label = "notes"
    for code in ("n", *label, "ret"):
        key(code)
        time.sleep(.2)
    wait("PARROTS QUEUE total=1 active=1 selected=1 state=ACTIVE store=Session save")
    screen("queue-saved")
    check_desk_label((out / "queue-saved.ppm").read_bytes(), label)
    key("esc")
    wait("rdesk.elf ended DONE: exit 0")
    time.sleep(.5)
    key("ret")
    wait("PARROTS QUEUE total=1 active=1 selected=1 state=ACTIVE store=Queue restored")
    screen("queue-restored")
    check_desk_label((out / "queue-restored.ppm").read_bytes(), label)
    key("f")
    wait("PARROTS QUEUE total=1 active=0 selected=1 state=FINISHED store=Session save")
    key("esc")
    wait("rdesk.elf ended DONE: exit 0", 2)
    time.sleep(.5)
    key("ret")
    wait("PARROTS QUEUE total=1 active=0 selected=1 state=FINISHED store=Queue restored")
    screen("queue-finished-restored")
    check_desk_label((out / "queue-finished-restored.ppm").read_bytes(), label)
    key("esc")
    wait("rdesk.elf ended DONE: exit 0", 3)


def session(qemu, iso, out, machine, counted, measurement, timeout, glyphs, queue_desk):
    out.mkdir()
    serial, sock = out / "serial.txt", out / "qmp.sock"
    cmd = [qemu, "-machine", machine, "-accel", "tcg", "-cpu", "qemu64", "-m", "64M", "-boot", "order=d", "-cdrom", str(iso),
           "-vga", "std", "-nic", "none", "-audiodev", "none,id=snd0", "-device", "AC97,audiodev=snd0", "-display", "none",
           "-monitor", "none", "-qmp", "unix:%s,server=on,wait=off" % sock, "-serial", "file:%s" % serial]
    if counted:
        cmd += ["-icount", "shift=3,align=off,sleep=off"]
    result = {"machine": machine, "clock": "instruction-counted" if counted else "ordinary", "argv": cmd}
    with (out / "qemu-stderr.txt").open("wb") as err:
        guest = subprocess.Popen(cmd, stderr=err)
    try:
        end = time.monotonic() + 20
        while not sock.exists():
            if time.monotonic() > end or guest.poll() is not None:
                raise RuntimeError("QEMU did not offer its monitor")
            time.sleep(0.1)
        channel = socket.socket(socket.AF_UNIX)
        channel.settimeout(20)
        channel.connect(str(sock))
        stream = channel.makefile("rw")
        events = []

        def rpc(command, **arguments):
            stream.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if "event" in reply:
                    events.append(reply)
                elif "error" in reply:
                    raise RuntimeError("%s: %s" % (command, reply["error"]))
                elif "return" in reply:
                    return reply["return"]

        def log():
            return serial.read_text(errors="replace") if serial.exists() else ""

        def wait(text, count=1):
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                seen = log()
                if "it is in SAFE" in seen:
                    raise RuntimeError("the guest entered SAFE; see serial.txt for the failed self-tests")
                if seen.count(text) >= count:
                    return
                if guest.poll() is not None:
                    raise RuntimeError("QEMU exited with status %s while waiting for: %s" % (guest.returncode, text))
                time.sleep(0.25)
            raise TimeoutError("waited %s s for: %s" % (timeout, text))

        def key(*names):
            rpc("send-key", keys=[{"type": "qcode", "data": n} for n in names], **{"hold-time": 80})

        def screen(name):
            time.sleep(1.5)
            rpc("screendump", filename=str(out / (name + ".ppm")))

        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("no QMP greeting")
        rpc("qmp_capabilities")
        media = [d for d in rpc("query-block") if "inserted" in d]
        result["inserted_media"] = len(media)
        if len(media) != 1 or not media[0]["inserted"]["ro"]:
            raise RuntimeError("expected exactly one inserted medium, read-only")

        wait(measurement + " (measured; equal to the value the loader named)")
        wait("leaves no memory behind")
        seen = log()
        result["startup_pass_lines"] = sum(line.startswith("  pass ") for line in seen.splitlines())
        result["startup_fail_lines"] = sum(line.lower().startswith("  fail") for line in seen.splitlines())
        def wait_screen(name, matches):
            path = out / (name + ".ppm")
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                time.sleep(0.25)
                if guest.poll() is not None:
                    raise RuntimeError("guest powered off before " + name)
                rpc("screendump", filename=str(path))
                rows = read_screen(path.read_bytes(), glyphs)
                if rows and matches(rows):
                    return rows
            raise TimeoutError(name + " did not appear")

        def launcher_visible(rows):
            status = f"Verified at start: all {STARTUP_CHECKS} self-tests passed."
            return any(status in row for row in rows)

        def corner_verified(rows):
            """The small word in the corner of every desktop surface when the kernel verified itself."""
            return any("verified" in row for row in rows) and not any("NOT VERIFIED" in row for row in rows)

        def home_visible(rows):
            """The desktop: its dock, whose first place is all the programs, and the corner word."""
            return corner_verified(rows) and any("Programs" in row for row in rows) and any("keys" in row for row in rows) and not any("may use:" in row for row in rows)

        def launcher_list_visible(rows):
            """The launcher: a chosen program with what it may use, and the corner word."""
            return corner_verified(rows) and any("> surf.elf" in row for row in rows) and any("may use:" in row for row in rows)

        def home_activity(rows):
            """The outcomes on the desktop's activity widget, oldest first."""
            return [word for row in rows if "surf.elf" in row for word in ("DONE", "STOPPED") if " " + word + " " in row]

        def placed(rows):
            """Whether anything the person can place is on the desktop: a shortcut or a widget."""
            text = "\n".join(rows)
            return any(word in text for word in ("surf.elf", "RECENT ACTIVITY", "THIS MACHINE", "SESSION"))

        # The first screen is the desktop, and it is empty: nothing is on it that the person did not put
        # there. Enter opens the launcher. Each screen must be recognised as itself and not as the other.
        home = wait_screen("home", home_visible)
        if launcher_list_visible(home) or placed(home):
            raise RuntimeError("the first desktop was not empty")
        # The programs are on a bar across the foot of the desktop by default, with the word on it; B
        # changes to the dock in the middle, where the word is in the corner on a row of its own, and back.
        def one_row(view):
            return any("Programs" in row and "verified" in row for row in view)
        if not one_row(home):
            raise RuntimeError("the first desktop did not have the bar across its foot")
        key("b")
        wait_screen("home-dock", lambda view: home_visible(view) and not one_row(view))
        key("b")
        wait_screen("home-bar", lambda view: home_visible(view) and one_row(view))
        # The corner says one word; V says the whole of it, in the sentence every kernel screen has used.
        key("v")
        wait_screen("verification", launcher_visible)
        key("esc")
        wait_screen("home-again", home_visible)
        # The person turns on the activity widget, then opens the launcher and places a shortcut.
        key("w")
        wait_screen("widgets", lambda view: any("WIDGETS ON THE DESKTOP" in row for row in view))
        key("ret")
        key("esc")
        wait_screen("home-with-widget", lambda view: home_visible(view) and any("RECENT ACTIVITY" in row for row in view) and not home_activity(view))
        key("ret")
        listed = wait_screen("launcher", launcher_list_visible)
        if home_visible(listed):
            raise RuntimeError("the launcher was also recognised as the desktop")
        key("s")
        wait_screen("launcher-with-shortcut", lambda view: launcher_list_visible(view) and any("in the dock" in row for row in view))
        key("ret")
        wait("SURFBIRD media ready")
        screen("surfbird")
        key("spc")
        wait("surf.elf ended DONE: exit 0")
        key("ret")
        wait("SURFBIRD media ready", 2)
        key("ctrl", "alt", "delete")
        wait("surf.elf ended STOPPED")
        key("r")
        rows = wait_screen("register", lambda view: not register_problems(view))
        (out / "register.txt").write_text("\n".join(rows) + "\n")
        result["register_problems"] = register_problems(rows)
        key("esc")
        wait_screen("back-at-launcher", launcher_list_visible)
        # Esc leaves the launcher for the desktop and does not power off; the desktop now shows, from the
        # register, the program that finished and the one that was stopped.
        key("esc")
        after = wait_screen("home-after-run", lambda view: home_visible(view) and not launcher_list_visible(view))
        result["home_activity"] = home_activity(after)
        key("r")
        wait_screen("reopened-register", lambda view: view == rows)
        key("esc")
        wait_screen("home-before-poweroff", lambda view: home_visible(view) and any("surf.elf" in row for row in view))
        if queue_desk:
            key("spc")
            wait_screen("launcher-before-queue", launcher_list_visible)
            exercise_queue(key, wait, screen, out)
            wait_screen("launcher-after-queue", corner_verified)
        key("p")
        wait("ALL SELF-TESTS PASS.")
        try:
            result["exit_code"] = guest.wait(timeout=40)
        except subprocess.TimeoutExpired:
            raise RuntimeError("the guest did not power off after P")
        while not any(e.get("event") == "SHUTDOWN" for e in events):
            line = stream.readline()
            if not line:
                break
            reply = json.loads(line)
            if "event" in reply:
                events.append(reply)
        result["shutdown"] = next((e["data"] for e in events if e.get("event") == "SHUTDOWN"), None)
        final = log()
        result["final_pass_lines"] = sum(line.startswith("  pass ") for line in final.splitlines())
        # The kernel says which optional parts it was built with on its last line, at power off.
        result["guest_ethernet_compiled_in"] = "Guest Ethernet support is compiled in" in final
        problems = []
        if result["startup_fail_lines"]:
            problems.append("a startup self-test failed")
        if result["startup_pass_lines"] != STARTUP_CHECKS:
            problems.append("expected %d startup self-tests to pass, counted %d" % (STARTUP_CHECKS, result["startup_pass_lines"]))
        if result["final_pass_lines"] != STARTUP_CHECKS + 1:
            problems.append("expected one more passing check at power off (the kernel's register of itself), counted %d in all" % result["final_pass_lines"])
        problems += result["register_problems"]
        if result.get("home_activity") != ["DONE", "STOPPED"]:
            problems.append("the desktop did not show the two runs ending DONE then STOPPED: %s" % result.get("home_activity"))
        if result["guest_ethernet_compiled_in"]:
            problems.append("guest Ethernet is compiled into a CD that is meant to have none")
        if result["exit_code"] != 0 or not result["shutdown"] or result["shutdown"].get("guest") is not True:
            problems.append("the guest did not request its own shutdown with QEMU exit 0")
        result["problems"] = problems
    except (OSError, RuntimeError, TimeoutError, ValueError) as error:
        result["problems"] = [str(error)]
    finally:
        if guest.poll() is None:
            guest.kill()
            guest.wait()
        if sock.exists():
            sock.unlink()
    result["passed"] = not result["problems"]
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print("%-4s %-19s %s%s" % (machine, result["clock"], "passed" if result["passed"] else "FAILED: ", "; ".join(result["problems"])), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("built", type=Path, help="directory holding pcp-os-tester.iso and receipt.json")
    parser.add_argument("evidence", type=Path, help="new directory for serial logs, screens and results")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--timeout", type=float, default=90, help="seconds to wait for each expected line")
    parser.add_argument("--queue-desk", action="store_true", help="require the second app and its saved queue")
    args = parser.parse_args()
    receipt = json.loads((args.built / "receipt.json").read_text())
    queue_desk = "rdesk.elf" in receipt.get("on_disc", receipt.get("files", {}))
    if args.queue_desk and not queue_desk:
        sys.exit("the receipt has no queue desk")
    iso = (args.built / "pcp-os-tester.iso").resolve(strict=True)
    if hashlib.sha256(iso.read_bytes()).hexdigest() != receipt["iso_sha256"]:
        sys.exit("the ISO is not the one its receipt describes")
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    glyphs = kernel_glyphs()
    version = subprocess.check_output([args.qemu, "--version"], text=True).splitlines()[0]
    results = [session(args.qemu, iso, evidence / ("%s-%s" % (machine, "counted" if counted else "ordinary")), machine, counted,
                       receipt["kernel_measurement"], args.timeout, glyphs, queue_desk)
               for machine in ("pc", "q35") for counted in (False, True)]
    summary = {"iso_sha256": receipt["iso_sha256"], "source_commit": receipt["source_commit"], "kernel_measurement": receipt["kernel_measurement"],
               "qemu": version, "host": " ".join(os.uname()[i] for i in (0, 2, 4)), "passed": all(r["passed"] for r in results),
               "sessions": [{k: r.get(k) for k in ("machine", "clock", "passed", "startup_pass_lines", "final_pass_lines", "register_problems", "exit_code", "problems")} for r in results]}
    (evidence / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
