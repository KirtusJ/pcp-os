#!/usr/bin/env python3
"""Bounded strict-byte comparison of separately stripped test-only kernel ELFs.

Input binaries are built on the same runner from exact commit pins. This tool
does not perform the stripping; it checks its bytes and the A/A control.
An identity result concerns this toolchain's stripped files only, not a
production release ISO, all toolchains, or code/runtime equivalence.
"""
import hashlib
import json
from pathlib import Path
import sys


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def original_digest(folder, name):
    witness = folder / (name + ".elf_sha256")
    try:
        digest = witness.read_text().strip()
    except OSError as exc:
        raise ValueError("missing original ELF hash witness: " + name) from exc
    if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        raise ValueError("invalid original ELF hash witness: " + name)
    return digest


def observe(folder):
    names = ("base_1", "base_2", "candidate_off")
    files = {name: folder/(name+".stripped.elf32") for name in names}
    for path in files.values():
        if not path.exists() or not path.read_bytes().startswith(b"\x7fELF"):
            raise ValueError("missing or invalid stripped ELF: "+str(path))
    digests = {name: sha(path) for name, path in files.items()}
    sizes = {name: files[name].stat().st_size for name in names}
    original = {name: original_digest(folder, name) for name in names}
    original_stable = original["base_1"] == original["base_2"]
    stable = digests["base_1"] == digests["base_2"] and original_stable
    matches = digests["base_1"] == digests["candidate_off"]
    return {
       "scope":"exact pinned build under one source/toolchain/staging contract only",
       "tool":"GNU objcopy --strip-all (runner-installed binutils)",
       "source":"74c4d4a9934ad68e1b5be380971bf3377a464c6d",
       "candidate":"f1b7430a9c1aaabc5587f19b16e0c1e76646ef81",
       "A_A_stripped_image_reproducible":digests["base_1"] == digests["base_2"],
       "A_A_unmodified_ELF_digest_reproducible":original_stable,
       "A_A_controls_both_reproducible":stable,
       "candidate_stripped_ELF_byte_identical_to_baseline":matches if stable else None,
       "stripped_sha256":digests,
       "stripped_sizes_bytes":sizes,
       "unmodified_whole_ELF_identical":(original["base_1"] == original["candidate_off"]) if original_stable else None,
       "unmodified_ELF_digest_witnesses":original,
       "production_ISO_or_live_authority_verified":False,
       "disposition":("A_A_UNSTABLE" if not stable else
                      "STRIPPED_IDENTICAL_IN_SCOPE" if matches else
                      "STRIPPED_DIFFERENT_IN_SCOPE")
    }


def main():
    if len(sys.argv)!=2:
        raise SystemExit("usage: compare_stripped_images.py <stripped-file-dir>")
    folder=Path(sys.argv[1])
    result=observe(folder)
    (folder/"stripped-image-result.json").write_text(json.dumps(result,indent=2,sort_keys=True)+"\n")
    print(json.dumps(result,sort_keys=True))
    return 0 if result["A_A_controls_both_reproducible"] else 1


if __name__=="__main__":
    raise SystemExit(main())
