#!/usr/bin/env python3
"""Compare public Tailorbird builds at the same 1080p PCP OS desktop."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

from check_display import Monitor, digest, wait_serial


DESKTOP = (1920, 1080)
TITLE_SHA256 = "a35240f2eea7b8ccc7536d9d8e8402659126f5dc8862590605fbb60544fb0322"
TEAL = bytes.fromhex("41bad7")
PANEL = bytes.fromhex("18364f")
GROUND = bytes.fromhex("10283f")


def frame(monitor, path):
    monitor.execute("screendump", filename=str(path))
    with path.open("rb") as stream:
        if stream.readline().strip() != b"P6":
            raise ValueError("expected a native QEMU P6 image")
        width, height = map(int, stream.readline().split())
        if stream.readline().strip() != b"255":
            raise ValueError("expected RGB24 samples")
        pixels = stream.read()
    if min(width, height) <= 0 or len(pixels) != 3 * width * height:
        raise ValueError("incomplete native frame")
    return (width, height), pixels


def contains_paint(size, pixels, screen):
    width, height = size
    def at(x, y):
        start = (y * width + x) * 3
        return pixels[start:start + 3]

    if screen == "desktop":
        # #120 at ui_scale=1 draws the selected program at (20,100).
        return (size == DESKTOP and at(21, 110) == TEAL
                and at(25, 110) == PANEL and at(10, 210) == GROUND
                and bytes.fromhex("f2f7fc") in crop(pixels, width, 30, 100, 130, 120))
    # Wait for the app's top bar and the gap between bar and panels.
    # The two builds draw the same panels at different native scales.
    if size not in (DESKTOP, (960, 540)):
        return False
    scale = width // 960
    return (at(2 * scale, 5 * scale) == PANEL
            and at(200 * scale, 80 * scale) == GROUND
            and at(22 * scale, 100 * scale) == PANEL)


def sample(monitor, folder, name, screen):
    path = folder / (name + ".ppm")
    deadline = time.monotonic() + monitor.timeout
    while time.monotonic() < deadline:
        size, pixels = frame(monitor, path)
        if contains_paint(size, pixels, screen):
            return size, pixels, {"size": size, "ppm_sha256": digest(path)}
        time.sleep(0.1)
    raise TimeoutError(f"{name}: expected paint missing; last frame kept")


def crop(pixels, width, left, top, right, bottom):
    return b"".join(pixels[3 * (width * y + left):3 * (width * y + right)]
                    for y in range(top, bottom))


def exercise(args, report):
    out = args.output.resolve()
    serial = out / "serial.log"
    command = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg",
               "-icount", "shift=3", "-cpu", "max", "-m", "256M",
               "-display", "none", "-nic", "none", "-vga", "none",
               "-device", "VGA,vgamem_mb=32", "-serial", f"file:{serial}",
               "-monitor", "none", "-qmp", "stdio", "-no-reboot",
               "-device", "isa-debug-exit,iobase=0xf4,iosize=4",
               "-kernel", str(args.kernel.resolve()), "-initrd", str(args.app.resolve()),
               "-append", f"image={args.measurement} desktop=1920x1080 ui_scale=1"]
    with (out / "qemu.stderr").open("wb") as errors:
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=errors)
        monitor = Monitor(process, 120)
        try:
            if "QMP" not in monitor.receive(time.monotonic() + monitor.timeout):
                raise RuntimeError("missing QMP greeting")
            monitor.execute("qmp_capabilities")
            def wait(marker):
                wait_serial(process, serial, marker.encode(), monitor.timeout)

            wait("desktop: 1920x1080 at 1x UI scale")
            size, pixels, report["launcher"] = sample(monitor, out, "launcher", "desktop")
            initial_bar = crop(pixels, size[0], 0, 0, 520, 150)
            monitor.key("ret")
            wait(f"starting {args.app.name} ({args.app.stat().st_size} bytes, SHA-256 {report['app_sha256']})")
            if args.profile == "candidate":
                wait("Tailorbird first frame presented")
            size, pixels, report["app"] = sample(monitor, out, "app", "app")
            scale = size[0] // 960
            title = crop(pixels, size[0], 22 * scale, 16 * scale, 202 * scale, 46 * scale)
            if len(set(title)) <= 3:
                raise ValueError("title crop contains only panel background")
            report["app"]["title_rgb_sha256"] = hashlib.sha256(title).hexdigest()
            monitor.key("esc")
            wait(f"{args.app.name} ended DONE: exit 0")
            end_size, pixels, report["returned"] = sample(monitor, out, "returned", "desktop")
            report["launcher_header_and_selection_restored"] = (
                crop(pixels, end_size[0], 0, 0, 520, 150) == initial_bar)
            monitor.key("p")
            report["qemu_exit"] = process.wait(timeout=45)
            log = serial.read_text(errors="replace")
            if (report["qemu_exit"] != 33 or "ALL SELF-TESTS PASS" not in log
                    or "  FAIL" in log or args.measurement + " (measured; equal" not in log):
                raise RuntimeError("guest shutdown or measured kernel check failed")
            if not report["launcher_header_and_selection_restored"]:
                raise ValueError("restored launcher pixels differ")
            expected = DESKTOP if args.profile == "candidate" else (960, 540)
            if size != expected:
                raise ValueError(f"app size {size} differs from {expected}")
            if args.profile == "candidate" and report["app"]["title_rgb_sha256"] != TITLE_SHA256:
                raise ValueError("candidate title differs from the precommitted reference")
        finally:
            monitor.selector.close()
            if process.poll() is None:
                process.kill()
            process.wait()
            process.stdin.close()
            process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", required=True, type=Path)
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("--measurement", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--profile", required=True, choices=("original", "candidate"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"profile": args.profile, "kernel_sha256": digest(args.kernel),
              "app_sha256": digest(args.app), "measurement": args.measurement}
    try:
        exercise(args, report)
        report["passed"] = True
    except (ValueError, OSError, RuntimeError, TimeoutError, subprocess.TimeoutExpired) as exc:
        report.update(passed=False, error=str(exc))
    data = json.dumps(report, indent=2)
    (args.output / "result.json").write_text(data + "\n")
    print(data)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
