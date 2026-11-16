"""Synthetic positive and adverse cases, not an installed-PCP-OS anti-brick test."""
import struct
import unittest
import zlib

from selector_model import (
    SECTOR, NoSafeBoot, candidates, make_record, mutate_prefix,
    parse_record, read_selector, witness,
)

IMAGE_A = b"known-good-synthetic-boot-slot"
IMAGE_B = b"unproven-synthetic-candidate"
IMAGES = (IMAGE_A, IMAGE_B)
OLD = make_record(1, 0, 0, IMAGES)
NEW = make_record(2, 1, 0, IMAGES)


class MirroredSelectorTest(unittest.TestCase):
    def test_all_two_copy_prefix_write_cuts_preserve_last_good_candidate(self):
        self.assertEqual(witness()["crash_cutpoints_tested"], 258)

    def test_old_mirror_must_be_usable_even_after_a_torn_new_copy(self):
        for i in range(SECTOR):
            with self.subTest(prefix_bytes=i):
                self.assertEqual(candidates(mutate_prefix(OLD, NEW, i)+OLD, IMAGES), [0])
                self.assertEqual(candidates(NEW+mutate_prefix(OLD, NEW, i), IMAGES), [1, 0])

    def test_both_fully_written_copies_choose_pending_then_known_good(self):
        self.assertEqual(candidates(NEW+NEW, IMAGES), [1, 0])
        self.assertEqual(candidates(NEW+OLD, IMAGES), [1, 0])
        self.assertEqual(candidates(OLD+NEW, IMAGES), [1, 0])

    def test_corrupt_pending_never_replaces_valid_fallback(self):
        self.assertEqual(candidates(NEW+NEW, (IMAGE_A, b"bad")), [0])

    def test_missing_known_good_prevents_a_nonbricking_claim(self):
        with self.assertRaisesRegex(NoSafeBoot, "known-good"):
            candidates(NEW+NEW, (b"bad", IMAGE_B))

    def test_both_selector_mirrors_invalid_must_fail_closed(self):
        with self.assertRaisesRegex(NoSafeBoot, "both"):
            candidates(bytes(2*SECTOR), IMAGES)

    def test_conflicting_equal_generation_records_must_fail_closed(self):
        conflict = make_record(2, 0, 0, IMAGES)
        with self.assertRaisesRegex(NoSafeBoot, "equivocation"):
            candidates(NEW+conflict, IMAGES)

    def test_trusted_highwater_refuses_lower_sequence(self):
        with self.assertRaisesRegex(NoSafeBoot, "rollback"):
            candidates(OLD+OLD, IMAGES, trusted_min_sequence=2)

    def test_no_external_highwater_allows_stale_valid_history(self):
        # Expected adverse behavior in this model, never a protected update.
        self.assertEqual(candidates(OLD+OLD, IMAGES), [0])

    def test_checksum_is_not_signature(self):
        # An attacker can recompute CRC and declare altered image digest. A
        # future authenticated update implementation must independently
        # verify signed metadata and/or an isolated trusted image hash.
        forged = make_record(999, 1, 1, (IMAGE_A, b"hostile"))
        self.assertEqual(candidates(forged+forged, (IMAGE_A, b"hostile")), [1])
        self.assertEqual(candidates(forged+forged, (IMAGE_A, b"hostile"),
                                    trusted_min_sequence=5), [1])
        # Both results are examples of model INSUFFICIENCY, not successes.

    def test_crc_covers_full_record_even_on_first_byte_mutation(self):
        for idx in (0, 7, 8, 15, 16, SECTOR-5, SECTOR-1):
            with self.subTest(idx=idx):
                corrupt = bytearray(OLD)
                corrupt[idx] ^= 0x80
                self.assertIsNone(parse_record(bytes(corrupt)))

    def test_structurally_changed_record_with_recomputed_crc_is_rejected(self):
        tampered = bytearray(OLD)
        tampered[100] = 1 # reserved padding is always zero
        struct.pack_into("<I", tampered, SECTOR-4, zlib.crc32(tampered[:-4]))
        self.assertIsNone(parse_record(bytes(tampered)))

    def test_invalid_metadata_lengths_do_not_fall_back_silently(self):
        with self.assertRaises(ValueError):
            read_selector(OLD)
        with self.assertRaises(ValueError):
            mutate_prefix(OLD, NEW, SECTOR+1)
        with self.assertRaises(ValueError):
            make_record(1, 2, 0, IMAGES)
        with self.assertRaises(ValueError):
            make_record(-1, 0, 0, IMAGES)


if __name__ == "__main__":
    unittest.main()
