#!/usr/bin/env python3
"""TEST-ONLY mirrored boot selector model with adversarial prefix-write cuts.

This is deliberately NOT the installed PCP OS bootloader, updater, UEFI
firmware, GPT, authenticated release verifier or an update installer.
Demonstrates one required nonbricking *design invariant*: a pending update
must retain an independently verified known-good boot candidate even when
the newly written selector is torn at any byte boundary.

The trusted anti-rollback floor is supplied externally, not inferred from
the mutable selectors themselves. A matching CRC is NOT a signature.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import zlib

SECTOR = 128
MAGIC = b"PCP-D13\0"
PREFIX = struct.Struct("<8sQBBB5x32s32s")
SELECTOR_BYTES = 2 * SECTOR


class NoSafeBoot(ValueError):
    pass


def digest(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def make_record(sequence: int, selected: int, known_good: int,
                images: tuple[bytes, bytes]) -> bytes:
    if not 0 <= sequence < (1 << 64):
        raise ValueError("invalid selector sequence")
    if selected not in (0, 1) or known_good not in (0, 1):
        raise ValueError("invalid slot")
    core = PREFIX.pack(MAGIC, sequence, selected, known_good, 0,
                       digest(images[0]), digest(images[1]))
    body = core + bytes(SECTOR - 4 - len(core))
    return body + struct.pack("<I", zlib.crc32(body))


def parse_record(raw: bytes):
    if len(raw) != SECTOR:
        return None
    if zlib.crc32(raw[:-4]) != struct.unpack_from("<I", raw, SECTOR - 4)[0]:
        return None
    magic, seq, selected, good, reserved, image_a, image_b = PREFIX.unpack_from(raw)
    if magic != MAGIC or selected not in (0, 1) or good not in (0, 1) or reserved != 0:
        return None
    if any(raw[PREFIX.size:SECTOR - 4]):
        return None
    return {"seq": seq, "selected": selected, "known_good": good,
            "digests": (image_a, image_b), "raw": raw}


def read_selector(disk: bytes, *, trusted_min_sequence=None):
    if len(disk) != SELECTOR_BYTES:
        raise ValueError("wrong selector storage length")
    records = [
        r for r in (
            parse_record(disk[:SECTOR]), parse_record(disk[SECTOR:])
        ) if r is not None
    ]
    if not records:
        raise NoSafeBoot("both selector copies invalid")
    if len(records) == 2 and records[0]["seq"] == records[1]["seq"] \
            and records[0]["raw"] != records[1]["raw"]:
        raise NoSafeBoot("same-generation selector equivocation")
    selected = max(records, key=lambda r: r["seq"])
    if trusted_min_sequence is not None:
        if not 0 <= trusted_min_sequence < (1 << 64):
            raise ValueError("invalid external trusted floor")
        if selected["seq"] < trusted_min_sequence:
            raise NoSafeBoot("selector rollback against external trusted floor")
    return selected


def candidates(disk: bytes, images: tuple[bytes, bytes], *,
               trusted_min_sequence=None):
    """Return order of *verified* image candidates; never claim boot success."""
    r = read_selector(disk, trusted_min_sequence=trusted_min_sequence)
    verified = [digest(images[i]) == r["digests"][i] for i in range(2)]
    if not verified[r["known_good"]]:
        raise NoSafeBoot("known-good image bytes missing or altered")
    choices = []
    for i in (r["selected"], r["known_good"]):
        if verified[i] and i not in choices:
            choices.append(i)
    return choices


def mutate_prefix(before: bytes, after: bytes, prefix_bytes: int) -> bytes:
    if len(before) != len(after) or not 0 <= prefix_bytes <= len(after):
        raise ValueError("invalid cut")
    return after[:prefix_bytes] + before[prefix_bytes:]


def witness():
    # The image bytes are synthetic and intentionally NOT bootable.
    images = (b"KNOWN_GOOD_SYNTHETIC_A", b"PENDING_SYNTHETIC_B")
    old = make_record(41, 0, 0, images)
    new = make_record(42, 1, 0, images)
    cut_stats = {"before_new_selector_flush": 0,
                 "before_second_copy_flush": 0,
                 "candidate_available_after_full_commit": False}
    observed = []
    for written in range(SECTOR + 1):
        # Crash while *first* copy is being written; second remains known-good.
        raw = mutate_prefix(old, new, written) + old
        paths = candidates(raw, images)
        if 0 not in paths:
            raise AssertionError("known good lost during first selector write")
        cut_stats["before_new_selector_flush"] += 1
        observed.append(tuple(paths))

        # Crash while *second* selector copy is being written, after first
        # copy completed. The first copy must retain the known-good fallback.
        raw = new + mutate_prefix(old, new, written)
        paths = candidates(raw, images)
        if 0 not in paths:
            raise AssertionError("known good lost during second selector write")
        cut_stats["before_second_copy_flush"] += 1
        observed.append(tuple(paths))
    cut_stats["candidate_available_after_full_commit"] = candidates(new+new, images) == [1, 0]
    if not cut_stats["candidate_available_after_full_commit"]:
        raise AssertionError("fully written selector does not offer B then A")

    # Negative controls. A valid CRC of a selector is not the original
    # author's signature, and neither candidate can be called "booted".
    bad_a = (b"CORRUPTED_KNOWN_GOOD_A", images[1])
    bad_b = (images[0], b"CORRUPTED_PENDING_B")
    assert candidates(new+new, bad_b) == [0]
    try:
        candidates(new+new, bad_a)
        raise AssertionError("corrupt fallback was accepted")
    except NoSafeBoot:
        pass
    try:
        candidates(bytes(SELECTOR_BYTES), images)
        raise AssertionError("invalid selectors were accepted")
    except NoSafeBoot:
        pass
    try:
        candidates(old+old, images, trusted_min_sequence=42)
        raise AssertionError("rollback against externally trusted floor was accepted")
    except NoSafeBoot:
        pass
    equivocal = make_record(42, 0, 0, images)
    try:
        candidates(new+equivocal, images)
        raise AssertionError("equal-generation disagreement was accepted")
    except NoSafeBoot:
        pass
    # A completely valid old selector *is* accepted when the floor is not
    # externally supplied. That is the deliberate anti-rollback gap witness.
    assert candidates(old+old, images) == [0]
    return {
        "scope": "synthetic two-slot 128-byte selector metadata only",
        "model_is_real_PCP_OS_bootloader": False,
        "hardware_firmware_write_performed": False,
        "real_power_cut_tested": False,
        "real_boot_media_tested": False,
        "cryptographic_authentication_performed": False,
        "guest_booted": False,
        "known_good_preserved_by_model_under_prefix_write": True,
        "crash_cutpoints_tested": sum((SECTOR + 1 for _ in range(2))),
        "per_copy_checks": cut_stats,
        "negative_controls": [
            "candidate image digest mismatch -> known good only",
            "known-good image corrupt -> no assurance",
            "both selectors corrupt -> refusal",
            "external highwater floor rejects a replayed selector",
            "equal-generation conflicting copies refuse",
            "absent external floor permits older valid selector (risk)",
        ],
        "signature_would_require_real_trusted_verification": True,
        "explicit_recovery_authorization_untested": True,
        "result": "SYNTHETIC_SELECTOR_INVARIANT_ONLY",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = witness()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, sort_keys=True, indent=2)+"\n",
                           encoding="utf-8")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
