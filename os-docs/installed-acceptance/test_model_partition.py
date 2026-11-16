"""Exercise the checker against real GPT/FAT images containing small test payloads."""
import hashlib
import json
import shutil
import struct
import subprocess
import tempfile
import unittest
import uuid
import zlib
from pathlib import Path

from check_model_partition import check

REVISION = "a" * 40
DIRECTORY = "::/models/qwen/qwen2.5-coder-0.5b/" + REVISION
MODEL = b"model fixture\0\r\n\x1a\xff"
LICENSE = b"license fixture\r\n"
TOOLS = ("partx", "mformat", "mmd", "mcopy")


def make_disk(path, names):
    # Two separate four-MiB volumes, each with a primary/backup GPT entry.
    sectors = 20480
    entries = bytearray(128 * 128)
    disk_guid = uuid.uuid4().bytes_le
    starts = (2048, 10240)
    for index, (start, name) in enumerate(zip(starts, names)):
        entry = struct.pack("<16s16sQQQ72s",
                            uuid.UUID("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7").bytes_le,
                            uuid.uuid4().bytes_le, start, start + 8191, 0, name.encode("utf-16le"))
        entries[index * 128:(index + 1) * 128] = entry
    with path.open("wb") as target:
        target.truncate(sectors * 512)
        mbr = bytearray(512)
        mbr[450] = 0xee
        struct.pack_into("<II", mbr, 454, 1, sectors - 1)
        mbr[510:512] = b"\x55\xaa"
        target.write(mbr)
        for here, peer, table in ((1, sectors - 1, 2), (sectors - 1, 1, sectors - 33)):
            header = bytearray(struct.pack("<8sIIIIQQQQ16sQIII", b"EFI PART", 65536, 92,
                                          0, 0, here, peer, 34, sectors - 34, disk_guid,
                                          table, 128, 128, zlib.crc32(entries)))
            struct.pack_into("<I", header, 16, zlib.crc32(header))
            target.seek(here * 512)
            target.write(header)
            target.seek(table * 512)
            target.write(entries)
    volumes = [f"{path}@@{start * 512}" for start in starts]
    for image in volumes:
        subprocess.run(["mformat", "-i", image, "-T", "8192", "::"], check=True)
    return volumes


@unittest.skipUnless(all(shutil.which(tool) for tool in TOOLS), "requires util-linux partx and GNU mtools")
class InstalledModelTests(unittest.TestCase):
    def run_case(self, *, model=MODEL, license_text=LICENSE, misplaced=False, duplicate=False,
                 scope="factory", after_release=None, invalid_factory_hash=False):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            disk = work / "installed.raw"
            label = "PCP OS models"
            # Put the model partition in slot 1 deliberately. Its label determines the location.
            models, other = make_disk(disk, (label, label if duplicate else "PCP OS user data"))
            volume = other if misplaced else models
            parent = "::"
            for name in DIRECTORY[3:].split("/"):
                parent += "/" + name
                subprocess.run(["mmd", "-i", volume, parent], check=True)
            for name, content in (("qwen.gguf", model), ("LICENSE.txt", license_text)):
                source = work / name
                source.write_bytes(content)
                subprocess.run(["mcopy", "-i", volume, source, f"{DIRECTORY}/{name}"], check=True)

            with disk.open("rb") as fd:
                disk_digest = hashlib.file_digest(fd, "sha256").hexdigest()
            manifest = work / "release.json"
            manifest.write_text(json.dumps({
                "git_commit": "b" * 40,
                "output_sha256": {"PCP-OS.img": "" if invalid_factory_hash else disk_digest},
                "qwen": {"revision": REVISION, "model_sha256": hashlib.sha256(MODEL).hexdigest(),
                         "license_sha256": hashlib.sha256(LICENSE).hexdigest()},
            }))
            if after_release:
                volume, destination = (other, "::/notes.txt") if after_release == "notes.txt" else (
                    models, f"{DIRECTORY}/{after_release}")
                source = work / "replacement"
                source.write_bytes(b"saved after release\n")
                subprocess.run(["mcopy", "-o", "-i", volume, source, destination], check=True)
                self.assertEqual(subprocess.check_output(["mcopy", "-i", volume, destination, "-"]),
                                 source.read_bytes())
            return check(disk, manifest, scope=scope)

    def test_read_both_files(self):
        result = self.run_case()
        self.assertEqual(result["partition"]["number"], 1)
        self.assertEqual(result["scope"], "factory")
        self.assertIs(result["factory_disk_matches"], True)
        self.assertEqual(result["disk_sha256"], result["factory_disk_sha256"])
        self.assertEqual(result["files"][DIRECTORY[2:] + "/qwen.gguf"]["bytes"], len(MODEL))
        self.assertEqual(result["files"][DIRECTORY[2:] + "/LICENSE.txt"]["bytes"], len(LICENSE))

    def test_identical_files_on_wrong_partition(self):
        with self.assertRaisesRegex(ValueError, "Cannot read .*qwen.gguf"):
            self.run_case(misplaced=True)

    def test_different_model(self):
        with self.assertRaisesRegex(ValueError, "qwen.gguf: .* differs"):
            self.run_case(model=MODEL[:-1] + b"X")

    def test_different_license(self):
        with self.assertRaisesRegex(ValueError, "LICENSE.txt: .* differs"):
            self.run_case(license_text=b"L" + LICENSE[1:])

    def test_ambiguous_partition(self):
        with self.assertRaisesRegex(ValueError, "found 2"):
            self.run_case(duplicate=True)

    def test_factory_rejects_user_save(self):
        with self.assertRaisesRegex(ValueError, "Installed disk SHA-256 differs"):
            self.run_case(after_release="notes.txt")

    def test_model_files_after_user_save(self):
        result = self.run_case(scope="model-files", after_release="notes.txt")
        self.assertEqual(result["scope"], "model-files")
        self.assertIs(result["factory_disk_matches"], False)
        self.assertNotEqual(result["disk_sha256"], result["factory_disk_sha256"])
        for name, content in (("qwen.gguf", MODEL), ("LICENSE.txt", LICENSE)):
            self.assertEqual(result["files"][f"{DIRECTORY[2:]}/{name}"],
                             {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()})

    def test_changed_files_against_original_manifest(self):
        for name in ("qwen.gguf", "LICENSE.txt"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, f"{name}: .* differs"):
                self.run_case(scope="model-files", after_release=name)

    def test_model_files_requires_factory_reference(self):
        with self.assertRaisesRegex(ValueError, "Expected a factory disk SHA-256"):
            self.run_case(scope="model-files", invalid_factory_hash=True)


if __name__ == "__main__":
    unittest.main()
