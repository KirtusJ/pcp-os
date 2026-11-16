#!/usr/bin/env python3
"""Known-answer and adverse tests for the isolated ELF section comparator."""
import struct
import tempfile
from pathlib import Path
import unittest

from compare_elf_sections import compare, sections


def elf32(text=b"\x90\xc3", rodata=b"ABCD"):
    names=b"\x00.shstrtab\x00.text\x00.rodata\x00"
    blob=bytearray(0x100+4*40)
    blob[:16]=b"\x7fELF\x01\x01\x01"+bytes(9)
    struct.pack_into("<HHIIIIIHHHHHH",blob,16,2,3,1,0,0,0x100,0,52,0,0,40,4,1)
    blob[64:64+len(names)]=names
    offtext=128
    offrodata=160
    blob[offtext:offtext+len(text)]=text
    blob[offrodata:offrodata+len(rodata)]=rodata
    hdr=0x100
    struct.pack_into("<IIIIIIIIII",blob,hdr+40,1,3,0,0,64,len(names),0,0,1,0)
    struct.pack_into("<IIIIIIIIII",blob,hdr+80,11,1,6,0,offtext,len(text),0,0,1,0)
    struct.pack_into("<IIIIIIIIII",blob,hdr+120,17,1,2,0,offrodata,len(rodata),0,0,1,0)
    return bytes(blob)


class SectionComparatorControls(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)

    def parse(self,name,data):
        path=self.root/name
        path.write_bytes(data)
        return sections(path)

    def test_identical_image_all_sections_match(self):
        x=self.parse("base",elf32())
        result=compare(x,x)
        self.assertTrue(result["whole_elf_bytes_identical"])
        self.assertTrue(result["allocated_section_payloads_identical"])
        self.assertEqual(result["differences"],[])
        self.assertEqual(x["elf_class"],32)

    def test_rodata_only_change_is_allocated(self):
        x=self.parse("base",elf32())
        y=self.parse("other",elf32(rodata=b"ABCE"))
        result=compare(x,y)
        self.assertEqual([z["name"] for z in result["differences"]],[".rodata"])
        self.assertFalse(result["allocated_section_payloads_identical"])
        self.assertFalse(result["whole_elf_bytes_identical"])

    def test_text_only_change_is_allocated(self):
        x=self.parse("base",elf32())
        y=self.parse("other",elf32(text=b"\x90\x90"))
        result=compare(x,y)
        self.assertEqual([z["name"] for z in result["differences"]],[".text"])

    def test_non_section_padding_can_change_entire_elf(self):
        raw=bytearray(elf32())
        x=self.parse("base",raw)
        raw[220]=13
        y=self.parse("other",raw)
        result=compare(x,y)
        self.assertFalse(result["whole_elf_bytes_identical"])
        self.assertTrue(result["all_section_payloads_identical"])
        self.assertEqual(result["differences"],[])

    def test_malformed_elf_and_section_bounds_rejected(self):
        with self.assertRaisesRegex(ValueError,"invalid ELF"):
            self.parse("bad",b"BAD").get("sections")
        blob=bytearray(elf32())
        struct.pack_into("<I",blob,0x100+120+20,999999)  # rodata sh_size
        with self.assertRaisesRegex(ValueError,"section data extends"):
            self.parse("invalid",blob)

    def test_section_layout_drift_is_not_silently_ignored(self):
        raw=bytearray(elf32())
        struct.pack_into("<I",raw,0x100+120,11)  # wrong .rodata sh_name
        x=self.parse("base",elf32())
        y=self.parse("other",raw)
        with self.assertRaisesRegex(ValueError,"topology"):
            compare(x,y)


if __name__=="__main__":
    unittest.main()
