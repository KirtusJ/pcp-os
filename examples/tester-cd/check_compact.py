#!/usr/bin/env python3
"""Boot the single-app tester CD at 800×600 and distinguish F5 from compact Home."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import subprocess

from check import REPO, STARTUP_CHECKS, kernel_glyphs, optical_command, read_screen, verification_visible

monitor_tools = runpy.run_path(str(REPO / "examples/optical-disk-check/check.py"))
Monitor = monitor_tools["Monitor"]
wait_for = monitor_tools["wait_for"]


def check_compact(guest, out, receipt, result):
    monitor = Monitor(guest)
    monitor.call("qmp_capabilities")
    inserted = [row["inserted"] for row in monitor.call("query-block") if "inserted" in row]
    if len(inserted) != 1 or not inserted[0]["ro"]:
        raise RuntimeError("expected one read-only CD")
    serial = out / "serial.txt"
    wait_for(lambda: serial.exists() and "desktop: 800x600 at 1x UI scale" in serial.read_text(errors="replace"),
             "compact desktop did not start at 800x600", 90)
    log = serial.read_text(errors="replace")
    result["startup_checks"] = sum(line.startswith("  pass ") for line in log.splitlines())
    if (result["startup_checks"] != STARTUP_CHECKS or any(line.lower().startswith("  fail") for line in log.splitlines())
            or receipt["kernel_measurement"] + " (measured; equal to the value the loader named)" not in log):
        raise RuntimeError("compact boot failed its startup checks or kernel measurement")

    glyphs = kernel_glyphs()

    def capture(name, matches):
        path = out / (name + ".ppm")

        def ready():
            monitor.call("screendump", filename=str(path))
            frame = path.read_bytes()
            if not frame.startswith(b"P6\n800 600\n255\n"):
                return None
            rows = read_screen(frame, glyphs)
            if rows and matches(frame, rows):
                return frame, rows

        return wait_for(ready, name + " did not appear", 90)

    startup = f"  Verified at start: all {STARTUP_CHECKS} self-tests passed."
    home, rows = capture("home-before-f5", lambda frame, view: view[1][7:] == "PCP OS | DESKTOP"
                         and view[3] == startup and view[5] == "   Programs"
                         and view[-2] == "  Up/Down choose   Enter open   F1 keys   F10 off")
    if verification_visible(rows):
        raise RuntimeError("the F5 check accepted compact Home before F5 was pressed")
    monitor.key("f5")
    verification, rows = capture("verification", lambda frame, view: verification_visible(view))
    result["register"] = rows[6].strip()
    monitor.key("esc")
    restored, _ = capture("home-after-esc", lambda frame, view: frame == home)
    result["frame_sha256"] = {name: hashlib.sha256(frame).hexdigest()
                              for name, frame in [("home-before-f5", home), ("verification", verification),
                                                  ("home-after-esc", restored)]}
    monitor.key("f10")
    capture("power-off", lambda frame, view: view[1] == "  POWER OFF" and view[3] == "  Switch PCP OS off?"
            and view[-2] == "  Enter switch off    Esc go back")
    monitor.key("ret")
    result["exit_code"] = guest.wait(timeout=40)
    while not any(event.get("event") == "SHUTDOWN" for event in monitor.events):
        monitor.events.append(monitor.read())
    result["shutdown"] = next(event["data"] for event in monitor.events if event.get("event") == "SHUTDOWN")
    log = serial.read_text(errors="replace")
    result["final_checks"] = sum(line.startswith("  pass ") for line in log.splitlines())
    if (result["exit_code"] != 0 or result["shutdown"] != {"guest": True, "reason": "guest-shutdown"}
            or result["final_checks"] != STARTUP_CHECKS + 1 or "ALL SELF-TESTS PASS." not in log):
        raise RuntimeError("compact guest failed its final checks or shutdown")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("built", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    receipt = json.loads((args.built / "receipt.json").read_text())
    iso = (args.built / "pcp-os-tester.iso").resolve(strict=True)
    if hashlib.sha256(iso.read_bytes()).hexdigest() != receipt["iso_sha256"]:
        raise ValueError("the ISO is not the one its receipt describes")
    out = args.evidence.resolve()
    out.mkdir(parents=True, exist_ok=False)
    command = optical_command(args.qemu, iso, "pc", out / "serial.txt", "stdio")
    command += ["-global", "VGA.vgamem_mb=4"]
    result = {key: receipt[key] for key in ("source_commit", "source_tree", "iso_sha256", "kernel_measurement")}
    result["qemu"] = subprocess.check_output([args.qemu, "--version"], text=True).splitlines()[0]
    result["argv"] = command
    result["passed"] = False
    with (out / "qemu-stderr.txt").open("wb") as err:
        guest = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
        try:
            check_compact(guest, out, receipt, result)
            result["passed"] = True
        except (OSError, RuntimeError, TimeoutError, subprocess.TimeoutExpired, ValueError) as error:
            result["error"] = str(error)
        finally:
            if guest.poll() is None:
                guest.kill()
                guest.wait()
            (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
