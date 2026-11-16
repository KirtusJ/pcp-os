"""Exercise a pinned Core Specification through the normal PCP OS guest file API."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys
import subprocess
import tempfile


HERE = Path(__file__).resolve().parent
KERNEL = HERE.parents[1] / "PCP_OS_Rust/crates/pcpos-kernel"
CORE = "MODULES/01_Core/SOURCE/Specification.md"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(*args, cwd):
    subprocess.run(args, cwd=cwd, check=True)


def reference_object(directory, reference):
    (directory / "reference").write_bytes(reference)
    run("objcopy", "-I", "binary", "-O", "elf64-x86-64", "-B", "i386:x86-64",
        "--rename-section", ".data=.rodata,alloc,load,readonly,data,contents",
        "reference", "reference.o", cwd=directory)


def build(directory, reference):
    reference_object(directory, reference)
    run("cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-ffreestanding",
        "-fno-stack-protector", "-fno-pie", "-mno-red-zone", "-mcmodel=large",
        "-c", HERE / "readback.c", "-o", "readback.o", cwd=directory)
    run("ld", "--build-id=none", "-z", "noexecstack", "-T", KERNEL / "programs/pcp-fib/link.ld",
        "readback.o", "reference.o", "-o", "core-read.elf", cwd=directory)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--method", required=True, type=Path, help="method directory containing MANIFEST.json")
    p.add_argument("--manifest-sha256", required=True, help="expected separately obtained release digest")
    p.add_argument("--kernel", type=Path, required=True, help="bootable PCP OS multiboot ELF")
    p.add_argument("--qemu", default="qemu-system-x86_64")
    p.add_argument("--firmware", type=Path, help="optional QEMU firmware directory")
    p.add_argument("--timeout", type=int, default=120, help="seconds per guest")
    args = p.parse_args()
    manifest = (args.method / "MANIFEST.json").read_bytes()
    if digest(manifest) != args.manifest_sha256:
        p.error("method manifest does not match the expected SHA-256")
    rows = [f for f in json.loads(manifest)["files"] if f["path"] == CORE]
    source = (args.method / CORE).read_bytes()
    if len(rows) != 1 or not source or digest(source) != rows[0]["sha256"]:
        p.error("Core Specification does not match a unique entry in the pinned manifest")
    kernel = args.kernel.resolve(strict=True)
    measure = subprocess.check_output([sys.executable, KERNEL / "tools/measure.py", kernel], text=True).strip()
    qemu = shutil.which(args.qemu)
    if not qemu:
        p.error("QEMU executable was not found")
    qemu = str(Path(qemu).resolve())
    print(f"method={digest(manifest)} core={digest(source)} bytes={len(source)} kernel={measure}", flush=True)
    cases = [
        ("exact", source, "MATCH", 0), ("changed last byte", source[:-1] + bytes([source[-1] ^ 1]), "DIFFERENT", 1),
        ("truncated", source[:-1], "DIFFERENT", 1), ("appended", source + b"x", "DIFFERENT", 1),
        ("absent", None, "MISSING", 2), ("permission denied", source, "DENIED", 3),
    ]
    with tempfile.TemporaryDirectory() as temp:
        directory = Path(temp)
        build(directory, source)
        worker_hash = digest((directory / "core-read.elf").read_bytes())
        for label, data, expected, status in cases:
            modules = "core-read.elf"
            if data is not None:
                (directory / "core-spec.md").write_bytes(data)
                modules += ",core-spec.md"
            command = [qemu]
            if args.firmware:
                command += ["-L", str(args.firmware.resolve())]
            command += ["-machine", "microvm,isa-serial=on,pic=on,pit=on,rtc=on", "-accel", "tcg,tb-size=4", "-icount", "shift=3,align=off,sleep=off", "-cpu", "max", "-m", "32M",
                        "-nodefaults", "-nic", "none", "-display", "none", "-monitor", "none",
                        "-serial", "stdio", "-no-reboot", "-kernel", str(kernel), "-initrd", modules,
                        "-append", f"auto=core-read.elf then=off image={measure}" +
                        (" grant=none" if expected == "DENIED" else ""),
                        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
            try:
                boot = subprocess.run(command, cwd=directory, capture_output=True, text=True, timeout=args.timeout)
            except subprocess.TimeoutExpired as error:
                last = (error.stdout or b"")[-2300:].decode(errors="replace")
                raise SystemExit(f"{label}: timed out after {args.timeout} s\n{last}") from None
            log = boot.stdout
            receipts = re.findall(r"^CORE READBACK (.+)$", log, re.M)
            start = f"starting core-read.elf ("
            exits = re.findall(r"^core-read\.elf ended \w+: exit (\d+)\b", log, re.M)
            if (boot.returncode != 33 or receipts != [expected] or exits != [str(status)]
                    or not any(line.startswith(start) and worker_hash in line for line in log.splitlines())
                    or measure + " (measured; equal to the value the loader named)" not in log):
                raise SystemExit(f"{label}: QEMU {boot.returncode}\n{log[-2300:]}\n{boot.stderr}")
            print(f"PASS {label}: {expected}, guest exit {status}, worker {worker_hash}", flush=True)


if __name__ == "__main__":
    main()
