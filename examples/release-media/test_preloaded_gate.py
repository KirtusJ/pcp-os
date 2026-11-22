"""Adverse controls for the offline preloaded release-candidate inventory gate."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
import uuid
import zlib

from preloaded_gate import build, digest, inventory, verify_notices, verify_tree
from hybrid_readback import verify_embedded_fat


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

    def test_qualified_packed_workers_require_their_source_notice(self):
        notice = {"bytes": 1308, "sha256": "aa99af64d67d449bcff124bb9d449e624c5aac78484cbd36de8426a8624db348"}
        for worker in ("1400620be191ec0d6d18e6ff4da363cb6983913115067e3ee424abce5131a32c",
                       "e454fbaf009feba660fba2ed7bf1f187cc18c4f4f8278125a7e8dfb3eb519bf7"):
            with self.subTest(worker=worker):
                records = dict(self.records)
                records["PCPOS/WORKER.ELF"] = {"bytes": 286088, "sha256": worker}
                records["PCPOS/MODEL-NOTICE.txt"] = notice
                with self.assertRaisesRegex(ValueError, "exact ggml runtime notice"):
                    self.approved(records)
                records["PCPOS/RUNTIME-NOTICE.txt"] = {"bytes": 1308, "sha256": "0" * 64}
                with self.assertRaisesRegex(ValueError, "exact ggml runtime notice"):
                    self.approved(records)
                records["PCPOS/RUNTIME-NOTICE.txt"] = notice
                self.assertEqual(self.approved(records), records)

    def test_changed_byte_refused(self):
        (self.root / "PCPOS/TAILOR.ELF").write_bytes(b"Foreground")
        with self.assertRaisesRegex(ValueError, "payload size or digest differs"):
            verify_tree(self.root, self.records, self.model_pin)

    def test_qwen_model_requires_the_exact_publisher_license_with_any_worker(self):
        model = "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785"
        license_record = {"bytes": 11544, "sha256": "5de36594c10839788a8c589443a8ef9d8b8d17c65a1b5807206ae037fc36c6bd"}
        with self.assertRaisesRegex(ValueError, "publisher license"):
            verify_tree(self.root, self.records, model)
        self.records["PCPOS/MODEL-LICENSE.txt"] = {"bytes": 11544, "sha256": "0" * 64}
        with self.assertRaisesRegex(ValueError, "publisher license"):
            verify_tree(self.root, self.records, model)
        self.records["PCPOS/MODEL-LICENSE.txt"] = license_record
        verify_notices(self.records, model)
        with self.assertRaisesRegex(ValueError, "missing=.*MODEL-LICENSE"):
            verify_tree(self.root, self.records, model)

    def test_case_variant_of_runtime_notice_is_refused(self):
        record = {"bytes": 3, "sha256": sha(b"abc")}
        self.records["PCPOS/RUNTIME-NOTICE.txt"] = record
        self.records["PCPOS/runtime-notice.txt"] = record
        with self.assertRaisesRegex(ValueError, "unexpected release path|case-colliding"):
            self.approved(self.records)

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

    def test_cli_reopens_finished_medium_and_keeps_existing_output(self):
        # Exercise the normal invocation with synthetic, non-distributable test data.
        for name in ("PCPOS/MODEL-LICENSE.txt", "PCPOS/MODEL-NOTICE.txt", "PCPOS/RUNTIME-NOTICE.txt"):
            text = ("fixture text for " + name).encode()
            (self.root / name).write_bytes(text)
            self.parts[name] = text
        records = self.make_records()
        inventory_file = self.temp / "expected.json"
        inventory_file.write_text(json.dumps(records, sort_keys=True))
        output = self.temp / "fixture.iso"
        interpreter = [sys.executable] + (["-O"] if sys.flags.optimize else [])
        command = [*interpreter, Path(__file__).with_name("preloaded_gate.py"),
                   self.root, "--inventory", inventory_file, "--inventory-sha256",
                   digest(inventory_file), "--model-sha256", self.model_pin,
                   "--fat-bytes", str(128 << 20), output]
        first_env = dict(os.environ, TZ="UTC0", SOURCE_DATE_EPOCH="1704067200")
        first = subprocess.run(command, env=first_env, capture_output=True, text=True)
        self.assertEqual(first.returncode, 0, first.stderr)
        image_pin = digest(output)
        self.assertIn(image_pin, first.stdout)
        again = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(again.returncode, 0)
        self.assertEqual(digest(output), image_pin)
        # A different approved FAT digest cannot clear the same embedded bytes.
        with self.assertRaisesRegex(ValueError, "embedded FAT differs"):
            verify_embedded_fat(output, 128 << 20, "0" * 64)

        # A fresh build with the same approved bytes must survive a change of builder clock.
        output.unlink()
        second_env = dict(os.environ, TZ="HST10", SOURCE_DATE_EPOCH="1740873600")
        second = subprocess.run(command, env=second_env, capture_output=True, text=True)
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(digest(output), image_pin)

    def test_existing_output_is_never_overwritten(self):
        image = self.temp / "exists.iso"
        image.write_bytes(b"original")
        with self.assertRaises(FileExistsError):
            build(self.root, self.records, self.model_pin, 128 << 20, image)
        self.assertEqual(image.read_bytes(), b"original")

    def test_insufficient_fat_capacity_refused(self):
        with self.assertRaisesRegex(ValueError, "FAT size"):
            build(self.root, self.records, self.model_pin, 1 << 20, self.temp / "unused.iso")


class FinalGptGeometryTests(unittest.TestCase):
    """Synthetic, CRC-correct final images; no claim of a genuine FAT or boot."""

    def fixture(self, second=None, missing_esp_guid=False):
        sector, block = 512, 2048
        last_lba = 4095
        raw = bytearray((last_lba + 1) * sector)
        raw[2048 * sector:2304 * sector] = b"F" * (256 * sector)
        table = bytearray(128 * 128)
        esp_type = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b").bytes_le
        table[0:16] = esp_type
        if not missing_esp_guid:
            table[16:32] = uuid.UUID("11111111-2222-3333-4444-555555555555").bytes_le
        struct.pack_into("<QQ", table, 32, 2048, 2303)
        if second is not None:
            table[128:144] = uuid.UUID("21686148-6449-6e6f-744e-656564454649").bytes_le
            table[144:160] = uuid.UUID("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").bytes_le
            struct.pack_into("<QQ", table, 128 + 32, *second)
        table_crc = zlib.crc32(table)
        for lba, other, table_lba in ((1, last_lba, 2), (last_lba, 1, 4063)):
            raw[table_lba * sector:table_lba * sector + len(table)] = table
            header = bytearray(sector)
            struct.pack_into("<8sIIII", header, 0, b"EFI PART", 0x10000, 92, 0, 0)
            struct.pack_into("<QQQQ", header, 24, lba, other, 34, 4062)
            header[56:72] = uuid.UUID("12345678-1234-5678-1234-567812345678").bytes_le
            struct.pack_into("<QIII", header, 72, table_lba, 128, 128, table_crc)
            struct.pack_into("<I", header, 16, zlib.crc32(header[:92]))
            raw[lba * sector:(lba + 1) * sector] = header

        for lba, kind in ((16, 1), (17, 0), (18, 2), (19, 255)):
            descriptor = bytearray(block)
            descriptor[0] = kind
            descriptor[1:7] = b"CD001\x01"
            if kind == 0:
                descriptor[7:39] = b"EL TORITO SPECIFICATION".ljust(32, b" ")
                struct.pack_into("<I", descriptor, 71, 20)
            if kind == 2:
                descriptor[88:91] = b"%/E"
            raw[lba * block:(lba + 1) * block] = descriptor
        catalog = bytearray(64)
        catalog[0:2] = b"\x01\xef"
        catalog[30:34] = b"\x55\xaa\x88\x00"
        struct.pack_into("<H", catalog, 28, (-sum(struct.unpack("<16H", catalog[:32]))) & 0xffff)
        struct.pack_into("<HI", catalog, 38, 256, 512)
        raw[20 * block:20 * block + len(catalog)] = catalog
        image = self.folder / ("gpt-" + str(second) + str(missing_esp_guid) + ".iso")
        image.write_bytes(raw)
        return image, sha(b"F" * (256 * sector))

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.folder = Path(temp.name)

    def test_exactly_one_esp_readback(self):
        image, pin = self.fixture()
        self.assertEqual(verify_embedded_fat(image, 131072, pin), (1048576, 131072))

    def test_bounded_nonoverlapping_extra_partition_is_valid(self):
        image, pin = self.fixture(second=(2304, 2400))
        self.assertEqual(verify_embedded_fat(image, 131072, pin), (1048576, 131072))

    def test_invalid_active_partitions_refused_with_valid_gpt_crcs(self):
        for label, bounds, refusal in (
            ("overlap", (2200, 2350), "overlapping active GPT partitions"),
            ("outside_usable", (4063, 4075), "outside usable range"),
        ):
            with self.subTest(label=label):
                image, pin = self.fixture(second=bounds)
                with self.assertRaisesRegex(ValueError, refusal):
                    verify_embedded_fat(image, 131072, pin)

    def test_esp_without_unique_guid_refused(self):
        image, pin = self.fixture(missing_esp_guid=True)
        with self.assertRaisesRegex(ValueError, "missing or duplicated active GPT partition GUID"):
            verify_embedded_fat(image, 131072, pin)


if __name__ == "__main__":
    unittest.main()
