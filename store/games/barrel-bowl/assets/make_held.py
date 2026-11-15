"""Draw the bowler's brass-banded spare ball and leather gloves. Requires Pillow."""
import math
import random
from pathlib import Path
import struct
from PIL import Image, ImageDraw


def draw_ball():
    canvas = Image.new("P", (88, 48), 255)
    pen = ImageDraw.Draw(canvas)
    rng = random.Random(4360)
    cx, cy, radius = 44, 33, 29
    for y in range(48):
        for x in range(88):
            u, v = (x-cx)/radius, (y-cy)/radius
            if u*u+v*v > 1:
                continue
            z = math.sqrt(max(0, 1-u*u-v*v))
            light = max(0, min(1, -.55*u-.5*v+.7*z))
            val = 109 - int(17*light)
            if .44 < v+.09*u < .62 or -.58 < v+.09*u < -.5:
                val = (66,68,70,72,75)[min(4, int(light*5))]
            if u*u+v*v > .94:val=7
            elif rng.random() < .07:val = max(0,val-1)
            pen.point((x,y),val)
    for x,y in [(35,20),(44,18),(41,28)]:
        pen.ellipse((x-2,y-2,x+2,y+2),fill=7)
        pen.arc((x-2,y-2,x+2,y+2),15,152,fill=99)
    pen.arc((19,7,64,54),199,251,fill=92,width=2)
    # The left and right gloves cradle the ball from below. The far thumb
    # catches the same cool highlight as the cast iron.
    for flip in [False,True]:
        def pts(path):
            return [(87-x if flip else x,y) for x,y in path]
        pen.polygon(pts([(1,47),(8,40),(12,31),(16,30),(18,32),(17,39),(26,34),(30,35),(31,38),(26,42),(23,47)]),fill=7)
        pen.polygon(pts([(4,47),(10,40),(13,33),(16,32),(16,38),(12,43),(16,43),(25,36),(29,36),(28,39),(22,44),(20,47)]),fill=140)
        pen.line(pts([(13,37),(14,34),(16,34)]),fill=145)
        pen.line(pts([(19,41),(26,36),(28,36)]),fill=145)
        pen.line(pts([(10,43),(16,46)]),fill=137,width=2)
        pen.line(pts([(6,46),(12,47)]),fill=135)
    return canvas


def encode(image):
    w,h=image.size
    offsets, stream=[],bytearray()
    for x in range(w):
        offsets.append(8+w*4+len(stream))
        y=0
        while y<h:
            if image.getpixel((x,y))==255:
                y+=1
                continue
            start=y
            while y<h and image.getpixel((x,y))!=255:y+=1
            stream.extend((start,y-start,0))
            stream.extend(image.getpixel((x,i)) for i in range(start,y))
            stream.append(0)
        stream.append(255)
    return struct.pack('<4h',w,h,0,0)+struct.pack('<'+'I'*w,*offsets)+stream

if __name__ == '__main__':
    Path(__file__).with_name('held.pat').write_bytes(encode(draw_ball()))
