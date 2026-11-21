"""Type a queue entry, shut down, cold boot the same disk, and verify the restored UI and bytes."""
import argparse
import ast
import json
from pathlib import Path
import re
import runpy
import shutil
import struct
import subprocess
import sys
import time

from check import MEASURE, digest, require
from create import create

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "store/apps/parrots-all-the-way-down/guest"
LABEL = "cold morning"
optical = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))
Monitor = optical["Monitor"]


def check_frame(frame, status):
    header = b"P6\n800 480\n255\n"
    require(frame.startswith(header) and len(frame) == len(header) + 800 * 480 * 3, "incomplete queue screen")
    pixels = memoryview(frame)[len(header):]
    # The app mixes its lowercase 5x7 glyphs with the shared uppercase pixel font.
    screen = (APP / "src/screen.rs").read_text()
    lower = ast.literal_eval("[" + screen.split("const LOWER:", 1)[1].split("= [", 1)[1].split("];", 1)[0] + "]")
    glyph = {chr(ord("a") + i): shape for i, shape in enumerate(lower)}
    shared = (ROOT / "store/native/guest/src/ink.rs").read_text()
    for char, rows in re.findall(r"b'(.)' => (\[[^\]]+\])", shared):
        glyph[char] = ast.literal_eval(rows)
    glyph[" "] = [0] * 7
    for x, y, text in ((618, 43, status), (73, 160, LABEL)):
        for i, char in enumerate(text):
            for dy, expected in enumerate(glyph[char]):
                bits = 0
                for dx in range(5):
                    at = 3 * ((y + dy) * 800 + x + i * 6 + dx)
                    bits = (bits << 1) | (pixels[at:at + 3] == b"\xff\xfc\xf0")
                require(bits == expected, f"queue screen mismatch in {text!r} at character {i}")


def check_journal(image):
    # After the first save in a fresh volume, slot 1 contains generation 2.
    start = 130 * 512
    header = image[start:start + 512]
    require(header[:8] == b"PCPSLOT1" and digest(header[:52]) == header[52:84], "bad slot header")
    generation, length = struct.unpack_from("<QI", header, 8)
    payload = image[start + 512:start + 512 + length]
    require(generation == 2 and digest(payload) == header[20:52], "bad queue checkpoint")
    name_len, size = struct.unpack_from("<HI", payload)
    require(payload[6:6 + name_len] == b"parrots.queue", "wrong stored filename")
    record = payload[6 + name_len:]
    require(len(record) == size == 2064, "wrong journal size")
    expected = b"PARROTS1" + bytes([1, 0, 0, 0, 1, 0]) + LABEL.encode()
    require(record[:-4] == expected.ljust(size - 4, b"\0"), "unexpected saved queue event")
    checksum = 2166136261
    for byte in record[:-4]:
        checksum = ((checksum ^ byte) * 16777619) & 0xffffffff
    require(checksum == int.from_bytes(record[-4:], "little"), "bad queue journal checksum")


def wait_for(vm, serial, text, timeout):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        if serial.exists() and text in serial.read_text(errors="replace"):
            return
        require(vm.poll() is None, f"guest exited before {text!r}")
        time.sleep(0.02)
    raise TimeoutError(text)


def main(args):
    out = args.output.resolve()
    out.mkdir()
    identity = create(out / "volume")
    disk = out / "volume/data.raw"
    shutil.copyfile(args.kernel, out / "kernel.elf")
    shutil.copyfile(args.queue, out / "queue.elf")
    measure = subprocess.check_output([sys.executable, MEASURE, out / "kernel.elf"], text=True).strip()
    qemu = [args.qemu]
    if args.bios_dir:
        qemu += ["-L", str(args.bios_dir.resolve())]
    saved_image = None
    receipts = []
    for boot, status in (("save", "Disk save"), ("restore", "Queue restored")):
        serial = out / (boot + ".serial")
        picture = out / (boot + ".ppm")
        # Keep the unattended save path; cold-reopen through the desktop used by testers.
        entry = "auto=queue.elf then=off " if boot == "save" else ""
        cmd = qemu + ["-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "256M", "-smp", "1",
                      "-nic", "none", "-display", "none", "-vga", "std", "-qmp", "stdio", "-monitor", "none",
                      "-no-reboot", "-kernel", "kernel.elf", "-initrd", "queue.elf", "-append",
                      f"{entry}store=ide0 store_id={identity} image={measure}",
                      "-serial", "file:" + serial.name, "-drive", "if=none,id=store,format=raw,file=volume/data.raw",
                      "-device", "ide-hd,drive=store,bus=ide.0,unit=0,serial=PCP-VM-STORE",
                      "-device", "isa-debug-exit,iobase=0xf4,iosize=4"]
        with (out / (boot + ".stderr")).open("wb") as err:
            vm = subprocess.Popen(cmd, cwd=out, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
            try:
                monitor = Monitor(vm)
                monitor.call("qmp_capabilities")
                if boot == "restore":
                    desktop = optical["DesktopScreens"](monitor, out, args.timeout)
                    desktop.home("restore-home")
                    desktop.verification("restore-home")
                    monitor.key("f")
                    desktop.wait("restore-files", lambda rows: any("FILES: 1 file(s), 3 KiB" in row for row in rows)
                                 and any("VM files | saved when an app syncs" in row for row in rows)
                                 and any("parrots.queue" in row and "3 KiB" in row for row in rows))
                    monitor.key("esc")
                    desktop.home("restore-from-files")
                    monitor.key("spc")
                    desktop.launcher("restore-launcher", "queue.elf")
                    monitor.key("ret")
                wait_for(vm, serial, "PARROTS READY", args.timeout)
                if boot == "save":
                    wait_for(vm, serial, "state=NONE store=New queue", args.timeout)
                    for key in ["n", *["spc" if c == " " else c for c in LABEL], "ret"]:
                        monitor.key(key)
                        time.sleep(0.14)  # let QEMU release each key before typing the next
                wait_for(vm, serial, "total=1 active=1 selected=1 state=ACTIVE store=" + status, args.timeout)
                monitor.call("screendump", filename=str(picture))
                check_frame(picture.read_bytes(), status)
                monitor.key("esc")
                if boot == "restore":
                    desktop.launcher("restore-after-app", "queue.elf")
                    monitor.key("p")
                require(vm.wait(timeout=args.timeout) == 33, boot + " failed to shut down")
            finally:
                if vm.poll() is None:
                    vm.kill()
                    vm.wait()
                vm.stdin.close()
                vm.stdout.close()
        log = serial.read_text(errors="replace")
        require(measure + " (measured; equal to the value the loader named)" in log, "wrong kernel")
        require("queue.elf ended DONE: exit 0" in log, "queue did not finish")
        image = disk.read_bytes()
        check_journal(image)
        if saved_image is None:
            saved_image = image
        else:
            require(image == saved_image, "reopening the queue changed the disk")
        (out / (boot + ".raw")).write_bytes(image)
        receipts.append(dict(boot=boot, status=status, disk_sha256=digest(image).hex(),
                             screenshot_sha256=digest(picture.read_bytes()).hex()))
        print(boot, status, LABEL, flush=True)
    (out / "results.json").write_text(json.dumps(dict(kernel_measurement=measure, label=LABEL, boots=receipts),
                                                indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("queue", type=Path)
    parser.add_argument("output", type=Path, help="new directory for the guest, disk and original observations")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--bios-dir", type=Path)
    parser.add_argument("--timeout", type=int, default=60)
    main(parser.parse_args())
