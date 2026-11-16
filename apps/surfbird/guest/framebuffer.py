"""Read the guest framebuffer and compare text cells to the kernel font."""

import ast
from pathlib import Path

W, H = 640, 360


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


def load_glyphs():
    source = (
        Path(__file__).resolve().parents[3]
        / "PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"
    )
    literal = source.read_text().split(" = [", 1)[1].split("];", 1)[0]
    return ast.literal_eval("[" + literal + "]")


def check_line(pixels, glyphs, x, y, text, ink, bg):
    for i, char in enumerate(text):
        for j, alpha in enumerate(glyphs[ord(char) - 32]):
            start = 3 * ((y + j // 10) * W + x + i * 10 + j % 10)
            expected = bytes(
                (
                    ((ink >> shift) & 255) * alpha
                    + ((bg >> shift) & 255) * (255 - alpha)
                )
                // 255
                for shift in (16, 8, 0)
            )
            if pixels[start : start + 3] != expected:
                raise ValueError(
                    f"unexpected framebuffer glyph {char!r} at row {y}, column {i}"
                )
