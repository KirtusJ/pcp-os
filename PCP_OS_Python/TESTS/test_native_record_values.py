"""Check Python register JSON grammar against the native Rust Value contract.

Does not need a model, a method packet, a network connection or an OS kernel.
Only writes fresh, disposable temporary workspace files.
"""
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from pcpos.base import PcposError, USAGE, INTEGRITY, ZERO, canon, now_utc
from pcpos.store import Workspace


def nested(value, count):
    for _ in range(count):
        value = [value]
    return value


class NativeRecordValueContract(unittest.TestCase):
    def setUp(self):
        temp = TemporaryDirectory(prefix="pcpos-wire-")
        self.addCleanup(temp.cleanup)
        self.ws = Workspace.create(Path(temp.name) / "workspace")

    def forge(self, *, seq=0, body=None):
        event = {"seq": seq, "prev": ZERO, "ts": now_utc(), "actor": "P",
                 "auth": "DECLARED", "kind": "X_LOCAL", "body": {"value": 1} if body is None else body}
        self.ws.register_path.write_bytes(canon(event) + b"\n")

    def test_clean_native_values_are_accepted(self):
        ok = {"min": -(1 << 63), "max": (1 << 63) - 1,
              "null": None, "false": False, "true": True,
              "text": "snowman ☃ and emoji 🪶", "nested": nested(7, 30)}
        self.forge(body=ok)
        events, head = self.ws.events()
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]["body"], ok)
        self.assertEqual(len(head), 64)

    def test_boolean_and_float_sequence_cannot_be_zero(self):
        for value in (False, 0.0):
            with self.subTest(seq=value):
                self.forge(seq=value)
                with self.assertRaises(PcposError) as caught:
                    self.ws.events()
                self.assertEqual(caught.exception.code, INTEGRITY)

    def test_native_reader_rejects_unrepresentable_nested_values(self):
        for name, value in (
            ("float", 1.5), ("float_integral", 1.0),
            ("int_high", 1 << 63), ("int_low", -(1 << 63) - 1),
            ("lone_surrogate_value", "\ud800"),
            ("lone_surrogate_key", {"\udfff": 1}),
            ("too_deep", nested(1, 31)),
        ):
            with self.subTest(case=name):
                self.forge(body={"v": value})
                with self.assertRaises(PcposError) as caught:
                    self.ws.events()
                self.assertEqual(caught.exception.code, INTEGRITY)

    def test_invalid_append_never_commits_unreadable_native_record(self):
        for name, bad in (
            ("int_high", 1 << 63), ("int_low", -(1 << 63) - 1),
            ("surrogate", "\ud800"), ("too_deep", nested(0, 31)),
        ):
            with self.subTest(case=name):
                prior = self.ws.register_path.read_bytes()
                with self.assertRaises(PcposError) as caught:
                    self.ws.append("X_LOCAL", {"v": bad}, "P")
                self.assertEqual(caught.exception.code, USAGE)
                self.assertEqual(self.ws.register_path.read_bytes(), prior)
        for kind, body, actor, auth in ((True, {}, "P", "DECLARED"),
                                         ("X_LOCAL", [], "P", "DECLARED"),
                                         ("X_LOCAL", {}, True, "DECLARED"),
                                         ("X_LOCAL", {}, "P", "made-up")):
            with self.subTest(kind=kind, body=body, actor=actor, auth=auth):
                prior = self.ws.register_path.read_bytes()
                with self.assertRaises(PcposError):
                    self.ws.append(kind, body, actor, auth=auth)
                self.assertEqual(self.ws.register_path.read_bytes(), prior)

    def test_native_boundary_values_still_append(self):
        self.ws.append("X_LOCAL", {"n": -(1 << 63), "b": False}, "P")
        self.ws.append("X_LOCAL", {"n": (1 << 63) - 1, "emoji": "🪶"}, "P")
        self.assertEqual(len(self.ws.events()[0]), 2)


if __name__ == "__main__":
    unittest.main()
