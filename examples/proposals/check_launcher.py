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
DEFAULT_FOLDER = "taylor"
APPROVAL = "Creating a file needs your approval."
CUT_NOTICE = "Loader line cut. Cannot run."


def check_cut_access(words):
    if CUT_NOTICE not in words:
        raise AssertionError("Missing cut-line refusal: " + words)
    for stale in ("Access:", "may use:", "starting it lets it use:", "Can list and read",
                  "Creating a file needs", "You approve each new file", "Ethernet grant:", "Optional game cable:",
                  "Enter open", "Enter start"):
        if stale in words:
            raise AssertionError("Unavailable action or grant on a cut line: " + words)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("guest", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--size", default="800x600")
    parser.add_argument("--cut-line", action="store_true", help="show the pre-launch refusal on an incomplete command line")
    parser.add_argument("--without-grant", action="store_true")
    # The folder the launcher must name is the one in effect, however it came to be: the usual one, the
    # loader's build_folder= option, or a name typed on the kernel's Projects folder screen (F7) in
    # this same boot. With another folder in effect the usual folder's line must not be on the screen.
    parser.add_argument("--boot-folder")
    parser.add_argument("--typed-folder")
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
    if args.boot_folder:
        command += " build_folder=" + args.boot_folder
    if args.cut_line:
        command += " padding=" + "x" * 1200
    effective = args.typed_folder or args.boot_folder or DEFAULT_FOLDER
    SCOPE = "Can list and read %s/" % effective
    STALE = None if effective == DEFAULT_FOLDER else "Can list and read %s/" % DEFAULT_FOLDER
    serial = output / "serial.txt"
    invocation = [args.qemu, "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "256M",
                  # The loader's line begins with the kernel's path as given here, and a line over 255
                  # bytes is not taken whole (then nothing is started): so the kernel is named by its
                  # file name alone, from its own folder.
                  "-kernel", kernel.name, "-initrd", str(guest), "-append", command,
                  "-vga", "std", "-display", "none", "-nic", "none", "-serial", "file:" + str(serial),
                  "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"]
    with subprocess.Popen(invocation, stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=kernel.parent) as vm:
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
            if args.typed_folder:
                key("f7")
                capture("folder-before", lambda text: "PROJECTS FOLDER" in text)
                for letter in args.typed_folder:
                    key({"/": "slash", ".": "dot", "-": "minus"}.get(letter, letter))
                key("ret")
                set_text = capture("folder-set", lambda text: "Set." in text or "Not changed" in text)
                if "Set." not in set_text:
                    raise AssertionError("The Projects folder screen did not take the name: " + set_text)
                key("esc")
                capture("home-again", lambda text: "Programs" in text)
            key("ret")
            launcher = capture("programs", lambda text: guest.name in text and
                               ("Access:" in text or "may use:" in text or CUT_NOTICE in text))
            if args.cut_line:
                check_cut_access(launcher)
            creation = "You approve each new file." if width // 10 - 4 < len(APPROVAL) else APPROVAL
            required = not args.without_grant and not args.cut_line
            if (SCOPE in launcher) != required or (creation in launcher) != required:
                raise AssertionError("Programs has incorrect folder or approval disclosure: " + launcher)
            if STALE and STALE in launcher:
                raise AssertionError("Programs names the usual folder while another is in effect: " + launcher)
            if width < 960 and not args.cut_line and "Enter open" not in launcher:
                raise AssertionError("Programs lost its launch instruction: " + launcher)
            if args.cut_line:
                key("ctrl", "p")
                key("esc")
                capture("pin-home", lambda s: "Programs" in s)
                key("down" if width < 1280 else "right")
                pin = capture("pin-chosen", lambda s: "Not started: loader line cut" in s)
                if "Enter starts" in pin:
                    raise AssertionError("Pinned program still offers to start: " + pin)
                key("ret")
                capture("refused", lambda s: "was not started" in s or "Not started: loader line cut" in s)
                log = serial.read_text()
                if guest.name + " was not started:" not in log or "starting " + guest.name + " (" in log:
                    raise AssertionError("Expected a refusal before starting the guest")
                key("up" if width < 1280 else "left")
                key("ret")
                capture("programs-again", lambda s: CUT_NOTICE in s)
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
            if args.cut_line:
                check_cut_access(details)
                if "the loader's line was not taken whole" not in serial.read_text():
                    raise AssertionError("Missing native cut-line observation")
            if (SCOPE in details) != required or (APPROVAL in details) != required:
                raise AssertionError("Details has incorrect folder or approval disclosure: " + details)
            if STALE and STALE in details:
                raise AssertionError("Details names the usual folder while another is in effect: " + details)
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
