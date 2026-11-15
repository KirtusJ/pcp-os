"""Build Barrel Bowl's native Doom E1M9 geometry and optional graphics overlay.

The installer calls apply(lumps), which loads the bundled assets with Python's
standard library. Run this module directly to redraw the art with Pillow.
"""
import argparse
from pathlib import Path
import struct


HERE = Path(__file__).resolve().parent
PWAD = HERE / "alley.wad"
MAP_ORDER = "E1M9 THINGS LINEDEFS SIDEDEFS VERTEXES SEGS SSECTORS NODES SECTORS REJECT BLOCKMAP".split()
FLATS = "BLLANE BLGUTR BLCARP BLCEIL BLARRO BLFOUL".split()
WALLS = "BLHERO BLWALL BLSIDE BLBACK BLEXIT BLMARQ".split()
SPRITES = ["BALL" + frame + "0" for frame in "ABC"]
# Extra cuts at ±128 let the 256-unit overhead marquee end on its metal frame.
XS = [-256, -216, -152, -128, -112, -32, 32, 112, 128, 152, 216, 256]
YS = [-640, -544, -416, -352, -288, -224, -160, 0, 320, 560, 640]


def records(fmt, rows):
    return b"".join(struct.pack(fmt, *row) for row in rows)


def map_lumps():
    """An axis-aligned BSP. Each leaf is one clockwise, convex rectangle."""
    vertices, sectors, sides, lines, segs, subsectors, nodes = [], [], [], [], [], [], []
    vertex_ids, sector_ids, line_ids, leaves = {}, {}, {}, {}

    def vertex(xy):
        if xy not in vertex_ids:
            vertex_ids[xy] = len(vertices)
            vertices.append(xy)
        return vertex_ids[xy]

    def surface(ix, iy):
        x = abs((XS[ix] + XS[ix + 1]) // 2)
        y = (YS[iy] + YS[iy + 1]) // 2
        if y < -352:
            flat, floor, light = ("BLLANE", 0, 212) if x < 112 else ("BLCARP", 0, 184)
        elif x < 112:
            flat, floor, light = "BLLANE", 0, 236
            if -352 < y < -288:
                flat = "BLFOUL"
            elif -224 < y < -160:
                flat = "BLARRO"
        elif x < 152:
            flat, floor, light = "BLGUTR", -6, 208
        else:
            flat, floor, light = "BLCARP", 0, 200
        if y > 560:
            light = 242
        spec = (floor, 128, flat.encode(), b"BLCEIL", light, 0, 0)
        if spec not in sector_ids:
            sector_ids[spec] = len(sectors)
            sectors.append(spec)
        return sector_ids[spec]

    active = {(i, j) for i in range(len(XS) - 1) for j in range(len(YS) - 1)
              if YS[j] >= -352 or (XS[i] >= -216 and XS[i + 1] <= 216)}

    for ix, iy in sorted(active, key=lambda cell: (cell[1], cell[0])):
        x, xx = XS[ix:ix + 2]
        y, yy = YS[iy:iy + 2]
        sec = surface(ix, iy)
        first = len(segs)
        corners = [(x, y), (x, yy), (xx, yy), (xx, y)]
        neighbors = [(ix - 1, iy), (ix, iy + 1), (ix + 1, iy), (ix, iy - 1)]
        for side, (a, b, neighbor) in enumerate(zip(corners, corners[1:] + corners[:1], neighbors)):
            va, vb = vertex(a), vertex(b)
            edge = tuple(sorted((va, vb)))
            inner = neighbor in active
            top = low = mid = b"-"
            shift = special = 0
            if inner:
                low = b"BLWALL"  # visible only across the six-unit gutter lip
                if a[1] == b[1] == -160 and -128 <= a[0] <= 128 and -128 <= b[0] <= 128:
                    mid = b"BLMARQ"
                    shift = (a[0] + 128) if side == 1 else (128 - a[0])
            elif side == 1 and yy == YS[-1]:
                mid, shift = b"BLHERO", a[0] + 256
            elif side == 3 and y == YS[0]:
                mid, shift = b"BLBACK", 216 - a[0]
            elif side == 2 and xx == 216 and y == -544:
                mid, special = b"BLEXIT", 11
            else:
                mid = b"BLSIDE" if y >= -160 and (y // 128) % 3 != 0 else b"BLWALL"
                shift = (a[1] if side == 0 else -a[1]) % 128
            sd = len(sides)
            sides.append((shift, 0, top, low, mid, sec))
            if edge in line_ids:
                li = line_ids[edge]
                lines[li][-1] = sd
                direction = 1
            else:
                li = line_ids[edge] = len(lines)
                lines.append([va, vb, 4 if inner else 1, special, 0, sd, 65535])
                direction = 0
            angle = (16384, 0, 49152, 32768)[side]
            segs.append((va, vb, angle, li, direction, 0))
        leaves[ix, iy] = len(subsectors)
        subsectors.append((4, first))

    def bounds(cells):
        return (max(YS[j + 1] for i, j in cells), min(YS[j] for i, j in cells),
                min(XS[i] for i, j in cells), max(XS[i + 1] for i, j in cells))

    def branch(cells):
        if len(cells) == 1:
            return 32768 | leaves[next(iter(cells))]
        top, bottom, left, right = bounds(cells)
        cols = sorted({i for i, j in cells})
        rows = sorted({j for i, j in cells})
        if len(rows) == 1 or (len(cols) > 1 and right - left >= top - bottom):
            cut = cols[len(cols) // 2]
            # Looking along a northbound partition, the right child is east.
            front = {c for c in cells if c[0] >= cut}
            axis = (XS[cut], bottom, 0, top - bottom)
        else:
            cut = rows[len(rows) // 2]
            front = {c for c in cells if c[1] < cut}
            axis = (left, YS[cut], right - left, 0)
        back = cells - front
        children = [branch(front), branch(back)]
        nodes.append((*axis, *bounds(front), *bounds(back), *children))
        return len(nodes) - 1

    branch(active)
    # Each block references every horizontal/vertical linedef touching its box.
    x0, y0 = XS[0] - 8, YS[0] - 8
    width = (XS[-1] - x0) // 128 + 1
    height = (YS[-1] - y0) // 128 + 1
    offsets, blocks = [], []
    for row in range(height):
        for col in range(width):
            bx, by = x0 + 128 * col, y0 + 128 * row
            indices = []
            for li, (va, vb, *_) in enumerate(lines):
                (ax, ay), (cx, cy) = vertices[va], vertices[vb]
                if (max(ax, cx) >= bx and min(ax, cx) <= bx + 128 and
                        max(ay, cy) >= by and min(ay, cy) <= by + 128):
                    indices.append(li)
            offsets.append(4 + width * height + len(blocks))
            blocks.extend((0, *indices, -1))
    blockmap = struct.pack("<4h", x0, y0, width, height) + records("<h", ((w,) for w in offsets + blocks))
    payloads = [b"", records("<5h", [(0, -480, 90, 1, 7)]), records("<7H", lines),
                records("<hh8s8s8sh", sides), records("<2h", vertices), records("<6H", segs),
                records("<2H", subsectors), records("<12h2H", nodes), records("<hh8s8s3h", sectors),
                bytes((len(sectors) ** 2 + 7) // 8), blockmap]
    return [(name.encode(), data) for name, data in zip(MAP_ORDER, payloads)]


def picture(image, sprite=False):
    width, height = image.size
    if height > 254:
        raise ValueError("Patch height exceeds the original Doom column format.")
    offsets, posts = [], bytearray()
    for x in range(width):
        offsets.append(8 + width * 4 + len(posts))
        y = 0
        while y < height:
            if sprite and image.getpixel((x, y)) == 255:
                y += 1
                continue
            first = y
            pixels = bytearray()
            while y < height and (not sprite or image.getpixel((x, y)) != 255):
                pixels.append(image.getpixel((x, y)))
                y += 1
            posts.extend((first, len(pixels), 0))
            posts.extend(pixels)
            posts.append(0)
        posts.append(255)
    anchor = (width // 2, height) if sprite else (0, 0)
    return struct.pack("<4h", width, height, *anchor) + records("<I", ((v,) for v in offsets)) + posts


def generate(output):
    from alley_art import graphics, balls
    images = graphics()
    lumps = map_lumps() + [(b"F_START", b"")]
    for flat in FLATS:
        im = images[flat]
        if im.size != (64, 64) or len(im.tobytes()) != 4096:
            raise ValueError(f"Flat {flat} must contain 4096 palette indices.")
        lumps.append((flat.encode(), im.tobytes()))
    lumps.extend([(b"F_END", b""), (b"P_START", b"")])
    lumps.extend((wall.encode(), picture(images[wall])) for wall in WALLS)
    lumps.extend([(b"P_END", b""), (b"S_START", b"")])
    frames = balls()
    if len(frames) != len(SPRITES):
        raise ValueError("Expected three ball rotation frames.")
    lumps.extend((name.encode(), picture(im, True)) for name, im in zip(SPRITES, frames))
    lumps.append((b"S_END", b""))
    lumps.append((b"BWHELD", (HERE / "held.pat").read_bytes()))
    data = bytearray(12)
    directory = []
    for name, payload in lumps:
        directory.append((len(data), len(payload), name))
        data.extend(payload)
    struct.pack_into("<4sII", data, 0, b"PWAD", len(directory), len(data))
    data.extend(records("<II8s", directory))
    output.write_bytes(data)


def packaged_lumps():
    raw = PWAD.read_bytes()
    tag, count, directory = struct.unpack_from("<4sII", raw)
    if tag != b"PWAD" or len(raw) != directory + 16 * count:
        raise ValueError("Invalid bundled Barrel Bowl map.")
    return [(name.rstrip(b"\x00"), raw[off:off + size])
            for off, size, name in struct.iter_unpack("<II8s", raw[directory:])]


def extend_textures(pnames, texture1, patches):
    original_count, = struct.unpack_from("<I", pnames)
    count, = struct.unpack_from("<I", texture1)
    positions = struct.unpack_from(f"<{count}I", texture1, 4)
    new_names = bytearray(pnames[4:])
    texture_records = [texture1[start:end] for start, end in zip(positions, positions[1:] + (len(texture1),))]
    for i, (name, data) in enumerate(patches):
        width, height = struct.unpack_from("<2h", data)
        new_names.extend(struct.pack("<8s", name))
        texture_records.append(struct.pack("<8sIhhIh5h", name, 0, width, height, 0, 1,
                                           0, 0, original_count + i, 0, 0))
    cursor = 4 + 4 * len(texture_records)
    offsets = []
    for record in texture_records:
        offsets.append(cursor)
        cursor += len(record)
    return (struct.pack("<I", original_count + len(patches)) + new_names,
            struct.pack("<I", len(texture_records)) + records("<I", ((v,) for v in offsets)) + b"".join(texture_records))


def apply(lumps):
    """Install the supplied standalone alley in an already assembled Omega IWAD."""
    overlay = packaged_lumps()
    names = [n for n, _ in lumps]
    replacements = {name: dict(overlay)[name] for name in (s.encode() for s in MAP_ORDER)}
    start = names.index(b"E1M9")
    if names[start:start + len(MAP_ORDER)] != list(replacements):
        raise ValueError("E1M9 map records must follow classic Doom order.")
    resources = dict(overlay)
    patches = [(s.encode(), resources[s.encode()]) for s in WALLS]
    pnames, texture1 = extend_textures(dict(lumps)[b"PNAMES"], dict(lumps)[b"TEXTURE1"], patches)
    inserts = {b"F_END": [(s.encode(), resources[s.encode()]) for s in FLATS],
               b"P_END": patches, b"S_END": [(s.encode(), resources[s.encode()]) for s in SPRITES]}
    result = []
    for index, (name, data) in enumerate(lumps):
        if name in inserts:
            result.extend(inserts[name])
        if start <= index < start + len(MAP_ORDER):
            data = replacements[name]
        elif name == b"PNAMES":
            data = pnames
        elif name == b"TEXTURE1":
            data = texture1
        result.append((name, bytes(data)))
    result.append((b"BWHELD", resources[b"BWHELD"]))
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=PWAD)
    args = parser.parse_args()
    generate(args.output)
    print(f"Wrote {args.output}")
