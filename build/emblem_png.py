#!/usr/bin/env python3
"""The product emblem as a PNG, for the plugins' listing icon: the reference bird (the Core plate, plain wing)
on the Core plate's navy. The same shapes and colours as the covers and the backgrounds (build_os_docs.py, BIRD["core"];
design tokens, module.core.plate). It is the product's mark, so it is the same in every edition.

Standard library only, and no font: the picture is computed, sample by sample, so the bytes do not depend on a
drawing library. 512 by 512, 8-bit RGB, 4 by 4 samples per pixel.

    python3 emblem_png.py <out.png>
"""
import struct, sys, zlib

SIZE, SS = 512, 4
FIELD = (0x10, 0x28, 0x3F)
K, OX, OY = 2.9, 81.1, 64.2          # the bird's own coordinates, scaled and centred
hexc = lambda h: (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16))
SHAPES = [("ellipse", (54.7, 61.1, 41, 48.7), "#187C78"), ("ellipse", (67, 29.8, 27.7, 27.7), "#E2F2ED"),
          ("polygon", ((88.5, 19.5), (117.2, 33.9), (88.5, 42.1)), "#E3A24B"), ("ellipse", (73.1, 23.6, 4.4, 4.4), "#18324B"),
          ("ellipse", (49, 71.3, 27.15, 30.2), "#41BAD7"), ("polygon", ((23.9, 94.4), (3.4, 130.2), (44.4, 105.6)), "#187C78")]

def inside(kind, g, x, y):
    if kind == "ellipse":
        cx, cy, rx, ry = g
        return ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0
    hit = False
    n = len(g)
    for i in range(n):
        (x1, y1), (x2, y2) = g[i], g[(i + 1) % n]
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            hit = not hit
    return hit

def bbox(kind, g):
    if kind == "ellipse":
        cx, cy, rx, ry = g
        return cx - rx, cy - ry, cx + rx, cy + ry
    return min(p[0] for p in g), min(p[1] for p in g), max(p[0] for p in g), max(p[1] for p in g)

def render():
    n = SIZE * SS
    top = [[-1] * n for _ in range(n)]                    # index of the topmost shape at each sample, or -1 for the field
    for idx, (kind, g, _) in enumerate(SHAPES):
        x0, y0, x1, y1 = bbox(kind, g)
        for sy in range(max(0, int((y0 * K + OY) * SS) - 1), min(n, int((y1 * K + OY) * SS) + 2)):
            y = ((sy + 0.5) / SS - OY) / K
            row = top[sy]
            for sx in range(max(0, int((x0 * K + OX) * SS) - 1), min(n, int((x1 * K + OX) * SS) + 2)):
                if inside(kind, g, ((sx + 0.5) / SS - OX) / K, y):
                    row[sx] = idx
    colours = [hexc(c) for _, _, c in SHAPES] + [FIELD]
    raw = bytearray()
    for py in range(SIZE):
        raw.append(0)
        rows = top[py * SS:(py + 1) * SS]
        for px in range(SIZE):
            r = g = b = 0
            for row in rows:
                for v in row[px * SS:(px + 1) * SS]:
                    c = colours[v]
                    r += c[0]; g += c[1]; b += c[2]
            d = SS * SS
            raw += bytes(((r + d // 2) // d, (g + d // 2) // d, (b + d // 2) // d))
    return bytes(raw)

def png(raw):
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")

def emblem():
    return png(render())

if __name__ == "__main__":
    open(sys.argv[1], "wb").write(emblem())
