"""Adverse controls for the offline preloaded release-candidate inventory gate."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest

from preloaded_gate import build, inventory, verify_tree


def sha(data):
    return hashlib.sha256(data).hexdigest()


class GateTests(unittest.TestCase):
    def setUp(self):
        context = tempfile.TemporaryDirectory()
        self.addCleanup(context.cleanup)
        self.temp = Path(context.name)
        self.root = self.temp / "payload"
        (self.root / "PCPOS").mkdir(parents=True)
        (self.root / "EFI" / "BOOT").mkdir(parents=True)
        self.parts = {
            "EFI/BOOT/BOOTX64.EFI": b"efi",
            "PCPOS/KERNEL.ELF": b"kernel",
            "PCPOS/TAILOR.ELF": b"foreground",
            "PCPOS/WORKER.ELF": b"worker",
            "PCPOS/shard-00000.bin": b"abcd",
            "PCPOS/shard-00001.bin": b"xy",
        }
        self.model_pin = sha(b"abcdxy")
        self.parts["PCPOS/model.manifest"] = (
            b"PCPMDL01" + struct.pack("<QII", 6, 4, 2) +
            bytes.fromhex(self.model_pin) + bytes.fromhex(sha(b"abcd")) +
            bytes.fromhex(sha(b"xy"))
        )
        for name, content in self.parts.items():
            (self.root / name).write_bytes(content)
        self.records = self.make_records()
        self.assertEqual(verify_tree(self.root, self.records, self.model_pin), 7)

    def make_records(self):
        return {name: {"bytes": len(content), "sha256": sha(content)}
                for name, content in self.parts.items()}

    def approved(self, data):
        file = self.temp / "expected-files.json"
        raw = json.dumps(data, sort_keys=True).encode()
        file.write_bytes(raw)
        return inventory(file, sha(raw))

    def test_approved_inventory(self):
        self.assertEqual(self.approved(self.records), self.records)

    def test_changed_byte_refused(self):
        (self.root / "PCPOS/TAILOR.ELF").write_bytes(b"Foreground")
        with self.assertRaisesRegex(ValueError, "payload size or digest differs"):
            verify_tree(self.root, self.records, self.model_pin)

    def test_extra_file_in_reused_staging_refused(self):
        (self.root / "PCPOS/UNLISTED.BIN").write_bytes(b"surplus")
        with self.assertRaisesRegex(ValueError, "extra="):
            verify_tree(self.root, self.records, self.model_pin)

    def test_missing_file_refused(self):
        (self.root / "PCPOS/WORKER.ELF").unlink()
        with self.assertRaisesRegex(ValueError, "missing="):
            verify_tree(self.root, self.records, self.model_pin)

    def test_empty_extra_directory_refused(self):
        (self.root / "PCPOS/leftover").mkdir()
        with self.assertRaisesRegex(ValueError, "directories="):
            verify_tree(self.root, self.records, self.model_pin)

    def test_symlink_refused(self):
        item = self.root / "PCPOS/WORKER.ELF"
        item.unlink()
        item.symlink_to("KERNEL.ELF")
        with self.assertRaisesRegex(ValueError, "symlink"):
            verify_tree(self.root, self.records, self.model_pin)

    def test_publisher_pin_mismatch_refused(self):
        with self.assertRaisesRegex(ValueError, "approved publisher"):
            verify_tree(self.root, self.records, "0" * 64)

    def test_manifest_mismatch_even_if_rehashed_inventory(self):
        data = bytearray(self.parts["PCPOS/model.manifest"])
        data[56] ^= 1
        (self.root / "PCPOS/model.manifest").write_bytes(data)
        self.parts["PCPOS/model.manifest"] = bytes(data)
        records = self.make_records()
        with self.assertRaisesRegex(ValueError, "manifest shard digest differs"):
            verify_tree(self.root, records, self.model_pin)

    def test_incomplete_contiguous_shards_refused(self):
        bad = dict(self.records)
        bad["PCPOS/shard-00002.bin"] = bad.pop("PCPOS/shard-00001.bin")
        with self.assertRaisesRegex(ValueError, "contiguous"):
            self.approved(bad)

    def test_boolean_length_refused(self):
        bad = json.loads(json.dumps(self.records))
        bad["PCPOS/KERNEL.ELF"]["bytes"] = True
        with self.assertRaisesRegex(ValueError, "invalid bytes"):
            self.approved(bad)

    def test_duplicate_json_keys_refused(self):
        file = self.temp / "duplicate.json"
        raw = b'{"PCPOS/KERNEL.ELF":{},"PCPOS/KERNEL.ELF":{}}'
        file.write_bytes(raw)
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            inventory(file, sha(raw))

    def test_inventory_pin_mismatch_refused(self):
        file = self.temp / "expected-files.json"
        file.write_text(json.dumps(self.records))
        with self.assertRaisesRegex(ValueError, "independently approved"):
            inventory(file, "0" * 64)

    def test_absent_required_executable_refused(self):
        bad = dict(self.records)
        del bad["EFI/BOOT/BOOTX64.EFI"]
        with self.assertRaisesRegex(ValueError, "missing fixed"):
            self.approved(bad)

    def test_existing_output_is_never_overwritten(self):
        image = self.temp / "exists.iso"
        image.write_bytes(b"original")
        with self.assertRaises(FileExistsError):
            build(self.root, self.records, self.model_pin, 128 << 20, image)
        self.assertEqual(image.read_bytes(), b"original")

    def test_insufficient_fat_capacity_refused(self):
        with self.assertRaisesRegex(ValueError, "FAT size"):
            build(self.root, self.records, self.model_pin, 1 << 20, self.temp / "unused.iso")


if __name__ == "__main__":
    unittest.main()
