"""Exact-byte and path controls for the Python object store (temporary files only)."""
import hashlib
import os
from pathlib import Path
import tempfile
import unittest

from pcpos.base import INTEGRITY, PcposError
from pcpos.store import Workspace


class ObjectStoreIntegrity(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="pcpos-object-test-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.ws = Workspace.create(self.root / "workspace")

    def test_normal_write_reuse_and_read(self):
        data = b"synthetic original object"
        digest = hashlib.sha256(data).hexdigest()
        self.assertEqual(self.ws.put_bytes(data), digest)
        self.assertEqual(self.ws.put_bytes(data), digest)
        source = self.root / "input"
        source.write_bytes(data)
        self.assertEqual(self.ws.put_file(source), (digest, len(data)))
        self.assertEqual(self.ws.get(digest), data)

    def test_corrupt_existing_put_bytes_refuses_and_preserves(self):
        data = b"expected object value"
        digest = hashlib.sha256(data).hexdigest()
        target = self.ws.object_path(digest)
        target.parent.mkdir()
        target.write_bytes(b"CORRUPT")
        with self.assertRaises(PcposError) as caught:
            self.ws.put_bytes(data)
        self.assertEqual(caught.exception.code, INTEGRITY)
        self.assertEqual(target.read_bytes(), b"CORRUPT")

    def test_corrupt_existing_put_file_refuses_and_preserves(self):
        data = b"expected file bytes"
        source = self.root / "input"
        source.write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        target = self.ws.object_path(digest)
        target.parent.mkdir()
        target.write_bytes(b"CORRUPT")
        with self.assertRaises(PcposError) as caught:
            self.ws.put_file(source)
        self.assertEqual(caught.exception.code, INTEGRITY)
        self.assertEqual(target.read_bytes(), b"CORRUPT")

    def test_symlinked_objects_directory_refuses(self):
        outside = self.root / "outside"
        outside.mkdir()
        self.ws.objects.rmdir()
        try:
            self.ws.objects.symlink_to(outside, target_is_directory=True)
        except OSError as e:
            self.skipTest("symlink unavailable: %s" % e)
        with self.assertRaises(PcposError) as caught:
            self.ws.put_bytes(b"synthetic")
        self.assertEqual(caught.exception.code, INTEGRITY)
        self.assertEqual(list(outside.rglob("*")), [])

    def test_hardlinked_digest_target_refuses(self):
        data = b"expected object"
        digest = hashlib.sha256(data).hexdigest()
        target = self.ws.object_path(digest)
        target.parent.mkdir()
        outside = self.root / "other-file"
        outside.write_bytes(b"SENTINEL")
        os.link(outside, target)
        with self.assertRaises(PcposError) as caught:
            self.ws.put_bytes(data)
        self.assertEqual(caught.exception.code, INTEGRITY)
        self.assertEqual(outside.read_bytes(), b"SENTINEL")


if __name__ == "__main__":
    unittest.main()
