"""Check both filesystem views of a mastered ISO, then package it for the boot helper."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import uuid
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build import BASIC_FILES, digest


def esp_offset(iso):
    with iso.open("rb") as stream:
        stream.seek(512)
        header = stream.read(92)
        if header[:8] != b"EFI PART":
            raise ValueError("Missing GPT header")
        table, count, size = struct.unpack_from("<QII", header, 72)
        if count > 4096 or not 128 <= size <= 4096:
            raise ValueError("Invalid GPT partition array")
        stream.seek(table * 512)
        offsets = []
        for _ in range(count):
            entry = stream.read(size)
            if uuid.UUID(bytes_le=entry[:16]) == uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b"):
                offsets.append(struct.unpack_from("<Q", entry, 32)[0] * 512)
        if len(offsets) != 1:
            raise ValueError("Expected one EFI system partition")
        return offsets[0]


def package(out):
    iso = out / "pcp-os-x86_64-uefi-model-free.iso"
    manifest = out / "inputs.json"
    inputs = json.loads(manifest.read_bytes())
    expected = inputs["files"]
    if inputs["profile"] != "model-free" or set(expected) != BASIC_FILES:
        raise ValueError("Unexpected media profile or inventory")
    with tempfile.TemporaryDirectory() as tmp:
        fat = Path(tmp) / "fat"
        joliet = Path(tmp) / "joliet"
        fat.mkdir()
        subprocess.run(["mcopy", "-s", "-i", f"{iso}@@{esp_offset(iso)}", "::/*", fat], check=True)
        subprocess.run(["xorriso", "-read_fs", "norock", "-osirrox", "on", "-indev", iso,
                        "-extract", "/", joliet], check=True)
        for root in (fat, joliet):
            actual = {p.relative_to(root).as_posix(): digest(p) for p in root.rglob("*") if p.is_file()}
            if actual != expected:
                raise ValueError(f"{root.name} inventory differs from inputs.json")

    archive = iso.with_suffix(".zip")
    with zipfile.ZipFile(archive, "x", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        z.write(iso, iso.name)
    receipt = {"profile": inputs["profile"], "archive": archive.name, "filename": iso.name,
               "bytes": iso.stat().st_size, "sha256": digest(iso),
               "inputs_sha256": digest(manifest), "kernel_measurement": inputs["measurement"]}
    (out / "image.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    package(Path(sys.argv[1]).resolve())
