"""Build a private Doom episode image from the installed Store assets and a local IWAD."""

import argparse
import hashlib
import os
from pathlib import Path
import struct
import sys
import tempfile

GAMES = Path(__file__).resolve().parent.parent
CAMPAIGN = GAMES / "ultra-omega-barrel" / "assets"
sys.path.insert(0, str(CAMPAIGN))
import install
import omega_arena

FREEDOOM = "7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d"
MODS = ("ultra-omega-barrel", "barrel-bowl", "midnight-barrel-express", "forklift-certified")


def translate_records(payload, stride, x_offsets, y_offsets, dx, dy):
    data = bytearray(payload)
    if len(data) % stride:
        raise ValueError("Unexpected classic Doom map record size")
    for start in range(0, len(data), stride):
        for offsets, step in ((x_offsets, dx), (y_offsets, dy)):
            for offset in offsets:
                pos = start + offset
                value, = struct.unpack_from("<h", data, pos)
                struct.pack_into("<h", data, pos, value + step)
    return bytes(data)


def freedoom_arena(lumps):
    # The recovered game promotes a Baron at precisely (304, 2864) to its final boss.
    # Translate the complete Freedoom 0.13.0 map so one all-skill Baron occupies that point.
    start = next(i for i, (name, _) in enumerate(lumps) if name == b"E1M8")
    result = list(lumps)
    for i in range(start + 1, start + 11):
        name, data = lumps[i]
        if name == b"THINGS":
            if (1152, -1568, 180, 3003, 7) not in struct.iter_unpack("<5h", data):
                raise ValueError("Expected the Freedoom Phase 1 v0.13.0 E1M8 boss anchor")
            data = translate_records(data, 10, (0,), (2,), -848, 4432)
        elif name == b"VERTEXES":
            data = translate_records(data, 4, (0,), (2,), -848, 4432)
        elif name == b"NODES":
            # partition origin, then top/bottom/left/right for each child
            data = translate_records(data, 28, (0, 12, 14, 20, 22), (2, 8, 10, 16, 18), -848, 4432)
        elif name == b"BLOCKMAP":
            data = translate_records(data[:4], 4, (0,), (2,), -848, 4432) + data[4:]
        result[i] = (name, data)
    return result


def patch(path):
    return install.parse_wad(path.read_bytes(), b"PWAD")


def private_lumps(game, data):
    original = install.parse_wad(data, b"IWAD")
    if hashlib.sha256(data).hexdigest() == FREEDOOM:
        arena = freedoom_arena(original) if game == MODS[0] else original
    else:
        # The classic encounter script references original E1M8 sector and side numbers.
        # A registered four-episode (Ultimate Doom 1.9) image is supported; never apply
        # these coordinate/sector edits to an unidentified map or to the shareware IWAD.
        if hashlib.md5(data, usedforsecurity=False).hexdigest() != "c4fe9fd920207691a9f493668e0a2083":
            raise ValueError("Use Freedoom Phase 1 v0.13.0 or the Ultimate Doom 1.9 IWAD")
        arena = omega_arena.apply(original)
    resources = patch(CAMPAIGN / "foundry.wad") + patch(CAMPAIGN / "music/foundry-score.wad")
    merged = install.merge_art(arena, patch(CAMPAIGN / "art/omega-sprites.wad"), resources)
    if game == MODS[0]:
        return merged

    assets = GAMES / game / "assets"
    sys.path.insert(0, str(assets))
    if game == "barrel-bowl":
        import install_bowl
        return install_bowl.add_audio(install_bowl.lane.apply(merged), patch(install_bowl.DEFAULT_MUSIC))
    if game == "midnight-barrel-express":
        import install_rail
        return install_rail.merge(install_rail.train.apply(merged), patch(install_rail.ART),
                                  patch(install_rail.MUSIC), patch(install_rail.STATION))
    import install_fork
    return install_fork.merge(install_fork.dock.apply(merged), patch(install_fork.SPRITES),
                              patch(install_fork.DRIVER), patch(install_fork.MUSIC), patch(install_fork.PERMIT))


def prepare(game, base, output):
    if os.path.lexists(output):
        raise FileExistsError("Choose a new output path")
    lumps = private_lumps(game, base.read_bytes())
    temp = tempfile.NamedTemporaryFile(dir=output.parent, delete=False)
    try:
        with temp:
            install.write_wad(temp, lumps)
        if os.name == "nt":
            # Windows rename refuses to replace another writer's destination, including on exFAT.
            os.rename(temp.name, output)
        else:
            os.link(temp.name, output)
    finally:
        Path(temp.name).unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game", choices=MODS)
    parser.add_argument("iwad", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        prepare(args.game, args.iwad, args.output)
    except (OSError, ValueError, struct.error) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
