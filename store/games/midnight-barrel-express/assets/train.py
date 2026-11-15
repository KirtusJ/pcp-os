"""Build or install Midnight Barrel Express's native shareware E1M9 train.

The installer needs only train.py and train.wad. Rebuilding the artwork uses Pillow.
"""
import argparse
import math
from pathlib import Path
import random
import struct

HERE = Path(__file__).resolve().parent
PWAD = HERE / "train.wad"
MAP_ORDER = "E1M9 THINGS LINEDEFS SIDEDEFS VERTEXES SEGS SSECTORS NODES SECTORS REJECT BLOCKMAP".split()
FLATS = "RLTRACK RLGRAVL RLWOOD RLCARGO RLCOAL RLCAB RLLINK RLROOF RLNIGHT".split()
WALLS = "RLMESAE RLMESAW RLSIG RLCOACH RLWAGON RLTENDER RLCABIN RLLOCKA RLLOCKB RLBRAKE RLTRIM RLSILL RLGUARD RLHEAD1 RLHEAD2 RLHEAD3".split()
XS = [-416, -192, -96, -40, 40, 96, 192, 416]
YS = [-1088, -768, -464, -432, -304, -240, -64, -32, 128, 192, 352, 384, 768, 1088]
CARS = [(-768, -464), (-432, -64), (-32, 352), (384, 768)]


def car_at(y):
    return next((index for index, (start, end) in enumerate(CARS) if start < y < end), None)


