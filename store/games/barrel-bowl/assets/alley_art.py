"""Original indexed-pixel artwork for the Barrel Bowl map. Requires Pillow to rebuild."""
import math
import random

from PIL import Image, ImageDraw


INK, NIGHT, NAVY = 7, 245, 243
BRASS, LIGHT, PINK = 163, 225, 250
CLEAR = 255
# Boxy, clipped-corner capitals for the tube sign and the exit panel.
LETTERS = {
    "A": ("01110", "11011", "11011", "11111", "11011", "11011", "11011"),
    "B": ("11110", "11011", "11011", "11110", "11011", "11011", "11110"),
    "D": ("11110", "11011", "11011", "11011", "11011", "11011", "11110"),
    "E": ("11111", "11000", "11000", "11110", "11000", "11000", "11111"),
    "G": ("01110", "11001", "11000", "11011", "11001", "11011", "01110"),
    "H": ("11011", "11011", "11011", "11111", "11011", "11011", "11011"),
    "I": ("11111", "00100", "00100", "00100", "00100", "00100", "11111"),
    "L": ("11000", "11000", "11000", "11000", "11000", "11000", "11111"),
    "M": ("10001", "11011", "11111", "10101", "10001", "10001", "10001"),
    "N": ("10001", "11001", "11001", "10101", "10011", "10011", "10001"),
    "O": ("01110", "11011", "11011", "11011", "11011", "11011", "01110"),
    "R": ("11110", "11011", "11011", "11110", "11100", "11010", "11011"),
    "S": ("01111", "11000", "11000", "01110", "00011", "00011", "11110"),
    "T": ("11111", "00100", "00100", "00100", "00100", "00100", "00100"),
    "W": ("10001", "10001", "10001", "10101", "10101", "11011", "10001"),
    "X": ("11011", "11011", "01010", "00100", "01010", "11011", "11011"),
}


def new(size, ink):
    return Image.new("P", size, ink)


def spark(d, x, y, r, color):
    d.polygon([(x, y-r), (x+1, y-1), (x+r, y), (x+1, y+1),
               (x, y+r), (x-1, y+1), (x-r, y), (x-1, y-1)], fill=color)


def lamp(d, x, y):
    d.ellipse((x-3, y-3, x+3, y+3), fill=78, outline=INK)
    d.ellipse((x-1, y-1, x+1, y+1), fill=161)
    d.point((x-1, y-1), fill=LIGHT)


def text_mask(text, sx, sy, slant):
    width = len(text)*6*sx-sx
    mask = Image.new("L", (width+slant, 7*sy))
    d = ImageDraw.Draw(mask)
    for index, letter in enumerate(text):
        for row, line in enumerate(LETTERS[letter]):
            for v in range(sy):
                y = row*sy+v
                skew = round(slant*(1-y/(7*sy-1)))
                for col, flag in enumerate(line):
                    if flag == "1":
                        x = (index*6+col)*sx+skew
                        d.line((x, y, x+sx-1, y), fill=255)
    return mask


