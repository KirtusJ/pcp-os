"""Master a model-free release candidate from an independently pinned component receipt."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
import tempfile


BASIC_FILES = {
    "EFI/BOOT/BOOTX64.EFI", "EFI/BOOT/grub.cfg", "PCPOS/KERNEL.ELF", "PCPOS/TAILOR.ELF",
    "PCPOS/SURF.ELF", "PCPOS/CLIP.PPM", "PCPOS/CLIP.WAV", "PCPOS/ASSET-NOTICES.TXT",
    "PCPOS/ASSET-LICENSE.MD", "PCPOS/PCPOS-NOTICE.MD", "PCPOS/GRUB-LICENSE.TXT",
}
FIXTURE_BANNER = b"THIS KERNEL IS BUILT WITH model-memory-fixture"
EPOCH = "1794614400"


def digest(file):
    with file.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def checked_parts(parts, lock_pin):
    lock = parts / "inputs.json"
    raw = lock.read_bytes()
    if hashlib.sha256(raw).hexdigest() != lock_pin:
        raise ValueError("component receipt digest differs")
    receipt = json.loads(raw)
    if receipt["format"] != "pcpos-release-parts/1" or receipt["profile"] != "model-free":
        raise ValueError("expected model-free release parts")
    for name in ("os_head", "guest_os_head", "tailorbird_head"):
        if not re.fullmatch("[0-9a-f]{40}", receipt[name]):
            raise ValueError("missing exact source commit: " + name)
    if receipt["kernel_features"] != ["proposals"] or receipt["tailorbird_features"] != []:
        raise ValueError("unexpected resolved features for the model-free profile")
    files = receipt["files"]
    if set(files) != BASIC_FILES or any(not re.fullmatch("[0-9a-f]{64}", value) for value in files.values()):
        raise ValueError("incomplete or unexpected release payload")
    root = parts / "files"
    for name, expected in files.items():
        file = root / name
        if not file.is_file() or file.is_symlink() or not file.stat().st_size or digest(file) != expected:
            raise ValueError("component differs: " + name)
    if FIXTURE_BANNER in (root / "PCPOS/KERNEL.ELF").read_bytes():
        raise ValueError("kernel announces the model memory test fixture")
    return files


def master(parts, lock_pin, output):
    files = checked_parts(parts, lock_pin)
    helper = Path(__file__).resolve().parents[1] / "model-media-iso/build.py"
    hybrid = runpy.run_path(str(helper))["build"]
    env = dict(os.environ, LC_ALL="C", TZ="UTC", SOURCE_DATE_EPOCH=EPOCH)
    with tempfile.TemporaryDirectory(prefix=".release-fat-", dir=output.parent) as directory:
        stage = Path(directory)
        volume = stage / "boot.fat"
        with volume.open("xb") as blank:
            blank.truncate(64 << 20)
        def fat(*args):
            subprocess.run(args, env=env, check=True)
        fat("mkfs.fat", "--invariant", "-F", "32", "-n", "PCPOS", volume)
        for name in ("EFI", "EFI/BOOT", "PCPOS"):
            fat("mmd", "-i", volume, "::/" + name)
        for name, expected in sorted(files.items()):
            fat("mcopy", "-i", volume, parts / "files" / name, "::/" + name)
            # Check bytes read from the volume, including changes to the inputs during copying.
            check = stage / "readback"
            fat("mcopy", "-i", volume, "::/" + name, check)
            if digest(check) != expected:
                raise ValueError("FAT readback differs: " + name)
            check.unlink()
        return hybrid(volume, output)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("parts", type=Path)
    p.add_argument("--inputs-sha256", required=True)
    p.add_argument("output", type=Path)
    args = p.parse_args()
    size, sha = master(args.parts.resolve(), args.inputs_sha256, args.output.absolute())
    print(f"{sha}  {args.output} ({size} bytes)")


if __name__ == "__main__":
    main()
