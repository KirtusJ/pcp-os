#!/usr/bin/env python3
"""Test-only byte inventory for a proposed *optional* dual-Qwen PCP OS image.

This NEVER authenticates parameter count, model architecture, licenses, an ISO,
an installed partition, model inference, or a Tailorbird service. It accepts
only a separately hash-pinned manifest and its exact local regular files.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat

SCHEMA = "pcpos.optional-dual-qwen-file-inventory.v1"
SLOTS = {"qwen-8b": 8, "qwen-12b": 12}
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
HEX40 = re.compile(r"[0-9a-f]{40}\Z")
IDENT = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\Z")
TOKEN = re.compile(r"[A-Za-z0-9_.-]+\Z")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def fields(obj: object, wanted: set[str], context: str) -> None:
    require(type(obj) is dict and set(obj) == wanted,
            f"{context}: unexpected/missing fields")


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "manifest: duplicate JSON key")
        result[key] = value
    return result


def no_constant(value):
    raise ValueError(f"non-JSON constant: {value}")


def sha256_file(f):
    digest = hashlib.sha256()
    size = 0
    while chunk := f.read(1024 * 1024):
        digest.update(chunk)
        size += len(chunk)
    return size, digest.hexdigest()


def safe_parts(path: object) -> list[str]:
    require(type(path) is str and path != "" and not path.startswith("/"),
            "asset path must be relative")
    require("\\" not in path and ":" not in path and "\0" not in path,
            "asset path contains unsupported characters")
    parts = path.split("/")
    require(all(part not in ("", ".", "..") and TOKEN.fullmatch(part)
                for part in parts), "asset path is noncanonical or traverses")
    return parts


def verify_asset(root: Path, spec: object, seen_paths: set[str],
                 seen_files: set[tuple[int, int]]) -> dict:
    fields(spec, {"path", "bytes", "sha256"}, "asset")
    path = spec["path"]
    parts = safe_parts(path)
    require(path not in seen_paths, "asset path reused across slots")
    seen_paths.add(path)
    expected_bytes, expected_digest = spec["bytes"], spec["sha256"]
    require(type(expected_bytes) is int and expected_bytes > 0,
            "asset bytes must be positive integer")
    require(type(expected_digest) is str and HEX64.fullmatch(expected_digest),
            "asset SHA-256 must be lowercase 64-hex")

    location = root
    for i, part in enumerate(parts):
        location /= part
        st = location.lstat()  # missing, symlink, directory and file differ
        require(not stat.S_ISLNK(st.st_mode), "asset path contains symlink")
        require(stat.S_ISDIR(st.st_mode) if i < len(parts) - 1
                else stat.S_ISREG(st.st_mode),
                "asset path is not a regular file below directories")
    fd = os.open(location, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(fd, "rb") as handle:
        actual = os.fstat(handle.fileno())
        require(stat.S_ISREG(actual.st_mode) and
                (actual.st_dev, actual.st_ino) == (st.st_dev, st.st_ino),
                "asset replaced during open")
        identity = (actual.st_dev, actual.st_ino)
        require(identity not in seen_files, "two asset paths alias one inode")
        seen_files.add(identity)
        nbytes, digest = sha256_file(handle)
        after = os.fstat(handle.fileno())
        # Detect observable mutation while hashing. A trusted immutable
        # factory snapshot is still required against adversarial races.
        require((actual.st_dev, actual.st_ino, actual.st_size,
                 actual.st_mtime_ns, actual.st_ctime_ns) ==
                (after.st_dev, after.st_ino, after.st_size,
                 after.st_mtime_ns, after.st_ctime_ns),
                "asset changed during read")
        final_path = location.lstat()
        require(stat.S_ISREG(final_path.st_mode) and
                (final_path.st_dev, final_path.st_ino) ==
                (actual.st_dev, actual.st_ino),
                "asset path changed after read")
    require(nbytes == expected_bytes, f"{path}: byte count mismatches manifest")
    require(digest == expected_digest, f"{path}: SHA-256 mismatches manifest")
    return {"path": path, "bytes": nbytes, "sha256": digest}


def check(root: Path, manifest_bytes: bytes,
          expected_manifest_sha256: str) -> dict:
    require(type(expected_manifest_sha256) is str and
            HEX64.fullmatch(expected_manifest_sha256),
            "separate manifest pin must be lowercase 64-hex")
    observed_manifest = hashlib.sha256(manifest_bytes).hexdigest()
    require(observed_manifest == expected_manifest_sha256,
            "manifest content differs from separately supplied pin")
    obj = json.loads(manifest_bytes.decode("utf-8"),
                     object_pairs_hook=unique_pairs, parse_constant=no_constant)
    fields(obj, {"schema", "source_revision", "models", "runtime", "tailorbird"},
           "manifest")
    require(obj["schema"] == SCHEMA, "unexpected manifest schema")
    require(type(obj["source_revision"]) is str and
            HEX40.fullmatch(obj["source_revision"]) and
            obj["source_revision"] != "0" * 40,
            "missing exact source commit")
    require(type(obj["models"]) is list and len(obj["models"]) == 2,
            "both model slots required")
    require(root.is_dir() and not root.is_symlink(), "untrusted model root")
    slots = set()
    model_ids = set()
    weights_digests = set()
    paths = set()
    inodes = set()
    models = []
    for spec in obj["models"]:
        fields(spec, {"slot", "parameters_billions", "upstream_id",
                      "revision", "quantization", "weights", "license"},
               "model slot")
        slot = spec["slot"]
        require(type(slot) is str and slot in SLOTS and slot not in slots,
                "missing, duplicate or unknown model slot")
        slots.add(slot)
        require(type(spec["parameters_billions"]) is int and
                spec["parameters_billions"] == SLOTS[slot],
                "declared model parameter slot mismatch")
        ident = spec["upstream_id"]
        require(type(ident) is str and IDENT.fullmatch(ident) and
                ident not in model_ids, "missing or duplicate upstream ID")
        model_ids.add(ident)
        require(type(spec["revision"]) is str and
                HEX40.fullmatch(spec["revision"]) and
                spec["revision"] != "0" * 40, "unversioned model weights")
        require(type(spec["quantization"]) is str and
                TOKEN.fullmatch(spec["quantization"]), "invalid quantization")
        weights = verify_asset(root, spec["weights"], paths, inodes)
        license_file = verify_asset(root, spec["license"], paths, inodes)
        require(weights["sha256"] not in weights_digests,
                "two model slots contain byte-identical weights")
        weights_digests.add(weights["sha256"])
        models.append({"slot": slot, "upstream_id": ident,
                       "revision": spec["revision"],
                       "quantization": spec["quantization"],
                       "weights": weights, "license": license_file})
    require(slots == set(SLOTS), "incomplete required slot set")
    runtime = verify_asset(root, obj["runtime"], paths, inodes)
    tailorbird = verify_asset(root, obj["tailorbird"], paths, inodes)
    return {
        "scope": "FILE_BYTES_ONLY_NOT_ISO_BOOT_OR_MODEL_INFERENCE",
        "manifest_sha256": observed_manifest,
        "source_revision": obj["source_revision"],
        "model_slots": sorted(models, key=lambda x: x["slot"]),
        "runtime": runtime,
        "tailorbird": tailorbird,
        "bytes_in_checked_files": sum(item["bytes"] for item in
                                      [runtime, tailorbird] +
                                      [field for model in models
                                       for field in (model["weights"],
                                                     model["license"])]),
        "model_inference_observed": False,
        "iso_verified": False,
        "actual_parameter_counts_authenticated": False,
        "license_rights_approved": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True,
                        help="read-only mounted model/runtime file root")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--expected-manifest-sha256", required=True,
                        help="independent publisher/handoff digest, not from manifest")
    args = parser.parse_args()
    try:
        result = check(args.root, args.manifest.read_bytes(),
                       args.expected_manifest_sha256)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        parser.exit(1, f"dual-Qwen file inventory refused: {exc}\n")
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
