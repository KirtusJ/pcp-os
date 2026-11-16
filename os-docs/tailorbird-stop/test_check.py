"""Negative controls for the cancellation trace and application-store reader."""
import hashlib
from pathlib import Path
import struct
import tempfile
import unittest

import check


def write_slot(region, sector, generation, payload):
    pos = sector * check.SECTOR
    header = b"PCPSLOT1" + struct.pack("<QI", generation, len(payload)) + hashlib.sha256(payload).digest()
    region[pos:pos + 84] = header + hashlib.sha256(header).digest()
    start = pos + check.SECTOR
    region[start:start + len(payload)] = payload


def entry(name, source):
    return struct.pack("<HI", len(name), len(source)) + name + source


class StopOracles(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / "fixture"

    def test_trace_requires_exact_order_and_whole_events(self):
        expected = [check.STAGED, check.SAVED, check.STAGED, check.ACCEPTED, check.STOPPED,
                    check.STAGED, check.ACCEPTED, check.STOPPED, check.RAN]
        self.path.write_bytes(b"\r\n".join(expected) + b"\r\n")
        check.check_trace(self.path, expected)
        cases = {
            "early read": expected[:4] + [check.RAN] + expected[4:],
            "extra save": expected[:5] + [check.SAVED] + expected[5:],
            "missing explicit read": expected[:-1],
            "look-alike marker": expected[:-1] + [check.RAN + b" FAILED"],
        }
        for name, altered in cases.items():
            with self.subTest(case=name), self.assertRaises(ValueError):
                self.path.write_bytes(b"\r\n".join(altered) + b"\r\n")
                check.check_trace(self.path, expected)

    def test_active_generation_requires_exact_path_and_bytes(self):
        region = bytearray(check.SECTOR * (1 + 2 * check.SLOT_SECTORS))
        label = b"PCPOS USER STORE 1\n"
        region[:len(label)] = label
        source = b"print original"
        approved = entry(b"apps/lark.txt", source)
        write_slot(region, 1, 2, approved)
        self.path.write_bytes(region)
        self.assertEqual(check.checkpoint(self.path, 0, source)["generation"], 2)
        cases = [
            ("draft only", 3, entry(b"tailor-drafts-v1/lark", source)),
            ("stale bytes", 3, entry(b"apps/lark.txt", b"print changed")),
            ("duplicate generation", 2, approved),
        ]
        for name, generation, payload in cases:
            with self.subTest(case=name), self.assertRaises(ValueError):
                altered = region.copy()
                write_slot(altered, 1 + check.SLOT_SECTORS, generation, payload)
                self.path.write_bytes(altered)
                check.checkpoint(self.path, 0, source)
        for offset in (check.SECTOR + 32, check.SECTOR + 85, check.SECTOR * 2):
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                altered = region.copy()
                altered[offset] ^= 1
                self.path.write_bytes(altered)
                check.checkpoint(self.path, 0, source)


if __name__ == "__main__":
    unittest.main()
