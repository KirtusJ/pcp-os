#!/usr/bin/env python3
"""Measure QEMU scanout before, during, and after Tailorbird in a supplied PCP OS image."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import subprocess
import sys
import time
import uuid


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def dimensions(value):
    match = re.fullmatch(r"([1-9][0-9]*)x([1-9][0-9]*)", value)
    if not match:
        raise argparse.ArgumentTypeError("use WIDTHxHEIGHT")
    width, height = map(int, match.groups())
    if width > 8192 or height > 8192:
        raise argparse.ArgumentTypeError("maximum supported dimension is 8192")
    return width, height


class Monitor:
    def __init__(self, guest, timeout):
        self.guest = guest
        self.timeout = timeout
        self.buffer = bytearray()
        self.selector = selectors.DefaultSelector()
        self.selector.register(guest.stdout, selectors.EVENT_READ)

    def receive(self, deadline):
        while b"\n" not in self.buffer:
            if not self.selector.select(max(0, deadline - time.monotonic())):
                raise TimeoutError("QMP reply timed out")
            data = os.read(self.guest.stdout.fileno(), 65536)
            if not data:
                raise RuntimeError(f"QMP disconnected, QEMU status {self.guest.poll()}")
            self.buffer.extend(data)
        row, _, rest = self.buffer.partition(b"\n")
        self.buffer = rest
        return json.loads(row)

    def execute(self, action, **args):
        deadline = time.monotonic() + self.timeout
        self.guest.stdin.write(json.dumps({"execute": action, "arguments": args}).encode() + b"\n")
        self.guest.stdin.flush()
        while time.monotonic() < deadline:
            reply = self.receive(deadline)
            if "return" in reply:
                return reply["return"]
            if "error" in reply:
                raise RuntimeError(f"{action}: {reply['error']}")
        raise TimeoutError(action)

    def key(self, key):
        self.execute("send-key", keys=[{"type": "qcode", "data": key}], **{"hold-time": 70})


def wait_serial(guest, serial, marker, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        raw = serial.read_bytes() if serial.exists() else b""
        if marker in raw:
            return
        # The app can exit while the launcher and QEMU stay alive.
        if marker.startswith((b"TAILORBIRD", b"Tailorbird ended", b"QWEN")) and b"Tailorbird ended FAILED:" in raw:
            raise RuntimeError("Tailorbird exited unsuccessfully; see serial.log")
        if guest.poll() is not None:
            raise RuntimeError(f"QEMU exited {guest.returncode} before {marker!r}")
        time.sleep(.1)
    raise TimeoutError(f"serial marker {marker!r} missing after {timeout} seconds")


class TailorbirdLaunch:
    """Read complete guest records after the launch key, within one app lifetime."""

    def __init__(self, serial, expected_digest):
        self.serial = serial
        with serial.open("rb") as stream:
            self.offset = stream.seek(0, os.SEEK_END)
            if self.offset:
                stream.seek(-1, os.SEEK_END)
            self.skip_partial = self.offset > 0 and stream.read(1) != b"\n"
        self.expected_digest = expected_digest
        self.digest = None
        self.connected = False
        self.ready = False
        self.ended = False
        self.exit_from = None

    def read(self):
        with self.serial.open("rb") as stream:
            stream.seek(self.offset)
            for raw in stream:
                if not raw.endswith(b"\n"):
                    break
                start = self.offset
                self.offset += len(raw)
                if self.skip_partial:
                    self.skip_partial = False
                    continue
                line = raw[:-1].removesuffix(b"\r")
                launch = re.fullmatch(rb"starting Tailorbird \([0-9]+ bytes, SHA-256 ([0-9a-f]{64})\)", line)
                if launch:
                    if self.digest is not None:
                        raise RuntimeError("another Tailorbird launch appeared during the display check")
                    self.digest = launch[1].decode("ascii")
                    if self.expected_digest is not None and self.digest != self.expected_digest:
                        raise RuntimeError(f"launched Tailorbird SHA-256 differs: {self.digest}")
                elif self.digest is not None and not self.ended:
                    if line == b"QWEN READY":
                        self.ready = True
                    elif re.fullmatch(rb"TAILORBIRD MCP CONNECTED(?: [0-9]{4}-[0-9]{2}-[0-9]{2})?", line):
                        self.connected = True
                    elif line.startswith(b"Tailorbird ended "):
                        if not re.fullmatch(rb"Tailorbird ended DONE: exit 0(?: after [^\r\n]+)?", line):
                            raise RuntimeError("Tailorbird did not report a clean exit; see serial.log")
                        if self.exit_from is None or start < self.exit_from:
                            raise RuntimeError("Tailorbird ended before the return key")
                        self.ended = True

    def wait(self, guest, timeout, *, require_ready=False, until_exit=False):
        deadline = time.monotonic() + timeout
        while True:
            self.read()
            if until_exit and self.ended:
                return
            if not until_exit and self.connected and (self.ready or not require_ready):
                return
            if guest.poll() is not None:
                raise RuntimeError(f"QEMU exited {guest.returncode} while waiting for Tailorbird")
            if time.monotonic() >= deadline:
                raise TimeoutError("Tailorbird launch, connection, requested readiness or exit missing; see serial.log")
            time.sleep(.1)


TITLE_SHA256 = "a35240f2eea7b8ccc7536d9d8e8402659126f5dc8862590605fbb60544fb0322"


def crop(pixels, width, left, top, right, bottom):
    return b"".join(pixels[3 * (width * y + left):3 * (width * y + right)]
                    for y in range(top, bottom))


def read_frame(path):
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


def title_record(size, pixels):
    """Compare the specified 1080p title rectangle against its public RGB digest."""
    record = {"reference_sha256": TITLE_SHA256, "observed_sha256": None, "matches": False}
    if size == (1920, 1080):
        native_title = crop(pixels, size[0], 44, 32, 404, 92)
        record["observed_sha256"] = hashlib.sha256(native_title).hexdigest()
        record["matches"] = record["observed_sha256"] == TITLE_SHA256
    return record


def capture(monitor, out, name, check_title=False):
    time.sleep(1)
    path = out / f"{name}.ppm"
    monitor.execute("screendump", filename=str(path))
    size, raster = read_frame(path)
    width, height = size
    if raster == raster[:3] * (width * height):
        raise ValueError(f"{name}: scanout is a single uniform colour")
    result = {"width": width, "height": height, "ppm_sha256": digest(path)}
    if check_title:
        result["title_1080p"] = title_record(size, raster)
    return result


def run(args, report):
    out = args.output.resolve()
    serial = out / "serial.log"
    variables = out / "variables.fd"
    shutil.copyfile(args.vars, variables)
    width, height = args.mode
    endpoint = {"edid": True, "xres": width, "yres": height, "vgamem_mb": args.vram}
    # QEMU's drive syntax requires a doubled comma inside a filename.
    def drive_path(path):
        return str(path.resolve()).replace(",", ",,")
    command = [args.qemu, "-uuid", report["uuid"], "-machine", "q35", "-accel", "tcg,tb-size=8",
               "-cpu", "max", "-m", str(args.ram), "-nic", "none", "-no-reboot",
               "-display", "none", "-monitor", "none", "-qmp", "stdio",
               "-serial", f"file:{serial}", "-vga", "none", "-device",
               f"VGA,id=gfx,edid=on,xres={width},yres={height},vgamem_mb={args.vram}",
               "-drive", f"if=pflash,format=raw,readonly=on,file={drive_path(args.code)}",
               "-drive", f"if=pflash,format=raw,file={drive_path(variables)}",
               "-drive", f"if=none,id=os,format=raw,snapshot=on,file={drive_path(args.image)}",
               "-device", "virtio-blk-pci,drive=os,disable-modern=on,bootindex=1"]
    if args.firmware_dir:
        command.extend(["-L", str(args.firmware_dir.resolve())])
    with (out / "qemu.stderr").open("wb") as err:
        guest = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
        monitor = Monitor(guest, args.timeout)
        try:
            if "QMP" not in monitor.receive(time.monotonic() + args.timeout):
                raise RuntimeError("missing QMP greeting")
            monitor.execute("qmp_capabilities")
            observed = {k: monitor.execute("qom-get", path="/machine/peripheral/gfx", property=k)
                        for k in endpoint}
            report["virtual_display"] = observed
            if observed != endpoint:
                raise ValueError(f"virtual display properties differ: {observed}")
            wait_serial(guest, serial, b"Offline Qwen acquisition test", args.timeout)
            monitor.key("home")
            time.sleep(.2)
            monitor.key("ret")
            wait_serial(guest, serial, b"PCP OS launcher ready", args.timeout)
            screen = report["scanout"]
            screen["launcher"] = capture(monitor, out, "launcher")
            launch = TailorbirdLaunch(serial, args.app_sha256)
            monitor.key("ret")
            launch.wait(guest, args.timeout, require_ready=args.require_qwen_ready)
            report["app_sha256"] = launch.digest
            screen["tailorbird"] = capture(monitor, out, "tailorbird", args.check_title_1080p)
            launch.exit_from = serial.stat().st_size
            monitor.key("esc")
            launch.wait(guest, args.timeout, until_exit=True)
            screen["returned"] = capture(monitor, out, "returned")
            launch.read()
            app = screen["tailorbird"]["ppm_sha256"]
            if app in {screen["launcher"]["ppm_sha256"], screen["returned"]["ppm_sha256"]}:
                raise RuntimeError("application and launcher produced identical scanouts")
            raw = serial.read_bytes()
            report["qwen_reported_ready"] = launch.ready
            report["guest_firmware_readback"] = [m.decode() for m in
                re.findall(rb"EFI GOP readback [^\r\n]+", raw)]
            report["three_samples_match_requested_mode"] = all(
                (s["width"], s["height"]) == args.mode for s in screen.values())
        finally:
            monitor.selector.close()
            guest.terminate()
            try:
                guest.wait(8)
            except subprocess.TimeoutExpired:
                guest.kill()
                guest.wait()
            guest.stdin.close()
            guest.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("image", "code", "vars", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--mode", type=dimensions, required=True)
    parser.add_argument("--image-sha256", required=True)
    parser.add_argument("--app-sha256", help="expected SHA-256 in the selected Tailorbird launch record")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--firmware-dir", type=Path)
    parser.add_argument("--ram", type=int, default=1536, help="guest RAM in MiB")
    parser.add_argument("--vram", type=int, default=32, help="VGA RAM in MiB")
    parser.add_argument("--timeout", type=int, default=180, help="seconds per observed milestone")
    parser.add_argument("--require-qwen-ready", action="store_true")
    parser.add_argument("--check-title-1080p", action="store_true",
                        help="compare the app title with the precommitted 1920x1080 reference")
    args = parser.parse_args()
    if args.app_sha256 is not None and not re.fullmatch(r"[0-9a-f]{64}", args.app_sha256):
        parser.error("--app-sha256 requires 64 lowercase hexadecimal digits")
    if args.check_title_1080p and args.mode != (1920, 1080):
        parser.error("--check-title-1080p requires --mode 1920x1080")
    image_hash = digest(args.image)
    if image_hash != args.image_sha256:
        parser.error("input disk SHA-256 differs from the requested disk")
    if min(args.ram, args.vram, args.timeout) < 1:
        parser.error("RAM, VRAM and timeout must be positive")
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"uuid": str(uuid.uuid4()), "image_sha256": image_hash, "requested": args.mode,
              "ram_mib": args.ram, "vram_mib": args.vram, "scanout": {}}
    try:
        run(args, report)
        passed = report["three_samples_match_requested_mode"]
        if args.check_title_1080p:
            passed = passed and report["scanout"]["tailorbird"]["title_1080p"]["matches"]
        code = 0 if passed else 1
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        report["incomplete"] = str(error)
        code = 2
    result = json.dumps(report, indent=2) + "\n"
    (args.output / "result.json").write_text(result)
    print(result, end="")
    return code


if __name__ == "__main__":
    sys.exit(main())
