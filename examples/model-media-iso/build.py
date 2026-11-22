"""Master a UEFI hybrid ISO from one unpartitioned, prepared FAT volume."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid


# Match the fixed date in examples/tester-cd/build.py. Both ISO dates and GPT
# identities must be independent of the time or location of this build.
ISO_DATE = "2026111400000000"
BOOT_PATH = "EFI/BOOT/BOOTX64.EFI"


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def inventory(directory):
    entries = {}
    for path in directory.rglob("*"):
        name = path.relative_to(directory).as_posix()
        entries[name] = None if path.is_dir() else (path.stat().st_size, digest(path))
    return entries


def build(volume, output):
    if not volume.is_file():
        raise ValueError("input must be a regular FAT image file")
    if os.path.lexists(output):
        raise FileExistsError(output)
    with tempfile.TemporaryDirectory(prefix=".pcpos-iso-", dir=output.parent) as temp:
        work = Path(temp)
        fat = work / "esp.fat"
        tree = work / "tree"
        image = work / "master.iso"
        tree.mkdir()
        shutil.copyfile(volume, fat)
        disk_guid = str(uuid.uuid5(uuid.NAMESPACE_URL, "urn:sha256:" + digest(fat)))
        subprocess.run(["mcopy", "-sm", "-i", fat, "::/*", tree], check=True)
        expected = inventory(tree)
        if not any(name.upper() == BOOT_PATH and info is not None and info[0] > 0 for name, info in expected.items()):
            raise ValueError("missing or empty EFI/BOOT/BOOTX64.EFI on the FAT volume")
        # Use names absent everywhere in the input because -hide matches leaf names.
        names = {Path(name).name.casefold() for name in expected}
        stem = "pcpos-master"
        while stem + ".fat" in names or stem + ".cat" in names:
            stem += "_"
        esp_name, catalog = stem + ".fat", stem + ".cat"
        fat.rename(work / esp_name)
        subprocess.run([
            "xorriso", "-as", "mkisofs", "-quiet", "-iso-level", "3", "-r", "-J",
            "-V", "PCP_OS", "--modification-date=" + ISO_DATE,
            "--set_all_file_dates", ISO_DATE, "--gpt_disk_guid", disk_guid,
            "-graft-points", "-hide", esp_name, "-hide-joliet", esp_name,
            "-c", catalog, "--boot-catalog-hide", "-efi-boot-part", "--efi-boot-image",
            "-e", esp_name, "-no-emul-boot", "-o", image, ".", f"/{esp_name}=../{esp_name}",
        ], cwd=tree, check=True)
        shutil.rmtree(tree)
        subprocess.run(["xorriso", "-read_fs", "norock", "-osirrox", "on",
                        "-indev", image, "-extract", "/", tree], check=True)
        actual = inventory(tree)
        for name in sorted(expected.keys() | actual.keys()):
            if name not in actual or name not in expected or expected[name] != actual[name]:
                raise ValueError(f"Joliet read-back differs at {name!r}")
        size, sha = image.stat().st_size, digest(image)
        # Publish only the complete checked file; a concurrent builder cannot replace it.
        os.link(image, output)
    return size, sha


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("volume", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    size, sha = build(args.volume.resolve(), args.output.absolute())
    print(f"{sha}  {args.output} ({size} bytes)")
