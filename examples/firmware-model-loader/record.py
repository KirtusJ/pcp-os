"""Portable identities and build receipt for a qualified EFI binary."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def executable(name):
    found = shutil.which(name)
    if found is None:
        raise ValueError(f"required build tool is missing: {name}")
    return Path(found).absolute()


def identity(path):
    reported = subprocess.check_output([path, "--version"], text=True).strip()
    return {"sha256": digest(path), "version": reported.splitlines()[0]}


def write_record(path, *, binary, input_pin, config, lock, tools, sdk, commands):
    libraries = ("crt0-efi-x86_64.o", "elf_x86_64_efi.lds", "libefi.a", "libgnuefi.a")
    record = {
        "format": "pcpos-efi-build/1",
        "target": "x86_64-unknown-none",
        "profile": "efi",
        "input_receipt_sha256": input_pin,
        "configuration_sha256": digest(config),
        "cargo_lock_sha256": digest(lock),
        "tools": tools,
        "commands": commands,
        "target_rustflags": ["-C", "relocation-model=pic", "-C", "no-redzone=yes",
                             "--remap-path-prefix", "<checkout>=/pcpos-source",
                             "--remap-path-prefix", "<build>=/pcpos-efi-build"],
        "gnu_efi_files": {name: digest(sdk / "lib" / name) for name in libraries},
        "output": {"bytes": binary.stat().st_size, "sha256": digest(binary)},
    }
    path.write_text(json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def publish(binary, output, receipt=None, destination=None):
    """Publish only new files. A failed receipt publication releases the newly linked output."""
    if receipt is None:
        os.link(binary, output)
        return
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as temporary:
        staged = Path(temporary.name)
    try:
        shutil.copyfile(receipt, staged)
        os.link(binary, output)
        try:
            os.link(staged, destination)
        except BaseException:
            output.unlink()
            raise
    finally:
        staged.unlink(missing_ok=True)
