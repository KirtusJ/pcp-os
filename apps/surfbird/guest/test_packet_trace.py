import struct
import tempfile
import unittest
from pathlib import Path

from packet_trace import verify


class GrantTraceTest(unittest.TestCase):
    def test_denied_link_must_be_quiet_even_for_a_spoofed_source(self):
        header = struct.pack("<IHHIIII", 0xA1B2C3D4, 2, 4, 0, 0, 65535, 1)
        frame = bytes.fromhex("ffffffffffff 02abcdef1234 88b5") + bytes(46)
        record = struct.pack("<IIII", 0, 0, len(frame), len(frame))
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / "denied.pcap"
            capture.write_bytes(header)
            self.assertEqual(verify(capture, 80, [], "denied", {})["frames"], 0)
            capture.write_bytes(header + record + frame)
            with self.assertRaisesRegex(ValueError, "empty capture"):
                verify(capture, 80, [], "denied", {})
