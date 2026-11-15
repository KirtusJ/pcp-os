"""Extract the seven original on-foot driver frames from the Lucky sprite pack."""
import argparse
from pathlib import Path
import struct

from install import parse_wad


def make_driver(source, destination):
    source_lumps = dict(parse_wad(source.read_bytes(), b"PWAD"))
    frames = [f"LUCK{frame}0".encode("ascii") for frame in "ABCDEFG"]
    lumps = [(b"S_START", b"")] + [(name, source_lumps[name]) for name in frames] + [(b"S_END", b"")]
    data = bytearray(12)
    directory = bytearray()
    for name, payload in lumps:
        directory.extend(struct.pack("<ii8s", len(data), len(payload), name))
        data.extend(payload)
    struct.pack_into("<4sii", data, 0, b"PWAD", len(lumps), len(data))
    destination.write_bytes(data + directory)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lucky_sprites", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    make_driver(args.lucky_sprites, args.output)
