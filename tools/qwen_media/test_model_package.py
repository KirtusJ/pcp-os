"""Disposable tests for offline Qwen GGUF media integrity; no downloaded models used."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

import model_package as m


def fake_gguf(data=b"example local model weights", version=3):
    return b"GGUF" + version.to_bytes(4, "little") + b"\x00" * 8 + data


class ModelPackageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.folder = Path(self.tmp.name) / "models" / "Qwen3-0.6B" / ("a" * 40)
        self.folder.mkdir(parents=True)
        self.file = self.folder / "Qwen3-0.6B-Q4_K_M.gguf"
        self.file.write_bytes(fake_gguf())
        self.expected = hashlib.sha256(self.file.read_bytes()).hexdigest()

    def build(self):
        return m.build(self.folder, self.file.name, "Qwen3-0.6B", "a" * 40,
                       "Qwen/Qwen3-0.6B-GGUF", self.expected)

    def test_build_and_verify_exact_pins(self):
        pin = self.build()
        v = m.verify(self.folder, pin)
        self.assertEqual(v["status"], "BYTES_MATCH_RELEASE_PIN")
        self.assertEqual(v["sha256"], self.expected)

    def test_wrong_model_pin_refused_before_manifest(self):
        with self.assertRaises(m.Refused):
            m.build(self.folder, self.file.name, "Qwen3-0.6B", "a" * 40,
                    "Qwen/Qwen3-0.6B-GGUF", "0" * 64)
        self.assertFalse((self.folder / "MODEL_MANIFEST.json").exists())

    def test_wrong_manifest_pin_refused(self):
        self.build()
        with self.assertRaises(m.Refused):
            m.verify(self.folder, "0" * 64)

    def test_refuses_no_activation_pin(self):
        self.build()
        with self.assertRaises(m.Refused):
            m.verify(self.folder, "")

    def test_model_mutation_refused(self):
        pin = self.build()
        self.file.write_bytes(fake_gguf(b"tampered same source"))
        with self.assertRaises(m.Refused):
            m.verify(self.folder, pin)

    def test_missing_file_refused(self):
        pin = self.build()
        self.file.unlink()
        with self.assertRaises(m.Refused):
            m.verify(self.folder, pin)

    def test_model_symlink_refused(self):
        content = self.file.read_bytes()
        self.file.unlink()
        outside = Path(self.tmp.name) / "outside.gguf"
        outside.write_bytes(content)
        self.file.symlink_to(outside)
        with self.assertRaises(m.Refused):
            self.build()

    def test_parent_symlink_refused(self):
        outer = Path(self.tmp.name) / "alias"
        outer.symlink_to(self.folder, target_is_directory=True)
        with self.assertRaises(m.Refused):
            m.build(outer, self.file.name, "Qwen3-0.6B", "a"*40,
                    "Qwen/Qwen3-0.6B-GGUF", self.expected)

    def test_hardlink_refused(self):
        other = Path(self.tmp.name) / "copy.gguf"
        os.link(self.file, other)
        with self.assertRaises(m.Refused):
            self.build()

    def test_truncated_and_unknown_gguf_refused(self):
        self.file.write_bytes(b"GGUF")
        self.expected = hashlib.sha256(self.file.read_bytes()).hexdigest()
        with self.assertRaises(m.Refused):
            self.build()
        self.file.write_bytes(fake_gguf(version=999))
        self.expected = hashlib.sha256(self.file.read_bytes()).hexdigest()
        with self.assertRaises(m.Refused):
            self.build()

    def test_duplicate_manifest_refused_even_with_matching_pin(self):
        pin = self.build()
        manifest = self.folder / "MODEL_MANIFEST.json"
        original = manifest.read_bytes()
        self.assertIn(b'"schema":', original)
        mutated = original.replace(b'"schema":', b'"schema":"pcpos-qwen-gguf-1","schema":', 1)
        manifest.write_bytes(mutated)
        forged = hashlib.sha256(mutated).hexdigest()
        with self.assertRaises(m.Refused):
            m.verify(self.folder, forged)

    def test_unrecognized_field_refused_even_with_matching_pin(self):
        self.build()
        manifest = self.folder / "MODEL_MANIFEST.json"
        d = json.loads(manifest.read_text())
        d["execute"] = "yes"
        mutated = m.canonical_bytes(d)
        manifest.write_bytes(mutated)
        with self.assertRaises(m.Refused):
            m.verify(self.folder, hashlib.sha256(mutated).hexdigest())

    def test_noncanonical_json_refused(self):
        self.build()
        manifest = self.folder / "MODEL_MANIFEST.json"
        d = json.loads(manifest.read_text())
        mutated = json.dumps(d, indent=2).encode()+b"\n"
        manifest.write_bytes(mutated)
        with self.assertRaises(m.Refused):
            m.verify(self.folder, hashlib.sha256(mutated).hexdigest())

    def test_traversal_and_bad_revision_refused(self):
        for name in ("../escaped.gguf", "a/b.gguf", "..\\evil.gguf", ".", ""):
            with self.subTest(name=name):
                with self.assertRaises(m.Refused):
                    m.plain_file_name(name)
        with self.assertRaises(m.Refused):
            m.build(self.folder, self.file.name, "Qwen3-0.6B", "main",
                    "Qwen/Qwen3-0.6B-GGUF", self.expected)

    def test_refuse_clobber_existing_manifest(self):
        self.build()
        before = (self.folder / "MODEL_MANIFEST.json").read_bytes()
        with self.assertRaises(m.Refused):
            self.build()
        self.assertEqual((self.folder / "MODEL_MANIFEST.json").read_bytes(), before)

    def test_fifo_refused_without_blocking(self):
        fifo = self.folder / "pipe.gguf"
        os.mkfifo(fifo)
        with self.assertRaises(m.Refused):
            m.read_regular(fifo)

    def test_symlink_manifest_refused(self):
        pin = self.build()
        manifest = self.folder / "MODEL_MANIFEST.json"
        saved = self.folder / "saved.txt"
        manifest.rename(saved)
        manifest.symlink_to(saved)
        with self.assertRaises(m.Refused):
            m.verify(self.folder, pin)


if __name__ == "__main__":
    unittest.main()
