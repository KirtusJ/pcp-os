#!/usr/bin/env python3
"""Assemble Midnight Barrel Express from an installed Omega campaign IWAD."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import struct
import tempfile

from install import parse_wad, write_wad
import train


HERE = Path(__file__).resolve().parent
OMEGA_SHA256 = "0f89fdd2da917bc2d094ee57a7624381b5bfed919db1304cca3b024d7ffcbc49"
ART = HERE / "rail-sprites.wad"
MUSIC = HERE / "music" / "rail-music.wad"
STATION = HERE / "station.wad"
EFFECTS = (b"DSRWHIST", b"DSRLOCK", b"DSRBRAKE", b"DSRBELL", b"DSRWHEEL")
SPRITE = re.compile(rb"(COND|BRKM|BRAK)[A-Z][0-8](?:[A-Z][1-8])?")


def merge(lumps, art, music, station):
    art_names = [n for n, _ in art]
    if len(art_names) != len(set(art_names)):
        raise ValueError("Duplicate entry in Express artwork.")
    first = art.index((b"S_START", b""))
    last = art.index((b"S_END", b""))
    if first >= last:
        raise ValueError("Invalid Express sprite section.")
    sprites = art[first + 1:last]
    required = {f"{kind}{chr(65 + frame)}0".encode("ascii")
                for kind, count in (("COND", 8), ("BRKM", 6), ("BRAK", 3))
                for frame in range(count)}
    if (any(not SPRITE.fullmatch(name) or not payload for name, payload in sprites)
            or not required.issubset(name for name, _ in sprites)):
        raise ValueError("Unexpected Express sprites.")
    outside = art[:first] + art[last + 1:]
    if len(outside) != 1 or outside[0][0] != b"RLTITLE" or not outside[0][1]:
        raise ValueError("Unexpected non-sprite entry in Express artwork.")
    audio = dict(music)
    if len(audio) != len(music) or set(audio) != {*EFFECTS, b"D_E1M9"}:
        raise ValueError("Unexpected Express audio pack.")
    screens = dict(station)
    if len(screens) != len(station) or set(screens) != {b"RLTICKET", b"RLPANEL"} or not all(screens.values()):
        raise ValueError("Unexpected Express station screens.")
    added = sprites + outside + station + [(name, audio[name]) for name in EFFECTS]
    names = [name for name, _ in lumps]
    if any(names.count(n) != 1 for n in (b"S_START", b"S_END", b"D_E1M9")):
        raise ValueError("Unexpected Omega resource directory.")
    if any(name in names for name, _ in added):
        raise ValueError("Express assets are already installed.")
    result = []
    for name, data in lumps:
        if name == b"S_END":
            result.extend(sprites)
        if name == b"D_E1M9":
            data = audio[name]
        result.append((name, data))
    result.extend(outside)
    result.extend(station)
    result.extend((name, audio[name]) for name in EFFECTS)
    return result


def install_rail(campaign, output, art=ART, music=MUSIC, station=STATION):
    if os.path.lexists(output):
        raise FileExistsError(f"Choose a new output filename. {output} already exists.")
    base = campaign.read_bytes()
    if hashlib.sha256(base).hexdigest() != OMEGA_SHA256:
        raise ValueError("The input must be the installed Ultra Omega Barrel campaign. Run install.py first.")
    rail = train.apply(parse_wad(base, b"IWAD"))
    packs = [parse_wad(path.read_bytes(), b"PWAD") for path in (art, music, station)]
    result = merge(rail, *packs)
    temp = tempfile.NamedTemporaryFile(dir=output.parent, delete=False)
    try:
        with temp:
            write_wad(temp, result)
            temp.flush()
            os.fsync(temp.fileno())
        os.link(temp.name, output)
    finally:
        os.unlink(temp.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaign_iwad", type=Path)
    parser.add_argument("new_iwad", type=Path)
    parser.add_argument("--sprites", type=Path, default=ART)
    parser.add_argument("--music", type=Path, default=MUSIC)
    parser.add_argument("--station", type=Path, default=STATION)
    args = parser.parse_args()
    try:
        install_rail(args.campaign_iwad, args.new_iwad, args.sprites, args.music, args.station)
    except (ValueError, OSError, struct.error) as exc:
        parser.exit(1, f"{parser.prog}: {exc}\n")
    print(f"Created {args.new_iwad}")


if __name__ == "__main__":
    main()
