"""Bake the interface lettering from the two Source Sans 3 OTFs in a directory.

Inputs are from adobe-fonts/source-sans at ed1808970eb3c7301c9a523bee26473ba0bb62fa.
Run with Python, Pillow and the path to the OTF directory. The guest uses the
checked-in masks and needs no font service.
"""

import argparse
import hashlib
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
FONTS = {
    "Regular": "08df266400933d3178d081a45f94a08814c3e55b4b7dd2e0ff69cb1329f13ab6",
    "Semibold": "36f1cd2c344aa3109e64429bce41a41e4b2b923beb2db19b8dbf9bb56f05a4f9",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fonts", type=Path)
    folder = parser.parse_args().fonts
    for weight, expected in FONTS.items():
        path = folder / f"SourceSans3-{weight}.otf"
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError(f"Unexpected font: {path.name}")
    for z in (1, 2):
        regular = ImageFont.truetype(str(folder / "SourceSans3-Regular.otf"), 16 * z)
        metrics = bytearray()
        coverage = bytearray()
        for code in range(32, 127):
            ch = chr(code)
            tile = Image.new("L", (16 * z, 20 * z))
            ImageDraw.Draw(tile).text(
                (z, 15 * z), ch, font=regular, anchor="ls", fill=255
            )
            left, top, right, bottom = tile.getbbox() or (0, 0, 0, 0)
            metrics.extend(
                struct.pack(
                    "<5BH",
                    round(regular.getlength(ch)),
                    left,
                    top,
                    right - left,
                    bottom - top,
                    len(coverage),
                )
            )
            coverage.extend(tile.crop((left, top, right, bottom)).tobytes())
        (ROOT / f"labels-{z}x.bin").write_bytes(metrics + coverage)
        semibold = ImageFont.truetype(str(folder / "SourceSans3-Semibold.otf"), 21 * z)
        title = Image.new("L", (120 * z, 24 * z))
        ImageDraw.Draw(title).text(
            (0, 17 * z), "Mockingbird", font=semibold, anchor="ls", fill=255
        )
        (ROOT / f"wordmark-{z}x.alpha").write_bytes(title.tobytes())


if __name__ == "__main__":
    main()
