"""Fail-closed, independently pinned inventory gate for a preloaded UEFI candidate.

This is a candidate-media builder, NOT a distribution authorization or a signature.
The approved inventory pin and publisher model digest must be supplied independently.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import stat
import struct
import subprocess
import tempfile

from hybrid_readback import verify_embedded_fat


FIXED = {
    "EFI/BOOT/BOOTX64.EFI",
    "PCPOS/KERNEL.ELF",
    "PCPOS/TAILOR.ELF",
    "PCPOS/WORKER.ELF",
    "PCPOS/model.manifest",
}
PUBLISHER_TEXTS = {"PCPOS/MODEL-LICENSE.txt", "PCPOS/MODEL-NOTICE.txt"}
RUNTIME_NOTICE = "PCPOS/RUNTIME-NOTICE.txt"
# Tailorbird #33 qualification binaries and their engine #404 notice.
PACKED_WORKERS = {
    "e454fbaf009feba660fba2ed7bf1f187cc18c4f4f8278125a7e8dfb3eb519bf7",
    "1400620be191ec0d6d18e6ff4da363cb6983913115067e3ee424abce5131a32c",
}
GGML_NOTICE = {"bytes": 1308, "sha256": "aa99af64d67d449bcff124bb9d449e624c5aac78484cbd36de8426a8624db348"}
QWEN3_8B_Q4_K_M = "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785"
QWEN_MODEL_LICENSE = {"bytes": 11544, "sha256": "5de36594c10839788a8c589443a8ef9d8b8d17c65a1b5807206ae037fc36c6bd"}
SHARD = re.compile(r"PCPOS/shard-([0-9]{5})\.bin\Z")
HEX = re.compile(r"[0-9a-f]{64}\Z")
EPOCH = "1794614400"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unique_pairs(pairs):
    found = {}
    for name, value in pairs:
        if name in found:
            raise ValueError("duplicate JSON key: " + name)
        found[name] = value
    return found


def verify_notices(expected, model_sha256=None):
    if expected["PCPOS/WORKER.ELF"]["sha256"] in PACKED_WORKERS and expected.get(RUNTIME_NOTICE) != GGML_NOTICE:
        raise ValueError("packed worker requires the exact ggml runtime notice at " + RUNTIME_NOTICE)
    if model_sha256 == QWEN3_8B_Q4_K_M and expected.get("PCPOS/MODEL-LICENSE.txt") != QWEN_MODEL_LICENSE:
        raise ValueError("Qwen3-8B Q4_K_M requires the publisher license at PCPOS/MODEL-LICENSE.txt")


def inventory(path, approved_sha256):
    """Read a manifest only after checking its independently approved raw-byte digest."""
    if not HEX.fullmatch(approved_sha256):
        raise ValueError("expected an exact lowercase inventory SHA-256")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != approved_sha256:
        raise ValueError("inventory does not match the independently approved SHA-256")
    found = json.loads(raw, object_pairs_hook=unique_pairs)
    if type(found) is not dict:
        raise ValueError("inventory must be a JSON object")
    shard_numbers = []
    folded = set()
    for name, entry in found.items():
        if type(name) is not str or (name not in FIXED and name not in PUBLISHER_TEXTS
                                     and name != RUNTIME_NOTICE and not SHARD.fullmatch(name)):
            raise ValueError("unexpected release path: " + str(name))
        if name.casefold() in folded:
            raise ValueError("case-colliding release path: " + name)
        folded.add(name.casefold())
        if type(entry) is not dict or set(entry) != {"bytes", "sha256"}:
            raise ValueError("invalid inventory entry: " + name)
        if type(entry["bytes"]) is not int or entry["bytes"] <= 0 or (
            type(entry["sha256"]) is not str or not HEX.fullmatch(entry["sha256"])
        ):
            raise ValueError("invalid bytes or digest: " + name)
        match = SHARD.fullmatch(name)
        if match:
            shard_numbers.append(int(match.group(1)))
    if not FIXED.issubset(found):
        raise ValueError("missing fixed executable or manifest path")
    verify_notices(found)
    if not shard_numbers or sorted(shard_numbers) != list(range(len(shard_numbers))):
        raise ValueError("shards must be contiguous and start at zero")
    return found


def verify_tree(root, expected, model_sha256):
    """Check every path, length, SHA-256 and model shard against the independent pins."""
    if not HEX.fullmatch(model_sha256):
        raise ValueError("expected an exact lowercase publisher model SHA-256")
    verify_notices(expected, model_sha256)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("payload root must be a real directory")
    actual_files = set()
    actual_dirs = set()
    for item in root.rglob("*"):
        name = item.relative_to(root).as_posix()
        mode = item.lstat().st_mode
        if stat.S_ISDIR(mode):
            actual_dirs.add(name)
        elif stat.S_ISREG(mode):
            actual_files.add(name)
        else:
            raise ValueError("symlink or special file in payload: " + name)
    expected_dirs = set()
    for name in expected:
        pieces = name.split("/")
        expected_dirs.update("/".join(pieces[:n]) for n in range(1, len(pieces)))
    if actual_files != set(expected) or actual_dirs != expected_dirs:
        missing = sorted(set(expected) - actual_files)
        extra = sorted(actual_files - set(expected))
        stray_dirs = sorted(actual_dirs - expected_dirs)
        raise ValueError(f"payload inventory differs: missing={missing}, extra={extra}, directories={stray_dirs}")
    for name, entry in expected.items():
        file = root / name
        if file.stat().st_size != entry["bytes"] or digest(file) != entry["sha256"]:
            raise ValueError("payload size or digest differs: " + name)

    data = (root / "PCPOS/model.manifest").read_bytes()
    if len(data) < 56 or data[:8] != b"PCPMDL01":
        raise ValueError("invalid packed model header")
    total, shard_bytes, count = struct.unpack_from("<QII", data, 8)
    names = sorted(name for name in expected if SHARD.fullmatch(name))
    if count != len(names) or shard_bytes == 0 or total == 0 or len(data) != 56 + 32 * count:
        raise ValueError("packed model shape differs")
    if data[24:56].hex() != model_sha256:
        raise ValueError("manifest does not bind the approved publisher model")
    whole = hashlib.sha256()
    for i, name in enumerate(names):
        length = min(shard_bytes, total - i * shard_bytes)
        if length <= 0 or expected[name]["bytes"] != length:
            raise ValueError("manifest shard length differs: " + name)
        part = hashlib.sha256()
        with (root / name).open("rb") as stream:
            while chunk := stream.read(1 << 20):
                part.update(chunk)
                whole.update(chunk)
        if part.digest() != data[56 + i * 32:88 + i * 32]:
            raise ValueError("manifest shard digest differs: " + name)
    if whole.hexdigest() != model_sha256 or total != sum(expected[n]["bytes"] for n in names):
        raise ValueError("full model digest or logical length differs")
    return len(expected)


def execute(*command):
    subprocess.run(command, check=True)


def verify_fat(volume, expected, model_sha256, scratch):
    """Compare the FAT's independently copied-out file tree, not just the staging tree."""
    extracted = scratch / "fat-readback"
    extracted.mkdir()
    execute("mcopy", "-sm", "-i", str(volume), "::/*", str(extracted))
    return verify_tree(extracted, expected, model_sha256)


