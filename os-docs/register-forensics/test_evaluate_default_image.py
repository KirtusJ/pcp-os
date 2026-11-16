#!/usr/bin/env python3
"""Known-valid and known-invalid controls for the D11 image-identity checker."""
import tempfile
from pathlib import Path
import unittest

from evaluate_default_image import judge, load_observation, LABELS


def image(elf, measured):
    return {"elf_sha256": elf*64, "measured_image_sha256": measured*64}


def good():
    # Distinct opt-in image and two exactly repeated control builds.
    return {"base_1": image("a", "b"), "base_2": image("a", "b"),
            "candidate_off": image("a", "b"), "candidate_on": image("c", "d")}


def serial_for(digest="b"):
    return (b"running image SHA-256: " + digest.encode()*64
            + b" (measured; equal to the value the loader named)\n"
            + b"ALL SELF-TESTS PASS\n")


class ImageIdentityOracleControls(unittest.TestCase):
    def test_complete_valid_identity(self):
        result=judge(good(),33,serial_for())
        self.assertEqual(result["disposition"],"BYTE_IDENTICAL_IN_EXACT_BUILD_CONTRACT")
        self.assertTrue(all(result["checks"].values()))

    def test_irreproducible_baseline_is_incomparable(self):
        images=good()
        images["base_2"]=image("9","b")
        self.assertEqual(judge(images,33,serial_for())["disposition"],"INCOMPARABLE_BUILD_CONTROL")
        images["base_2"]=image("a","9")
        self.assertEqual(judge(images,33,serial_for())["disposition"],"INCOMPARABLE_BUILD_CONTROL")

    def test_changed_default_measured_code_fails(self):
        images=good()
        images["candidate_off"]=image("c","9")
        self.assertEqual(judge(images,33,serial_for("9"))["disposition"],"DEFAULT_MEASURED_CODE_DIFFERS")

    def test_changed_elf_with_same_measured_code_is_distinguished(self):
        images=good()
        images["candidate_off"]=image("9","b")
        self.assertEqual(judge(images,33,serial_for())["disposition"],
                         "DEFAULT_ELF_BYTES_DIFFER_BUT_MEASUREMENT_MATCHES")

    def test_opt_in_not_separated_fails(self):
        images=good()
        images["candidate_on"]=image("c","b")
        self.assertEqual(judge(images,33,serial_for())["disposition"],
                         "FEATURE_SEPARATION_NOT_ESTABLISHED")

    def test_bad_guest_exit_or_wrong_serial_fails(self):
        images=good()
        for exit_code, serial in ((0,serial_for()), (33,b"ALL SELF-TESTS PASS\n"),
                                  (33,serial_for()+b"  FAIL in one self-test\n"),
                                  (33,serial_for("c"))):
            with self.subTest(exit_code=exit_code, serial=serial):
                self.assertEqual(judge(images,exit_code,serial)["disposition"],
                                 "DEFAULT_GUEST_SELFTEST_NOT_ACCEPTED")

    def test_missing_or_malformed_digest_file_is_not_silent(self):
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)
            for label in LABELS:
                (path/(label+".elf_sha256")).write_text("a"*64+"\n")
                (path/(label+".measurement")).write_text("b"*64+"\n")
            self.assertEqual(len(load_observation(path)),4)
            (path/"candidate_on.elf_sha256").write_text("not-a-digest")
            with self.assertRaisesRegex(ValueError,"invalid SHA-256"):
                load_observation(path)
            (path/"candidate_on.elf_sha256").unlink()
            with self.assertRaisesRegex(ValueError,"missing hash"):
                load_observation(path)


if __name__ == "__main__":
    unittest.main()
