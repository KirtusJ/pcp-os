#!/usr/bin/env python3
"""Evaluate only the four exact images staged by the pinned GitHub workflow.

Compares A/A reproducibility and the no-feature build of D10 #240 against
D10 #233. A matching digest does not establish conformance, safety, or
the behavior of arbitrary other hardware/toolchains.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent
LABELS = ("base_1", "base_2", "candidate_off", "candidate_on")
PIN_BASE = "74c4d4a9934ad68e1b5be380971bf3377a464c6d"
PIN_DIAGNOSTIC = "9fa9819103a67b0f2b124991a2d0d88b4c64b755"


def load_observation(data_dir):
    images = {}
    for name in LABELS:
        digest_file = data_dir / (name + ".elf_sha256")
        measure_file = data_dir / (name + ".measurement")
        if not digest_file.is_file() or not measure_file.is_file():
            raise ValueError(f"missing hash/measurement for {name}")
        digest, measure = digest_file.read_text().strip(), measure_file.read_text().strip()
        if len(digest) != 64 or len(measure) != 64 or any(c not in "0123456789abcdef" for c in digest+measure):
            raise ValueError(f"invalid SHA-256 representation for {name}")
        images[name] = {"elf_sha256": digest, "measured_image_sha256": measure}
    return images


def judge(images, status, serial_bytes):
    base1, base2, off, on = [images[n] for n in LABELS]
    reproducible_elf = base1["elf_sha256"] == base2["elf_sha256"]
    reproducible_measure = base1["measured_image_sha256"] == base2["measured_image_sha256"]
    nofeat_elf_equal = off["elf_sha256"] == base1["elf_sha256"]
    nofeat_measure_equal = off["measured_image_sha256"] == base1["measured_image_sha256"]
    feature_on_differs = on["measured_image_sha256"] != off["measured_image_sha256"]
    guest_accept = (status == 33 and b"ALL SELF-TESTS PASS" in serial_bytes
                    and b"  FAIL" not in serial_bytes
                    and off["measured_image_sha256"].encode()+b" (measured; equal" in serial_bytes)
    if not reproducible_elf or not reproducible_measure:
        disposition = "INCOMPARABLE_BUILD_CONTROL"
    elif not nofeat_measure_equal:
        disposition = "DEFAULT_MEASURED_CODE_DIFFERS"
    elif not nofeat_elf_equal:
        disposition = "DEFAULT_ELF_BYTES_DIFFER_BUT_MEASUREMENT_MATCHES"
    elif not feature_on_differs:
        disposition = "FEATURE_SEPARATION_NOT_ESTABLISHED"
    elif not guest_accept:
        disposition = "DEFAULT_GUEST_SELFTEST_NOT_ACCEPTED"
    else:
        disposition = "BYTE_IDENTICAL_IN_EXACT_BUILD_CONTRACT"
    return {
        "scope": "test-only exact pinned repo images; no production admission",
        "base_commit": PIN_BASE,
        "candidate_commit": PIN_DIAGNOSTIC,
        "build": images,
        "checks": {
            "base_two_clean_builds_same_elf_sha256": reproducible_elf,
            "base_two_clean_builds_same_measured_code": reproducible_measure,
            "candidate_without_feature_equals_base_elf_sha256": nofeat_elf_equal,
            "candidate_without_feature_equals_base_measured_code": nofeat_measure_equal,
            "candidate_feature_on_measured_code_distinct_from_feature_off": feature_on_differs,
            "candidate_no_feature_guest_selftest_exit33": guest_accept
        },
        "qemu_exit": status,
        "guest_serial_sha256": hashlib.sha256(serial_bytes).hexdigest(),
        "disposition": disposition,
        "runtime_authorization_or_installed_model_verified": False,
        "shipping_media_rebuilt_or_promoted": False
    }


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: evaluate_default_image.py <result-directory> <qemu-exit-file>")
    data = Path(sys.argv[1])
    images = load_observation(data)
    status = int(Path(sys.argv[2]).read_text().strip())
    raw = (data / "guest-serial.log").read_bytes()
    result = judge(images, status, raw)
    (data / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True)+"\n")
    print(json.dumps(result, sort_keys=True))
    return 0 if result["disposition"] == "BYTE_IDENTICAL_IN_EXACT_BUILD_CONTRACT" else 1


if __name__ == "__main__":
    raise SystemExit(main())
