"""Receipt checks performed before mastering starts. Run with python3 test_build.py."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from build import BASIC_FILES, FIXTURE_BANNER, checked_parts, digest, master


class ReleaseInputs(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.parts = Path(temp.name)
        for name in BASIC_FILES:
            file = self.parts / "files" / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(name.encode())
        self.kernel = self.parts / "files/PCPOS/KERNEL.ELF"
        self.receipt = {
            "format": "pcpos-release-parts/1", "profile": "model-free",
            "os_head": "1" * 40, "tailorbird_head": "2" * 40, "guest_os_head": "3" * 40,
            "kernel_features": ["proposals"], "tailorbird_features": [],
            "files": {name: digest(self.parts / "files" / name) for name in BASIC_FILES},
        }
        self.pin = self.write_lock()

    def write_lock(self):
        raw = json.dumps(self.receipt, sort_keys=True).encode()
        (self.parts / "inputs.json").write_bytes(raw)
        return hashlib.sha256(raw).hexdigest()

    def refused(self, pin, message):
        output = self.parts / "candidate.iso"
        with self.assertRaisesRegex(ValueError, message):
            master(self.parts, pin, output)
        self.assertFalse(output.exists())

    def test_complete_receipt(self):
        self.assertEqual(set(checked_parts(self.parts, self.pin)), BASIC_FILES)

    def test_lock_pin(self):
        self.refused("0" * 64, "receipt digest")

    def test_changed_payload(self):
        self.kernel.write_bytes(b"changed")
        self.refused(self.pin, "component differs")

    def test_fixture_feature_with_matching_lock(self):
        self.receipt["kernel_features"].append("model-memory-fixture")
        self.refused(self.write_lock(), "resolved features")

    def test_fixture_banner_with_matching_file_and_lock(self):
        self.kernel.write_bytes(b"ELF bytes " + FIXTURE_BANNER + b" end")
        self.receipt["files"]["PCPOS/KERNEL.ELF"] = digest(self.kernel)
        self.refused(self.write_lock(), "test fixture")

    def test_missing_component(self):
        del self.receipt["files"]["PCPOS/CLIP.WAV"]
        self.refused(self.write_lock(), "release payload")

    def test_abbreviated_source(self):
        self.receipt["os_head"] = "1234567"
        self.refused(self.write_lock(), "exact source commit")


if __name__ == "__main__":
    unittest.main()
