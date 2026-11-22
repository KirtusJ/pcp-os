#!/usr/bin/env python3
"""Read the scoped folder and creation rule in Programs and Details before starting a guest."""
import argparse
import hashlib
from pathlib import Path
import runpy
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OPTICAL = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))
READ = runpy.run_path(str(ROOT / "examples/tester-cd/check_compact_pins.py"))["read"]
SCOPE = "Can list and read taylor/"
APPROVAL = "Creating a file needs your approval."

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("guest", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--size", default="800x600")
    parser.add_argument("--without-grant", action="store_true")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    width, height = map(int, args.size.split("x"))
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    kernel, guest = args.kernel.resolve(), args.guest.resolve()
    measure = ROOT / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
    pin = subprocess.check_output([sys.executable, measure, kernel], text=True).strip()
    command = f"image={pin} desktop={args.size} ui_scale=1"
    if not args.without_grant:
        command += " propose_image=" + hashlib.sha256(guest.read_bytes()).hexdigest()
    serial = output / "serial.txt"
    invocation = [args.qemu, "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "256M",
                  "-kernel", str(kernel), "-initrd", str(guest), "-append", command,
                  "-vga", "std", "-display", "none", "-nic", "none", "-serial", "file:" + str(serial),
                  "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"]
    with subprocess.Popen(invocation, stdin=subprocess.PIPE, stdout=subprocess.PIPE) as vm:
        try:
            monitor = OPTICAL["Monitor"](vm)
            monitor.call("qmp_capabilities")
            screens = OPTICAL["DesktopScreens"](monitor, output, 20)
            screens.read = READ

            def key(*keys):
                monitor.key(*keys)
                time.sleep(0.15)

            def capture(tag, condition):
                rows = screens.wait(tag, lambda r: condition(" ".join(" ".join(r).split())))
                (output / (tag + ".txt")).write_text("\n".join(rows) + "\n")
                return " ".join(" ".join(rows).split())

            capture("home", lambda text: "Programs" in text)
            key("ret")
            launcher = capture("programs", lambda text: guest.name in text and
                               ("Access:" in text or "may use:" in text))
            creation = "You approve each new file." if width // 10 - 4 < len(APPROVAL) else APPROVAL
            required = not args.without_grant
            if (SCOPE in launcher) != required or (creation in launcher) != required:
                raise AssertionError("Programs has incorrect folder or approval disclosure: " + launcher)
            if width < 960 and "Enter open" not in launcher:
                raise AssertionError("Programs lost its launch instruction: " + launcher)
            key("ctrl", "d")
            details = capture("details-0", lambda s: "kernel" in s and "PROGRAMS" in s)
            if width < 960:
                # Advance by overlapping half-pages so a wrapped label reads across page boundaries.
                for index in range(1, 32):
                    if "FILES FROM THE LOADER" in details:
                        break
                    key("down")
                    page = capture(f"details-{index}", lambda s: "scroll" in s.lower() or "Esc back" in s)
                    details += " " + page
                if "FILES FROM THE LOADER" not in details:
                    raise AssertionError("Details never reached the end of the program records")
            if (SCOPE in details) != required or (APPROVAL in details) != required:
                raise AssertionError("Details has incorrect folder or approval disclosure: " + details)
            key("esc")
            key("f10")
            capture("power", lambda s: "Switch PCP OS off?" in s)
            key("ret")
            if vm.wait(timeout=20) != 33 or "ALL SELF-TESTS PASS." not in serial.read_text():
                raise AssertionError("Unexpected kernel shutdown")
        finally:
            if vm.poll() is None:
                vm.kill()
                vm.wait()

if __name__ == "__main__":
    main()
