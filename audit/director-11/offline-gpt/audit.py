#!/usr/bin/env python3
"""Director 11: test-only PMBR versus narrow GPT-mirror acceptance boundary.

Source is the exact, unmodified installed-acceptance gpt_integrity.py from
KirtusJ/pcp-os 59f26ebe6397e00e8a9b8cce08487ad8a5319c0c.
This is NOT a full UEFI disk validator, an installed image, boot/recovery
test, firmware update authorization or proof of a real bricking hazard.

UEFI Specification 2.11, Section 5.2.3, requires a protective MBR at LBA0
for a bootable GPT disk and specifies the 0x55AA signature and protective 0xEE
partition. Section 5.3.2 specifies the GPT revision for version 1.0.
The inspected function's stated purpose is narrower: GPT mirror integrity.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import tempfile
from unittest.mock import patch
import uuid
import zlib

PIN_COMMIT = "59f26ebe6397e00e8a9b8cce08487ad8a5319c0c"
PIN_BLOB = "620c82a7d686230d1b2ed82421b349da7a72f6be"
SECTORS = 32768
ENTRY_LEN = 128 * 128
TYPE = uuid.UUID("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7").bytes_le
UEFI_2_11 = "https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html"


def checked_module(source):
    data = source.read_bytes()
    blob_sha = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
    if blob_sha != PIN_BLOB:
        raise ValueError("source Git blob does not match pinned installed checker")
    spec = importlib.util.spec_from_file_location("d11_source_pinned_gpt_integrity", str(source))
    if spec is None or spec.loader is None:
        raise ValueError("missing source module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def build_fixture(path, pmbr="valid", revision=0x10000):
    guid = uuid.UUID("fc62edba-c6ba-49b1-b416-ac9a1af5b817").bytes_le
    entries = bytearray(ENTRY_LEN)
    entries[:128] = struct.pack(
        "<16s16sQQQ72s", TYPE,
        uuid.UUID("c0a00000-0000-0000-0000-000000000001").bytes_le,
        2048, 8191, 0, bytes(72)
    )
    with path.open("wb") as disk:
        disk.truncate(SECTORS * 512)
        if pmbr != "absent":
            mbr = bytearray(512)
            mbr[446:462] = struct.pack(
                "<B3sB3sII", 0, b"\x00\x02\x00",
                0x07 if pmbr == "wrong_type" else 0xEE,
                b"\xff\xff\xff", 1, SECTORS - 1
            )
            if pmbr != "bad_signature":
                mbr[510:512] = b"\x55\xaa"
            disk.seek(0)
            disk.write(mbr)
        for here, other, table_lba in (
            (1, SECTORS - 1, 2),
            (SECTORS - 1, 1, SECTORS - 33)
        ):
            header = bytearray(struct.pack(
                "<8sIIIIQQQQ16sQIII", b"EFI PART", revision, 92, 0, 0,
                here, other, 34, SECTORS - 34, guid, table_lba,
                128, 128, zlib.crc32(entries)
            ))
            struct.pack_into("<I", header, 16, zlib.crc32(header))
            disk.seek(here * 512)
            disk.write(header)
            disk.seek(table_lba * 512)
            disk.write(entries)


def verify(module, path, trace=False):
    with path.open("rb") as disk:
        if trace:
            with patch.object(module.os, "pread", wraps=os.pread) as reads:
                module.require_gpt_mirror(disk.fileno(), os.fstat(disk.fileno()).st_size)
            offsets = [c.args[2] for c in reads.call_args_list]
            return {"pread_calls": len(offsets), "any_LBA0_read": any(i < 512 for i in offsets),
                    "minimum_read_offset": min(offsets)}
        module.require_gpt_mirror(disk.fileno(), os.fstat(disk.fileno()).st_size)


def audit(source):
    module = checked_module(source)
    cases = []
    with tempfile.TemporaryDirectory(prefix="d11-pmbr-") as temp:
        disk = Path(temp) / "synthetic.raw"
        inputs = (
            ("valid_protective_mbr", "valid", 0x10000, True),
            ("absent_protective_mbr", "absent", 0x10000, True),
            ("wrong_protective_record_type", "wrong_type", 0x10000, True),
            ("invalid_mbr_55aa_signature", "bad_signature", 0x10000, True),
            ("mirrored_unknown_GPT_revision_1_1", "valid", 0x10001, True),
        )
        for name, pmbr, revision, expected_accept in inputs:
            build_fixture(disk, pmbr, revision)
            accepted = True
            error = None
            try:
                verify(module, disk)
            except ValueError as exc:
                accepted, error = False, str(exc)
            if accepted != expected_accept:
                raise AssertionError(f"{name}: changed upstream behavior {error}")
            row = {"case": name, "actual_gpt_mirror_accept": accepted,
                   "error": error, "UEFI_bootable_GPT_content_validated": False}
            if name == "valid_protective_mbr":
                trace = verify(module, disk, trace=True)
                if trace["any_LBA0_read"]:
                    raise AssertionError("source has begun inspecting LBA0: reassess PMBR boundary")
                row["observed_read_trace"] = trace
            cases.append(row)

        # Two negative controls distinguish weak acceptance coverage from a
        # broken harness that returns true for absolutely everything.
        build_fixture(disk)
        with disk.open("r+b") as f:
            f.seek(512)
            f.write(b"Z")
        try:
            verify(module, disk)
        except ValueError as exc:
            cases.append({"case": "corrupt_primary_GPT_header",
                          "actual_gpt_mirror_accept": False, "error": str(exc)})
        else:
            raise AssertionError("corrupt primary GPT header was not refused")

        build_fixture(disk)
        with disk.open("r+b") as f:
            f.seek((SECTORS - 33) * 512 + 7)
            f.write(b"Z")
        try:
            verify(module, disk)
        except ValueError as exc:
            cases.append({"case": "corrupt_backup_GPT_table",
                          "actual_gpt_mirror_accept": False, "error": str(exc)})
        else:
            raise AssertionError("corrupt GPT backup table was not refused")

    if len(cases) != 7:
        raise AssertionError("incomplete specified PMBR control set")
    return {
        "source_commit": PIN_COMMIT,
        "source_git_blob": PIN_BLOB,
        "source_path": "os-docs/installed-acceptance/gpt_integrity.py",
        "uefi_reference": UEFI_2_11,
        "scope": "read-only GPT mirror function; host-only synthetic 512-byte-sector images",
        "case_count": len(cases),
        "cases": cases,
        "uncovered_UEFI_protective_MBR_fields": ["LBA0", "0x55AA signature", "type 0xEE"],
        "uncovered_revision_exactness": "version 1.0 GPT requires 0x00010000",
        "source_function_changed": False,
        "real_UEFI_boot_performed": False,
        "real_disk_firmware_or_NVRAM_modified": False,
        "physical_anti_bricking_assurance_established": False,
        "disposition": "NARROW_GPT_MIRROR_BOUNDARY_REPRODUCED_NOT_FULL_UEFI_GPT_COMPLIANCE",
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    result = audit(args.source)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
