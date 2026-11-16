"""Valid mirrored GPT CRCs do not establish a valid active partition map."""
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
TYPE = uuid.UUID("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7").bytes_le
A = uuid.UUID("c0a00000-0000-0000-0000-000000000001").bytes_le
B = uuid.UUID("c0a00000-0000-0000-0000-000000000002").bytes_le


def entry(guid, first, last, *, type_guid=TYPE):
    return struct.pack("<16s16sQQQ72s", type_guid, guid, first, last, 0, bytes(72))


def fixture(path, entries):
    array = bytearray(ENTRY_LEN)
    for index, member in enumerate(entries):
        array[index * 128:(index + 1) * 128] = member
    with path.open("wb") as disk:
        disk.truncate(SECTORS * 512)
        for here, other, table in ((1, SECTORS - 1, 2), (SECTORS - 1, 1, SECTORS - 33)):
            header = bytearray(struct.pack(
                "<8sIIIIQQQQ16sQIII",
                b"EFI PART", 0x10000, 92, 0, 0, here, other, 34, SECTORS - 34,
                uuid.UUID("fc62edba-c6ba-49b1-b416-ac9a1af5b817").bytes_le,
                table, 128, 128, zlib.crc32(array)))
            struct.pack_into("<I", header, 16, zlib.crc32(header))
            disk.seek(here * 512)
            disk.write(header)
            disk.seek(table * 512)
            disk.write(array)


class GptEntrySemanticsTest(unittest.TestCase):
    def check(self, entries, error=None):
        with tempfile.TemporaryDirectory() as tmp:
            disk = Path(tmp) / "synthetic.raw"
            fixture(disk, entries)
            with disk.open("rb") as opened:
                if error:
                    with self.assertRaisesRegex(ValueError, error):
                        require_gpt_mirror(opened.fileno(), os.fstat(opened.fileno()).st_size)
                else:
                    require_gpt_mirror(opened.fileno(), os.fstat(opened.fileno()).st_size)

    def test_disjoint_partitions(self):
        self.check((entry(A, 2048, 8191), entry(B, 8192, 12287)))

    def test_partitions_with_gap(self):
        self.check((entry(A, 2048, 8191), entry(B, 12288, 14000)))

    def test_overlapping_partitions(self):
        self.check((entry(A, 2048, 8191), entry(B, 4096, 10239)), "overlap")

    def test_overlapping_single_sector(self):
        self.check((entry(A, 5000, 5000), entry(B, 5000, 5000)), "overlap")

    def test_adjacent_single_sector(self):
        self.check((entry(A, 5000, 5000), entry(B, 5001, 5001)))

    def test_duplicate_partition_guid(self):
        self.check((entry(A, 2048, 8191), entry(A, 8192, 12287)), "duplicate")

    def test_zero_partition_guid(self):
        self.check((entry(A, 2048, 8191), entry(bytes(16), 8192, 12287)), "zero")

    def test_reversed_partition_bounds(self):
        self.check((entry(A, 2048, 8191), entry(B, 9000, 8999)), "reversed")

    def test_partition_before_first_usable(self):
        self.check((entry(A, 2048, 8191), entry(B, 33, 2047)), "outside")

    def test_partition_after_last_usable(self):
        self.check((entry(A, 2048, 8191), entry(B, 12288, SECTORS - 33)), "outside")

    def test_unused_entry_is_ignored(self):
        self.check((entry(A, 2048, 8191),
                    entry(bytes(16), 1, SECTORS - 1, type_guid=bytes(16))))

    def test_empty_partition_map(self):
        self.check(())


if __name__ == "__main__":
    unittest.main()