def letters(im, text, xy, scale, ramp, slant=0):
    x, y = xy
    mask = text_mask(text, *scale, slant)
    def layer(color, dx, dy):
        im.paste(color, (x+dx, y+dy), mask)
    for offset in range(5, 0, -1):
        layer(INK, offset, offset+1)
    for dx, dy in [(-2, 0), (2, 0), (0, -2), (0, 2), (-1, -1), (1, -1), (-1, 1), (1, 1)]:
        layer(ramp[0], dx, dy)
    layer(ramp[1], 0, 0)
    # Brighter face on the upper half, then the thin reflected top edge of each segment.
    upper=mask.copy()
    ImageDraw.Draw(upper).rectangle((0, mask.height//2, mask.width, mask.height), fill=0)
    im.paste(ramp[2], (x, y), upper)
    p=mask.load()
    draw=ImageDraw.Draw(im)
    for yy in range(mask.height):
        for xx in range(mask.width):
            if p[xx, yy] and (yy==0 or not p[xx, yy-1]):
                draw.point((x+xx, y+yy), fill=ramp[3])


def fascia(width):
    im=new((width, 128), NIGHT)
    d=ImageDraw.Draw(im)
    for y in range(10, 118, 4):
        d.line((0, y, width-1, y), fill=NAVY)
        d.line((0, y+1, width-1, y+1), fill=244)
    for y in (4, 121):
        d.rectangle((0, y, width-1, y+2), fill=77)
        d.line((0, y, width-1, y), fill=BRASS)
    return im


def pin(d, cx, base, s, lean):
    silhouette=[(-.26, 0), (-.36, -.10), (-.41, -.26), (-.36, -.43), (-.16, -.66),
                (-.13, -.74), (-.19, -.85), (-.16, -.95), (0, -1), (.16, -.95),
                (.19, -.85), (.14, -.73), (.16, -.66), (.36, -.43), (.41, -.26), (.34, -.09), (.26, 0)]
    def xy(x, y):
        return cx+round(x*s*.56-lean*y), base+round(y*s)
    polygon=[xy(x, y) for x, y in silhouette]
    d.polygon(polygon, fill=49, outline=INK)
    d.polygon([xy(-.34, -.34), xy(-.15, -.64), xy(-.02, -.65), xy(-.14, -.33)], fill=224)
    d.polygon([xy(.04, -.02), xy(.28, -.36), xy(.37, -.29), xy(.27, -.06)], fill=131)
    for y in (-.70, -.77):
        d.line([xy(-.13, y), xy(.14, y)], fill=182, width=max(1, s//26))


def hero():
    im=fascia(512)
    d=ImageDraw.Draw(im)
    # The long stepped diamond gives the sign a silhouette against the dark fascia.
    outline=[(10, 65), (57, 14), (447, 14), (501, 63), (452, 112), (61, 112), (10, 65)]
    d.line(outline, fill=254, width=5)
    d.line(outline, fill=252, width=2)
    for i in range(4):
        y=24+i*4
        d.line((72+i*6, y, 438-i*6, y), fill=244)
        d.line((64+i*11, 101+i*2, 450-i*11, 101+i*2), fill=243)
    for x in range(90, 432, 21):
        d.point((x, 20), fill=163)
        d.point((x, 107), fill=163)
    # An asymmetric cluster of ivory pins, an inky rolling ball and two long speed lines.
    for dx, dy in [(0, 0), (-6, 5), (-8, 10)]:
        d.line((14, 73+dy, 40+dx, 73+dy), fill=BRASS if dy==0 else 73, width=2)
    pin(d, 44, 84, 42, -13)
    pin(d, 50, 81, 47, 4)
    d.ellipse((47, 64, 74, 91), fill=254, outline=INK, width=2)
    d.arc((48, 65, 74, 90), 185, 293, fill=PINK, width=3)
    for xx, yy in [(58, 70), (63, 71), (59, 75)]:
        d.ellipse((xx, yy, xx+2, yy+2), fill=INK)
    letters(im, "BARREL", (84, 44), (5, 6), (74, 164, 162, 225), 7)
    letters(im, "BOWL", (277, 39), (6, 7), (254, 252, 250, 169), 8)
    for x, y, r, c in [(425, 51, 7, 225), (450, 72, 11, 250), (470, 57, 4, 161),
                    (434, 85, 3, 250), (39, 47, 4, 161), (80, 34, 3, 250)]:
        spark(d, x, y, r, c)
    for x in (6, 505):
        for y in (9, 117):
            lamp(d, x, y)
    return im


def wall(style):
    im=fascia(128)
    d=ImageDraw.Draw(im)
    d.rectangle((0, 79, 127, 119), fill=110)
    for i in (3, 65):
        d.rectangle((i, 86, i+57, 114), fill=111, outline=108)
        for slot in range(i+5, i+51, 5):
            d.line((slot, 108, slot+8, 94), fill=INK, width=2)
    # Painted rails continue along the wall when tiles repeat.
    for y, tone in [(68, 253), (69, 250), (70, 253), (74, 78), (75, BRASS)]:
        d.line((0, y, 127, y), fill=tone)
    if style=="side":
        for x in (10, 45, 80):
            d.line([(x, 28), (x+22, 42), (x, 57)], fill=254, width=6)
            d.line([(x, 27), (x+20, 41), (x, 55)], fill=252, width=3)
            d.line([(x+2, 27), (x+20, 41)], fill=PINK)
    elif style=="back":
        d.rectangle((10, 22, 117, 62), fill=INK, outline=74)
        for i in range(9):
            x=17+i*11
            d.rectangle((x, 28, x+6, 55), fill=244, outline=243)
            for j in range(2+(i*7)%4):
                y=51-j*5
                d.line((x+2, y, x+4, y), fill=BRASS if j<3 else 252)
        d.line((11, 22, 116, 22), fill=BRASS)
    elif style=="exit":
        d.rectangle((9, 21, 118, 62), fill=INK, outline=BRASS, width=2)
        letters(im, "EXIT", (19, 28), (3, 4), (124, 119, 113, 225), 2)
        d.polygon([(93, 35), (99, 35), (99, 31), (110, 41), (99, 51), (99, 47), (93, 47)], fill=161)
    else:
        for x in (15, 80):
            d.rectangle((x, 28, x+32, 53), outline=243)
            for xx in range(x+4, x+28, 4):
                d.line((xx, 47, xx+5, 34), fill=111, width=2)
    for x in (4, 63, 124):
        for y in (9, 116):
            d.point((x, y), fill=93)
            d.point((x+1, y+1), fill=INK)
    return im


def marquee():
    im=new((256, 32), NIGHT)
    d=ImageDraw.Draw(im)
    for y in (0, 31):
        d.line((0, y, 255, y), fill=163)
    for y in (3, 28):
        d.line((0, y, 255, y), fill=254)
    for center, sign in [(26, 1), (229, -1)]:
        for shift in (0, 8, 16):
            x=center-sign*shift
            d.line([(x-sign*5, 9), (x, 15), (x-sign*5, 21)], fill=250, width=2)
    for x in (39, 216):
        spark(d, x, 15, 5, 162)
    letters(im, "MIDNIGHT", (48, 8), (2, 2), (74, 162, 160, 225), 3)
    letters(im, "LANES", (149, 8), (2, 2), (74, 162, 160, 225), 3)
    return im


def lane():
    im=new((64, 64), 67)
    pix=im.load()
    per_board=[66, 64, 67, 65, 64, 66, 63, 65]
    joints=[21, 42, 10, 56, 35, 17, 46, 7]
    for x in range(64):
        board, u=divmod(x, 8)
        for y in range(64):
            grain=math.sin(y*math.tau/64+board*.7+u*.45)
            shade=per_board[board]
            if u==0:
                shade+=4
            elif u==1:
                shade-=1
            elif grain>.88:
                shade+=1
            if y==joints[board] and u:
                shade+=3
            pix[x, y]=shade
    return im


def floor(style):
    if style in ("lane", "arrow", "foul"):
        im=lane()
        d=ImageDraw.Draw(im)
        if style=="arrow":
            d.polygon([(32, 17), (38, 32), (33, 29), (32, 36), (31, 29), (26, 32)], fill=76)
            d.line((32, 19, 28, 29), fill=145)
        if style=="foul":
            d.rectangle((0, 29, 63, 34), fill=78)
            d.line((0, 28, 63, 28), fill=162)
            d.line((0, 35, 63, 35), fill=69)
        return im
    im=new((64, 64), NIGHT)
    d=ImageDraw.Draw(im)
    if style=="gutter":
        # Symmetric concave steel reflects one rail on each lip.
        bands=[108, 102, 105, 108, 109, 110, 111, INK, INK, 111, 110, 109, 108, 105, 101, 108]
        for x in range(64):
            d.line((x, 0, x, 63), fill=bands[(x%32)//2])
        for x in (0, 32):
            d.line((x, 0, x, 63), fill=141)
            for y in (0, 32):
                d.point((x+3, y), fill=101)
    elif style=="carpet":
        rng=random.Random(71)
        for _ in range(370):
            d.point((rng.randrange(64), rng.randrange(64)), fill=rng.choice([244, 243, 111, 245]))
        for x, y, color in [(9, 11, 253), (46, 34, 196), (27, 52, BRASS)]:
            d.line((x-3, y+1, x, y-2, x+3, y), fill=color)
        for x, y, color in [(31, 19, 162), (52, 56, 253), (8, 40, 196)]:
            spark(d, x, y, 2, color)
    else:
        d.rectangle((1, 1, 62, 62), fill=110, outline=108)
        d.rectangle((5, 5, 58, 58), fill=111, outline=INK)
        for x in range(11, 57, 5):
            d.line((x, 12, x, 51), fill=110)
        for x, y in [(3, 3), (60, 60)]:
            d.point((x, y), fill=101)
    return im


def graphics():
    return {"BLHERO":hero(), "BLMARQ":marquee(), "BLWALL":wall("wall"), "BLSIDE":wall("side"),
            "BLBACK":wall("back"), "BLEXIT":wall("exit"), "BLLANE":floor("lane"),
            "BLGUTR":floor("gutter"), "BLCARP":floor("carpet"), "BLCEIL":floor("ceiling"),
            "BLARRO":floor("arrow"), "BLFOUL":floor("foul")}


def balls():
    frames=[]
    c=12
    for frame in range(3):
        im=new((25, 25), CLEAR)
        d=ImageDraw.Draw(im)
        d.ellipse((0, 0, 24, 24), fill=INK)
        d.ellipse((1, 1, 23, 23), fill=74)
        d.ellipse((3, 2, 21, 21), fill=109, outline=BRASS)
        d.arc((1, 1, 23, 23), 175, 292, fill=162, width=2)
        d.arc((2, 2, 22, 22), 10, 85, fill=77, width=2)
        # Barrel-head stampings and three finger bores rotate together.
        a=math.tau*(frame/3-.10)
        def loc(u, v):
            return (c+round(u*math.cos(a)-v*math.sin(a)),
                    c+round(u*math.sin(a)+v*math.cos(a)))
        for deg in range(0, 360, 60):
            ang=math.radians(deg)
            u, v=math.cos(ang), math.sin(ang)
            d.line([loc(u*5, v*5), loc(u*8, v*8)], fill=105)
            d.point(loc(u*9, v*9), fill=69)
        for u, v in [(-2, -4), (2, -4), (0, 0)]:
            x, y=loc(u, v)
            d.ellipse((x-1, y-1, x+1, y+2), fill=INK)
            d.point((x, y+2), fill=95)
        d.arc((4, 3, 20, 20), 187, 266, fill=98)
        d.point((5, 6), fill=225)
        frames.append(im)
    return frames
