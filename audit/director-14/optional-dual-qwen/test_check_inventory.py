#!/usr/bin/env python3
"""Synthetic byte controls only; never count these as installed Qwen inference."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_inventory as inventory
from check_inventory import SCHEMA, check


def digest(data):
    return hashlib.sha256(data).hexdigest()


def asset(path, data):
    return {"path": path, "bytes": len(data), "sha256": digest(data)}


class DualQwenInventoryControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.files = {
            "models/8b/model.gguf": b"GGUFsynthetic-8B-NOT-WEIGHTS",
            "models/8b/LICENSE.txt": b"test-only 8b license fixture\n",
            "models/12b/model.gguf": b"GGUFsynthetic-12B-NOT-WEIGHTS",
            "models/12b/LICENSE.txt": b"test-only 12b license fixture\n",
            "bin/runner": b"dummy runtime, not an executable",
            "bin/tailorbird": b"dummy UI, not an executable",
        }
        for name, data in self.files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        self.manifest = {
            "schema": SCHEMA,
            "source_revision": "a" * 40,
            "models": [
                {"slot": slot, "parameters_billions": parameter,
                 "upstream_id": f"fixture/model-{parameter}",
                 "revision": chr(97+i) * 40,
                 "quantization": "Q4_K_M",
                 "weights": asset(f"models/{parameter}b/model.gguf",
                                  self.files[f"models/{parameter}b/model.gguf"]),
                 "license": asset(f"models/{parameter}b/LICENSE.txt",
                                  self.files[f"models/{parameter}b/LICENSE.txt"])}
                for i, (slot, parameter) in enumerate(
                    [("qwen-8b", 8), ("qwen-12b", 12)])
            ],
            "runtime": asset("bin/runner", self.files["bin/runner"]),
            "tailorbird": asset("bin/tailorbird", self.files["bin/tailorbird"])
        }

    def run_check(self, data=None, pin=None):
        payload = json.dumps(data if data is not None else self.manifest,
                             sort_keys=True, separators=(",", ":")).encode()
        return check(self.root, payload, digest(payload) if pin is None else pin)

    def test_positive_file_inventory_only(self):
        result = self.run_check()
        self.assertEqual([x["slot"] for x in result["model_slots"]],
                         ["qwen-12b", "qwen-8b"])
        self.assertEqual(result["bytes_in_checked_files"],
                         sum(map(len, self.files.values())))
        for field in ("model_inference_observed", "iso_verified",
                      "actual_parameter_counts_authenticated",
                      "license_rights_approved"):
            self.assertIs(result[field], False)

    def test_one_slot_missing(self):
        self.manifest["models"].pop()
        with self.assertRaisesRegex(ValueError, "both model slots"):
            self.run_check()

    def test_duplicate_slot(self):
        self.manifest["models"][1]["slot"] = "qwen-8b"
        with self.assertRaisesRegex(ValueError, "model slot"):
            self.run_check()

    def test_parameter_claim_mismatch(self):
        self.manifest["models"][1]["parameters_billions"] = 14
        with self.assertRaisesRegex(ValueError, "parameter slot"):
            self.run_check()

    def test_duplicate_upstream_identity(self):
        self.manifest["models"][1]["upstream_id"] = "fixture/model-8"
        with self.assertRaisesRegex(ValueError, "upstream ID"):
            self.run_check()

    def test_same_weights_under_different_model_slots(self):
        target = self.root / "models/12b/model.gguf"
        same = self.files["models/8b/model.gguf"]
        target.write_bytes(same)
        self.manifest["models"][1]["weights"] = asset("models/12b/model.gguf", same)
        with self.assertRaisesRegex(ValueError, "byte-identical weights"):
            self.run_check()

    def test_modified_model_bytes(self):
        (self.root / "models/8b/model.gguf").write_bytes(b"GGUFtampered")
        with self.assertRaisesRegex(ValueError, "SHA-256|byte count"):
            self.run_check()

    def test_modified_license_bytes(self):
        (self.root / "models/12b/LICENSE.txt").write_bytes(b"rewritten\n")
        with self.assertRaisesRegex(ValueError, "SHA-256|byte count"):
            self.run_check()

    def test_modified_runtime_bytes(self):
        (self.root / "bin/runner").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "SHA-256|byte count"):
            self.run_check()

    def test_modified_tailorbird_bytes(self):
        (self.root / "bin/tailorbird").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "SHA-256|byte count"):
            self.run_check()

    def test_parent_traversal_refused(self):
        self.manifest["models"][0]["weights"]["path"] = "../model.gguf"
        with self.assertRaisesRegex(ValueError, "noncanonical or traverses"):
            self.run_check()

    def test_absolute_path_refused(self):
        self.manifest["models"][0]["weights"]["path"] = "/etc/hosts"
        with self.assertRaisesRegex(ValueError, "relative"):
            self.run_check()

    def test_duplicate_asset_path_refused(self):
        self.manifest["models"][1]["license"] = copy.deepcopy(
            self.manifest["models"][0]["license"])
        with self.assertRaisesRegex(ValueError, "path reused"):
            self.run_check()

    def test_symlinked_model_path_refused(self):
        source = self.root / "models/8b/model.gguf"
        data = source.read_bytes()
        source.unlink()
        source.symlink_to(self.root / "models/12b/model.gguf")
        self.manifest["models"][0]["weights"] = asset("models/8b/model.gguf", data)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.run_check()

    def test_hardlink_alias_refused(self):
        source = self.root / "models/8b/LICENSE.txt"
        target = self.root / "models/12b/LICENSE.txt"
        target.unlink()
        os.link(source, target)
        self.manifest["models"][1]["license"] = asset(
            "models/12b/LICENSE.txt", source.read_bytes())
        with self.assertRaisesRegex(ValueError, "alias one inode"):
            self.run_check()

    def test_missing_model_file_refused(self):
        (self.root / "models/8b/model.gguf").unlink()
        with self.assertRaises(FileNotFoundError):
            self.run_check()

    def test_wrong_separately_supplied_manifest_digest(self):
        with self.assertRaisesRegex(ValueError, "separately supplied pin"):
            self.run_check(pin="0" * 64)

    def test_duplicate_json_key_refused_even_when_values_identical(self):
        payload = json.dumps(self.manifest, separators=(",", ":")).encode()
        payload = payload.replace(b'{"schema":', b'{"schema":"ignored","schema":', 1)
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            check(self.root, payload, digest(payload))

    def test_unknown_field_refused(self):
        self.manifest["models"][0]["fake_checked"] = True
        with self.assertRaisesRegex(ValueError, "unexpected/missing"):
            self.run_check()

    def test_unknown_or_unpinned_revision_refused(self):
        self.manifest["models"][1]["revision"] = "latest"
        with self.assertRaisesRegex(ValueError, "unversioned"):
            self.run_check()

    def test_noncanonical_or_cross_platform_path_refused(self):
        for path in ("models//8b/model.gguf", r"models\8b\model.gguf",
                     "models/./8b/model.gguf", "C:/models/8b/model.gguf"):
            with self.subTest(path=path):
                self.manifest["models"][0]["weights"]["path"] = path
                with self.assertRaisesRegex(ValueError, "path"):
                    self.run_check()


    def test_all_zero_source_revision_is_not_a_real_commit(self):
        # A 40-hex null object ID cannot identify an actual Git source.
        self.manifest["source_revision"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "source commit"):
            self.run_check()

    def test_all_zero_model_revision_is_not_an_upstream_pin(self):
        # The same missing-identity sentinel must not pin model weights.
        self.manifest["models"][1]["revision"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "unversioned"):
            self.run_check()


    def test_same_inode_modified_immediately_after_hash_is_refused(self):
        # One original file is hashed, then changed in place to same-length
        # bytes while its open handle remains live.
        target = self.root / "models/8b/model.gguf"
        target_ino = target.stat().st_ino
        previous = inventory.sha256_file

        def mutate_after_hash(handle):
            observation = previous(handle)
            if os.fstat(handle.fileno()).st_ino == target_ino:
                target.write_bytes(self.files["models/8b/model.gguf"].replace(
                    b"-8B-", b"-9B-"))
            return observation

        with patch.object(inventory, "sha256_file",
                          side_effect=mutate_after_hash):
            with self.assertRaisesRegex(ValueError, "changed during read"):
                self.run_check()

    def test_path_atomically_replaced_after_hash_is_refused(self):
        # Previously the old correct inode could be hashed successfully
        # even after its pathname was replaced by another file.
        target = self.root / "models/8b/model.gguf"
        target_ino = target.stat().st_ino
        swap = self.root / "models/8b/swapped.tmp"
        swap.write_bytes(b"NONMODEL-REPLACEMENT-FILE")
        previous = inventory.sha256_file

        def swap_after_hash(handle):
            observation = previous(handle)
            if os.fstat(handle.fileno()).st_ino == target_ino:
                os.replace(swap, target)
            return observation

        with patch.object(inventory, "sha256_file", side_effect=swap_after_hash):
            with self.assertRaisesRegex(ValueError, "path changed after read"):
                self.run_check()

    def test_path_removed_after_hash_is_not_success(self):
        target = self.root / "models/8b/model.gguf"
        target_ino = target.stat().st_ino
        previous = inventory.sha256_file

        def unlink_after_hash(handle):
            observation = previous(handle)
            if os.fstat(handle.fileno()).st_ino == target_ino:
                target.unlink()
            return observation

        with patch.object(inventory, "sha256_file",
                          side_effect=unlink_after_hash):
            with self.assertRaises(FileNotFoundError):
                self.run_check()


if __name__ == "__main__":
    unittest.main()
