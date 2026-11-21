"""Boot the newly compiled default kernel through the published Limine BIOS CD and its demo modules."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from trial import until

import details_screen


def run(distribution, kernel, out):
    tools = distribution / "audit/public-live-cd"
    sys.path.insert(0, str(tools / "candidates/keyboard-01"))
    from candidate import check_boot_table, digest, extract, inspect_catalog, measure
    import check as public_session
    from check_register import check_register
    source = distribution / "PCP-OS.iso"
    if digest(source) != public_session.ISO_SHA:
        raise ValueError("Unexpected source CD")
    elf = kernel / "target/pcpos-kernel.elf32"
    measurement = measure(kernel / "tools", elf)
    out.mkdir()
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        original, rebuilt = work / "original", work / "rebuilt"
        extract(source, original)
        key = ("image=" + public_session.IMAGE_SHA).encode()
        config = (original / "limine.conf").read_bytes()
        if config.count(key) != 1 or measurement == public_session.IMAGE_SHA:
            raise ValueError("Expected one measured grant and a newly built kernel")
        config = config.replace(key, f"image={measurement} {details_screen.OPTIONS}".encode())
        conf = work / "limine.conf"
        conf.write_bytes(config)
        cd = work / "default.iso"
        subprocess.run(["xorriso", "-no_rc", "-indev", source, "-outdev", cd,
                        "-boot_image", "any", "replay", "-map", elf, "/kernel.elf32",
                        "-map", conf, "/limine.conf", "-commit", "-end"], check=True)
        extract(cd, rebuilt)
        def members(directory):
            return {p.relative_to(directory) for p in directory.rglob("*") if p.is_file()}
        files = members(original)
        if files != members(rebuilt) or digest(rebuilt / "kernel.elf32") != digest(elf) or (rebuilt / "limine.conf").read_bytes() != config:
            raise ValueError("Unexpected recovered files or measured grant")
        boot = Path("limine-bios-cd.bin")
        for item in files - {boot, Path("boot.cat"), Path("kernel.elf32"), Path("limine.conf")}:
            if digest(original / item) != digest(rebuilt / item):
                raise ValueError(f"Changed carried file {item}")
        a, b = ((p / boot).read_bytes() for p in (original, rebuilt))
        if a[:8] != b[:8] or a[24:] != b[24:]:
            raise ValueError("Changed Limine boot code outside its optical table")
        for image, tree, payload in ((source, original, a), (cd, rebuilt, b)):
            inspect_catalog(image, (tree / "boot.cat").read_bytes(), check_boot_table(image, payload))
        receipt = {"source_iso": public_session.ISO_SHA, "display_options": details_screen.OPTIONS, "test_iso": digest(cd), "elf": digest(elf), "measurement": measurement}
        (out / "image.json").write_text(json.dumps(receipt, indent=2) + "\n")
        session = out / "session"
        session.mkdir()
        serial = session / "serial.txt"
        command = ["qemu-system-x86_64", "-machine", "pc", "-accel", "tcg", "-cpu", "qemu64",
                   "-m", "64M", "-boot", "order=d", "-cdrom", str(cd), "-nic", "none", "-vga", "std",
                   "-display", "none", "-monitor", "none", "-serial", "file:" + str(serial), "-qmp", "stdio",
                   "-audiodev", "none,id=snd0", "-device", "AC97,audiodev=snd0",
                   "-icount", "shift=3,align=off,sleep=off"]
        (session / "qemu-command.json").write_text(json.dumps(command, indent=2) + "\n")
        with (session / "qemu.stderr").open("wb") as err, subprocess.Popen(
                command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as vm:
            try:
                monitor = details_screen.monitor_for(vm)
                log = lambda: serial.read_text(errors="replace") if serial.exists() else ""
                wait = lambda test, label: until(test, label, vm)
                wait(lambda: "leaves no memory behind" in log() or "it is in SAFE" in log(), "startup")
                startup = log()
                if (measurement + " (measured; equal" not in startup or "long mode true" not in startup
                        or "it is in SAFE" in startup or sum(line.startswith("  pass ") for line in startup.splitlines()) != 71):
                    raise RuntimeError("Unexpected optical kernel measurement or startup checks")
                details_screen.finish_session(monitor.call, wait, log, session, public_session)
                details_screen.power_off(monitor, vm)
                if "ALL SELF-TESTS PASS." not in log():
                    raise RuntimeError("Missing final optical self-test result")
                (session / "power-events.json").write_text(json.dumps(monitor.events, indent=2) + "\n")
                (session / "qemu-exit.txt").write_text(f"{vm.returncode}\n")
            finally:
                if vm.poll() is None:
                    vm.kill()
                    vm.wait()
        check_register(kernel / "src/font.rs", session)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("distribution", "kernel", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    run(args.distribution.resolve(), args.kernel.resolve(), args.output.resolve())
