"""Compare a refused public ELF with a separately measured Q35/OVMF kernel trial."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time

import details_screen

MODULES = ("surf.elf", "clip.ppm", "clip.wav")
EXPECTED_MODULES = ("b87f1185f86961ff8afdb21315c811f0c071c474d09737613146be46cbb98b34",
                    "f6a1d9a0702f472f5b02cf0a528586e6d4447acbad2bd92eb0fc6d70eea4df4d",
                    "70ca6a81bc6f94db7d14a61630ff02c517d20a65c31273b16ff02e233018e244")


def until(predicate, description, vm, seconds=60):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        if vm.poll() is not None:
            raise RuntimeError(f"Guest exited before {description}")
        time.sleep(.1)
    raise TimeoutError(description)


def echoed(serial, prefix, vm, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if serial.read_bytes().partition(b"AWAIT-PLACEMENT")[2].lstrip(b"\r\n").startswith(prefix):
            return True
        if vm.poll() is not None:
            raise RuntimeError(f"QEMU exited during GRUB serial echo for {prefix.decode('ascii')}")
        time.sleep(.1)
    return False


def confirm_decision(answer, serial_in, serial, vm, resent=None):
    # A burst on the emulated UART can leave GRUB waiting on a partial word.
    # Keep the writer open and wait for each serial echo before sending more.
    #
    # A single character can also be lost on its way in: GRUB then echoes nothing and waits. Such a
    # character is sent again, at most twice, after five seconds without its echo. This cannot turn
    # one decision into another. If the first copy was only slow and both arrive, GRUB reads a word
    # that is neither ALLOW nor DENY: it does not hand off, the echo no longer matches, and this
    # function fails. The word GRUB acted on is still confirmed, whole, on its own line below.
    with serial_in.open("wb", buffering=0) as stream:
        for length in range(1, len(answer) + 1):
            prefix = answer[:length]
            for attempt in range(3):
                stream.write(prefix[-1:])
                if echoed(serial, prefix, vm, 5 if attempt < 2 else 50):
                    break
                if resent is not None:
                    resent.append(prefix.decode("ascii"))
            else:
                raise TimeoutError(f"GRUB serial echo for {prefix.decode('ascii')}")
        stream.write(b"\r")
        decision = answer.decode("ascii")
        marker = f"\nPLACEMENT-DECISION-{decision}\n"
        until(lambda: marker in serial.read_text(errors="replace"),
              f"GRUB confirmation of {decision}", vm)
    return decision


def prepare(iso, kernel, measurement, root, work):
    grub = root / "boot/grub"
    grub.mkdir(parents=True)
    if kernel:
        shutil.copyfile(kernel, root / "kernel.elf32")
    for name in (*MODULES,) if kernel else ("kernel.elf32", *MODULES):
        subprocess.run(["xorriso", "-osirrox", "on", "-indev", iso, "-extract", "/" + name, root / name], check=True)
    if tuple(digest(root / n) for n in MODULES) != EXPECTED_MODULES:
        raise ValueError("Changed demo modules")
    if not kernel and digest(root / "kernel.elf32") != KERNEL_SHA:
        raise ValueError("Changed published kernel")
    options = " " + details_screen.OPTIONS if kernel else ""
    cfg = f"""serial --unit=0 --speed=115200
terminal_input serial
terminal_output serial
echo FIRMWARE-${{grub_platform}}
echo {BEGIN}
lsmmap
echo {END}
echo AWAIT-PLACEMENT
read decision
echo
echo PLACEMENT-DECISION-${{decision}}
if [ "x$decision" = "xALLOW" ]; then
    if multiboot /kernel.elf32 image={measurement}{options}; then
        module /surf.elf surf.elf
        module /clip.ppm clip.ppm
        module /clip.wav clip.wav
        echo ATTEMPT-HANDOFF
        boot
    fi
