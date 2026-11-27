"""Boot the checked-in model-free desktop in a disposable x86-64 UEFI VM."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile


HERE = Path(__file__).resolve().parent


def find_firmware(qemu, directory):
    if directory:
        roots = [directory]
    else:
        prefix = qemu.resolve().parent
        roots = [Path("/usr/share/OVMF"), Path("/usr/share/edk2/ovmf"),
                 prefix / "share", prefix.parent / "share/qemu", prefix / "share/qemu"]
    pairs = [("OVMF_CODE_4M.fd", "OVMF_VARS_4M.fd"),
             ("edk2-x86_64-code.fd", "edk2-i386-vars.fd")]
    for root in roots:
        for code, variables in pairs:
            if (root / code).is_file() and (root / variables).is_file():
                return root / code, root / variables
    raise ValueError("Supply --firmware DIR containing an unsigned x86-64 OVMF code and variables pair.")


def unpack(work, image):
    with zipfile.ZipFile(HERE / image["archive"]) as bundle:
        info = bundle.getinfo(image["filename"])
        if len(bundle.infolist()) != 1 or info.file_size != image["bytes"]:
            raise ValueError("The ISO archive inventory or byte count differs from image.json.")
        sha = hashlib.sha256()
        with bundle.open(info) as source, (work / "desktop.iso").open("xb") as out:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                sha.update(chunk)
                out.write(chunk)
    if sha.hexdigest() != image["sha256"]:
        raise ValueError("The ISO SHA-256 differs from image.json.")


def check_boot(command, work, measurement):
    log = work / "serial.log"
    with subprocess.Popen([*command, "-display", "none", "-monitor", "none",
                           "-serial", "file:serial.log"], cwd=work) as guest:
        try:
            deadline = time.monotonic() + 120
            while time.monotonic() < deadline:
                serial = log.read_text(errors="replace") if log.exists() else ""
                for line in serial.splitlines():
                    if line.startswith("desktop: "):
                        measured = measurement + " (measured; equal to the value the loader named)"
                        if ("long mode true" not in serial or measured not in serial
                                or "  FAIL " in serial or "it is in SAFE" in serial):
                            raise ValueError("The desktop started with an unexpected kernel or startup state.")
                        print(f"{line}\nVerified x86-64 UEFI boot of the model-free ISO.", flush=True)
                        return
                if guest.poll() is not None:
                    raise RuntimeError(f"QEMU exited before desktop startup (status {guest.returncode}).\n{serial[-800:]}")
                time.sleep(0.2)
            raise RuntimeError(f"Timed out waiting for the desktop.\n{serial[-800:]}")
        finally:
            if guest.poll() is None:
                guest.kill()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="boot headlessly, verify the measured desktop, then close QEMU")
    parser.add_argument("--qemu", default="qemu-system-x86_64", help="QEMU executable name or path")
    parser.add_argument("--firmware", type=Path, help="directory with matching OVMF code and variables files")
    args = parser.parse_args()
    program = shutil.which(args.qemu)
    if not program:
        parser.error("Install qemu-system-x86_64 or pass --qemu PATH.")
    qemu = Path(program).resolve()
    code, variables = find_firmware(qemu, args.firmware)
    image = json.loads((HERE / "image.json").read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="pcpos-desktop-") as temporary:
        work = Path(temporary)
        unpack(work, image)
        shutil.copyfile(code, work / "code.fd")
        shutil.copyfile(variables, work / "vars.fd")
        command = [str(qemu), "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "1024",
                   "-drive", "if=pflash,unit=0,format=raw,readonly=on,file=code.fd",
                   "-drive", "if=pflash,unit=1,format=raw,file=vars.fd",
                   "-drive", "media=cdrom,format=raw,readonly=on,file=desktop.iso",
                   "-boot", "order=d", "-nic", "none", "-no-reboot"]
        if args.check:
            check_boot(command, work, image["kernel_measurement"])
        else:
            subprocess.run(command, cwd=work, check=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError, zipfile.BadZipFile, subprocess.CalledProcessError) as exc:
        sys.exit(str(exc))
