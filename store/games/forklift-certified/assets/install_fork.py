#!/usr/bin/env python3
"""Assemble the private Forklift bonus IWAD from an installed Omega campaign."""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import tempfile

import dock
from install import parse_wad, write_wad


HERE = Path(__file__).resolve().parent
OMEGA_SHA256 = "0f89fdd2da917bc2d094ee57a7624381b5bfed919db1304cca3b024d7ffcbc49"
SPRITES = HERE / "fork-sprites.wad"
DRIVER = HERE / "driver.wad"
MUSIC = HERE / "music" / "fork-music.wad"
PERMIT = HERE / "permit.wad"
EFFECTS = (b"DSFRBEEP", b"DSFRHYD", b"DSFRSMAS", b"DSFRCLEA")


def merge(lumps, sprite_pack, driver_pack, music_pack, permit_pack):
    expected_frames = {f"FORK{frame}{angle}".encode("ascii") for frame in "ABCDE" for angle in range(1, 9)}
    expected_frames.update(f"FORK{frame}0".encode("ascii") for frame in "FGH")
    expected_frames.update(f"CARG{frame}0".encode("ascii") for frame in "ABCD")
    sprites = dict(sprite_pack)
    if len(sprites) != len(sprite_pack) or set(sprites) != expected_frames | {b"S_START", b"S_END"}:
        raise ValueError("Unexpected loading-dock sprite pack.")
    if sprites.pop(b"S_START") or sprites.pop(b"S_END"):
        raise ValueError("Invalid sprite markers.")
    driver = dict(driver_pack)
    driver_frames = {f"LUCK{f}0".encode("ascii") for f in "ABCDEFG"}
    if len(driver) != len(driver_pack) or set(driver) != driver_frames | {b"S_START", b"S_END"}:
        raise ValueError("Unexpected on-foot driver pack.")
    if driver.pop(b"S_START") or driver.pop(b"S_END"):
        raise ValueError("Invalid on-foot sprite markers.")
    sprites.update(driver)
    audio = dict(music_pack)
    if len(audio) != len(music_pack) or set(audio) != {*EFFECTS, b"D_E1M9"}:
        raise ValueError("Unexpected loading-dock audio pack.")
    if len(permit_pack) != 1 or permit_pack[0][0] != b"FKPERMIT" or not permit_pack[0][1]:
        raise ValueError("The permit pack must contain FKPERMIT.")
    names = [name for name, _ in lumps]
    if any(names.count(n) != 1 for n in (b"D_E1M9", b"S_START", b"S_END")):
        raise ValueError("Unexpected game resource directory.")
    if any(n in names for n in (*sprites, *EFFECTS, b"FKPERMIT")):
        raise ValueError("Forklift assets are already present.")
    result = []
    for name, payload in lumps:
        if name == b"S_END":
            result.extend(sprites.items())
        if name == b"D_E1M9":
            payload = audio[name]
        result.append((name, payload))
    result.extend((name, audio[name]) for name in EFFECTS)
    result.extend(permit_pack)
    return result


def install_fork(campaign, output, sprites=SPRITES, music=MUSIC, permit=PERMIT, driver=DRIVER):
    if os.path.lexists(output):
        raise FileExistsError(f"Choose a new output filename. {output} already exists.")
    base = campaign.read_bytes()
    if hashlib.sha256(base).hexdigest() != OMEGA_SHA256:
        raise ValueError("The input must be the installed Ultra Omega Barrel campaign. Run install.py first.")
    map_data = dock.apply(parse_wad(base, b"IWAD"))
    resources = [parse_wad(path.read_bytes(), b"PWAD") for path in (sprites, driver, music, permit)]
    result = merge(map_data, *resources)
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
    parser.add_argument("--sprites", type=Path, default=SPRITES)
    parser.add_argument("--music", type=Path, default=MUSIC)
    parser.add_argument("--permit", type=Path, default=PERMIT)
    parser.add_argument("--driver", type=Path, default=DRIVER)
    args = parser.parse_args()
    try:
        install_fork(args.campaign_iwad, args.new_iwad, args.sprites, args.music, args.permit, args.driver)
    except (ValueError, OSError, struct.error) as exc:
        parser.exit(1, f"{parser.prog}: {exc}\n")
    print(f"Created {args.new_iwad}")


if __name__ == "__main__":
    main()
