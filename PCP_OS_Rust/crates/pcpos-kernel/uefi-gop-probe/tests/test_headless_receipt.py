"""Adverse fixtures for an exact no-GOP EFI_NOT_FOUND observation.

The real QEMU log is separately evaluated in the workflow; these are synthetic
mutations intended to falsify the acceptance oracle, not evidence of firmware.
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from headless_receipt import EvidenceMismatch, check_headless_receipt, EFI_NOT_FOUND

ENTRY = "PCPOS_EFI_ENTRY_V1"
STATUS = f"PCPOS_GOP_PROBE_V1 NO_GOP status={EFI_NOT_FOUND}"
RETURN = f"PCPOS_EFI_RETURN_V1 {EFI_NOT_FOUND}"
GOOD = "\r\n".join((ENTRY, STATUS, RETURN, ""))


class ExactNoGopOracleTests(unittest.TestCase):
    def assert_rejected(self, raw: str, exit_code: int = 124):
        with self.assertRaises(EvidenceMismatch):
            check_headless_receipt(raw, exit_code)

    def test_accepts_exact_not_found_with_bounded_qemu_timeout(self):
        check_headless_receipt(GOOD, 124)

    def test_accepts_explicit_qemu_normal_exit_with_same_efi_failure(self):
        check_headless_receipt(GOOD, 0)

    def test_rejects_no_gop_line_with_efi_success(self):
        self.assert_rejected(GOOD.replace(STATUS, "PCPOS_GOP_PROBE_V1 NO_GOP status=0x0000000000000000"))

    def test_rejects_efi_return_success_after_correct_not_found(self):
        self.assert_rejected(GOOD.replace(RETURN, "PCPOS_EFI_RETURN_V1 0x0000000000000000"))

    def test_rejects_correct_return_after_incorrect_no_gop_status(self):
        self.assert_rejected(GOOD.replace(STATUS, "PCPOS_GOP_PROBE_V1 NO_GOP status=0x8000000000000003"))

    def test_rejects_consistent_but_wrong_efi_unsupported_status_and_return(self):
        bad = GOOD.replace(EFI_NOT_FOUND, "0x8000000000000003")
        self.assert_rejected(bad)

    def test_rejects_duplicate_correct_status_with_an_extra_success_line(self):
        bad = GOOD.replace(RETURN, "PCPOS_GOP_PROBE_V1 NO_GOP status=0x0000000000000000\r\n" + RETURN)
        self.assert_rejected(bad)

    def test_rejects_duplicate_correct_return(self):
        self.assert_rejected(GOOD + RETURN + "\r\n")

    def test_rejects_substring_match_spoofing(self):
        self.assert_rejected(GOOD.replace(STATUS, "PREFIX " + STATUS))

    def test_rejects_different_status_with_correct_marker_as_substring(self):
        self.assert_rejected(GOOD.replace(STATUS, STATUS + " trailing-data"))

    def test_rejects_gop_before_mode_when_no_gop_reported(self):
        self.assert_rejected(GOOD + "PCPOS_GOP_PROBE_V1 BEFORE mode=0 max=1\r\n")

    def test_rejects_mode_switch_event_in_headless_read_only_path(self):
        self.assert_rejected(GOOD + "PCPOS_GOP_PROBE_V1 SET_MODE_STATUS 0x0000000000000000\r\n")

    def test_rejects_entry_return_marker_order_reversal(self):
        self.assert_rejected("\r\n".join((STATUS, RETURN, ENTRY, "")))

    def test_rejects_missing_efi_entry_marker(self):
        self.assert_rejected("\r\n".join((STATUS, RETURN, "")))

    def test_rejects_qemu_crash_or_invalid_exit(self):
        self.assert_rejected(GOOD, 1)

    def test_rejects_multiple_entry_markers(self):
        self.assert_rejected(ENTRY + "\r\n" + GOOD)

    def test_rejects_empty_report(self):
        self.assert_rejected("")

    def test_accepts_read_only_trace_with_incidental_blank_lines(self):
        check_headless_receipt("\r\n" + GOOD + "\r\n", 124)


if __name__ == "__main__":
    unittest.main()
