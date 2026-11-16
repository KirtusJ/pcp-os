#!/usr/bin/env python3
"""Bounded test-only native boot adjudicator for stripped PCP OS kernel images.

This script measures and evaluates only sources/builds explicitly pinned in
the accompanying Actions workflow. It never upgrades an ELF to a shipping
image, authorizes stripping for unrelated binaries, or asserts host-enforced
ACR or an installed model.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import sys

BASE="74c4d4a9934ad68e1b5be380971bf3377a464c6d"
CANDIDATE="f1b7430a9c1aaabc5587f19b16e0c1e76646ef81"
KINDS=("base_a","base_b","candidate_off")
HEX=re.compile(r"[0-9a-f]{64}\Z")
FORBIDDEN=(
    b"D10_REGISTER_RAW_BEGIN", b"D10_REGISTER_RAW_END",
    b"D10_REGISTER_UNMODIFIED", b"D10_REGISTER_INJECT"
)


def digest(path):
    value=path.read_text().strip()
    if not HEX.fullmatch(value):
        raise ValueError("invalid SHA-256 witness "+str(path))
    return value


def observe(folder):
    evidence={}
    for k in KINDS:
        evidence[k]={
            "raw_elf_sha256":digest(folder/(k+".raw.sha256")),
            "raw_image_measurement":digest(folder/(k+".raw.measurement")),
            "stripped_elf_sha256":digest(folder/(k+".stripped.sha256")),
            "stripped_image_measurement":digest(folder/(k+".stripped.measurement"))
        }
        size=int((folder/(k+".stripped.size")).read_text().strip())
        if size < 5000:
            raise ValueError("unexpected tiny normalized kernel image")
        evidence[k]["stripped_file_bytes"]=size
    serials={}
    for k in ("base_a","candidate_off"):
        body=(folder/(k+".serial")).read_bytes()
        code=int((folder/(k+".exit")).read_text().strip())
        expected=evidence[k]["stripped_image_measurement"].encode()
        serials[k]={
            "qemu_exit":code, "serial_sha256":hashlib.sha256(body).hexdigest(),
            "self_tests_present":b"ALL SELF-TESTS PASS" in body and b"  FAIL" not in body,
            "runtime_measured_pin_reported_equal":expected+b" (measured; equal" in body,
            "diagnostic_markers_absent":all(marker not in body for marker in FORBIDDEN)
        }
    base=evidence["base_a"]
    aa=evidence["base_b"]
    candidate=evidence["candidate_off"]
    checks={
        "raw_elf_reproducible_AA":base["raw_elf_sha256"]==aa["raw_elf_sha256"],
        "raw_code_measurement_reproducible_AA":base["raw_image_measurement"]==aa["raw_image_measurement"],
        "normalization_reproducible_AA":base["stripped_elf_sha256"]==aa["stripped_elf_sha256"],
        "source_original_raw_elf_differs":base["raw_elf_sha256"]!=candidate["raw_elf_sha256"],
        "stripped_bytes_identical_across_source":base["stripped_elf_sha256"]==candidate["stripped_elf_sha256"],
        "stripped_lengths_identical_across_source":base["stripped_file_bytes"]==candidate["stripped_file_bytes"],
        "image_measurement_original_equals_stripped_and_successor":all(
            v["raw_image_measurement"]==base["raw_image_measurement"]
            and v["stripped_image_measurement"]==base["raw_image_measurement"]
            for v in evidence.values()
        ),
        "both_stripped_elf_guests_pass_selftests_and_exit_33":all(
            v["qemu_exit"]==33 and v["self_tests_present"]
            for v in serials.values()
        ),
        "both_guest_image_pins_match_their_stripped_sources":all(
            v["runtime_measured_pin_reported_equal"] for v in serials.values()
        ),
        "both_guest_serials_lack_diagnostic_export_markers":all(
            v["diagnostic_markers_absent"] for v in serials.values()
        )
    }
    return {
        "kind":"D11_TEST_ONLY_NORMALIZED_ELF_NATIVE_BOOT",
        "sources":{"baseline":BASE,"diagnostic":CANDIDATE},
        "image_witnesses":evidence,
        "boot_witnesses":serials,
        "checks":checks,
        "disposition":("OBSERVED_IN_EXACT_CONTRACT" if all(checks.values()) else
                       "NOT_ESTABLISHED"),
        "original_raw_ELF_identity_claim":False,
        "shipping_ISO_produced":False,
        "installed_Qwen_or_PCPOS_host_enforcement_observed":False,
        "runtime_semantic_equivalence_proved":False
    }


def main():
    if len(sys.argv)!=2:
        raise SystemExit("usage: check_stripped_kernel_boot.py <witness-dir>")
    directory=Path(sys.argv[1])
    result=observe(directory)
    (directory/"stripped-native-boot-result.json").write_text(
        json.dumps(result,sort_keys=True,indent=2)+"\n")
    print(json.dumps(result,sort_keys=True))
    return 0 if result["disposition"]=="OBSERVED_IN_EXACT_CONTRACT" else 1


if __name__=="__main__":
    raise SystemExit(main())
