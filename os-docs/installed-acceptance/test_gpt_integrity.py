"""Synthetic GPT failures do not require FAT tooling, a model, or QEMU."""
import os
from pathlib import Path
import struct
import tempfile
import unittest
import uuid
import zlib

from gpt_integrity import require_gpt_mirror

SECTORS = 32768
ENTRY_LEN = 128 * 128


def make_fixture(path, *, backup_name=None):
    guid = uuid.uuid4().bytes_le
    entries = bytearray(ENTRY_LEN)
    name = "PCP OS models".encode("utf-16le").ljust(72, b"\0")
    entries[:128] = struct.pack("<16s16sQQQ72s",
                                uuid.UUID("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7").bytes_le,
                                uuid.uuid4().bytes_le, 2048, 8191, 0, name)
    with path.open("wb") as fd:
        fd.truncate(SECTORS * 512)
        for current, alternate, table_lba in ((1, SECTORS - 1, 2),
                                             (SECTORS - 1, 1, SECTORS - 33)):
            table = entries[:]
            if current != 1 and backup_name is not None:
                table[56:56 + len(backup_name.encode("utf-16le"))] = backup_name.encode("utf-16le")
            entry_crc = zlib.crc32(table)
            header = bytearray(struct.pack("<8sIIIIQQQQ16sQIII", b"EFI PART", 0x10000, 92,
                                           0, 0, current, alternate, 34, SECTORS - 34,
                                           guid, table_lba, 128, 128, entry_crc))
            struct.pack_into("<I", header, 16, zlib.crc32(header))
            fd.seek(current * 512)
            fd.write(header)
            fd.seek(table_lba * 512)
            fd.write(table)


class GptMirrorTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.disk = Path(tmp.name) / "disk.raw"
        make_fixture(self.disk)

    def validate(self):
        with self.disk.open("rb") as fd:
            require_gpt_mirror(fd.fileno(), os.fstat(fd.fileno()).st_size)

    def mutate(self, at):
        with self.disk.open("r+b") as fd:
            fd.seek(at)
            original = fd.read(1)
            fd.seek(at)
            fd.write(bytes([original[0] ^ 0x40]))

    def test_valid_pair(self):
        self.validate()

    def test_damaged_primary_header(self):
        self.mutate(512)
        with self.assertRaisesRegex(ValueError, "signature"):
            self.validate()

    def test_damaged_backup_header(self):
        self.mutate((SECTORS - 1) * 512)
        with self.assertRaisesRegex(ValueError, "signature"):
            self.validate()

    def test_damaged_backup_table(self):
        self.mutate((SECTORS - 33) * 512 + 7)
        with self.assertRaisesRegex(ValueError, "entry CRC"):
            self.validate()

    def test_distinct_valid_entry_arrays(self):
        make_fixture(self.disk, backup_name="PCP OS altered")
        with self.assertRaisesRegex(ValueError, "entries_crc disagree"):
            self.validate()

    def test_backup_header_aliases_primary_table(self):
        with self.disk.open("r+b") as fd:
            fd.seek((SECTORS - 1) * 512)
            hdr = bytearray(fd.read(92))
            struct.pack_into("<Q", hdr, 72, 2)
            struct.pack_into("<I", hdr, 16, 0)
            struct.pack_into("<I", hdr, 16, zlib.crc32(hdr))
            fd.seek((SECTORS - 1) * 512)
            fd.write(hdr)
        with self.assertRaisesRegex(ValueError, "distinct primary/backup"):
            self.validate()

    def test_unsupported_sector_size_or_truncated(self):
        with self.disk.open("r+b") as fd:
            fd.truncate(SECTORS * 512 - 1)
        with self.assertRaisesRegex(ValueError, "512-byte-sector"):
            self.validate()


if __name__ == "__main__":
    unittest.main()
