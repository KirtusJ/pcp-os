#!/usr/bin/env python3
"""Read-only pixel acceptance for a PCP OS register head on a native P6 scanout.

This cannot manufacture a guest observation: supply an actual QEMU screendump P6.
It checks all 64 SHA-256 digits, including the last cell, using the kernel font.
"""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import re

FONT = Path(__file__).resolve().parents[3] / "PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"
FONT_PATTERN = re.compile(r"pub static GLYPHS:[^=]+=\s*(\[.*?\]);", re.DOTALL)


def glyphs_from_file(path):
    match = FONT_PATTERN.search(Path(path).read_text())
    if not match:
        raise ValueError("kernel GLYPHS table missing")
    glyphs = ast.literal_eval(match.group(1))
    if len(glyphs) != 95 or any(len(row) != 200 or any(type(v) is not int or not 0 <= v <= 255 for v in row) for row in glyphs):
        raise ValueError("kernel glyph dimensions/coverage invalid")
    return glyphs


def read_ppm(path):
    data = Path(path).read_bytes()
    pos = 0
    def token():
        nonlocal pos
        while pos < len(data):
            if data[pos] in b" \r\n\t":
                pos += 1
            elif data[pos] == 35:
                end = data.find(b"\n", pos)
                if end < 0:
                    raise ValueError("unfinished PPM comment")
                pos = end + 1
            else:
                break
        start = pos
        while pos < len(data) and data[pos] not in b" \r\n\t":
            pos += 1
        if start == pos or pos == len(data):
            raise ValueError("invalid PPM header")
        value = data[start:pos]
        pos += 1  # consume exactly one separator after the token
        return value
    if token() != b"P6":
        raise ValueError("expected native binary P6 screenshot")
    width, height, depth = map(int, (token(), token(), token()))
    if not (0 < width <= 8192 and 0 < height <= 8192 and depth == 255):
        raise ValueError("invalid screenshot geometry or max value")
    pixels = data[pos:]
    if len(pixels) != width * height * 3:
        raise ValueError(f"PPM pixel count {len(pixels)} != {width * height * 3}")
    return width, height, pixels


def expected_text(prefix, head, glyphs, foreground, background):
    if not re.fullmatch(r"[0-9a-f]{64}", head):
        raise ValueError("expected lowercase full 64-character SHA-256 head")
    text = prefix + head
    if not text or any(not 32 <= ord(ch) < 127 for ch in text):
        raise ValueError("text not representable exactly by the kernel font")
    palette = [bytes((bg * (255-a) + fg * a + 127)//255 for fg, bg in zip(foreground, background)) for a in range(256)]
    lines = []
    for y in range(20):
        lines.append(b"".join(palette[alpha] for c in text for alpha in glyphs[ord(c) - 32][10*y:10*y+10]))
    return lines


def check_scanout(path, head, glyphs, *, prefix="HEAD ", col=2, row=2, foreground=b"\x41\xba\xd7", background=b"\x10\x28\x3f"):
    width, height, pixels = read_ppm(path)
    if col < 0 or row < 0:
        raise ValueError("negative grid coordinate")
    left, top = col * 10, row * 20
    extent = 10 * (len(prefix) + 64)
    if left + extent > width or top + 20 > height:
        raise ValueError(f"full SHA-256 row does not fit: x={left}, extent={extent}, display={width}x{height}")
    reference = expected_text(prefix, head, glyphs, foreground, background)
    mismatch = 0
    first = None
    for dy, expected in enumerate(reference):
        offset = 3 * ((top + dy) * width + left)
        actual = pixels[offset:offset + extent * 3]
        for dx in range(extent):
            start = dx * 3
            if actual[start:start+3] != expected[start:start+3]:
                mismatch += 1
                if first is None:
                    first = [left+dx, top+dy]
    return {"accepted": mismatch == 0, "size": [width, height], "head": head,
            "text": prefix + head, "rect": [left, top, left+extent, top+20],
            "mismatching_pixels": mismatch, "first_mismatch": first,
            "kind": "native P6 pixel comparison; no semantic or live-service inference"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", type=Path, required=True, help="an actual PCP OS native guest P6 QEMU screendump")
    parser.add_argument("--head", required=True, help="expected verified SHA-256 from the same guest run")
    parser.add_argument("--font", type=Path, default=FONT, help="the matching kernel's font.rs")
    parser.add_argument("--prefix", default="HEAD ", help="exact text immediately before the 64 hex characters")
    parser.add_argument("--col", type=int, default=2)
    parser.add_argument("--row", type=int, default=2)
    parser.add_argument("--fg", default="41bad7", help="RGB hex, default kernel TEAL")
    parser.add_argument("--bg", default="10283f", help="RGB hex, default kernel GROUND")
    args = parser.parse_args()
    try:
        if not all(re.fullmatch(r"[0-9a-fA-F]{6}", c) for c in (args.fg, args.bg)):
            raise ValueError("foreground/background must be six hex digits")
        result = check_scanout(args.image, args.head, glyphs_from_file(args.font), prefix=args.prefix,
                               col=args.col, row=args.row, foreground=bytes.fromhex(args.fg), background=bytes.fromhex(args.bg))
        print(json.dumps(result, sort_keys=True))
        return 0 if result["accepted"] else 1
    except (ValueError, OSError, SyntaxError) as exc:
        print(json.dumps({"accepted": False, "error": str(exc)}, sort_keys=True))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