def deck_at(x, y):
    if not -768 < y < 768:
        return False
    return abs(x) < (96 if car_at(y) is not None else 40)

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



    def center(ix, iy):
        return (XS[ix] + XS[ix+1]) // 2, (YS[iy] + YS[iy+1]) // 2

    def surface(ix, iy):
        x, y = center(ix, iy)
        car = car_at(y)
        if deck_at(x, y):
            flat = b"RLLINK" if car is None else (b"RLWOOD", b"RLCARGO", b"RLCOAL", b"RLCAB")[car]
            roof = b"RLNIGHT" if car is None or car == 2 else b"RLROOF"
            spec = (24, 144 if roof == b"RLNIGHT" else 112, flat, roof, 216, 0, 0)
        else:
            flat = b"RLTRACK" if abs(x) < 192 else b"RLGRAVL"
            spec = (-48, 144, flat, b"RLNIGHT", 224, 0, 0)
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
            shift = row = tag = 0
            flags = 4 if inner else 1


            if inner:
                own = deck_at(*center(ix, iy))
                across = deck_at(*center(*neighbor))
                if own != across:
                    # The floor side is visible from outside; the line keeps feet on deck.
                    flags |= 1
                    low, top = b"RLSILL", b"RLTRIM"
                    yy_mid = (a[1]+b[1])//2
                    which = car_at(yy_mid)
                    if a[0] == b[0] and abs(a[0]) == 96 and which is not None:
                        mid = (b"RLCOACH", b"RLWAGON", b"RLTENDER", b"RLCABIN")[which]
                        shift = (a[1] if side == 0 else -a[1]) % 128
                        if a[0] == -96 and -304 < yy_mid < -240:
                            mid = b"RLLOCKA"
                            shift = (a[1] + 304 if side == 0 else -240 - a[1]) % 64
                        if a[0] == 96 and 128 < yy_mid < 192:
                            mid = b"RLLOCKB"
                            shift = (a[1] - 128 if side == 0 else 192 - a[1]) % 64
                    else:
                        mid = b"RLGUARD"
                        shift = (a[0] if side == 1 else -a[0]) % 128
                        if a[1] == b[1] == 768 and -40 <= a[0] <= 40 and -40 <= b[0] <= 40:
                            mid = b"RLBRAKE"
                            shift = (a[0]+64 if side == 1 else 64-a[0]) % 128
                elif a[0] == b[0] and abs(a[0]) == 192:
                    # Both faces of a trackside signal are true transparent textures.
                    tag, mid = 772, b"RLSIG"
                    shift = (a[1] if side == 0 else -a[1]) % 1024
                elif (a[1] == b[1] and a[1] in (-432,-32,384)
                      and -40 <= a[0] <= 40 and -40 <= b[0] <= 40):
                    mid = {-432:b"RLHEAD1", -32:b"RLHEAD2", 384:b"RLHEAD3"}[a[1]]
                    shift = (a[0]+64 if side == 1 else 64-a[0]) % 128
                    top = b"RLTRIM"
                elif own:
                    top = b"RLTRIM"
            elif a[0] == b[0]:
                mid = b"RLMESAW" if a[0] < 0 else b"RLMESAE"
                tag = 771
                shift = (a[1] if side == 0 else -a[1]) % 256
                row = 28
            else:
                mid = b"RLMESAE" if side == 1 else b"RLMESAW"
                shift = (a[0] if side == 1 else -a[0]) % 256
                row = 28
            sd = len(sides)
            sides.append((shift, row, top, low, mid, sec))
            if edge in line_ids:
                li = line_ids[edge]
                lines[li][-1] = sd
                direction = 1
            else:
                li = line_ids[edge] = len(lines)
                lines.append([va, vb, flags, 0, tag, sd, 65535])
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


    things = [(0, -640, 90, 1, 7), (7, -635, 0, 2001, 7),
              (16, -580, 0, 2049, 7), (63, -360, 0, 2011, 7),
              (-54, -176, 270, 3004, 7), (52, 80, 270, 3004, 7),
              (40, -112, 0, 2049, 7), (-48, 320, 270, 9, 7),
              (-66, 238, 0, 2012, 7), (48, 442, 0, 2049, 7),
              (53, 512, 270, 3004, 7), (-64, 614, 0, 2012, 7)]
    payloads = [b"", records("<5h", things), records("<7H", lines),
                records("<hh8s8s8sh", sides), records("<2h", vertices), records("<6H", segs),
                records("<2H", subsectors), records("<12h2H", nodes), records("<hh8s8s3h", sectors),
                bytes((len(sectors) ** 2 + 7) // 8), blockmap]
    return [(name.encode(), data) for name, data in zip(MAP_ORDER, payloads)]


def packaged_lumps():
    raw = PWAD.read_bytes()
    if len(raw) < 12:
        raise ValueError("Incomplete train PWAD header.")
    tag, count, directory = struct.unpack_from("<4sII", raw)
    if tag != b"PWAD" or len(raw) != directory + 16 * count:
        raise ValueError("Invalid bundled train map.")
    result = []
    for off, size, name in struct.iter_unpack("<II8s", raw[directory:]):
        if off < 12 or off + size > directory:
            raise ValueError("Train PWAD entry extends outside its data section.")
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
    """Install the train map and all artwork in the supplied campaign IWAD."""
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
        raise ValueError("Unexpected train artwork or map records.")
    if any(name.encode() in names for name in FLATS + WALLS):
        raise ValueError("Input already contains train artwork.")
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



GLYPHS = {
    'A': ('01110','10001','11111','10001','10001'),
    'B': ('11110','10001','11110','10001','11110'),
    'C': ('01111','10000','10000','10000','01111'),
    'D': ('11110','10001','10001','10001','11110'),
    'E': ('11111','10000','11110','10000','11111'),
    'F': ('11111','10000','11110','10000','10000'),
    'G': ('01111','10000','10111','10001','01111'),
    'H': ('10001','10001','11111','10001','10001'),
    'I': ('111','010','010','010','111'),
    'K': ('10001','10010','11100','10010','10001'),
    'L': ('10000','10000','10000','10000','11111'),
    'M': ('10001','11011','10101','10001','10001'),
    'N': ('10001','11001','10101','10011','10001'),
    'O': ('01110','10001','10001','10001','01110'),
    'P': ('11110','10001','11110','10000','10000'),
    'R': ('11110','10001','11110','10100','10010'),
    'S': ('01111','10000','01110','00001','11110'),
    'T': ('11111','00100','00100','00100','00100'),
    'U': ('10001','10001','10001','10001','01110'),
    'W': ('10001','10001','10101','11011','10001'),
    'X': ('10001','01010','00100','01010','10001'),
    'Y': ('10001','01010','00100','00100','00100'),
    '1': ('010','110','010','010','111'),
    '2': ('111','001','111','100','111'),
    '3': ('111','001','111','001','111'),
}
CLEAR = 255


def text(d, string, cx, y, sx=1, sy=1, color=225):
    width = sum((len(GLYPHS[c][0])+1 if c!=' ' else 3)*sx for c in string)-sx
    x = cx-width//2
    for c in string:
        if c == ' ':
            x += 3*sx
            continue
        for j, row in enumerate(GLYPHS[c]):
            for i, pix in enumerate(row):
                if pix=='1':
                    u,v = x+i*sx, y+j*sy
                    d.rectangle((u,v,u+sx-1,v+sy-1),fill=color)
        x += (len(GLYPHS[c][0])+1)*sx


def mesa(west):
    from PIL import Image, ImageDraw
    im = Image.new('P',(256,128),245)
    d = ImageDraw.Draw(im)
    rng = random.Random(11 if west else 30)
    # Violet twilight dims upward, leaving a thin copper sunset on the mesa horizon.
    for y,col in [(20,244),(39,243),(51,242),(62,254),(67,46),(73,236)]:
        d.rectangle((0,y,255,127),fill=col)
    for _ in range(39):
        d.point((rng.randrange(256),rng.randrange(1,60)),fill=rng.choice([194,196,97,241]))
    if west:
        d.ellipse((174,24,185,35),fill=193)
        d.ellipse((177,22,188,33),fill=244)
    far = [(0,86),(21,77),(34,78),(50,65),(58,65),(75,75),(87,70),
           (99,68),(109,51),(119,51),(125,67),(145,69),(152,64),
           (177,65),(186,72),(196,65),(206,65),(218,80),(235,79),(255,86)]
    points = far if west else [(255-x,y+4) for x,y in reversed(far)]
    d.polygon(points+[(255,127),(0,127)],fill=241)
    d.line(points,fill=253,width=1)
    ledges = [(0,100),(18,96),(31,95),(40,83),(57,82),(65,65),(78,65),
              (83,83),(99,84),(107,97),(134,101),(148,90),(169,89),
              (176,82),(194,82),(203,95),(230,96),(255,100)]
    if not west:
        ledges=[(255-x,y+3) for x,y in reversed(ledges)]
    d.polygon(ledges+[(255,127),(0,127)],fill=239)
    d.line(ledges,fill=74,width=1)
    for _ in range(75):
        x,y=rng.randrange(256),rng.randrange(69,125)
        if im.getpixel((x,y))==239:
            d.line((x,y,x+1,y+3),fill=238 if y%4 else 77)
    d.rectangle((0,111,255,127),fill=111)
    for _ in range(27):
        x,y=rng.randrange(256),rng.randrange(112,126)
        d.line((x,y,x+4,y),fill=109)
    return im


def signals():
    from PIL import Image,ImageDraw
    im=Image.new('P',(1024,192),CLEAR)
    d=ImageDraw.Draw(im)
    # Each signal is opaque only on its mast and lamp faces. The landscape shows through.
    for x,green in [(125,True),(734,False)]:
        d.polygon([(x-6,188),(x-3,175),(x+4,175),(x+9,188)],fill=105,outline=7)
        d.rectangle((x-3,51,x+3,178),fill=141,outline=7)
        d.line((x-2,57,x-2,176),fill=130)
        for y in range(90,171,11):
            d.line((x-6,y,x+1,y),fill=100)
        d.rounded_rectangle((x-9,42,x+8,89),radius=5,fill=7,outline=102)
        for y,col in [(52,116 if green else 174),(66,75),(80,124 if green else 38)]:
            d.ellipse((x-5,y-4,x+4,y+5),fill=col)
            d.point((x-2,y-3),fill=225 if col in (116,174) else 133)
    for x in (276,470,887):
        d.rectangle((x,109,x+2,185),fill=141,outline=7)
        d.rectangle((x-8,105,x+10,116),fill=161,outline=7)
        for j in (-4,0,4):
            d.line((x+j,107,x+j+3,113),fill=7)
    # A tall cross-braced water tower and an open rock cutting are easy to track
    # from the center aisle, even between the narrower caboose window posts.
    x=490
    d.line((x-22,84,x-27,181),fill=142,width=4)
    d.line((x+22,84,x+27,181),fill=142,width=4)
    for y in (97,133):
        d.line((x-20,y,x+21,y+31),fill=138,width=3)
        d.line((x+20,y,x-21,y+31),fill=141,width=3)
    d.rectangle((x-25,49,x+25,84),fill=74,outline=7,width=2)
    for u in range(x-22,x+24,6):
        d.line((u,51,u,83),fill=142)
    for y in (56,75):d.line((x-25,y,x+25,y),fill=103,width=2)
    d.polygon([(x-31,49),(x,35),(x+31,49)],fill=140,outline=7)
    d.line((x-27,47,x,35),fill=132)
    d.rectangle((x-4,31,x+3,34),fill=109)
    d.point((x,30),fill=175)
    d.polygon([(848,184),(854,145),(867,143),(877,106),(891,99),(905,102),
               (909,132),(922,138),(934,183)],fill=238,outline=77)
    d.line((866,164,880,116,890,107),fill=71,width=2)
    # Trackside ballast piles and timber, low on the horizon.
    for x,w in [(32,21),(330,32),(543,38),(770,20),(949,34)]:
        d.polygon([(x,184),(x+3,179),(x+w//2,172),(x+w-3,176),(x+w,183)],fill=143,outline=7)
        d.line((x+4,179,x+w//2,173,x+w-3,177),fill=138)
    return im


def wagon(kind):
    from PIL import Image,ImageDraw
    size=(128,128)
    im=Image.new('P',size,CLEAR)
    d=ImageDraw.Draw(im)
    bottom=81 if kind in ('tender','guard') else 63
    if kind=='guard':
        d.rectangle((0,64,127,67),fill=132)
        d.line((0,68,127,68),fill=75)
        for x in (3,44,85,124):
            d.rectangle((x,64,x+2,119),fill=142)
        return im
    if kind=='tender':
        bottom=83
    colors={'coach':(243,206,195),'wagon':(239,43,64),'tender':(111,106,133),'cab':(125,122,147)}
    shadow,base,edge=colors[kind]
    d.rectangle((0,bottom,127,127),fill=shadow)
    d.rectangle((0,bottom+3,127,119),fill=base)
    for y in (bottom,bottom+2,bottom+9,bottom+33):
        d.line((0,y,127,y),fill=162 if y==bottom+2 else 74)
    for x in range(1,128,16):
        d.line((x,bottom+12,x,119),fill=shadow)
        d.point((x+2,bottom+6),fill=133)
    for x in (1,124):
        d.rectangle((x,0,x+2,bottom),fill=141)
        d.line((x,0,x,bottom),fill=edge)
    if kind!='tender':
        d.rectangle((0,0,127,4),fill=shadow)
        d.line((0,5,127,5),fill=163)
        for x in (31,92):
            d.line((x,5,x,bottom),fill=edge)
            d.line((x+1,5,x+1,bottom),fill=shadow)
    if kind in ('coach','cab'):
        cx=64
        for u in range(3):
            d.line((cx-25+u*3,bottom+15+u*3,cx-6,bottom+15+u*3),fill=162)
            d.line((cx+6,bottom+15+u*3,cx+25-u*3,bottom+15+u*3),fill=162)
        d.polygon([(cx,bottom+10),(cx+6,bottom+17),(cx,bottom+24),(cx-6,bottom+17)],outline=160)
    return im


def control(label):
    from PIL import Image,ImageDraw
    width=128 if label=='BRAKE' else 64
    im=Image.new('P',(width,128),CLEAR)
    d=ImageDraw.Draw(im)
    cx=width//2
    # Iron stand occupies the lower half, leaving the glass around the control clear.
    d.rectangle((0,70,width-1,126),fill=109)
    d.line((0,71,width-1,71),fill=162,width=2)
    d.rounded_rectangle((cx-29,10,cx+28,60),radius=4,fill=7,outline=163,width=2)
    for x in (cx-24,cx+24):
        d.ellipse((x-2,15,x+2,19),fill=112)
    if label=='BRAKE':
        text(d,'BRAKE',cx,22,1,2,225)
        text(d,'STAND',cx,41,1,2,161)
    else:
        text(d,'INTERLOCK',cx,19,1,1,225)
        text(d,label,cx,31,3,4,161)
    for x in (cx-21,cx+20):
        d.ellipse((x-3,74,x+4,81),fill=7,outline=134)
        d.line((x-1,76,x+2,77),fill=173)
    return im


def header(label):
    from PIL import Image,ImageDraw
    im=Image.new('P',(128,32),CLEAR)
    d=ImageDraw.Draw(im)
    d.rounded_rectangle((21,0,106,15),radius=2,fill=7,outline=163)
    text(d,label,64,3,1,2,225)
    d.rectangle((20,2,22,11),fill=116)
    d.rectangle((105,2,107,11),fill=116)
    return im


def trim(style):
    from PIL import Image,ImageDraw
    im=Image.new('P',(128,128),110)
    d=ImageDraw.Draw(im)
    for y in range(0,128,16):
        d.line((0,y,127,y),fill=141)
        d.line((0,y+2,127,y+2),fill=7)
        for x in range(5,128,12):
            d.point((x,y+1),fill=134)
    if style=='sill':
        for x in range(-12,128,26):
            d.ellipse((x,71,x+31,103),fill=7,outline=104,width=2)
            d.ellipse((x+11,82,x+20,92),fill=108,outline=136)
    return im


def flat(style):
    from PIL import Image,ImageDraw
    rng=random.Random(47)
    im=Image.new('P',(64,64),110)
    d=ImageDraw.Draw(im)
    if style in ('track','gravel'):
        d.rectangle((0,0,63,63),fill=143)
        for _ in range(1250):
            x,y=rng.randrange(64),rng.randrange(64)
            d.line((x,y,x+1,y),fill=rng.choice([142,143,235,238,109,111,76]))
        if style=='track':
            for y in (5,34):
                d.rectangle((0,y,63,y+6),fill=78)
                d.line((0,y,63,y),fill=72)
            for x in (12,49):
                d.rectangle((x,0,x+3,63),fill=103)
                d.line((x,0,x,63),fill=131)
                d.line((x+4,0,x+4,63),fill=7)
        return im
    if style in ('wood','cargo'):
        board=8 if style=='wood' else 16
        for x in range(64):
            ridge=x%board
            for y in range(64):
                color=(73 if style=='wood' else 71)+(1 if (x//board)%3==2 else 0)
                if ridge==0: color=77
                elif ridge==1:color-=2
                elif (x*5+y//9)%17==0:color+=1
                d.point((x,y),fill=color)
        if style=='cargo':
            d.rectangle((0,6,63,8),fill=107)
            for x in range(3,64,12):d.point((x,7),fill=131)
    elif style=='coal':
        d.rectangle((0,0,63,63),fill=110)
        for _ in range(240):
            x,y=rng.randrange(64),rng.randrange(64)
            d.line((x,y,x+2,y+1),fill=rng.choice([7,111,107,104]))
        d.line((10,0,10,63),fill=108)
    elif style in ('cab','link'):
        d.rectangle((0,0,63,63),fill=105 if style=='link' else 108)
        for y in range(0,64,8):
            for x in range(y%16,64,16):
                d.line((x,y,x+3,y+2),fill=101)
                d.line((x,y+1,x+3,y+3),fill=110)
        if style=='link':
            for y in (0,15,48):
                d.rectangle((0,y,63,y+2),fill=110)
                d.line((0,y,63,y),fill=134)
    elif style=='roof':
        d.rectangle((0,0,63,63),fill=142)
        for y in (1,32):
            d.rectangle((0,y,63,y+2),fill=109)
        d.rectangle((28,7,35,18),fill=162,outline=77)
        d.rectangle((30,9,33,16),fill=225)
    else:
        d.rectangle((0,0,63,63),fill=245)
        d.point((12,7),fill=195)
    return im


def picture(image):
    """Encode transparent gaps into native posts; reject offsets the renderer truncates."""
    width,height=image.size
    offsets,posts=[],bytearray()
    for x in range(width):
        offsets.append(8+4*width+len(posts))
        y=0
        while y<height:
            if image.getpixel((x,y))==CLEAR:
                y+=1
                continue
            top=y
            while y<height and image.getpixel((x,y))!=CLEAR:
                y+=1
            posts.extend((top,y-top,0))
            posts.extend(image.crop((x,top,x+1,y)).tobytes())
            posts.append(0)
        posts.append(255)
    if max(offsets)+3>65535:
        raise ValueError("Patch exceeds Doom's 16-bit texture column offset.")
    return struct.pack('<4h',width,height,0,0)+records('<I',((v,) for v in offsets))+posts


def generate(output):
    floor=[flat(style) for style in ('track','gravel','wood','cargo','coal','cab','link','roof','night')]
    walls=[mesa(False),mesa(True),signals(),wagon('coach'),wagon('wagon'),wagon('tender'),
           wagon('cab'),control('A'),control('B'),control('BRAKE'),trim('top'),trim('sill'),
           wagon('guard'),header('CARGO'),header('TENDER'),header('CAB')]
    lumps=map_lumps()+[(b'F_START',b'')]
    lumps.extend((name.encode(),im.tobytes()) for name,im in zip(FLATS,floor))
    lumps.extend([(b'F_END',b''),(b'P_START',b'')])
    lumps.extend((name.encode(),picture(im)) for name,im in zip(WALLS,walls))
    lumps.append((b'P_END',b''))
    out=bytearray(12)
    directory=[]
    for name,payload in lumps:
        directory.append((len(out),len(payload),name))
        out.extend(payload)
    struct.pack_into('<4sII',out,0,b'PWAD',len(lumps),len(out))
    out.extend(records('<II8s',directory))
    output.write_bytes(out)


if __name__=='__main__':
    cli=argparse.ArgumentParser(description=__doc__)
    cli.add_argument('--output',type=Path,default=PWAD)
    generate(cli.parse_args().output)
