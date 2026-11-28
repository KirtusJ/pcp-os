#!/usr/bin/env python3
"""Tiny artificial chunks only, never genuine bootable ISO or model assets."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from check_chunk_manifest import SCHEMA, read_chunks


def digest(b):
    return hashlib.sha256(b).hexdigest()


class ChunkOracleControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.slices = (b"FAKE", b"_ISO_", b"NOT_BOOTABLE")
        self.names = ("release/chunk-000", "release/chunk-001",
                      "release/chunk-002")
        for name, part in zip(self.names, self.slices):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(part)
        self.obj = {
            "schema": SCHEMA,
            "source_revision": "a" * 40,
            "iso": {"name": "PCP-OS-Qwen8B-12B-test.iso",
                    "bytes": sum(map(len, self.slices)),
                    "sha256": digest(b"".join(self.slices))},
            "parts": [
                {"index": i, "path": name, "bytes": len(part),
                 "sha256": digest(part)}
                for i, (name, part) in enumerate(zip(self.names, self.slices))
            ]
        }

    def run_case(self, obj=None, pin=None):
        payload = json.dumps(obj if obj is not None else self.obj,
                             sort_keys=True, separators=(",", ":")).encode()
        return read_chunks(self.root, payload, digest(payload) if pin is None
                           else pin)

    def test_valid_three_part_stream_matches_only_digest(self):
        report = self.run_case()
        self.assertTrue(report["chunk_reassembly_byte_digest_matches"])
        self.assertEqual(report["concatenated_sha256"],
                         digest(b"".join(self.slices)))
        self.assertEqual(len(report["parts"]), 3)
        self.assertFalse(report["iso_boot_or_filesystem_verified"])
        self.assertFalse(report["models_present_or_inference_verified"])
        self.assertFalse(report["publisher_identity_authenticated"])

    def test_one_part_missing(self):
        (self.root / self.names[1]).unlink()
        with self.assertRaises(FileNotFoundError):
            self.run_case()

    def test_tampered_part_rejected(self):
        (self.root / self.names[2]).write_bytes(b"NOT_BOOTABLE".lower())
        with self.assertRaisesRegex(ValueError, "part 2 digest differs"):
            self.run_case()

    def test_tampered_full_image_claim_rejected(self):
        self.obj["iso"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "full ISO digest"):
            self.run_case()

    def test_reordered_parts_rejected_even_if_indices_relabelled(self):
        parts = self.obj["parts"]
        parts[1], parts[2] = parts[2], parts[1]
        parts[1]["index"], parts[2]["index"] = 1, 2
        with self.assertRaisesRegex(ValueError, "full ISO digest"):
            self.run_case()

    def test_index_gap_and_duplicate_refused(self):
        for changed in (1, 0, True):
            with self.subTest(index=changed):
                self.obj["parts"][2]["index"] = changed
                with self.assertRaisesRegex(ValueError, "part index"):
                    self.run_case()

    def test_duplicate_part_path_refused(self):
        self.obj["parts"][2]["path"] = self.names[0]
        with self.assertRaisesRegex(ValueError, "repeated part path"):
            self.run_case()

    def test_hardlink_alias_refused(self):
        target = self.root / self.names[2]
        target.unlink()
        os.link(self.root / self.names[0], target)
        self.obj["parts"][2]["sha256"] = digest(self.slices[0])
        self.obj["parts"][2]["bytes"] = len(self.slices[0])
        self.obj["iso"]["bytes"] = (sum(map(len, self.slices)) -
                                    len(self.slices[2]) + len(self.slices[0]))
        self.obj["iso"]["sha256"] = digest(b"".join(self.slices[:2]) +
                                           self.slices[0])
        with self.assertRaisesRegex(ValueError, "alias one inode"):
            self.run_case()

    def test_symlink_part_refused(self):
        path = self.root / self.names[2]
        path.unlink()
        path.symlink_to(self.root / self.names[1])
        with self.assertRaisesRegex(ValueError, "symlinked"):
            self.run_case()

    def test_symlink_parent_refused(self):
        dirname = self.root / "release"
        second = self.root / "real-release"
        dirname.rename(second)
        dirname.symlink_to(second)
        with self.assertRaisesRegex(ValueError, "symlinked"):
            self.run_case()

    def test_path_traversal_and_absolute_refused(self):
        for bad in ("../hidden.bin", "/tmp/chunk", "release/../chunk-0",
                    "release//chunk-0", r"release\chunk"):
            with self.subTest(bad=bad):
                self.obj["parts"][0]["path"] = bad
                with self.assertRaisesRegex(ValueError, "path"):
                    self.run_case()

    def test_empty_part_refused(self):
        self.obj["parts"][2]["bytes"] = 0
        with self.assertRaisesRegex(ValueError, "empty or noninteger"):
            self.run_case()

    def test_boolean_byte_count_refused(self):
        self.obj["parts"][1]["bytes"] = True
        with self.assertRaisesRegex(ValueError, "empty or noninteger"):
            self.run_case()

    def test_empty_parts_list_refused(self):
        self.obj["parts"].clear()
        with self.assertRaisesRegex(ValueError, "empty or unbounded"):
            self.run_case()

    def test_wrong_total_length_refused(self):
        self.obj["iso"]["bytes"] += 1
        with self.assertRaisesRegex(ValueError, "assembled byte count"):
            self.run_case()

    def test_mismatched_independent_manifest_pin_refused(self):
        with self.assertRaisesRegex(ValueError, "separately supplied pin"):
            self.run_case(pin="0" * 64)

    def test_duplicate_json_object_key_refused(self):
        payload = json.dumps(self.obj, separators=(",", ":")).encode()
        payload = payload.replace(b'{"schema":',
                                  b'{"schema":"injected","schema":', 1)
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            read_chunks(self.root, payload, digest(payload))

    def test_unknown_field_refused(self):
        self.obj["claim_boot_verified"] = True
        with self.assertRaisesRegex(ValueError, "unexpected manifest"):
            self.run_case()

    def test_unversioned_or_zero_source_ref_refused(self):
        for rev in ("latest", "0" * 40):
            with self.subTest(revision=rev):
                self.obj["source_revision"] = rev
                with self.assertRaisesRegex(ValueError, "source revision"):
                    self.run_case()

    def test_iso_name_must_be_simple_iso(self):
        for name in ("../foo.iso", "PCP-OS.img", "a.iso/../../foo.iso"):
            with self.subTest(name=name):
                self.obj["iso"]["name"] = name
                with self.assertRaisesRegex(ValueError, "simple .iso filename"):
                    self.run_case()


if __name__ == "__main__":
    unittest.main()
