"""Format a fresh PCP OS user disk or run an isolated cold-boot regression."""

import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import tempfile


def format_disk(path):
    image = bytearray(259 * 512)
    marker = b"PCPOS USER STORE 1\n"
    image[:len(marker)] = marker
    commit = b"PCPSLOT1" + struct.pack("<QI", 1, 0) + hashlib.sha256(b"").digest()
    image[512:512 + 84] = commit + hashlib.sha256(commit).digest()
    with path.open("xb") as file:
        file.write(image)


def make_guests(root, clang, objcopy):
    source = Path(__file__).resolve().parents[1] / "programs/store-probe/probe.S"
    for action in range(1, 8):
        obj, raw = root / "guest.o", root / "guest.raw"
        subprocess.run([clang, "--target=x86_64-unknown-linux-gnu", f"-DACTION={action}",
                        "-c", str(source), "-o", str(obj)], check=True)
        subprocess.run([objcopy, "-j", ".text", "-O", "binary", str(obj), str(raw)], check=True)
        code = raw.read_bytes()
        size = 120 + len(code)
        elf = struct.pack("<16sHHIQQQIHHHHHH", b"\x7fELF\x02\x01\x01", 3, 62, 1,
                          120, 64, 0, 0, 64, 56, 1, 0, 0, 0)
        elf += struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, size, size, 4096) + code
        (root / f"action{action}.elf").write_bytes(elf)


def replay(args, root):
    root.mkdir(parents=True, exist_ok=False)
    disk = root / "guest.raw.disk"
    kernel = str(args.kernel.resolve())
    format_disk(disk)
    make_guests(root, args.clang, args.objcopy)
    qemu = [args.qemu]
    if args.bios_dir:
        qemu += ["-L", str(args.bios_dir.resolve())]

    def boot(label, action, *, denied=False, invalid=False):
        guest = f"action{action}.elf"
        serial = root / (label + ".serial")
        line = f"auto={guest} then=off store=ide0" + (" grant=none" if denied else "")
        cmd = qemu + ["-accel", "tcg", "-cpu", "max", "-m", "256M", "-nic", "none",
                      "-display", "none", "-vga", "none", "-monitor", "none", "-no-reboot",
                      "-icount", "shift=4,align=off,sleep=off", "-kernel", kernel,
                      "-initrd", guest, "-append", line, "-drive",
                      f"if=ide,index=0,format=raw,file={disk.name}", "-serial", "file:" + str(serial),
                      "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
        ret = subprocess.run(cmd, cwd=root, capture_output=True, timeout=args.timeout,
                             creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        lines = serial.read_text(errors="replace").splitlines()
        if invalid:
            expected = (ret.returncode == 35 and any("could not be recovered" in s for s in lines)
                        and "STORE PROBE OK" not in lines)
        else:
            expected = ret.returncode == 33 and "STORE PROBE OK" in lines
        if not expected:
            raise RuntimeError(f"{label}: QEMU status {ret.returncode}, inspect {serial}")
        print(f"{label}: passed (QEMU {ret.returncode})", flush=True)

    boot("create-and-fsync", 1)
    boot("cold-old", 2)
    old = disk.read_bytes()
    boot("replace-without-fsync", 5)
    boot("cold-old-after-uncommitted-rename", 2)
    boot("replace-and-fsync-directory", 3)
    boot("cold-new", 4)
    boot("denied-store-grant", 6, denied=True)
    boot("oversized-checkpoint", 7)
    boot("cold-new-after-refusals", 4)
    new = disk.read_bytes()
    # Generation 3 is in slot zero. Replay sectors in protocol order over the
    # earlier disk to simulate a lost payload write and a torn commit sector.
    candidate = bytearray(old)
    candidate[1024:1536] = new[1024:1536]
    disk.write_bytes(candidate)
    boot("cold-after-payload-only", 2)
    candidate[512:512 + 32] = new[512:512 + 32]
    disk.write_bytes(candidate)
    boot("cold-after-torn-commit", 2)
    candidate = bytearray(new)
    candidate[1024] ^= 1
    disk.write_bytes(candidate)
    boot("fallback-after-corrupt-newest-payload", 2)
    candidate[130*512] ^= 1  # discard the older valid header as well
    disk.write_bytes(candidate)
    boot("refuse-both-invalid", 2, invalid=True)
    candidate = bytearray(new)
    candidate[0] ^= 1
    disk.write_bytes(candidate)
    boot("refuse-unmarked-device", 4, invalid=True)
    if disk.read_bytes() != candidate:
        raise RuntimeError("unmarked disk changed")
    print("kernel SHA-256", hashlib.sha256(Path(kernel).read_bytes()).hexdigest())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    sub.add_parser("format").add_argument("disk", type=Path)
    test = sub.add_parser("test")
    test.add_argument("--kernel", required=True, type=Path)
    test.add_argument("--out", type=Path, help="new directory to keep disks and serial receipts")
    test.add_argument("--qemu", default="qemu-system-x86_64")
    test.add_argument("--bios-dir", type=Path)
    test.add_argument("--timeout", type=int, default=90, help="wall-clock seconds per cold boot")
    test.add_argument("--clang", default="clang")
    test.add_argument("--objcopy", default="llvm-objcopy")
    args = parser.parse_args()
    if args.mode == "format":
        format_disk(args.disk)
    elif args.out:
        replay(args, args.out.resolve())
    else:
        with tempfile.TemporaryDirectory() as temporary:
            replay(args, Path(temporary) / "boots")
