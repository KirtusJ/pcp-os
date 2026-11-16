#!/usr/bin/env python3
"""Adversarial candidate provenance controls, not real guest/Record-pointer proofs.

A unique *digest* is not a unique physical byte-range in guest memory.
A self-consistent decoy also cannot authenticate the active Rust Record.
"""
import unittest

from register_ram_oracle import compare, select_candidate
from test_register_ram_oracle import ledger


class TestCandidateProvenance(unittest.TestCase):
    def test_two_identical_complete_chains_are_not_unique_candidates(self):
        raw, head = ledger(n=41)
        memory = b'\x00' * 13 + raw + b'\x99' * 13 + raw + b'\x00'
        selected = select_candidate(memory, 41)
        self.assertEqual(selected["candidate_count"], 2)
        # Equal heads do not make the memory addresses the same. The kernel
        # may point to either or neither. Report the proven ambiguity.
        self.assertFalse(selected["found"], selected)
        self.assertEqual(selected["reason"], "multiple equally long RAM candidate locations")
        self.assertFalse(compare(memory, head, 41)["accepted"])

    def test_one_mem_snapshot_candidate_is_not_a_live_record_identity(self):
        # The whole memory is constructed and contains *no actual QEMU kernel*.
        # It can still pass all canonical/count/head comparisons. This confirms
        # the meaning of accepted is narrowly "candidate matches", never
        # "authenticated live Record".
        decoy, head = ledger(n=41, variant="decoy")
        memory = b"NO RUST RECORD EXISTS HERE\x00" + decoy + b"\0" * 16
        result = compare(memory, head, 41)
        self.assertTrue(result["accepted"], result)
        self.assertFalse(result["live_record_pointer_authenticated"])

    def test_distinct_equal_length_chains_still_rejected(self):
        good, head = ledger(n=41, variant="genuine")
        decoy, _ = ledger(n=41, variant="competing")
        result = select_candidate(good + b"\0"*8 + decoy, 41)
        self.assertFalse(result["found"])
        self.assertEqual(result["reason"], "ambiguous longest chains")

    def test_longer_decoy_cannot_falsely_match_shorter_visible_count(self):
        good, head = ledger(n=41, variant="genuine")
        longer, _ = ledger(n=42, variant="forged-longer")
        result = compare(good + b"\0"*8 + longer, head, 41)
        self.assertFalse(result["accepted"])
        self.assertEqual(result["longest_events"], 42)
        self.assertEqual(result["visible_events"], 41)

    def test_two_identical_short_prefixes_do_not_overrule_one_full_chain(self):
        short, _ = ledger(n=17)
        full, head = ledger(n=41)
        memory = short + b"\0" + short + b"\0" + full
        result = compare(memory, head, 41)
        self.assertTrue(result["accepted"], result)
        self.assertEqual(result["candidate_count"], 1)


if __name__ == "__main__":
    unittest.main()
