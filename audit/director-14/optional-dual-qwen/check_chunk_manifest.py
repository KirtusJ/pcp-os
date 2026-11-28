#!/usr/bin/env python3
"""Test-only streamed verification of a chunked *candidate* ISO byte sequence.

No ISO syntax, boot, model authenticity, filesystem, inference or licensing
is checked. A byte digest of a named sequence is not proof of a publisher.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat

from check_inventory import HEX40, HEX64, require, safe_parts, unique_pairs, no_constant

SCHEMA = "pcpos.optional-dual-qwen-chunk-inventory.v1"
ISO_NAME = re.compile(r"[A-Za-z0-9_.-]+\.iso\Z")


def read_chunks(root: Path, manifest: bytes, expected_manifest_sha256: str) -> dict:
    require(type(expected_manifest_sha256) is str and
            HEX64.fullmatch(expected_manifest_sha256),
            "separate manifest pin must be lowercase 64-hex")
    seen_manifest = hashlib.sha256(manifest).hexdigest()
    require(seen_manifest == expected_manifest_sha256,
            "manifest content differs from separately supplied pin")
    obj = json.loads(manifest.decode("utf-8"),
                     object_pairs_hook=unique_pairs, parse_constant=no_constant)
    require(type(obj) is dict and
            set(obj) == {"schema", "source_revision", "iso", "parts"},
            "unexpected manifest fields")
    require(obj["schema"] == SCHEMA, "unrecognized chunk manifest schema")
    require(type(obj["source_revision"]) is str and
            HEX40.fullmatch(obj["source_revision"]) and
            obj["source_revision"] != "0" * 40,
            "invalid or absent exact source revision")
    iso = obj["iso"]
    require(type(iso) is dict and
            set(iso) == {"name", "bytes", "sha256"},
            "unexpected final-image fields")
    require(type(iso["name"]) is str and ISO_NAME.fullmatch(iso["name"]),
            "final image must have simple .iso filename")
    require(type(iso["bytes"]) is int and iso["bytes"] > 0,
            "invalid declared final byte count")
    require(type(iso["sha256"]) is str and HEX64.fullmatch(iso["sha256"]),
            "invalid declared final SHA-256")
    parts = obj["parts"]
    require(type(parts) is list and 1 <= len(parts) <= 10000,
            "empty or unbounded part list")
    require(root.is_dir() and not root.is_symlink(),
            "root must be an explicit local directory")
    sum_digest = hashlib.sha256()
    bytes_seen = 0
    names: set[str] = set()
    identities: set[tuple[int, int]] = set()
    rows = []
    for index, part in enumerate(parts):
        require(type(part) is dict and
                set(part) == {"index", "path", "bytes", "sha256"},
                "unexpected part fields")
        require(type(part["index"]) is int and part["index"] == index,
                "part index missing, repeated or reordered")
        name = part["path"]
        components = safe_parts(name)
        require(name not in names, "repeated part path")
        names.add(name)
        require(type(part["bytes"]) is int and part["bytes"] > 0,
                "empty or noninteger part size")
        require(type(part["sha256"]) is str and
                HEX64.fullmatch(part["sha256"]),
                "invalid part SHA-256")

        path = root
        for i, component in enumerate(components):
            path /= component
            st = path.lstat()
            require(not stat.S_ISLNK(st.st_mode),
                    "chunk parent or file is symlinked")
            require(stat.S_ISDIR(st.st_mode) if i < len(components) - 1
                    else stat.S_ISREG(st.st_mode),
                    "chunk path is not a regular file under directories")
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        with os.fdopen(fd, "rb") as f:
            fstat = os.fstat(f.fileno())
            require(stat.S_ISREG(fstat.st_mode) and
                    (fstat.st_dev, fstat.st_ino) == (st.st_dev, st.st_ino),
                    "part changed between stat and open")
            identity = (fstat.st_dev, fstat.st_ino)
            require(identity not in identities, "two parts alias one inode")
            identities.add(identity)
            d = hashlib.sha256()
            count = 0
            while chunk := f.read(1024 * 1024):
                count += len(chunk)
                d.update(chunk)
                sum_digest.update(chunk)
            endstat = os.fstat(f.fileno())
            require((endstat.st_size, endstat.st_mtime_ns) ==
                    (fstat.st_size, fstat.st_mtime_ns),
                    "part metadata changed while streaming")
        require(count == part["bytes"], f"part {index} length differs")
        digest = d.hexdigest()
        require(digest == part["sha256"], f"part {index} digest differs")
        bytes_seen += count
        require(bytes_seen <= iso["bytes"], "parts exceed declared ISO size")
        rows.append({"index": index, "path": name, "bytes": count,
                     "sha256": digest})
    require(bytes_seen == iso["bytes"], "assembled byte count differs")
    overall = sum_digest.hexdigest()
    require(overall == iso["sha256"],
            "concatenated parts do not match expected full ISO digest")
    return {
        "scope": "DECLARED_CHUNK_CONCATENATION_ONLY",
        "source_revision": obj["source_revision"],
        "manifest_sha256": seen_manifest,
        "nominal_iso_name": iso["name"],
        "concatenated_bytes": bytes_seen,
        "concatenated_sha256": overall,
        "parts": rows,
        "chunk_reassembly_byte_digest_matches": True,
        "iso_boot_or_filesystem_verified": False,
        "models_present_or_inference_verified": False,
        "publisher_identity_authenticated": False,
        "license_rights_approved": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--expected-manifest-sha256", required=True,
                        help="obtained separately, not from the candidate manifest")
    args = parser.parse_args()
    try:
        observation = read_chunks(args.root, args.manifest.read_bytes(),
                                  args.expected_manifest_sha256)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        parser.exit(1, f"chunk-stream byte oracle refused: {exc}\n")
    print(json.dumps(observation, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
