"""Check publication and the recorded EFI receipt independently of the firmware build."""
from pathlib import Path
import hashlib
import json
import tempfile
import unittest

from record import publish, write_record


class BuildRecord(unittest.TestCase):
    def test_verified_digests_and_atomic_publication(self):
        with tempfile.TemporaryDirectory() as name:
            base = Path(name)
            binary = base / "candidate"
            binary.write_bytes(b"EFI fixture bytes")
            receipt = base / "generated.json"
            lib = base / "lib"
            lib.mkdir()
            for file in ["crt0-efi-x86_64.o", "elf_x86_64_efi.lds", "libefi.a", "libgnuefi.a"]:
                (lib / file).write_text(file, encoding="ascii")
            write_record(receipt, binary=binary, input_pin="ab" * 32, config=binary, lock=binary,
                         tools={"rustc": {"version": "fixture", "sha256": "ff" * 32}},
                         sdk=base, commands=[["ld", "<build>/firmware.o"]])
            captured = json.loads(receipt.read_bytes())
            self.assertEqual(captured["output"], {"bytes": len(binary.read_bytes()),
                             "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()})
            self.assertEqual(captured["gnu_efi_files"]["libefi.a"], hashlib.sha256(b"libefi.a").hexdigest())
            self.assertEqual(captured["input_receipt_sha256"], "ab" * 32)
            output, record = base / "BOOTX64.EFI", base / "public.json"
            record.write_bytes(b"existing record")
            with self.assertRaises(FileExistsError):
                publish(binary, output, receipt, record)
            self.assertFalse(output.exists())
            self.assertEqual(record.read_bytes(), b"existing record")
            record.unlink()
            publish(binary, output, receipt, record)
            self.assertEqual(record.read_bytes(), receipt.read_bytes())
            self.assertEqual(output.read_bytes(), binary.read_bytes())
            with self.assertRaises(FileExistsError):
                publish(binary, output, receipt, base / "other.json")
            self.assertFalse((base / "other.json").exists())


if __name__ == "__main__":
    unittest.main()
