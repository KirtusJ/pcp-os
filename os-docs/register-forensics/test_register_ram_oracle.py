#!/usr/bin/env python3
"""Synthetic positive and negative controls; these are NOT native guest acceptance."""
import hashlib
import unittest
from register_ram_oracle import canonical, compare, event_from_line, ZERO, quote


def ledger(n=41, label='kernel', variant='A'):
    lines = []
    prev = ZERO
    for i in range(n):
        body = {'label': label, 'workspace': 'acr:workspace:kernel', 'profile': 'ACR-OS-RECORD-1'} if i == 0 else {'event': variant, 'index': i}
        event = {'actor': 'host', 'auth': 'HOST', 'body': body,
                 'kind': 'WORKSPACE_CREATED' if i == 0 else 'HOST_EVENT', 'prev': prev,
                 'seq': i, 'ts': '2026-11-16T20:00:00Z'}
        line = canonical(event).encode('ascii')
        lines.append(line)
        prev = hashlib.sha256(line).hexdigest()
    return b'\n'.join(lines) + b'\n', prev


class TestIndependentMemoryScan(unittest.TestCase):
    def test_canonical_rust_controls(self):
        for s, expected in [('a', '"a"'), ('\x7f', '"\\u007f"'),
                            ('é', '"\\u00e9"'), ('\U0001f426', '"\\ud83d\\udc26"'),
                            ('\t\n\b\f', '"\\t\\n\\b\\f"')]:
            self.assertEqual(quote(s), expected)
        self.assertIsNone(event_from_line(b'{"actor":"x","actor":"y"}'))
        self.assertIsNone(event_from_line(b'{"actor":1.5}'))

    def test_full_independent_match(self):
        data, head = ledger()
        result = compare(b'PREFIX' + data + b'\0'*50, head, 41)
        self.assertTrue(result['accepted'], result)
        self.assertFalse(result['live_record_pointer_authenticated'])

    def test_stale_shorter_prefix_does_not_override_longest(self):
        full, head = ledger()
        shorter = full.rsplit(b'\n', 2)[0] + b'\n'
        self.assertTrue(compare(shorter + b'OLD\x00' + full, head, 41)['accepted'])

    def test_earlier_byte_tamper_breaks_link(self):
        data, head = ledger()
        corrupted = data.replace(b'"index":17', b'"index":18', 1)
        r = compare(corrupted, head, 41)
        self.assertFalse(r['accepted'], r)

    def test_last_line_tamper_changes_head_even_if_chain_links_validate(self):
        data, head = ledger()
        corrupted = data.replace(b'"index":40', b'"index":39', 1)
        r = compare(corrupted, head, 41)
        self.assertFalse(r['accepted'], r)
        self.assertEqual(r['longest_events'], 41)
        self.assertNotEqual(r['independent_head'], head)

    def test_ambiguous_longest_records_are_refused(self):
        a, ah = ledger(41, variant='a')
        b, bh = ledger(41, variant='b')
        self.assertNotEqual(ah, bh)
        r = compare(a + b'\0'*4 + b, ah, 41)
        self.assertFalse(r['accepted'], r)
        self.assertEqual(r['reason'], 'ambiguous longest chains')

    def test_changed_visible_last_digit_is_refused(self):
        data, head = ledger()
        changed = head[:-1] + ('0' if head[-1] != '0' else '1')
        self.assertFalse(compare(data, changed, 41)['accepted'])

    def test_wrong_expected_event_count_is_refused(self):
        data, head = ledger()
        self.assertFalse(compare(data, head, 40)['accepted'])
        self.assertFalse(compare(data, head, 42)['accepted'])

    def test_wrong_workspace_refuses(self):
        data, head = ledger(label='other')
        self.assertFalse(compare(data, head, 41)['accepted'])


if __name__ == '__main__':
    unittest.main()
