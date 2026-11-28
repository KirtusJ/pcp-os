#!/usr/bin/env python3
"""A pinned upstream register fixture, separate from synthetic-only controls."""
from pathlib import Path
import unittest
from register_ram_oracle import compare, event_from_line

REPO = Path(__file__).resolve().parents[2]
REFERENCE = REPO / 'PCP_OS_Rust/vectors/register_reference.jsonl'
# This head is also the exact independently recorded REFERENCE_HEAD in
# PCP_OS_Rust/crates/pcpos-kernel/src/main.rs at Director 11's base commit.
REFERENCE_HEAD = 'df54563c8abc6187b9df104b13e4665b82436ee466884212c08e9765cc07287a'


class TestPinnedCrossImplementationVector(unittest.TestCase):
    def test_released_reference_vector_canonical_and_head(self):
        data = REFERENCE.read_bytes()
        self.assertEqual(len(data.splitlines()), 4)
        for line in data.splitlines():
            self.assertIsNotNone(event_from_line(line))
        result = compare(b'\0' * 64 + data + b'\0' * 32,
                         REFERENCE_HEAD, 4, workspace_label='vector')
        self.assertTrue(result['accepted'], result)

    def test_changed_prior_event_fails_original_head(self):
        data = REFERENCE.read_bytes()
        changed = data.replace(b'"vector"', b'"vectoR"', 1)
        self.assertNotEqual(data, changed)
        self.assertFalse(compare(changed, REFERENCE_HEAD, 4, workspace_label='vectoR')['accepted'])

    def test_unfinished_reference_line_does_not_fake_complete_record(self):
        data = REFERENCE.read_bytes()
        self.assertFalse(compare(data[:-1], REFERENCE_HEAD, 4, workspace_label='vector')['accepted'])


if __name__ == '__main__':
    unittest.main()
