"""Run the archive replay CLI on a small ELF and synthetic serial records."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


@unittest.skipUnless(shutil.which("as") and shutil.which("ld"), "requires GNU as and ld")
class ArchivedReplay(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        # Two adjacent pages with known contents, matching the PCP OS measurement layout.
        subprocess.run(["as", "--64", "-o", "pages.o"], cwd=self.root, check=True,
                       input=b".text\n.globl _start\n_start:\n.fill 4096,1,65\n"
                             b".section .rodata\n.fill 4096,1,66\n")
        subprocess.run(["ld", "-m", "elf_x86_64", "-Ttext=0x1000", "--section-start=.rodata=0x2000",
                        "-o", "kernel.elf", "pages.o"], cwd=self.root, check=True)
        self.kernel_hash = hashlib.sha256((self.root / "kernel.elf").read_bytes()).hexdigest()
        self.measurement = hashlib.sha256(b"A" * 4096 + b"B" * 4096).hexdigest()
        self.worker = "1" * 64
        self.manifest = {"git_commit": "release-fixture",
                         "component_sha256": {"kernel": self.kernel_hash, "qwen-worker": self.worker}}
        (self.root / "release.json").write_text(json.dumps(self.manifest))

    def run_cli(self, mode, args=(), *, measured=None, tokens=(220, 19, 198)):
        image = self.measurement if measured is None else measured
        diagnostic = mode == "diagnostic"
        arguments = ("auto=qwen.elf grant=files then=off " if diagnostic else "") + "image=" + image
        lines = ["PCP OS EFI amd64 entry, native 64-bit handoff", "protections: long mode true",
                 "command line: " + arguments, f"running image (code and read-only data), SHA-256: {image} "
                 "(measured; equal to the value the loader named)"]
        if diagnostic:
            lines.extend([f"starting qwen.elf (128 bytes, SHA-256 {self.worker}) with: 3 17 488 220 17 284",
                          "QWEN READY", *[f"QWEN TOKEN {token}" for token in tokens],
                          "QWEN COMPLETE", "qwen.elf ended DONE: exit 0"])
        else:
            lines.append("PCP OS launcher ready: 0 programs, 0 files")
        serial = ("\r\n".join(lines + ["ALL SELF-TESTS PASS"]) + "\r\n").encode()
        (self.root / "boot.serial").write_bytes(serial)
        script = Path(__file__).with_name("replay.py").resolve()
        python = [sys.executable] + (["-O"] if sys.flags.optimize else [])
        return subprocess.run([*python, script, "--mode", mode, "--serial", "boot.serial",
                               "--manifest", "release.json", "--kernel-elf", "kernel.elf", *args],
                              cwd=self.root, text=True, capture_output=True)

    def test_recovery_receipt_hashes_raw_input_and_pins_the_kernel(self):
        result = self.run_cli("recovery")
        self.assertEqual(result.returncode, 0, result.stderr)
        serial = (self.root / "boot.serial").read_bytes()
        manifest = (self.root / "release.json").read_bytes()
        self.assertEqual(json.loads(result.stdout), {
            "scope": "archived-transcript", "mode": "recovery", "source_revision": "release-fixture",
            "serial_bytes": len(serial), "serial_sha256": hashlib.sha256(serial).hexdigest(),
            "manifest_sha256": hashlib.sha256(manifest).hexdigest(), "kernel_elf_sha256": self.kernel_hash,
            "release_kernel_measurement": self.measurement,
        })
        wrong = self.run_cli("recovery", measured="0" * 64)
        self.assertEqual(wrong.returncode, 1)
        self.assertEqual(wrong.stdout, "")
        self.assertIn("manifest-matched ELF", wrong.stderr)
        with (self.root / "kernel.elf").open("ab") as kernel:
            kernel.write(b"changed")
        wrong = self.run_cli("recovery")
        self.assertEqual(wrong.returncode, 1)
        self.assertIn("Kernel ELF SHA-256", wrong.stderr)

    def test_diagnostic_requires_an_external_token_reference(self):
        reference = ["--expected-tokens", "220", "19", "198"]
        good = self.run_cli("diagnostic", reference)
        self.assertEqual(good.returncode, 0, good.stderr)
        self.assertEqual(json.loads(good.stdout)["expected_tokens"], [220, 19, 198])
        self.assertEqual(self.run_cli("diagnostic").returncode, 2)
        self.assertEqual(self.run_cli("recovery", reference).returncode, 2)
        wrong = self.run_cli("diagnostic", reference, tokens=(220, 18, 198))
        self.assertEqual(wrong.returncode, 1)
        self.assertEqual(wrong.stdout, "")
        self.assertIn("Qwen generated", wrong.stderr)
