#!/usr/bin/env python3
"""Compare the installed model and license in a PCP OS raw factory disk with its manifest."""
import argparse
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

SECTOR_BYTES = 512


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


def check(disk_path, manifest_path):
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    revision = manifest["qwen"]["revision"]
    if not isinstance(revision, str) or not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("Expected a full Qwen revision in the release manifest")
    directory = f"/models/qwen/qwen2.5-coder-0.5b/{revision}"
    files = {}
    with disk_path.open("rb") as disk:
        before = os.fstat(disk.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError("Supply an offline regular raw disk image")
        disk_hash = hashlib.file_digest(disk, "sha256").hexdigest()
        if disk_hash != manifest["output_sha256"]["PCP-OS.img"]:
            raise ValueError("Installed disk SHA-256 differs from the release manifest")
        number, offset, length = model_partition(disk, before.st_size)
        for name, field in (("qwen.gguf", "model_sha256"), ("LICENSE.txt", "license_sha256")):
            path = f"{directory}/{name}"
            size, digest = read_file(disk, offset, path)
            if not size or digest != manifest["qwen"][field]:
                raise ValueError(f"{path}: {size} bytes, SHA-256 {digest} differs from the release or file is empty")
            files[path] = {"bytes": size, "sha256": digest}
        after = os.fstat(disk.fileno())
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise ValueError("Disk changed during the read")
    return {
        "source_revision": manifest["git_commit"], "qwen_revision": revision,
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(), "disk_sha256": disk_hash,
        "partition": {"name": "PCP OS models", "number": number,
                      "byte_offset": offset, "bytes": length},
        "files": files,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--disk", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = check(args.disk, args.manifest)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        sys.exit(f"Model partition check failed: {exc}")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
