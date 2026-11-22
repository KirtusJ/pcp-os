"""Check the model payload's on-disk boundaries independently of its whole-file SHA-256."""
import hashlib
from pathlib import Path
import struct
import tempfile
import unittest

from model_payload import readback


class ShardReadback(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.folder = Path(temp.name)
        self.a = self.folder / "shard-00000.bin"
        self.b = self.folder / "shard-00001.bin"
        first, last = b"A" * 1024, b"tail"
        self.a.write_bytes(first)
        self.b.write_bytes(last)
        model = first + last
        self.model_pin = hashlib.sha256(model).hexdigest()
        manifest = b"PCPMDL01" + struct.pack("<QII", len(model), len(first), 2)
        for data in (model, first, last):
            manifest += hashlib.sha256(data).digest()
        (self.folder / "model.manifest").write_bytes(manifest)
        self.manifest_pin = hashlib.sha256(manifest).hexdigest()

    def read(self):
        return readback(self.folder, self.model_pin, self.manifest_pin)

    def test_complete_inventory(self):
        self.assertEqual(set(self.read()), {
            "PCPOS/model.manifest", "PCPOS/shard-00000.bin", "PCPOS/shard-00001.bin"})

    def test_changed_boundary_with_intact_concatenation(self):
        first = self.a.read_bytes()
        self.a.write_bytes(first[:-1])
        self.b.write_bytes(first[-1:] + self.b.read_bytes())
        with self.assertRaisesRegex(ValueError, "shard-00000.bin"):
            self.read()

    def test_changed_last_byte(self):
        self.b.write_bytes(b"taiL")
        with self.assertRaisesRegex(ValueError, "shard-00001.bin"):
            self.read()

    def test_extra_or_missing_shard(self):
        extra = self.folder / "shard-00002.bin"
        extra.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "count"):
            self.read()
        extra.unlink()
        self.b.unlink()
        with self.assertRaisesRegex(ValueError, "count"):
            self.read()


if __name__ == "__main__":
    unittest.main()
