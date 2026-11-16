"""Adversarial QEMU IDE0-versus-IDE1 boundary replay on *temporary* disk files.

WARNING: This fixture intentionally demonstrates a wrong-device write when
both disks have the same *public static marker*. That is an EXPECTED ADVERSE
RESULT in a synthetic guest, NOT a pass of real disk identity handling.

The kernel is supplied by QEMU -kernel, so the image labelled 'boot' is a
decoy GPT-like physical disk, NOT proof that PCP-OS boots from that drive.
No firmware installation, real media, physical hotplug or encrypted volume
is tested. The script fails on any unpredicted disk mutation.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile

from store_probe import format_disk, make_guests

MARKER = b"PCPOS USER STORE 1\n"
DISK_BYTES = 259 * 512


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def gpt_like_disk(path: Path) -> None:
    """Make a *non-store* disposable GPT-like sentinel, not a real boot OS."""
    image = bytearray(1024 * 512)
    # Synthetic protective MBR and GPT header signature are sufficient
    # to ensure this is NOT mistaken for the raw PCP OS store marker.
    image[446 + 4] = 0xEE
    image[446 + 8:446 + 12] = (1).to_bytes(4, "little")
    image[446 + 12:446 + 16] = (1023).to_bytes(4, "little")
    image[510:512] = b"\x55\xaa"
    image[512:520] = b"EFI PART"
    with path.open("xb") as out:
        out.write(image)


def verified_disk_read(path: Path) -> bytes:
    data = path.read_bytes()
    assert len(data) >= DISK_BYTES, f"unexpected image size: {path}"
    return data


def qemu_case(
    args: argparse.Namespace,
    directory: Path,
    label: str,
    disks: list[tuple[int, Path]],
    action: int,
    expected_exit: int,
    boot_suffix: str = "",
) -> tuple[str, list[str]]:
    guest = directory / f"action{action}.elf"
    serial = directory / (label + ".serial")
    command = [
        args.qemu, "-accel", "tcg", "-cpu", "max", "-m", "256M",
        "-nic", "none", "-display", "none", "-vga", "none",
        "-monitor", "none", "-no-reboot",
        "-icount", "shift=4,align=off,sleep=off",
        "-kernel", str(args.kernel.resolve()),
        "-initrd", str(guest),
        "-append", f"auto=action{action}.elf then=off store=ide0{boot_suffix}",
        "-serial", "file:" + str(serial),
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]
    for index, disk in sorted(disks):
        if not disk.is_file() or not disk.resolve().is_relative_to(directory.resolve()):
            raise RuntimeError("Refusing any disk outside the disposable test directory")
        if index not in (0, 1):
            raise RuntimeError(f"unsupported fixture IDE index {index}")
        command += ["-drive", f"if=ide,index={index},format=raw,file={disk.resolve()}"]
    result = subprocess.run(
        command, cwd=directory, capture_output=True,
        timeout=args.timeout, check=False,
    )
    text = serial.read_text(errors="replace") if serial.exists() else ""
    lines = text.splitlines()
    expected_text = "STORE PROBE OK" if expected_exit == 33 else "could not be recovered"
    if result.returncode != expected_exit or not any(expected_text in x for x in lines):
        raise AssertionError(
            f"{label}: QEMU exit {result.returncode} (expected {expected_exit}), "
            f"missing {expected_text!r}; stdout={result.stdout[:300]!r}, "
            f"stderr={result.stderr[:300]!r}, serial-tail={lines[-24:]}"
        )
    print(f"{label}: expected QEMU exit {result.returncode}; {expected_text}", flush=True)
    return text, lines


def replay(args: argparse.Namespace, directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=False)
    kernel_bytes = args.kernel.read_bytes()
    print("source_kernel_sha256", digest(kernel_bytes), flush=True)
    print("QEMU source pin and disk paths are test-only; no physical disk paths accepted")
    make_guests(directory, args.clang, args.objcopy)

    store = directory / "intended-store.raw"
    boot = directory / "boot-sentinel.raw"
    clone = directory / "wrong-marked-primary.raw"
    format_disk(store)
    gpt_like_disk(boot)
    format_disk(clone)
    pristine_store = verified_disk_read(store)
    pristine_boot = verified_disk_read(boot)
    pristine_clone = verified_disk_read(clone)
    assert pristine_store.startswith(MARKER)
    assert pristine_clone.startswith(MARKER)
    assert not pristine_boot.startswith(MARKER)
    assert pristine_store == pristine_clone, "public static marker clone control changed"

    # Positive legacy baseline: a marked volume on exactly IDE0 works
    # and an unrelated second disk remains byte-for-byte unchanged.
    qemu_case(args, directory, "primary-marked-store", [(0, store), (1, boot)], 1, 33)
    committed_store = verified_disk_read(store)
    assert committed_store != pristine_store, "store IDE0 never committed"
    assert b"oldbytes" in committed_store, "stored guest payload absent after fsync"
    assert verified_disk_read(boot) == pristine_boot, "boot sentinel was modified"
    qemu_case(args, directory, "cold-retain-primary", [(0, store), (1, boot)], 2, 33)
    assert verified_disk_read(store) == committed_store, "cold read changed store bytes"
    assert verified_disk_read(boot) == pristine_boot, "cold read changed second disk"
    print("POSITIVE LEGACY SCOPE: ide0 marked store persists; secondary sentinel unmodified")

    # A GPT-like disk on IDE0 prevents this special store from mounting,
    # even with a valid intended store attached as IDE1.
    qemu_case(args, directory, "wrong-gpt-primary", [(0, boot), (1, store)], 1, 35)
    assert verified_disk_read(boot) == pristine_boot, "unmarked primary modified"
    assert verified_disk_read(store) == committed_store, "secondary store modified"
    print("SAFE REFUSAL: unmarked GPT-like primary; no device written")

    # Removing IDE0 while retaining the store as IDE1 must not
    # magically remap it or write it. This is *between-boots* removal,
    # NOT a live surprise-unplug or DMA containment test.
    qemu_case(args, directory, "ide0-absent", [(1, store)], 1, 35)
    assert verified_disk_read(store) == committed_store, "store modified after IDE0 removal"
    print("SAFE REFUSAL: primary absent at next boot, secondary preserved")

    # Counterexample: because the only on-media discriminator is a
    # *copyable fixed marker*, a marked WRONG IDE0 is accepted even while
    # the correctly intended previously committed volume is on IDE1.
    # Deliberately writes only a manufactured temporary decoy.
    qemu_case(args, directory, "cloned-marker-wrong-primary", [(0, clone), (1, store)], 1, 33)
    modified_clone = verified_disk_read(clone)
    assert modified_clone != pristine_clone, "expected adversarial wrong-device write absent"
    assert b"oldbytes" in modified_clone, "wrong disk write lacked guest payload"
    assert verified_disk_read(store) == committed_store, "intended second disk changed unexpectedly"
    print("EXPECTED ADVERSE WITNESS: copied public marker on IDE0 accepted and modified", flush=True)
    print("NOT AN ACCEPTANCE PASS: device role is NOT cryptographically or hardware-bound")

    print("boot_sentinel_sha256", digest(pristine_boot))
    print("intended_store_sha256", digest(committed_store))
    print("wrong_primary_before_sha256", digest(pristine_clone))
    print("wrong_primary_after_sha256", digest(modified_clone))
    print("PASS: fixture correctly observed positive, safe-refusal and adverse role-confusion arms")
    print("NO CLAIM: real boot disk, removable live device, NVMe/AHCI/USB, encrypted volume, or host PCP")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--objcopy", default="objcopy")
    parser.add_argument("--timeout", type=int, default=90)
    args = parser.parse_args()
    if not args.kernel.is_file():
        raise SystemExit("kernel ELF32 source missing; no QEMU run")
    if args.out:
        replay(args, args.out.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix="pcpos-store-boundary-") as d:
            replay(args, Path(d) / "disposable")
