#!/usr/bin/env python3
"""Positive/adverse fixtures for the bounded ELF PT_LOAD equality comparator."""
import struct
import tempfile
from pathlib import Path
import unittest
from compare_load_segments import map_segments, compare


def elf32(payload=b"ABCD", offset=128, vaddr=0x1000, flags=5):
    data=bytearray(256)
    data[:16]=b"\x7fELF\x01\x01\x01"+bytes(9)
    struct.pack_into("<HHIIIIIHHHHHH",data,16,2,3,1,0x1000,64,0,0,52,32,1,40,0,0)
    struct.pack_into("<IIIIIIII",data,64,1,offset,vaddr,vaddr,len(payload),4096,flags,4096)
    data[offset:offset+len(payload)]=payload
    return bytes(data)


def elf64(payload=b"ABCD"):
    data=bytearray(320)
    data[:16]=b"\x7fELF\x02\x01\x01"+bytes(9)
    struct.pack_into("<HHIQQQIHHHHHH",data,16,2,62,1,0x1000,64,0,0,64,56,1,64,0,0)
    struct.pack_into("<IIQQQQQQ",data,64,1,5,160,0x1000,0x1000,len(payload),4096,4096)
    data[160:160+len(payload)]=payload
    return bytes(data)


class LoaderSegmentControls(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)

    def parse(self,name,b):
        p=self.root/name
        p.write_bytes(b)
        return map_segments(p)

    def test_identical_elf32_loads_and_entry(self):
        x=self.parse("a",elf32())
        result=compare(x,x)
        self.assertTrue(result["whole_ELF_identical"])
        self.assertTrue(result["PT_LOAD_memory_contracts_and_payloads_identical"])
        self.assertTrue(result["every_program_header_content_and_offset_identical"])
        self.assertEqual(result["PT_LOAD_count"],1)

    def test_elf64_load_parsed(self):
        x=self.parse("x",elf64())
        self.assertEqual(x["class"],64)
        self.assertEqual(x["load_count"],1)
        self.assertTrue(compare(x,x)["entry_point_identical"])

    def test_changed_loaded_bytes_detected(self):
        x=self.parse("a",elf32())
        y=self.parse("b",elf32(payload=b"ABCE"))
        self.assertFalse(compare(x,y)["PT_LOAD_memory_contracts_and_payloads_identical"])

    def test_unloaded_padding_does_not_change_load_contract(self):
        a=bytearray(elf32())
        b=bytearray(a)
        b[210]=5
        x=self.parse("a",a)
        y=self.parse("b",b)
        z=compare(x,y)
        self.assertTrue(z["PT_LOAD_memory_contracts_and_payloads_identical"])
        self.assertFalse(z["whole_ELF_identical"])

    def test_equal_file_load_at_changed_offset_detects_header_difference(self):
        x=self.parse("a",elf32())
        y=self.parse("b",elf32(offset=160))
        r=compare(x,y)
        self.assertTrue(r["PT_LOAD_memory_contracts_and_payloads_identical"])
        self.assertFalse(r["PT_LOAD_offsets_identical"])
        self.assertFalse(r["every_program_header_content_and_offset_identical"])

    def test_flags_and_virtual_addresses_are_bounded(self):
        x=self.parse("a",elf32())
        for name,kwargs in [("different-flags",{"flags":4}),("different-address",{"vaddr":0x2000})]:
            with self.subTest(name=name):
                y=self.parse("b",elf32(**kwargs))
                self.assertFalse(compare(x,y)["PT_LOAD_memory_contracts_and_payloads_identical"])

    def test_malformed_load_header_fails(self):
        with self.assertRaisesRegex(ValueError,"supported ELF"):
            self.parse("invalid",b"bad")
        raw=bytearray(elf32())
        struct.pack_into("<I",raw,64+16,100_000) # filesz > memory and past bytes
        with self.assertRaises(ValueError):
            self.parse("overflow",raw)


if __name__=="__main__":
    unittest.main()
