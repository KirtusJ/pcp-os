#!/usr/bin/env python3
"""Synthetic bounded verdict controls; this does not execute objcopy or QEMU."""
from pathlib import Path
import tempfile
import unittest
from compare_stripped_images import observe

ELF=b"\x7fELF\x01\x01\x01\x00" + b"\x00"*32

class TestStrippedIdentityVerdict(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        self.write("base_1",ELF)
        self.write("base_2",ELF)
        self.write("candidate_off",ELF)

    def write(self,kind,value):
        (self.root/(kind+".stripped.elf32")).write_bytes(value)

    def test_exact_positive(self):
        result=observe(self.root)
        self.assertEqual(result["disposition"],"STRIPPED_IDENTICAL_IN_SCOPE")
        self.assertTrue(result["A_A_stripped_image_reproducible"])

    def test_candidate_changed_payload_remains_distinct(self):
        self.write("candidate_off",ELF+b"DIFFERENT LOADED DATA")
        result=observe(self.root)
        self.assertEqual(result["disposition"],"STRIPPED_DIFFERENT_IN_SCOPE")
        self.assertFalse(result["candidate_stripped_ELF_byte_identical_to_baseline"])

    def test_base_reproducibility_failure_blocks_comparison(self):
        self.write("base_2",ELF+b"BASE DRIFT")
        result=observe(self.root)
        self.assertEqual(result["disposition"],"A_A_UNSTABLE")
        self.assertIsNone(result["candidate_stripped_ELF_byte_identical_to_baseline"])

    def test_truncated_or_missing_file_fails(self):
        self.write("candidate_off",b"NOT ELF")
        with self.assertRaisesRegex(ValueError,"invalid stripped ELF"):
            observe(self.root)
        (self.root/"base_1.stripped.elf32").unlink()
        with self.assertRaisesRegex(ValueError,"missing or invalid"):
            observe(self.root)

if __name__=="__main__":
    unittest.main()
