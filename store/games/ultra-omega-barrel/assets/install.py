#!/usr/bin/env python3
"""Assemble Ultra Omega Barrel's private game-data file from Doom shareware 1.9."""
import argparse
from collections import Counter
import hashlib
import os
from pathlib import Path
import struct
import tempfile

import omega_arena


BASE_SHA1 = "5b2e249b9c5133ec987b3ea77596381dc0d6bc1d"
HERE = Path(__file__).resolve().parent
HEADER = struct.Struct("<4sii")
ENTRY = struct.Struct("<ii8s")
MARKERS = {b"S_START", b"S_END", b"SS_START", b"SS_END"}


def parse_wad(data, expected_tag):
    if len(data) < HEADER.size:
        raise ValueError("Incomplete WAD header.")
    tag, count, directory = HEADER.unpack_from(data)
    if tag != expected_tag or count < 0 or directory < HEADER.size:
        raise ValueError(f"Invalid {expected_tag.decode()} header.")
    if directory + count * ENTRY.size > len(data):
        raise ValueError("WAD directory extends beyond the file.")
    lumps = []
    for index in range(count):
        start, size, raw_name = ENTRY.unpack_from(data, directory + index * ENTRY.size)
        if start < 0 or size < 0 or start + size > len(data):
            raise ValueError(f"WAD entry {index} extends beyond the file.")
        name = raw_name.rstrip(b"\x00")
        if not name or b"\x00" in name:
            raise ValueError(f"Invalid WAD entry name at index {index}.")
        lumps.append((name, data[start:start + size]))
    return lumps


def unique_entries(lumps):
    entries = dict(lumps)
    if len(entries) != len(lumps):
        raise ValueError("The patch contains duplicate entry names.")
    return entries


def merge_art(original, actors, scenery):
    sprites = unique_entries([(name, data) for name, data in actors
                              if not (name in MARKERS and not data)])
    scenery = unique_entries(scenery)
    if not sprites or sprites.keys() & MARKERS or sprites.keys() & scenery.keys():
        raise ValueError("Missing sprites or conflicting patch entries.")
    names = [name for name, _ in original]
    first = names.index(b"S_START")
    last = names.index(b"S_END", first + 1)
    stock = Counter(names)
    sprite_names = set(names[first + 1:last])
    if any(name in stock and name not in sprite_names for name in sprites):
        raise ValueError("Sprite name conflicts with a non-sprite entry.")
    new_sounds = {name: data for name, data in scenery.items() if name == b"DSSHELL" and name not in stock}
    if any(name not in new_sounds and (stock[name] != 1 or name in sprite_names or name in MARKERS)
           for name in scenery):
        raise ValueError("Scenery replacements must each name one existing non-sprite resource.")
    merged = []
    for index, (name, data) in enumerate(original):
        if index == last:
            merged.extend(sprites.items())
        if first < index < last:
            data = sprites.pop(name, data)
        elif name in scenery:
            data = scenery[name]
        merged.append((name, data))
    merged.extend(new_sounds.items())
    return merged


def write_wad(file, lumps):
    directory = bytearray()
    file.write(bytes(HEADER.size))
    for name, data in lumps:
        if not 0 < len(name) <= 8:
            raise ValueError(f"Invalid WAD directory name {name!r}")
        directory.extend(ENTRY.pack(file.tell(), len(data), name))
        file.write(data)
    directory_offset = file.tell()
    file.write(directory)
    file.seek(0)
    file.write(HEADER.pack(b"IWAD", len(lumps), directory_offset))


def install(source, output, sprites, campaign, music):
    if os.path.lexists(output):
        raise FileExistsError(f"Choose a new output filename. {output} already exists.")
    base = source.read_bytes()
    if hashlib.sha1(base).hexdigest() != BASE_SHA1:
        raise ValueError("The input must be the original Doom shareware 1.9 IWAD.")
    original = parse_wad(base, b"IWAD")
    arena = omega_arena.apply(original)
    resources = parse_wad(campaign.read_bytes(), b"PWAD")
    resources.extend(parse_wad(music.read_bytes(), b"PWAD"))
    merged = merge_art(arena, parse_wad(sprites.read_bytes(), b"PWAD"), resources)
    temp = tempfile.NamedTemporaryFile(dir=output.parent, delete=False)
    try:
        with temp:
            write_wad(temp, merged)
            temp.flush()
            os.fsync(temp.fileno())
        # The hard link publishes the finished file without replacing an existing one.
        os.link(temp.name, output)
    finally:
        os.unlink(temp.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original_iwad", type=Path)
    parser.add_argument("new_iwad", type=Path)
    parser.add_argument("--sprites", type=Path, default=HERE / "art" / "omega-sprites.wad")
    parser.add_argument("--campaign", type=Path, default=HERE / "foundry.wad")
    parser.add_argument("--music", type=Path, default=HERE / "music" / "foundry-score.wad")
    args = parser.parse_args()
    try:
        install(args.original_iwad, args.new_iwad, args.sprites, args.campaign, args.music)
    except (ValueError, OSError, struct.error) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    print(f"Created {args.new_iwad}")


if __name__ == "__main__":
    main()
