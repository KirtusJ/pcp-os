#!/usr/bin/env python3
"""Assemble the private Barrel Bowl WAD from an installed Omega campaign."""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import tempfile

from install import parse_wad, write_wad
import lane


OMEGA_SHA256 = "0f89fdd2da917bc2d094ee57a7624381b5bfed919db1304cca3b024d7ffcbc49"
DEFAULT_MUSIC = Path(__file__).resolve().parent / "music" / "bowl-music.wad"


def add_audio(lumps, pack):
    """Replace the original alley track and append the new digital effects."""
    audio = dict(pack)
    if len(audio) != len(pack) or set(audio) != {b"D_E1M9", b"DSBRROLL", b"DSBRPINS", b"DSBRSTRK"}:
        raise ValueError("Unexpected entries in the bowling sound pack.")
    names = [name for name, _ in lumps]
    if names.count(b"D_E1M9") != 1 or any(name in names for name in audio if name.startswith(b"DS")):
        raise ValueError("The input has unexpected bowling audio entries.")
    result = [(name, audio.pop(name, data)) for name, data in lumps]
    result.extend(audio.items())
    return result


def install_bowl(campaign, output, music=DEFAULT_MUSIC):
    if os.path.lexists(output):
        raise FileExistsError(f"Choose a new output filename. {output} already exists.")
    base = campaign.read_bytes()
    if hashlib.sha256(base).hexdigest() != OMEGA_SHA256:
        raise ValueError("The input must be the installed Ultra Omega Barrel campaign. Run install.py first.")
    lumps = lane.apply(parse_wad(base, b"IWAD"))
    result = add_audio(lumps, parse_wad(music.read_bytes(), b"PWAD"))
    temporary = tempfile.NamedTemporaryFile(dir=output.parent, delete=False)
    try:
        with temporary:
            write_wad(temporary, result)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.link(temporary.name, output)
    finally:
        os.unlink(temporary.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaign_iwad", type=Path)
    parser.add_argument("new_iwad", type=Path)
    parser.add_argument("--music", type=Path, default=DEFAULT_MUSIC)
    args = parser.parse_args()
    try:
        install_bowl(args.campaign_iwad, args.new_iwad, args.music)
    except (ValueError, OSError, struct.error) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    print(f"Created {args.new_iwad}")


if __name__ == "__main__":
    main()
