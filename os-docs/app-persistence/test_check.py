"""Controls for exact approved-path and active-generation checks."""
import hashlib
import struct
import unittest

from check import MARKER, SECTOR, SLOT_SECTORS, inspect


def slot(generation, entries):
    payload = bytearray()
    for name, content in entries:
        key = name.encode()
        payload.extend(struct.pack("<HI", len(key), len(content)) + key + content)
    data = bytearray(SECTOR * SLOT_SECTORS)
    data[:8] = b"PCPSLOT1"
    struct.pack_into("<QI", data, 8, generation, len(payload))
    data[20:52] = hashlib.sha256(payload).digest()
    data[52:84] = hashlib.sha256(data[:52]).digest()
    data[SECTOR:SECTOR + len(payload)] = payload
    return data


class ReadbackTests(unittest.TestCase):
    app = ("apps/tern.txt", b"print reed")
    draft = ("tailor-drafts-v1/tern", b"TBD1\x04\x00ternprint reed")

    def test_empty_factory_and_empty_source_differ(self):
        store = MARKER + slot(1, []) + bytes(SECTOR * SLOT_SECTORS)
        self.assertFalse(inspect(store, "tern", None)["present"])
        with self.assertRaises(ValueError):
            inspect(store, "tern", b"")

    def test_draft_and_stale_app_do_not_count(self):
        store = MARKER + slot(11, [self.draft]) + slot(10, [self.app])
        self.assertFalse(inspect(store, "tern", None)["present"])
        with self.assertRaises(ValueError):
            inspect(store, "tern", self.app[1])

    def test_approved_app_survives_unrelated_entries(self):
        store = MARKER + slot(10, [self.draft]) + slot(11, [self.draft, self.app])
        self.assertEqual(inspect(store, "tern", self.app[1])["generation"], 11)
        with self.assertRaises(ValueError):
            inspect(store, "tern", self.app[1] + b"\n")

    def test_bad_newest_slot_reports_fallback(self):
        bad = slot(11, [self.draft])
        bad[SECTOR] ^= 1
        result = inspect(MARKER + bad + slot(10, [self.app]), "tern", self.app[1])
        self.assertEqual(result["generation"], 10)
        self.assertIn(0, result["rejected_slots"])

    def test_equal_generations_refused(self):
        with self.assertRaisesRegex(ValueError, "equal generations"):
            inspect(MARKER + slot(1, []) + slot(1, [self.app]), "tern", None)

    def test_malformed_duplicate_names_cannot_activate_slot(self):
        store = MARKER + slot(11, [self.app, self.app]) + slot(10, [])
        self.assertEqual(inspect(store, "tern", None)["generation"], 10)

    def test_unlaunchable_names_cannot_pass_present_or_absent_checks(self):
        for name in ("", "Tern", "7tern", "-tern", "tern.txt", "tern_1", "tern/app",
                     "tern\n", "térn", "a" * 49):
            with self.subTest(name=name):
                record = (f"apps/{name}.txt", self.app[1])
                for entries, expected in (([record], self.app[1]), ([], None)):
                    store = MARKER + slot(1, entries) + slot(2, entries)
                    with self.assertRaisesRegex(ValueError, "application name"):
                        inspect(store, name, expected)

    def test_names_at_the_guest_boundaries_are_launchable(self):
        for name in ("a", "a-", "a" + "0-" * 23 + "9"):
            with self.subTest(name=name):
                entries = [(f"apps/{name}.txt", self.app[1]), ("apps/Tern.txt", b"other")]
                store = MARKER + slot(1, []) + slot(2, entries)
                self.assertTrue(inspect(store, name, self.app[1])["present"])

    def test_short_store_refused(self):
        with self.assertRaisesRegex(ValueError, "short store"):
            inspect(MARKER, "tern", None)


if __name__ == "__main__":
    unittest.main()
