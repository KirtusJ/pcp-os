#!/usr/bin/env python3
"""Known-answer and adverse receipts for a synthetic normalized-kernel boot."""
from pathlib import Path
import tempfile
import unittest
from check_stripped_kernel_boot import observe, KINDS


class TestStrippedNativeBootReceipts(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        for k in KINDS:
            self.put(k+".raw.sha256",("a" if k!="candidate_off" else "b")*64)
            self.put(k+".raw.measurement","c"*64)
            self.put(k+".stripped.sha256","d"*64)
            self.put(k+".stripped.measurement","c"*64)
            self.put(k+".stripped.size","510896")
        for k in ("base_a","candidate_off"):
            self.put(k+".exit","33")
            self.put(k+".serial",(b"running image SHA-256: "+b"c"*64
                                +b" (measured; equal to the loader)\n"
                                +b"ALL SELF-TESTS PASS\n"))

    def put(self,name,data):
        (self.root/name).write_bytes(data if isinstance(data,bytes) else data.encode())

    def test_positive_contract(self):
        result=observe(self.root)
        self.assertEqual(result["disposition"],"OBSERVED_IN_EXACT_CONTRACT")
        self.assertTrue(all(result["checks"].values()))

    def test_original_a_a_not_hidden_by_normalization(self):
        self.put("base_b.raw.sha256","f"*64)
        result=observe(self.root)
        self.assertFalse(result["checks"]["raw_elf_reproducible_AA"])
        self.assertEqual(result["disposition"],"NOT_ESTABLISHED")

    def test_changed_stripped_candidate_rejected(self):
        self.put("candidate_off.stripped.sha256","1"*64)
        self.assertEqual(observe(self.root)["disposition"],"NOT_ESTABLISHED")

    def test_changed_runtime_measurement_detected(self):
        self.put("candidate_off.stripped.measurement","2"*64)
        result=observe(self.root)
        self.assertFalse(result["checks"]["image_measurement_original_equals_stripped_and_successor"])
        self.assertFalse(result["checks"]["both_guest_image_pins_match_their_stripped_sources"])

    def test_bad_boot_exit_detected(self):
        self.put("candidate_off.exit","124")
        result=observe(self.root)
        self.assertFalse(result["checks"]["both_stripped_elf_guests_pass_selftests_and_exit_33"])

    def test_unobserved_serial_receipt_detected(self):
        self.put("candidate_off.serial",b"ALL SELF-TESTS PASS\n")
        result=observe(self.root)
        self.assertFalse(result["checks"]["both_guest_image_pins_match_their_stripped_sources"])

    def test_diagnostic_export_marker_detected(self):
        p=self.root/"candidate_off.serial"
        p.write_bytes(p.read_bytes()+b"D10_REGISTER_RAW_BEGIN\n")
        result=observe(self.root)
        self.assertFalse(result["checks"]["both_guest_serials_lack_diagnostic_export_markers"])

    def test_invalid_digest_and_missing_file_detected(self):
        self.put("candidate_off.raw.measurement","not-a-sha")
        with self.assertRaisesRegex(ValueError,"invalid SHA-256"):
            observe(self.root)
        (self.root/"candidate_off.raw.measurement").unlink()
        with self.assertRaises(OSError):
            observe(self.root)

    def test_no_false_pass_if_raw_and_stripped_identity_mismatch(self):
        self.put("candidate_off.raw.sha256","a"*64)
        result=observe(self.root)
        self.assertFalse(result["checks"]["source_original_raw_elf_differs"])
        self.assertEqual(result["disposition"],"NOT_ESTABLISHED")


if __name__=="__main__":
    unittest.main()