fi
halt
"""
    (grub / "grub.cfg").write_text(cfg)
    cd = work / "guarded.iso"
    subprocess.run(["grub-mkrescue", "-d", "/usr/lib/grub/x86_64-efi", "-o", cd, root], check=True)
    return cd


def trial(iso, kernel, measurement, out, ram):
    out.mkdir()
    with tempfile.TemporaryDirectory() as temp:
        work = Path(temp)
        root = work / "cd"
        cd = prepare(iso, kernel, measurement, root, work)
        code = Path("/usr/share/OVMF/OVMF_CODE_4M.fd")
        variables = work / "vars.fd"
        shutil.copyfile("/usr/share/OVMF/OVMF_VARS_4M.fd", variables)
        serial = out / "serial.txt"
        # GRUB reads the placement decision on the serial input. Keep the serial
        # output in full while QMP uses the subprocess's separate standard streams.
        console = work / "console"
        serial_in = console.with_suffix(".in")
        os.mkfifo(serial_in)
        serial.touch()
        console.with_suffix(".out").symlink_to(serial)
        cmd = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", str(ram),
               "-smp", "1", "-vga", "std", "-nic", "none", "-display", "none", "-monitor", "none",
               "-chardev", f"pipe,id=console,path={console}", "-serial", "chardev:console", "-qmp", "stdio", "-audiodev", "none,id=snd",
               "-device", "AC97,audiodev=snd", "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={code}",
               "-drive", f"if=pflash,unit=1,format=raw,file={variables}", "-boot", "order=d,strict=on",
               "-cdrom", str(cd), "-icount", "shift=3,align=off,sleep=off"]
        report = {"source_iso": digest(iso), "elf": digest(root / "kernel.elf32"), "measurement": measurement,
                  "ram_mib": ram, "firmware_sha256": digest(code), "cd_sha256": digest(cd), "command": cmd, "admitted": False}
        def log():
            return serial.read_text(errors="replace") if serial.exists() else ""
        with (out / "qemu.stderr").open("wb") as err, subprocess.Popen(
                cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as vm:
            try:
                monitor = details_screen.monitor_for(vm)
                call = monitor.call
                until(lambda: "AWAIT-PLACEMENT" in log(), "GRUB memory inventory", vm)
                observed = log()
                if "FIRMWARE-efi" not in observed:
                    raise RuntimeError("Missing UEFI loader marker")
                memory = memory_ranges(observed)
                segments = load_ranges((root / "kernel.elf32").read_bytes())
                # ELF loaders manage whole 4 KiB pages, including zero-filled segment tails.
                pages = [(lo // 4096 * 4096, (hi + 4095) // 4096 * 4096) for lo, hi in segments]
                blocked = conflicts(pages, memory)
                report.update(load_pages=pages, conflicts=blocked, map=memory)
                if not kernel:
                    if not any("ACPI non-volatile storage RAM" in b["types"] for b in blocked):
                        raise RuntimeError("Public kernel control no longer shows the expected NVS conflict")
                    report["placement_decision"] = confirm_decision(b"DENY", serial_in, serial, vm, report.setdefault("characters_sent_again", []))
                    vm.wait(timeout=20)
                    if vm.returncode or "ATTEMPT-HANDOFF" in log() or "PCP OS kernel" in log():
                        raise RuntimeError("Public kernel did not halt at the placement gate")
                    return
                if blocked:
                    raise RuntimeError("Experimental kernel intersects unavailable firmware memory")
                def nvs_hashes():
                    hashes = {}
                    capture = work / "nvs.tmp"
                    for lo, hi, kind in memory:
                        if kind == "ACPI non-volatile storage RAM":
                            if hi - lo > 32 << 20:
                                raise ValueError("Unexpected NVS size")
                            call("pmemsave", val=lo, size=hi-lo, filename=str(capture))
                            hashes[f"{lo:x}-{hi:x}"] = digest(capture)
                    if not hashes:
                        raise ValueError("Missing NVS comparison ranges")
                    return hashes
                report["nvs_before"] = nvs_hashes()
                report["placement_decision"] = confirm_decision(b"ALLOW", serial_in, serial, vm, report.setdefault("characters_sent_again", []))
                report["admitted"] = True
                until(lambda: "leaves no memory behind" in log() or "it is in SAFE" in log(), "kernel startup", vm)
                screen = out / "launcher.ppm"
                def launcher():
                    call("screendump", filename=str(screen))
                    words = subprocess.check_output(["tesseract", screen, "stdout", "--psm", "6"],
                                                    text=True, stderr=subprocess.DEVNULL, timeout=10)
                    (out / "launcher.txt").write_text(words)
                    return has_regions(screen.read_bytes(), (details_screen.FOOTER,)) and "files from the loader" in words.lower()
                until(launcher, "graphical launcher", vm)
                startup = log()
                report["display_options"] = details_screen.OPTIONS
                report["nvs_at_launcher"] = nvs_hashes()
                report["startup_passes"] = sum(s.startswith("  pass ") for s in startup.splitlines())
                report["failed"] = [s.strip() for s in startup.splitlines() if s.startswith("  FAIL ")]
                report["loaded_modules"] = {}
                for name, size, addr in re.findall(r"file from the loader: ([^ ]+) \((\d+) bytes at (0x[\da-f]+)\)", startup):
                    lo, hi = int(addr,16), int(addr,16) + int(size)
                    if conflicts([(lo,hi)], memory) or any(lo < end and first < hi for first,end in pages):
                        raise ValueError("Module outside usable memory or inside kernel footprint")
                    report["loaded_modules"][name] = [lo,hi]
                words = (out / "launcher.txt").read_text().lower()
                if (report["failed"] or report["startup_passes"] != 71 or "all 71 self-tests passed" not in words
                        or set(report["loaded_modules"]) != set(MODULES)
                        or report["nvs_before"] != report["nvs_at_launcher"]
                        or measurement + " (measured; equal" not in startup):
                    raise RuntimeError("Guarded handoff did not satisfy startup, placement and NVS checks")
                pool = re.search(r"memory for programs: (\d+) frames of 4 KiB, (0x[\da-f]+)\.\.(0x[\da-f]+)", startup)
                if not pool:
                    raise RuntimeError("Missing measured frame pool")
                first, end = int(pool[2], 16), int(pool[3], 16)
                occupied = pages + list(report["loaded_modules"].values())
                if (end - first != int(pool[1]) * 4096 or end <= first or conflicts([(first, end)], memory)
                        or any(first < hi and lo < end for lo, hi in occupied)):
                    raise RuntimeError("Invalid program frame pool placement")
                report["pool"] = [first, end]
                details_screen.finish_session(call, lambda test, label: until(test, label, vm),
                                              log, out, public_session)
                report["nvs_before_shutdown"] = nvs_hashes()
                if report["nvs_before_shutdown"] != report["nvs_before"]:
                    raise RuntimeError("Firmware NVS changed during the interactive session")
                report["shutdown_event"] = details_screen.power_off(monitor, vm)
                report["shutdown"] = True
                if "ALL SELF-TESTS PASS." not in log():
                    raise RuntimeError("Missing final kernel self-test result")
            finally:
                if vm.poll() is None:
                    vm.kill(); vm.wait()
                (out / "result.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("distribution", type=Path)
    p.add_argument("kernel_dir", type=Path)
    p.add_argument("out", type=Path)
    args = p.parse_args()
    sys.path.insert(0, str(args.distribution.resolve() / "audit/public-live-cd/uefi-probe"))
    sys.path.insert(0, str(args.distribution.resolve() / "audit/public-live-cd"))
    import check as public_session
    from check import has_regions
    from check_register import check_register
    from inventory import BEGIN, END, KERNEL_SHA, SOURCE_SHA, conflicts, digest, load_ranges, memory_ranges
    iso = args.distribution.resolve() / "PCP-OS.iso"
    if digest(iso) != SOURCE_SHA:
        raise ValueError("Wrong published CD")
    kernel_dir = args.kernel_dir.resolve()
    candidate = kernel_dir / "target/pcpos-kernel.elf32"
    measured = subprocess.check_output([sys.executable, kernel_dir / "tools/measure.py", candidate], text=True).strip()
    out = args.out.resolve()
    out.mkdir()
    for ram in (128, 256):
        trial(iso, None, "0232c02a8946d66216da0ac65953a246ce3d41d7a966ae31aa83c3335b3259e3",
              out / f"public-{ram}", ram)
        destination = out / f"candidate-{ram}"
        trial(iso, candidate, measured, destination, ram)
        check_register(kernel_dir / "src/font.rs", destination)
