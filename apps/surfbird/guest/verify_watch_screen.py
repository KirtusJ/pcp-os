"""Compare the documented title and address with actual QEMU pixel regions."""

import ast
from pathlib import Path

W, H = 640, 360
BG = 0x10252C


def raster(path):
    with path.open("rb") as f:
        if f.readline().strip() != b"P6":
            raise ValueError("unexpected QEMU snapshot")
        line = f.readline()
        while line.startswith(b"#"):
            line = f.readline()
        if line.split() != [b"640", b"360"] or f.readline().strip() != b"255":
            raise ValueError("unexpected snapshot geometry")
        pixels = f.read()
    if len(pixels) != W * H * 3:
        raise ValueError("truncated snapshot")
    return pixels


def check_screens(work, title, canonical):
    source = (
        Path(__file__).resolve().parents[3]
        / "PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"
    )
    literal = source.read_text().split(" = [", 1)[1].split("];", 1)[0]
    glyphs = ast.literal_eval("[" + literal + "]")
    front, address, back = [
        raster(work / name) for name in ("title.ppm", "address.ppm", "back.ppm")
    ]
    if front != back or front == address:
        raise ValueError("back failed to restore the full screen")

    def line(pixels, y, text, ink):
        for i, char in enumerate(text):
            for j, alpha in enumerate(glyphs[ord(char) - 32]):
                start = 3 * ((y + j // 10) * W + 24 + i * 10 + j % 10)
                expected = bytes(
                    (
                        ((ink >> shift) & 255) * alpha
                        + ((BG >> shift) & 255) * (255 - alpha)
                    )
                    // 255
                    for shift in (16, 8, 0)
                )
                if pixels[start : start + 3] != expected:
                    raise ValueError(
                        f"unexpected framebuffer glyph {char!r} at row {y}, column {i}"
                    )

    y = 119
    while title:
        stop = len(title) if len(title) <= 58 else title.rfind(" ", 0, 59)
        if stop <= 0:
            stop = 58
        line(front, y, title[:stop], 0xF4F6F7)
        title = title[stop:].lstrip()
        y += 28
    line(address, 122, canonical, 0xB9E4E2)
