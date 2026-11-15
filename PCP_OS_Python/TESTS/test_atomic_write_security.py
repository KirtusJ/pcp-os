"""Regression controls for atomic_write's former predictable temporary filename.

These tests use only synthetic data under freshly allocated temporary directories.
"""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from pcpos.base import atomic_write


class AtomicWriteSecurity(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory(prefix="pcpos-atomic-test-")
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name)

    def test_regular_replace_and_no_leftover_temp_files(self):
        target = self.root / "data.jsonl"
        atomic_write(target, b"first")
        atomic_write(target, b"second")
        self.assertEqual(target.read_bytes(), b"second")
        self.assertFalse(target.is_symlink())
        self.assertEqual([p.name for p in self.root.iterdir()], ["data.jsonl"])

    @unittest.skipUnless(hasattr(os, "symlink"), "symbolic links unavailable")
    def test_preplaced_pid_temporary_symlink_does_not_change_sentinel(self):
        target = self.root / "record.jsonl"
        outside = self.root / "outside-sentinel"
        outside.write_bytes(b"DO NOT TOUCH")
        old_tmp = target.with_name(target.name + ".tmp%d" % os.getpid())
        try:
            old_tmp.symlink_to(outside)
        except OSError as error:
            self.skipTest("symlinks unavailable: %s" % error)
        atomic_write(target, b"new record bytes")
        self.assertEqual(outside.read_bytes(), b"DO NOT TOUCH")
        self.assertEqual(target.read_bytes(), b"new record bytes")
        self.assertFalse(target.is_symlink())
        self.assertTrue(old_tmp.is_symlink())

    def test_failed_replace_leaves_previous_bytes_and_cleans_new_temp(self):
        target = self.root / "record"
        target.write_bytes(b"old")
        with patch("pcpos.base.os.replace", side_effect=OSError("injected failure")):
            with self.assertRaisesRegex(OSError, "injected failure"):
                atomic_write(target, b"new")
        self.assertEqual(target.read_bytes(), b"old")
        self.assertEqual([p.name for p in self.root.iterdir()], ["record"])


if __name__ == "__main__":
    unittest.main()
