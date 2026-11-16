"""Synthetic controls for the read-only ELF conversion audit.

Synthetic fixtures are deliberately *not* records of a PCP OS kernel build/boot.
Run: python3 -m unittest discover -s tests -p test_boot_image_audit.py
"""
import sys
from pathlib import Path
import struct
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import boot_image_audit as tool


NAMES = b"\0.boot\0.text\0.rodata\0.data\0.bss\0.shstrtab\0"
SECTIONS = (
    (".boot", 0x100000, 0x1000, 0x1000, 1),
    (".text", 0x101000, 0x2000, 0x1000, 1),
    (".rodata", 0x102000, 0x3000, 0x1000, 1),
    (".data", 0x103000, 0x4000, 0x10, 1),
    (".bss", 0x104000, 0x5000, 0x1000, 8),
)


def fixture(bits, entry=0x10000C, header_off=0x1000):
    raw = bytearray(0x6000)
    raw[:16] = b"\x7fELF" + bytes([2 if bits == 64 else 1, 1, 1]) + bytes(9)
    struct.pack_into("<HHI", raw, 16, 2, 62 if bits == 64 else 3, 1)
    if bits == 64:
        struct.pack_into("<Q", raw, 24, entry)
        struct.pack_into("<Q", raw, 32, 0x40)
        struct.pack_into("<HH", raw, 54, 56, 4)
        struct.pack_into("<Q", raw, 40, 0x5500)
        struct.pack_into("<HHH", raw, 58, 64, 7, 6)
    else:
        struct.pack_into("<I", raw, 24, entry)
        struct.pack_into("<I", raw, 28, 0x34)
        struct.pack_into("<HH", raw, 42, 32, 4)
        struct.pack_into("<I", raw, 32, 0x5500)
        struct.pack_into("<HHH", raw, 46, 40, 7, 6)
    raw[0x1000:0x2000] = b"\x90" * 0x1000
    raw[0x2000:0x3000] = b"\x90" * 0x1000
    raw[0x3000:0x4000] = b"R" * 0x1000
    raw[0x4000:0x4010] = b"D" * 16
    struct.pack_into("<III", raw, header_off, tool.MAGIC, 3, (-tool.MAGIC - 3) & 0xFFFFFFFF)
    # Same semantically loaded segments in ELF64 and its converted ELF32 wrapper.
    segments = [
        (0x1000, 0x100000, 0x1000, 0x1000, 7),
        (0x2000, 0x101000, 0x1000, 0x1000, 5),
        (0x3000, 0x102000, 0x1000, 0x1000, 4),
        (0x4000, 0x103000, 0x10, 0x2000, 6),
    ]
    for i, (offset, addr, filesz, memsz, flags) in enumerate(segments):
        if bits == 64:
            struct.pack_into("<IIQQQQQQ", raw, 0x40 + i * 56,
                             1, flags, offset, addr, addr, filesz, memsz, 4096)
        else:
            struct.pack_into("<IIIIIIII", raw, 0x34 + i * 32,
                             1, offset, addr, addr, filesz, memsz, flags, 4096)
    raw[0x5300:0x5300 + len(NAMES)] = NAMES
    all_sections = [("", 0, 0, 0, 0)] + list(SECTIONS) + [(".shstrtab", 0, 0x5300, len(NAMES), 3)]
    for i, (name, addr, off, size, typ) in enumerate(all_sections):
        noff = NAMES.index(name.encode() + b"\0") if name else 0
        cursor = 0x5500 + i * (64 if bits == 64 else 40)
        if bits == 64:
            struct.pack_into("<IIQQQQIIQQ", raw, cursor, noff, typ, 0, addr, off, size, 0, 0, 0, 0)
        else:
            struct.pack_into("<IIIIIIIIII", raw, cursor, noff, typ, 0, addr, off, size, 0, 0, 0, 0)
    return bytes(raw)


