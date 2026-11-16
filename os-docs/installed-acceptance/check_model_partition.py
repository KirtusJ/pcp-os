#!/usr/bin/env python3
"""Compare the installed Qwen model and license with a PCP OS release manifest."""
import argparse
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import stat
import struct
import subprocess
import sys
import tempfile

from gpt_integrity import require_gpt_mirror

SECTOR_BYTES = 512
COPY_BYTES = 1024 * 1024


def model_partition(disk, size):
    fd = disk.fileno()
    listing = subprocess.run(
        ["partx", "--pairs", "--sector-size", str(SECTOR_BYTES), "--output",
         "NR,START,SECTORS,NAME,SCHEME", "--", f"/proc/self/fd/{fd}"],
        check=True, pass_fds=(fd,), capture_output=True, text=True,
    )
    matches = []
    for line in listing.stdout.splitlines():
        row = dict(field.split("=", 1) for field in shlex.split(line))
        if row["SCHEME"] == "gpt" and row["NAME"] == "PCP OS models":
            matches.append(row)
    if len(matches) != 1:
        raise ValueError(f"Expected one GPT partition named PCP OS models, found {len(matches)}")
    row = matches[0]
    offset = int(row["START"]) * SECTOR_BYTES
    length = int(row["SECTORS"]) * SECTOR_BYTES
    if offset <= 0 or length < SECTOR_BYTES or offset + length > size:
        raise ValueError("Model partition extends outside the disk")

    # mtools reads a filesystem at an offset. Check its declared size against the GPT extent.
    boot = os.pread(fd, SECTOR_BYTES, offset)
    if len(boot) != SECTOR_BYTES or boot[510:512] != b"\x55\xaa":
        raise ValueError("Missing FAT boot sector on model partition")
    sector_size = struct.unpack_from("<H", boot, 11)[0]
    sectors = struct.unpack_from("<H", boot, 19)[0] or struct.unpack_from("<I", boot, 32)[0]
    if sector_size not in (512, 1024, 2048, 4096) or not 0 < sector_size * sectors <= length:
        raise ValueError("Declared FAT size is invalid for the model partition")
    return int(row["NR"]), offset, length


def copy_partition(disk, offset, length, volume):
    """Copy exactly one partition into an empty temporary file, preserving zero ranges."""
    disk.seek(offset)
    remaining = length
    zero_block = bytes(COPY_BYTES)
    while remaining:
        block = disk.read(min(COPY_BYTES, remaining))
        if not block:
            raise ValueError("Incomplete model partition")
        if block == zero_block[:len(block)]:
            volume.seek(len(block), os.SEEK_CUR)
        else:
            volume.write(block)
        remaining -= len(block)
    # Seeking across trailing zeroes alone does not extend the destination file.
    volume.truncate(length)
    volume.flush()


def read_file(disk, offset, path):
    fd = disk.fileno()
    command = ["mcopy", "-i", f"/proc/self/fd/{fd}@@{offset}", "::" + path, "-"]
    digest = hashlib.sha256()
    size = 0
    with subprocess.Popen(command, stdout=subprocess.PIPE, pass_fds=(fd,)) as proc:
        for chunk in iter(lambda: proc.stdout.read(1024 * 1024), b""):
            size += len(chunk)
            digest.update(chunk)
        if proc.wait() != 0:
            raise ValueError(f"Cannot read {path} from the model partition")
    return size, digest.hexdigest()


def check(disk_path, manifest_path, *, scope="factory", expected_disk_sha256=None,
          expected_manifest_sha256=None):
    if scope not in ("factory", "model-files"):
        raise ValueError(f"Unknown verification scope: {scope}")
    for name, value in (("disk", expected_disk_sha256), ("manifest", expected_manifest_sha256)):
        if value is not None and (not isinstance(value, str) or not re.fullmatch("[a-fA-F0-9]{64}", value)):
            raise ValueError(f"Expected {name} SHA-256 must contain 64 hexadecimal digits")
    manifest_bytes = manifest_path.read_bytes()
    manifest_hash = hashlib.sha256(manifest_bytes).hexdigest()
    if expected_manifest_sha256 is not None and manifest_hash != expected_manifest_sha256.lower():
        raise ValueError(f"Manifest SHA-256 differs from expected: measured {manifest_hash}")
    manifest = json.loads(manifest_bytes)
    source_revision = manifest["git_commit"]
    if (not isinstance(source_revision, str) or
            not re.fullmatch(r"[0-9a-fA-F]{40}(?:[0-9a-fA-F]{24})?", source_revision) or
            not source_revision.strip("0")):
        raise ValueError("Expected a full, non-null release Git commit ID in the manifest")
    factory_hash = manifest["output_sha256"]["PCP-OS.img"]
    if not isinstance(factory_hash, str) or not re.fullmatch("[0-9a-f]{64}", factory_hash):
        raise ValueError("Expected a factory disk SHA-256 in the release manifest")
    revision = manifest["qwen"]["revision"]
    if not isinstance(revision, str) or not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("Expected a full Qwen revision in the release manifest")
    directory = f"/models/qwen/qwen2.5-coder-0.5b/{revision}"
    files = {}
    with disk_path.open("rb") as disk, ExitStack() as resources:
        before = os.fstat(disk.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError("Supply an offline regular raw disk image")
        disk_hash = hashlib.file_digest(disk, "sha256").hexdigest()
        if expected_disk_sha256 is not None and disk_hash != expected_disk_sha256.lower():
            raise ValueError(f"Disk SHA-256 differs from expected: measured {disk_hash}")
        factory_matches = disk_hash == factory_hash
        if scope == "factory" and not factory_matches:
            raise ValueError("Installed disk SHA-256 differs from the release manifest")
        require_gpt_mirror(disk.fileno(), before.st_size)
        number, offset, length = model_partition(disk, before.st_size)
        volume, volume_offset = disk, offset
        if not factory_matches:
            # Give mtools only the selected extent of a disk changed since release.
            volume = resources.enter_context(tempfile.TemporaryFile())
            volume_offset = 0
            copy_partition(disk, offset, length, volume)
        for name, field in (("qwen.gguf", "model_sha256"), ("LICENSE.txt", "license_sha256")):
            path = f"{directory}/{name}"
            size, digest = read_file(volume, volume_offset, path)
            if not size or digest != manifest["qwen"][field]:
                raise ValueError(f"{path}: {size} bytes, SHA-256 {digest} differs from the release or file is empty")
            files[path] = {"bytes": size, "sha256": digest}
        after = os.fstat(disk.fileno())
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise ValueError("Disk changed during the read")
    return {
        "scope": scope, "factory_disk_matches": factory_matches, "factory_disk_sha256": factory_hash,
        "source_revision": source_revision, "qwen_revision": revision,
        "manifest_sha256": manifest_hash, "disk_sha256": disk_hash,
        "partition": {"name": "PCP OS models", "number": number,
                      "byte_offset": offset, "bytes": length},
        "files": files,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--disk", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--expected-disk-sha256", metavar="HEX",
                        help="require the supplied disk to match this SHA-256")
    parser.add_argument("--expected-manifest-sha256", metavar="HEX",
                        help="require the supplied manifest to match this SHA-256")
    parser.add_argument("--scope", choices=("factory", "model-files"), default="factory",
                        help="factory requires the release disk hash; model-files also accepts a changed disk")
    args = parser.parse_args()
    try:
        result = check(args.disk, args.manifest, scope=args.scope,
                       expected_disk_sha256=args.expected_disk_sha256,
                       expected_manifest_sha256=args.expected_manifest_sha256)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        sys.exit(f"Model partition check failed: {exc}")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
