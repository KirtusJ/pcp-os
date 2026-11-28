"""Regenerate the bundled icon and native header masks with resvg-py and Pillow."""

import xml.etree.ElementTree as ET
from io import BytesIO
from pathlib import Path

from PIL import Image
from resvg_py import svg_to_bytes

ROOT = Path(__file__).resolve().parent
NS = "http://www.w3.org/2000/svg"
ET.register_namespace("", NS)


def render(svg, size):
    pixels = svg_to_bytes(
        svg_string=ET.tostring(svg, encoding="unicode"),
        width=size * 4,
        height=size * 4,
    )
    with Image.open(BytesIO(pixels)) as image:
        return image.convert("RGBA").resize((size, size), Image.Resampling.LANCZOS)


def main():
    svg = ET.parse(ROOT / "mockingbird.svg").getroot()
    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    icons = [render(svg, size) for size in sizes]
    icons[-1].save(ROOT / "mockingbird.png")
    icons[-1].save(
        ROOT / "mockingbird.ico",
        sizes=[icon.size for icon in icons],
        append_images=icons[:-1],
    )
    svg.remove(svg.find(f"{{{NS}}}rect[@id='tile']"))
    svg.set("viewBox", "4 2 56 56")
    for size in (32, 64):
        mask = render(svg, size).getchannel("A")
        (ROOT / f"header-{size}.alpha").write_bytes(mask.tobytes())


if __name__ == "__main__":
    main()