def build(root, expected, model_sha256, size, output):
    """Use a private, freshly formatted FAT; publish only after both readbacks pass."""
    if os.path.lexists(output):
        raise FileExistsError(output)
    if not output.parent.is_dir():
        raise ValueError("output parent directory does not exist")
    if size % 512 or size < max(128 << 20, sum(item["bytes"] for item in expected.values()) + (64 << 20)):
        raise ValueError("FAT size must be aligned and leave at least 64 MiB free")
    verify_tree(root, expected, model_sha256)
    with tempfile.TemporaryDirectory(prefix=".pcpos-preloaded-", dir=output.parent) as tmp:
        scratch = Path(tmp)
        fat = scratch / "boot.fat"
        with fat.open("xb") as stream:
            stream.truncate(size)
        execute("mkfs.fat", "--invariant", "-F", "32", "-n", "PCPOS", str(fat))
        for directory in ("EFI", "EFI/BOOT", "PCPOS"):
            execute("mmd", "-i", str(fat), "::/" + directory)
        for name in sorted(expected):
            execute("mcopy", "-i", str(fat), str(root / name), "::/" + name)
        verify_fat(fat, expected, model_sha256, scratch)
        verified_fat_sha256 = digest(fat)

        helper = Path(__file__).resolve().parents[1] / "model-media-iso/build.py"
        hybrid = runpy.run_path(str(helper))["build"]
        private_iso = scratch / "candidate.iso"
        image_size, image_sha256 = hybrid(fat, private_iso)
        verify_embedded_fat(private_iso, size, verified_fat_sha256)
        joliet = scratch / "joliet-readback"
        joliet.mkdir()
        execute("xorriso", "-read_fs", "norock", "-osirrox", "on", "-indev", str(private_iso), "-extract", "/", str(joliet))
        verify_tree(joliet, expected, model_sha256)
        if private_iso.stat().st_size != image_size or digest(private_iso) != image_sha256:
            raise ValueError("finished ISO digest differs after final readback")
        os.link(private_iso, output)
    return image_size, image_sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("payload_tree", type=Path, help="directory containing exactly EFI/ and PCPOS/")
    parser.add_argument("--inventory", type=Path, required=True, help="independently approved expected-files.json")
    parser.add_argument("--inventory-sha256", required=True)
    parser.add_argument("--model-sha256", required=True, help="publisher whole-model SHA-256, independently approved")
    parser.add_argument("--fat-bytes", type=int, required=True, help="fresh FAT volume size, 512-byte aligned")
    parser.add_argument("output", type=Path, help="new candidate ISO file; never a block device")
    args = parser.parse_args()
    output = args.output.absolute()
    records = inventory(args.inventory, args.inventory_sha256)
    size, sha = build(args.payload_tree, records, args.model_sha256, args.fat_bytes, output)
    print(f"{sha}  {output} ({size} bytes); independent exact inventory verified in FAT and Joliet")


if __name__ == "__main__":
    main()
