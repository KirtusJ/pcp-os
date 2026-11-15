"""Build and install the original Forklift Certified loading dock.

Use apply(lumps) in the installer. Run this module with Pillow to rebuild dock.wad.
"""
import argparse
from pathlib import Path
import random
import struct

HERE = Path(__file__).resolve().parent
PWAD = HERE / "dock.wad"
MAP_ORDER = "E1M9 THINGS LINEDEFS SIDEDEFS VERTEXES SEGS SSECTORS NODES SECTORS REJECT BLOCKMAP".split()
FLATS = "FKYARD FKWET FKRAIL FKSTOP FKEDGE FKBAY FKROOF FKHIGH".split()
WALLS = "FKPORT FKBLU FKROX FKAMBR FKBLK FKBAY1 FKBAY2 FKBAY3 FKDOOR FKBEAM".split()
XS = [-608, -544, -448, -416, -288, -256, -208, -128, -48, 48, 128, 208, 256, 288, 416, 448, 544, 608]
YS = [-512, -448, -416, -320, -256, -160, -48, 48, 112, 208, 304, 416, 480, 544]
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
        x = (XS[ix] + XS[ix + 1]) // 2
        y = (YS[iy] + YS[iy + 1]) // 2
        perimeter = abs(x) > 544 or y > 480 or y < -448
        paint_rails = 416 < abs(x) < 448
        cargo_pad = (288 < abs(x) < 416 and 112 < y < 208) or (abs(x) < 48 and -416 < y < -320)
        flat = "FKYARD"
        if perimeter:
            flat = "FKEDGE"
        elif cargo_pad:
            flat = "FKBAY"
        elif paint_rails:
            flat = "FKRAIL"
        elif -448 < y < -416:
            flat = "FKSTOP"
        elif (-208 < x < -48 and -256 < y < 48) or (48 < x < 208 and -48 < y < 304):
            flat = "FKWET"
        # The low perimeter canopy leaves an open, 192-unit-high central yard.
        ceiling = 128 if perimeter else 192
        light = 190 if perimeter else 214
        if cargo_pad or (perimeter and (abs(x) > 544 or abs(x) < 256)):
            light = 230
        spec = (0, ceiling, flat.encode(), b"FKROOF" if perimeter else b"FKHIGH", light, 0, 0)
        if spec not in sector_ids:
            sector_ids[spec] = len(sectors)
            sectors.append(spec)
        return sector_ids[spec]

    active = {(i, j) for i in range(len(XS)-1) for j in range(len(YS)-1)}

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
                # The high central roof meets a lower perimeter canopy.
                top = b"FKBEAM"
                shift = (a[0] if side in (1, 3) else a[1]) % 128
            elif side == 1:
                if -256 <= a[0] < 256:
                    mid, shift = b"FKPORT", a[0] + 256
                else:
                    mid = b"FKROX" if a[0] < 0 else b"FKBLU"
                    shift = a[0] % 128
            elif side == 3:
                if -128 < a[0] <= 128:
                    mid, shift = b"FKBAY3", 128 - a[0]
                elif 208 < a[0] <= 288:
                    mid, shift = b"FKDOOR", (288 - a[0]) % 128
                else:
                    mid, shift = b"FKAMBR", -a[0] % 128
            elif 48 <= min(a[1], b[1]) and max(a[1], b[1]) <= 304:
                mid = b"FKBAY1" if side == 0 else b"FKBAY2"
                shift = a[1] - 48 if side == 0 else 304 - a[1]
            else:
                mid = (b"FKBLK" if y < -160 else b"FKBLU") if side == 0 else b"FKROX"
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

    # First shotgun can be collected at spawn. Fixed pickups don't block a charge.
    things = [(0, -304, 90, 1, 7), (8, -299, 0, 2001, 7),
              (-80, -256, 0, 2049, 7), (80, -256, 0, 2049, 7),
              (-470, -268, 0, 2012, 7), (470, -268, 0, 2012, 7),
              (-472, 340, 0, 2049, 7), (472, 340, 0, 2049, 7),
              (-160, 408, 0, 2011, 7), (160, 408, 0, 2011, 7)]
    payloads = [b"", records("<5h", things), records("<7H", lines),
                records("<hh8s8s8sh", sides), records("<2h", vertices), records("<6H", segs),
                records("<2H", subsectors), records("<12h2H", nodes), records("<hh8s8s3h", sectors),
                bytes((len(sectors) ** 2 + 7) // 8), blockmap]
    return [(name.encode(), data) for name, data in zip(MAP_ORDER, payloads)]


def picture(image):
    """Encode each opaque wall column as one native Doom post."""
    width, height = image.size
    if height > 254:
        raise ValueError("Patch height exceeds the original Doom column format.")
    offsets, posts = [], bytearray()
    for x in range(width):
        offsets.append(8 + width * 4 + len(posts))
        posts.extend((0, height, 0))
        posts.extend(image.crop((x, 0, x + 1, height)).tobytes())
        posts.extend((0, 255))
    return struct.pack("<4h", width, height, 0, 0) + records("<I", ((v,) for v in offsets)) + posts


def packaged_lumps():
    raw = PWAD.read_bytes()
    if len(raw) < 12:
        raise ValueError("Incomplete dock PWAD header.")
    tag, count, directory = struct.unpack_from("<4sII", raw)
    if tag != b"PWAD" or len(raw) != directory + 16 * count:
        raise ValueError("Invalid bundled dock map.")
    result = []
    for off, size, name in struct.iter_unpack("<II8s", raw[directory:]):
        if off < 12 or off + size > directory:
            raise ValueError("Dock PWAD entry extends outside its data section.")
        result.append((name.rstrip(b"\x00"), raw[off:off+size]))
    return result


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
    """Install the loading dock map and all artwork in the supplied campaign IWAD."""
    overlay = packaged_lumps()
    names = [n for n, _ in lumps]
    resources = dict(overlay)
    replacements = {name: resources[name] for name in (s.encode() for s in MAP_ORDER)}
    start = names.index(b"E1M9")
    if names[start:start + len(MAP_ORDER)] != list(replacements):
        raise ValueError("E1M9 map records must follow classic Doom order.")
    patches = [(s.encode(), resources[s.encode()]) for s in WALLS]
    pnames, texture1 = extend_textures(dict(lumps)[b"PNAMES"], dict(lumps)[b"TEXTURE1"], patches)
    inserts = {b"F_END": [(s.encode(), resources[s.encode()]) for s in FLATS],
               b"P_END": patches}
    if set(resources) != {s.encode() for s in MAP_ORDER + FLATS + WALLS} | {b"F_START", b"F_END", b"P_START", b"P_END"}:
        raise ValueError("Unexpected dock artwork or map records.")
    if any(name.encode() in names for name in FLATS + WALLS):
        raise ValueError("Input already contains dock artwork.")
    if any(names.count(marker) != 1 for marker in (b"F_END", b"P_END", b"PNAMES", b"TEXTURE1")):
        raise ValueError("Campaign texture namespace is invalid.")
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
    return result


# Pixel-cut stencil letters. These glyphs do not depend on host fonts.
GLYPHS = {
    '0': ('111','101','101','101','111'), '1': ('010','110','010','010','111'),
    '2': ('111','001','111','100','111'), '3': ('111','001','111','001','111'),
    'A': ('01110','11011','11111','11011','11011'),
    'B': ('11110','11011','11110','11011','11110'),
    'C': ('01111','11000','11000','11000','01111'),
    'D': ('11110','11011','11011','11011','11110'),
    'E': ('11111','11000','11110','11000','11111'),
    'F': ('11111','11000','11110','11000','11000'),
    'G': ('01111','11000','11011','11001','01111'),
    'H': ('11011','11011','11111','11011','11011'),
    'I': ('111','010','010','010','111'),
    'K': ('11011','11110','11100','11110','11011'),
    'L': ('11000','11000','11000','11000','11111'),
    'N': ('11001','11101','11011','11001','11001'),
    'O': ('01110','11011','11011','11011','01110'),
    'P': ('11110','11011','11110','11000','11000'),
    'R': ('11110','11011','11110','11100','11011'),
    'S': ('01111','11000','01110','00011','11110'),
    'T': ('11111','00100','00100','00100','00100'),
    'Y': ('10001','11011','01110','00100','00100'),
}


def stencil(d, text, x, y, sx, sy, color):
    for char in text:
        if char == ' ':
            x += 3 * sx
            continue
        glyph = GLYPHS[char]
        for row, pattern in enumerate(glyph):
            for col, dot in enumerate(pattern):
                if dot == '1':
                    left, top = x + col*sx, y + row*sy
                    d.rectangle((left, top, left+sx-1, top+sy-1), fill=color)
        x += (len(glyph[0])+1)*sx


def ribbed(width, palette, seed):
    from PIL import Image, ImageDraw
    im = Image.new('P', (width, 128))
    d = ImageDraw.Draw(im)
    rng = random.Random(seed)
    a, b, c, shadow = palette
    for x in range(width):
        tone = [shadow, shadow, c, b, a, a, a, b, b, b, c, c, c, c, shadow, shadow][x%16]
        d.line((x, 4, x, 121), fill=tone)
    for y, tone in [(0, 7), (2, a), (3, c), (5, shadow), (111, shadow),
                    (114, c), (116, b), (118, shadow), (122, 111), (125, 7)]:
        d.rectangle((0, y, width-1, y+1), fill=tone)
    for _ in range(width*2):
        x, y = rng.randrange(width), rng.randrange(6, 109)
        if y > 88 or x % 16 < 3:
            d.line((x, y, x, y+rng.randrange(1, 8)), fill=rng.choice([shadow, 77, 109]))
    for x in range(7, width, 32):
        for y in (3, 115):
            d.point((x, y), fill=99)
            d.point((x+1, y+1), fill=7)
    return im


def cargo_box(style):
    from PIL import ImageDraw
    ramps = {
        'blue': (197, 204, 206, 242), 'oxide': (36, 40, 45, 239),
        'amber': (164, 71, 74, 78), 'black': (103, 106, 109, 111),
    }
    im = ribbed(128, ramps[style], 60+list(ramps).index(style))
    d = ImageDraw.Draw(im)
    for x in (4, 62, 121):
        d.rectangle((x, 4, x+3, 110), fill=7)
        d.line((x+1, 5, x+1, 108), fill=139)
        for y in (21, 68, 98):
            d.rectangle((x-2, y, x+4, y+2), fill=109, outline=104)
    d.rectangle((76, 16, 111, 31), fill=110, outline=143)
    stencil(d, '020', 82, 19, 2, 2, 130)
    for x in range(80, 109, 3):
        d.line((x, 39, x, 45-x%3), fill=131)
    d.rectangle((15, 74, 36, 92), fill=7, outline=75)
    d.polygon([(25, 76), (34, 89), (17, 89)], fill=163)
    d.line((25, 80, 25, 85), fill=7, width=2)
    d.point((25, 87), fill=7)
    return im


def lamp(d, x, y):
    d.rectangle((x-6, y-8, x+6, y+8), fill=7, outline=142)
    for radius, color in [(4, 124), (3, 118), (2, 112)]:
        d.ellipse((x-radius, y-radius, x+radius, y+radius), fill=color)
    d.point((x-1, y-1), fill=225)


def shutter(number):
    from PIL import Image, ImageDraw
    im = Image.new('P', (256, 128), 110)
    d = ImageDraw.Draw(im)
    for y in range(18, 119, 6):
        d.rectangle((8, y, 247, y+5), fill=104 if y < 70 else 107)
        d.line((8, y, 247, y), fill=101 if y < 70 else 104)
        d.line((8, y+4, 247, y+4), fill=110)
    for x in (5, 247):
        d.rectangle((x, 17, x+3, 119), fill=7)
        d.line((x, 18, x, 118), fill=134)
    d.rectangle((10, 4, 245, 18), fill=7, outline=145)
    stencil(d, 'BAY', 18, 7, 2, 2, 161)
    d.line((61, 11, 210, 11), fill=75)
    lamp(d, 232, 11)
    stencil(d, '0'+str(number), 90, 33, 11, 13, 109)
    stencil(d, '0'+str(number), 86, 30, 11, 13, 162)
    for y in (47, 77):
        d.line((65, y, 192, y), fill=105, width=2)
    for x in range(0, 256, 22):
        d.polygon([(x, 121), (x+10, 121), (x+5, 127), (x-5, 127)], fill=164)
    return im


def beacon():
    from PIL import ImageDraw
    im = ribbed(512, (105, 108, 111, 7), 171)
    d = ImageDraw.Draw(im)
    d.rectangle((22, 15, 489, 103), fill=7, outline=143, width=3)
    d.rectangle((26, 19, 485, 99), outline=75, width=1)
    for x in (31, 479):
        for y in (22, 95):
            d.ellipse((x-1, y-1, x+1, y+1), fill=99)
    # The company emblem is a forklift carrying a pallet inside a safety diamond.
    d.polygon([(65, 29), (98, 61), (65, 93), (32, 61)], fill=163, outline=75)
    d.line([(48, 68), (49, 59), (58, 58), (59, 48), (68, 48),
            (73, 60), (73, 68), (48, 68)], fill=7, width=4)
    d.line((59, 59, 64, 59, 69, 62), fill=7, width=2)
    for x in (52, 68):
        d.ellipse((x-4, 64, x+4, 72), fill=7)
        d.point((x, 68), fill=161)
    d.line((76, 48, 76, 68, 88, 68), fill=7, width=3)
    d.rectangle((79, 54, 88, 64), fill=7)
    stencil(d, 'NIGHT FREIGHT', 109, 36, 5, 8, 77)
    stencil(d, 'NIGHT FREIGHT', 107, 33, 5, 8, 160)
    d.line((108, 80, 436, 80), fill=75)
    stencil(d, 'PHOBOS PORT', 109, 85, 2, 2, 133)
    for x in (448, 467):
        lamp(d, x, 59)
    for x in range(0, 512, 20):
        d.polygon([(x, 115), (x+8, 115), (x, 126), (x-8, 126)], fill=74)
    return im


def exit_door():
    from PIL import ImageDraw
    im = ribbed(128, (105, 108, 110, 7), 281)
    d = ImageDraw.Draw(im)
    d.rectangle((15, 9, 62, 122), fill=111, outline=139, width=3)
    d.rectangle((22, 19, 55, 55), fill=7, outline=137)
    for x in range(26, 54, 5):
        d.line((x, 22, x, 52), fill=242)
    d.line((45, 75, 55, 75), fill=132, width=3)
    lamp(d, 71, 32)
    return im


def roof_beam():
    from PIL import Image, ImageDraw
    im = Image.new('P', (128, 128), 110)
    d = ImageDraw.Draw(im)
    for y in range(0, 128, 32):
        d.rectangle((0, y, 127, y+2), fill=104)
        d.line((0, y+3, 127, y+3), fill=7)
        d.line((0, y+27, 127, y+27), fill=7)
        for x in range(0, 128, 32):
            d.line((x, y+24, x+25, y+6), fill=107, width=3)
            d.point((x+2, y+1), fill=99)
    return im


def pavement(style):
    from PIL import Image, ImageDraw
    rng = random.Random(129)
    im = Image.new('P', (64, 64), 109)
    d = ImageDraw.Draw(im)
    for y in range(64):
        for x in range(64):
            grain = rng.random()
            tone = 108 if grain < .12 else 110 if grain > .75 else 109
            d.point((x, y), fill=tone)
    if style in ('wet', 'yard'):
        for x, y, w in [(7, 6, 5), (11, 9, 12), (3, 12, 7), (38, 37, 10),
                        (35, 39, 18), (44, 42, 5), (17, 55, 14)]:
            d.line((x, y, x+w, y), fill=106 if style=='wet' else 108)
            if style=='wet':
                d.line((x+3, y+1, x+w//2+2, y+1), fill=103)
        d.line([(0, 28), (8, 30), (15, 29), (23, 33), (30, 33)], fill=111)
    if style in ('rail', 'stop'):
        for x in range(10, 16):
            d.line((x, 0, x, 63), fill=163 if x>10 else 74)
        d.line((22, 0, 22, 63), fill=164, width=2)
        for _ in range(22):
            x, y = rng.randrange(10, 16), rng.randrange(64)
            d.point((x, y), fill=109)
        if style=='stop':
            im = im.transpose(Image.Transpose.ROTATE_90)
    elif style=='edge':
        d.rectangle((1, 1, 62, 62), outline=111)
        for y in (8, 39):
            for x in range(6, 58, 6):
                d.line((x, y, x, y+16), fill=7, width=3)
                d.line((x+2, y, x+2, y+15), fill=105)
    elif style=='bay':
        for x in range(-64, 128, 16):
            d.polygon([(x, 0), (x+5, 0), (x-58, 63), (x-63, 63)], fill=164)
        for _ in range(105):
            x, y = rng.randrange(64), rng.randrange(64)
            if im.getpixel((x, y))==164:
                d.point((x, y), fill=107)
    elif style=='roof':
        d.rectangle((0, 0, 63, 63), fill=111, outline=108)
        for x in range(3, 64, 8):
            d.line((x, 2, x, 61), fill=109)
    elif style=='high':
        d.rectangle((0, 0, 63, 63), fill=7)
        d.line((0, 63, 63, 0), fill=111)
        d.line((0, 1, 63, 1), fill=110)
    return im


def generate(output):
    surfaces = [pavement(style) for style in ('yard','wet','rail','stop','edge','bay','roof','high')]
    walls = [beacon(), cargo_box('blue'), cargo_box('oxide'), cargo_box('amber'),
             cargo_box('black'), shutter(1), shutter(2), shutter(3), exit_door(), roof_beam()]
    lumps = map_lumps() + [(b'F_START', b'')]
    lumps.extend((name.encode(), im.tobytes()) for name, im in zip(FLATS, surfaces))
    lumps.extend([(b'F_END', b''), (b'P_START', b'')])
    lumps.extend((name.encode(), picture(im)) for name, im in zip(WALLS, walls))
    lumps.append((b'P_END', b''))
    body = bytearray(12)
    directory = []
    for name, payload in lumps:
        directory.append((len(body), len(payload), name))
        body.extend(payload)
    struct.pack_into('<4sII', body, 0, b'PWAD', len(directory), len(body))
    body.extend(records('<II8s', directory))
    output.write_bytes(body)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=PWAD)
    generate(parser.parse_args().output)
