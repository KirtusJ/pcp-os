#!/usr/bin/env python3
"""Source-pinned parity adversaries supplied by Director 14, kept as tests-first."""
import hashlib
import unittest

from register_ram_oracle import compare, event_from_line
from test_register_ram_oracle import ledger


def add_note_escaped(line, escaped):
    key = b'"profile":'
    if line.count(key) != 1:
        raise AssertionError('the source-pinned first event changed')
    return line.replace(key, b'"note":"' + escaped.encode('ascii') + b'",' + key, 1)


class TestRustStringAndWorkspaceBoundary(unittest.TestCase):
    def test_valid_utf16_pair_and_unpaired_controls(self):
        data, _ = ledger(n=1)
        original = data[:-1]
        self.assertIsNotNone(event_from_line(original))
        # Rust accepts a proper UTF-16 high+low pair and decodes one scalar.
        valid_pair = add_note_escaped(original, r'\ud83d\udc26')
        self.assertIsNotNone(event_from_line(valid_pair))
        # Rust Value::Parser::string refuses isolated high or low surrogate.
        for invalid in (r'\ud800', r'\udc00'):
            with self.subTest(invalid=invalid):
                self.assertIsNone(event_from_line(add_note_escaped(original, invalid)))

    def test_same_label_different_workspace_must_not_select_kernel(self):
        data, head = ledger(n=1)
        self.assertTrue(compare(data, head, 1)['accepted'])
        changed = data.replace(b'"workspace":"acr:workspace:kernel"',
                               b'"workspace":"acr:workspace:different"', 1)
        self.assertNotEqual(data, changed)
        changed_head = hashlib.sha256(changed[:-1]).hexdigest()
        self.assertFalse(compare(changed, changed_head, 1)['accepted'])


if __name__ == '__main__':
    unittest.main()