class AuditTests(unittest.TestCase):
    def setUp(self):
        self.source = fixture(64)
        self.target = fixture(32)

    def test_valid_structural_pair(self):
        result = tool.audit(self.source, self.target)
        self.assertTrue(result["result"].startswith("PASS: STRUCTURAL ONLY"))
        self.assertEqual(result["multiboot_header_file_offset"], 4096)
        self.assertEqual(len(result["linked_elf64_sha256"]), 64)
        self.assertEqual(result["compared_load_segments"], 4)

    def test_reject_two_elf64_files(self):
        with self.assertRaisesRegex(tool.AuditError, "ELF class"):
            tool.audit(self.source, self.source)

    def test_reject_wrong_machine(self):
        t = bytearray(self.target)
        struct.pack_into("<H", t, 18, 62)
        with self.assertRaisesRegex(tool.AuditError, "e_machine"):
            tool.audit(self.source, bytes(t))

    def test_reject_altered_pt_load_address(self):
        t = bytearray(self.target)
        struct.pack_into("<I", t, 0x34 + 12, 0x200000)  # first PT_LOAD physical address
        with self.assertRaisesRegex(tool.AuditError, "PT_LOAD mapping"):
            tool.audit(self.source, bytes(t))

    def test_reject_missing_program_headers(self):
        t = bytearray(self.target)
        struct.pack_into("<H", t, 44, 0)
        with self.assertRaisesRegex(tool.AuditError, "program header"):
            tool.audit(self.source, bytes(t))

    def test_reject_entry_without_executable_segment(self):
        s, t = bytearray(self.source), bytearray(self.target)
        struct.pack_into("<I", s, 0x40 + 4, 6)  # drop execute permission from ELF64 boot segment
        struct.pack_into("<I", t, 0x34 + 24, 6)  # same in ELF32 wrapper
        with self.assertRaisesRegex(tool.AuditError, "not covered by an executable"):
            tool.audit(bytes(s), bytes(t))

    def test_reject_lost_multiboot_header(self):
        t = bytearray(self.target)
        struct.pack_into("<I", t, 0x1000, 0)
        with self.assertRaisesRegex(tool.AuditError, "changed .boot"):
            tool.audit(self.source, bytes(t))

    def test_reject_corrupt_multiboot_checksum_in_both_files(self):
        s, t = bytearray(self.source), bytearray(self.target)
        struct.pack_into("<I", s, 0x1008, 1)
        struct.pack_into("<I", t, 0x1008, 1)
        with self.assertRaisesRegex(tool.AuditError, "checksum"):
            tool.audit(bytes(s), bytes(t))

    def test_reject_truncated_section_table(self):
        with self.assertRaisesRegex(tool.AuditError, "section table"):
            tool.elf(self.target[:0x5500], 1, 3)

    def test_reject_changed_text(self):
        t = bytearray(self.target)
        t[0x2222] ^= 1
        with self.assertRaisesRegex(tool.AuditError, "changed .text"):
            tool.audit(self.source, bytes(t))

    def test_reject_different_entry(self):
        t = bytearray(self.target)
        struct.pack_into("<I", t, 24, 0x100010)
        with self.assertRaisesRegex(tool.AuditError, "entry point"):
            tool.audit(self.source, bytes(t))

    def test_reject_header_outside_first_8192_bytes(self):
        s, t = bytearray(self.source), bytearray(self.target)
        for b in (s, t):
            b[0x1000:0x100C] = b"\x90" * 12
            struct.pack_into("<III", b, 0x2000, tool.MAGIC, 3, (-tool.MAGIC - 3) & 0xFFFFFFFF)
        with self.assertRaisesRegex(tool.AuditError, "Multiboot 1 header"):
            tool.audit(bytes(s), bytes(t))

    def test_reject_duplicate_header(self):
        s, t = bytearray(self.source), bytearray(self.target)
        for b in (s, t):
            struct.pack_into("<III", b, 0x1100, tool.MAGIC, 3, (-tool.MAGIC - 3) & 0xFFFFFFFF)
        with self.assertRaisesRegex(tool.AuditError, "found 2"):
            tool.audit(bytes(s), bytes(t))

    def test_reject_unbounded_file(self):
        with self.assertRaisesRegex(tool.AuditError, "128 MiB"):
            tool.audit(b"a" * (tool.MAX_SIZE + 1), self.target)


if __name__ == "__main__":
    unittest.main()
