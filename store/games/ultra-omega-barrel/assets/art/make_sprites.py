"""Draw the Ultra Omega Barrel cast and pack it as native Doom picture lumps.

Run with Python 3 and Pillow. The optional preview uses a supplied IWAD palette.
The Store edition keeps environmental barrel frames from the player's IWAD.
"""
import argparse
import math
from pathlib import Path
import struct

from PIL import Image, ImageDraw

CLEAR = 255
INK = 7
SOOT = 110
STEEL = (91, 98, 103, 107, 110)
IRON = (133, 137, 141, 145, 151)
COPPER = (61, 66, 70, 74, 77)
BRASS = (162, 64, 69, 74, 78)
RED = (29, 35, 40, 44, 47)
GREEN = (114, 117, 120, 123, 125)
DRAB = (152, 154, 156, 158, 159)
LIME = 112
GOLD = 160
WHITE = 224


def lumps(path):
    data = path.read_bytes()
    _, count, directory = struct.unpack_from('<4sII', data)
    records = {}
    for i in range(count):
        p, size, name = struct.unpack_from('<II8s', data, directory + 16*i)
        records[name.rstrip(b'\0').decode('ascii')] = data[p:p+size]
    return records


def cylinder(draw, x, y, w, h, ramp, curve=2, worn=True):
    """Viewed slightly from above. Vertical ribs go dark away from the light."""
    shades=(3,2,1,0,0,1,1,1,2,2,2,3,3,4,4,4)
    for dx in range(w):
        ratio=dx/max(1,w-1)
        bow=round(curve*(1-(ratio*2-1)**2))
        tone=shades[min(15,int(ratio*16))]
        top=y+bow
        c=INK if dx in (0,w-1) else ramp[tone]
        draw.line((x+dx,top,x+dx,top+h-1), fill=c)
        if worn and 0 < dx < w-1 and h>5:
            r=(dx*17+w*31)%max(1,h-4)
            if dx%5 in (1,2):
                draw.point((x+dx,top+2+r),fill=ramp[min(4,tone+1)])
            if dx%7==2:
                draw.line((x+dx,top+2,x+dx,top+h-3),fill=ramp[min(4,tone+1)])
        draw.point((x+dx,top+h-1),fill=INK)


def band(draw,x,y,w,h,ramp=IRON,curve=2,bolts=True):
    cylinder(draw,x,y,w,h,ramp,curve,False)
    if h>=3:
        draw.line((x+2,y+1,x+w//5,y+curve),fill=ramp[1])
        draw.line((x+w//5,y+curve,x+w//2,y+curve),fill=ramp[0])
    if bolts and h>=4:
        for k in (.12,.32,.71,.9):
            xx=x+round(k*(w-1)); yy=y+round(curve*(1-(k*2-1)**2))+1
            draw.point((xx,yy),fill=84 if k<.5 else 134)
            draw.point((xx,yy+1),fill=INK)


def dome(draw,x,y,w,h,ramp=IRON):
    draw.ellipse((x,y,x+w-1,y+h-1),fill=INK)
    draw.ellipse((x+1,y+1,x+w-2,y+h-2),fill=ramp[2])
    draw.arc((x+1,y,x+w-2,y+h-2),180,295,fill=ramp[0])
    draw.arc((x+1,y,x+w-2,y+h-2),0,120,fill=ramp[4])


def foot(draw,x,y,w,lift):
    y-=lift
    draw.rectangle((x+2,y-2,x+w-3,y+2),fill=105)
    draw.rectangle((x,y+1,x+w-1,y+5),fill=INK)
    draw.line((x+1,y+1,x+w-3,y+1),fill=140)
    for i in range(x+1,x+w-2,3):
        draw.line((i,y+2,i-1,y+4), fill=100)
        draw.point((i+1,y+2),fill=108)


def warning(draw,x,y,w):
    for dx in range(w):
        draw.line((x+dx,y,x+dx,y+2),fill=164 if (dx//3)%2==0 else SOOT)
        if dx%6==1:draw.point((x+dx,y),fill=161)


def omega(draw,cx,y,scale=1,dim=False):
    glyph=("..#####..",".##...##.","##.....##","##.....##","##.....##",".##...##.","..##.##..","####.####")
    for gy,row in enumerate(glyph):
        for gx,c in enumerate(row):
            if c=="#":
                xx=cx-(9*scale)//2+gx*scale; yy=y+gy*scale
                draw.rectangle((xx,yy,xx+scale-1,yy+scale-1),fill=164 if dim else (GOLD if gy<3 else 162))


def face(draw,cx,y,w,h,glow,hot,tone,frame):
    """A broad hinged jaw, canted eyes and a separately shaded tooth rail."""
    x=cx-w//2
    mask=[(x,y),(x+w//2,y+2),(x+w-1,y),(x+w-2,y+h-2),(cx,y+h),(x+1,y+h-2)]
    draw.polygon(mask,fill=INK)
    eye_y=y+1
    for side in (-1,1):
        a=cx+side*(w//2-2); b=cx+side*max(2,w//7)
        yy=eye_y + (1 if frame in 'BE' else 0)
        draw.line((a,yy,b,yy+max(1,h//6)), fill=tone,width=2 if w>14 else 1)
        draw.line((a,yy,b,yy+max(1,h//6)),fill=hot if frame in 'CD' else glow)
        draw.point((b,yy+max(1,h//6)),fill=WHITE if frame in 'CD' else hot)
    y0=y+h-4
    for dx in range(1,w-1):
        arc=round(2*(1-abs((dx-w/2)/(w/2))))
        height=(2 if dx%4==1 else 1) if w>11 else 1
        draw.line((x+dx,y0+arc,x+dx,y0+arc+height-1),fill=hot if dx%3 else tone)
    draw.point((cx,y+h-1),fill=103)


def crown(draw,cx,y):
    # Seven interlocked heat sinks read as a crown at gameplay distance.
    for dx,size in [(-12,8),(-8,5),(-4,8),(0,12),(4,8),(8,5),(12,8)]:
        x=cx+dx; base=y+12
        draw.polygon([(x-2,base),(x-1,base-size),(x+1,base-size-1),(x+2,base)],fill=COOL)
        draw.line((x-1,base-size+1,x-1,base-1),fill=163)
        draw.point((x,base-size),fill=GOLD)
    band(draw,cx-14,y+10,29,5,BRASS,2)
    draw.polygon([(cx,y+8),(cx+2,y+12),(cx,y+16),(cx-2,y+12)],fill=176)
    draw.point((cx,y+11),fill=169)


COOL=74

def small(frame):
    im=Image.new('P',(36,39),CLEAR); d=ImageDraw.Draw(im)
    bob=int(frame in 'BD')
    foot(d,6,30,9,bob);foot(d,21,30,9,1-bob)
    for x in (3,28):
        d.rectangle((x,17+bob,x+4,24+bob),fill=INK)
        d.line((x+1,18+bob,x+1,22+bob),fill=104)
        d.point((x+2,18+bob),fill=84)
    y=5+bob
    cylinder(d,6,y+4,24,19,DRAB,2)
    dome(d,6,y,24,8,IRON)
    d.ellipse((9,y+2,26,y+5),fill=124)
    d.line((12,y+3,22,y+3),fill=116)
    d.point((14+bob,y+2),fill=LIME)
    band(d,5,y+5,26,4,COPPER,2)
    band(d,5,y+20,26,4,STEEL,2)
    face(d,18,y+10,17,8,115,LIME,120,frame)
    if frame=='D':
        d.polygon([(15,23),(12,26),(18,25),(21,29),(21,24),(24,24),(21,21)],fill=216)
        d.polygon([(17,22),(15,25),(19,24),(20,26),(20,23)],fill=GOLD)
    if frame=='E':d.line((9,14,13,17,10,20),fill=WHITE)
    return im,(18,35)


def medium(frame):
    im=Image.new('P',(52,60),CLEAR);d=ImageDraw.Draw(im);bob=int(frame in 'BD')
    foot(d,8,50,13,bob);foot(d,31,50,13,1-bob)
    y=5+bob
    for x in (5,40):
        cylinder(d,x,y+12,8,24,STEEL,1)
        for yy in range(19,35,4):band(d,x,y+yy,8,2,COPPER,1,False)
    cylinder(d,10,y+9,33,33,RED,3)
    # Bolted plate above a double-gasket cinch.
    dome(d,9,y+3,35,12,STEEL)
    dome(d,14,y+3,25,7,RED)
    d.rectangle((19,y+1,21,y+4),fill=100)
    d.rectangle((29,y,32,y+4),fill=INK)
    d.line((30,y,30,y+3),fill=90)
    for x in (9,39):
        d.polygon([(x-3,y+7),(x+1,y+10),(x+3,y+19),(x-1,y+17)],fill=INK)
        d.line((x-2,y+8,x,y+14),fill=132)
    band(d,8,y+12,37,5,COPPER,3)
    face(d,26,y+20,22,11,215,160,187,frame)
    # Orange V follows the vertical split in casing.
    d.line((14,y+18,18,y+21),fill=64);d.line((37,y+18,34,y+21),fill=72)
    band(d,8,y+33,37,5,IRON,2)
    band(d,10,y+40,33,4,COPPER,2)
    warning(d,20,y+39,12)
    if frame in 'CD': d.line((20,y+17,32,y+17),fill=211 if frame=='C' else 160)
    if frame=='D':
        d.polygon([(23,y+27),(26,y+23),(31,y+27),(34,y+31),(29,y+30),(26,y+35),(25,y+30),(19,y+32)],fill=217)
        d.ellipse((23,y+25,30,y+30),fill=210);d.line((25,y+27,28,y+27),fill=WHITE)
    if frame=='E':d.line((18,y+14,23,y+19,19,y+22),fill=160)
    return im,(26,56)


def arm(draw,x,y,side,attack):
    # Side-mounted fluted guns. Front lip is an ellipse looking down the bore.
    draw.polygon([(x-5,y+1),(x-3,y-6),(x+1,y-8),(x+5,y-3),(x+6,y+9),(x+2,y+19),(x-6,y+17)],fill=INK)
    cylinder(draw,x-5,y-3,11,15,RED,1)
    band(draw,x-5,y,11,4,BRASS,1)
    for dx in (-3,0,3):draw.line((x+dx,y+7,x+dx-1,y+15),fill=135)
    dome(draw,x-6,y+12,13,10,IRON)
    draw.ellipse((x-3,y+14,x+3,y+19),fill=INK)
    draw.line((x-2,y+17,x+2,y+17),fill=216 if attack else 187)
    if attack:
        draw.polygon([(x-2,y+16),(x+1,y+9),(x+2,y+15),(x+9,y+15),(x+4,y+19),(x+6,y+24),(x+1,y+21),(x-3,y+27),(x-3,y+20),(x-8,y+19)],fill=216)
        draw.ellipse((x-3,y+15,x+3,y+21),fill=210)
        draw.line((x,y+15,x,y+20),fill=WHITE)
    else:
        draw.point((x-3,y+15),fill=103)


def boss(frame):
    im=Image.new('P',(80,96),CLEAR);d=ImageDraw.Draw(im);step=int(frame in 'BD');y=3+step
    # Three pedal assemblies anchor the silhouette independently of the body.
    for x,lift in ((15,step*2),(35,1),(54,(1-step)*2)):
        foot(d,x,84,13,lift)
        cylinder(d,x+3,79-lift,7,6,STEEL,1)
    cylinder(d,17,y+25,47,48,RED,4)
    dome(d,18,y+17,45,16,IRON)
    for x in (17,55):
        d.polygon([(x-4,y+34),(x+1,y+23),(x+9,y+22),(x+10,y+34),(x+7,y+40),(x-2,y+39)],fill=INK)
        d.line((x-2,y+32,x+2,y+25,x+7,y+24),fill=162,width=2)
        for n in range(3):d.line((x+1,y+29+n*3,x+6,y+28+n*3),fill=70+n*3)
    band(d,18,y+31,45,7,BRASS,3)
    d.polygon([(23,y+40),(40,y+42),(58,y+39),(56,y+55),(41,y+61),(24,y+57)],fill=SOOT)
    face(d,41,y+41,29,15,116,GOLD,163,frame)
    for x in (22,59):
        for yy in (42,48,54):d.point((x,y+yy),fill=134)
    band(d,16,y+59,49,7,IRON,3)
    cylinder(d,19,y+65,43,9,DRAB,3)
    for x in (23,31,46,53):
        d.rectangle((x,y+68,x+2,y+72),fill=INK)
        d.point((x,y+68),fill=135)
    band(d,17,y+73,47,6,BRASS,3)
    # The gold omega lives on a second barrel welded above the main pressure vessel.
    cylinder(d,27,y+13,28,16,STEEL,2)
    band(d,26,y+12,30,3,RED,2)
    omega(d,41,y+17,1,False)
    d.line((29,y+20,32,y+21),fill=140);d.line((49,y+21,52,y+19),fill=140)
    crown(d,41,y)
    for x,side in ((11,-1),(69,1)):
        arm(d,x,y+38+(step if side==-1 else -step),side,frame=='D')
        # Thick hoses connect weapons to core.
        d.line((x,y+32,x-1,y+27,x+side*2,y+24,x-side*4,y+23),fill=108,width=3)
        d.line((x-1,y+31,x-2,y+28),fill=137)
    # Metal badge at belt, beneath it glows the core of a smaller barrel.
    d.polygon([(34,y+60),(48,y+60),(47,y+67),(41,y+70),(35,y+67)],fill=INK)
    d.polygon([(36,y+61),(46,y+61),(45,y+66),(41,y+68),(37,y+66)],fill=69)
    d.line((38,y+63,44,y+63),fill=115)
    if frame=='C':
        for x in (10,68):d.line((x-2,y+51,x+2,y+51),fill=GOLD)
        d.line((32,y+45,37,y+46),fill=WHITE); d.line((44,y+46,49,y+44),fill=WHITE)
    if frame=='E':
        d.line((31,y+31,33,y+36,30,y+40,36,y+45),fill=160,width=2)
        d.line((43,y+56,40,y+60,43,y+64),fill=214)
    return im,(40,90)


def scrap(draw,x,y,w,h,left,ramp):
    tip=-1 if left else 1
    draw.polygon([(x,y),(x+tip*w,y+h//4),(x+tip*(w-1),y+h-1),
                  (x+tip*2,y+h+2),(x-tip*2,y+h//2)],fill=INK)
    draw.polygon([(x,y+2),(x+tip*(w-1),y+h//4+1),(x+tip*(w-2),y+h-2),
                  (x+tip*3,y+h),(x-tip,y+h//2)],fill=ramp[2])
    draw.line((x,y+2,x+tip*(w-2),y+h//4+1),fill=ramp[0])
    draw.line((x+tip*2,y+h//2,x+tip*(w-2),y+h//2+2),fill=ramp[3])


def shell_death(factory,frame):
    src,(cx,base)=factory('E')
    w,h=src.size
    im=Image.new('P',src.size,CLEAR);d=ImageDraw.Draw(im)
    size=w/36
    rx=round(w*.40)
    red=RED if factory!=small else DRAB
    if frame=='I':
        # Rolled half-rim, dark recess and a clutch of jagged pieces on the floor.
        d.ellipse((cx-rx,base-6,cx+rx,base+2),fill=INK)
        d.ellipse((cx-rx+2,base-7,cx+rx-2,base),fill=red[2])
        d.ellipse((cx-rx+4,base-6,cx+rx-4,base-2),fill=SOOT)
        d.arc((cx-rx+1,base-7,cx+rx-2,base),175,295,fill=red[0])
        for i in range(6):
            xx=cx-rx+2+round(i*(2*rx-5)/6);lift=round(size*(2+(7*i)%5))
            d.polygon([(xx,base-3),(xx+1,base-3-lift),(xx+round(3*size),base-2)],fill=red[2])
            d.line((xx,base-3,xx+1,base-3-lift),fill=red[0])
        scrap(d,cx-2,base-round(6*size),round(4*size),3,True,COPPER)
        if factory==boss:
            # Detached, smashed crown forward of the wreck.
            d.line((cx+3,base-5,cx+14,base-2),fill=164,width=2)
            for xx in (5,9,13):d.line((cx+xx,base-5,cx+xx+2,base-9),fill=163)
        return im,(cx,base)
    delta={'F':1,'G':3,'H':5}[frame]
    seam=round(h*.56)
    # Drop the buckled bottom toward its own base.
    bottom=src.crop((0,seam,w,h))
    sh=max(6,round(bottom.height*(1-.15*delta)))
    bottom=bottom.resize((w,sh),Image.Resampling.NEAREST)
    im.paste(bottom,(0,base-sh+2),bottom.point(lambda n:0 if n==CLEAR else 255,'L'))
    dx=round(size*delta)
    # Peel the top apart along a jagged vertical split, revealing the next shell.
    if frame=='H':
        for side in (-1,1):
            xx=cx+side*int(w*.24);scrap(d,xx,base-round(10*size),round(6*size),round(7*size),side<0,red)
        band(d,cx-rx+2,base-round(8*size),rx*2-4,max(3,round(size*2)),IRON,2)
    else:
        shell=src.crop((0,0,w,seam+4))
        for side in (-1,1):
            half=shell.copy()
            dd=ImageDraw.Draw(half)
            for yy in range(half.height):
                split=cx+(yy//4)%3-1
                dd.line((split if side==-1 else 0,yy,w if side==-1 else split,yy),fill=CLEAR)
            angle=side*(-4 if frame=='F' else -14)
            half=half.rotate(angle,resample=Image.Resampling.NEAREST,fillcolor=CLEAR)
            im.paste(half,(side*dx,-dx),half.point(lambda n:0 if n==CLEAR else 255,'L'))
        inner=factory==boss and medium or small
        if frame=='G' and factory!=small:
            core,_=inner('C');core=core.resize((round(w*.30),round(h*.31)),Image.Resampling.NEAREST)
            im.paste(core,(cx-core.width//2,seam-core.height//2),core.point(lambda n:0 if n==CLEAR else 255,'L'))
    # A blast of fluid on F then fragments travelling outward on G/H.
    for j,(ux,uy) in enumerate([(-.35,-.18),(.38,-.27),(-.18,-.48),(.17,-.46),(-.44,.03),(.45,.05)]):
        x=round(cx+ux*w*(.5+delta*.10));y=round(seam+uy*h*(.5+delta*.09))
        d.line((x,y,x+(1 if ux<0 else -1),y+2),fill=GOLD if frame=='F' else COPPER[1])
        d.point((x,y),fill=WHITE if j%2==0 and frame=='F' else 161)
    if frame=='F':
        d.line((cx-1,seam-5,cx+1,seam-2,cx-2,seam+1,cx,seam+5),fill=GOLD)
        d.point((cx,seam),fill=WHITE)
    return im,(cx,base)


def crop_opaque(im,anchor):
    bounds=im.point(lambda p: 0 if p==CLEAR else 255).getbbox()
    return im.crop(bounds),(anchor[0]-bounds[0],anchor[1]-bounds[1])


def encode(im,anchor):
    im,(ox,oy)=crop_opaque(im,anchor)
    w,h=im.size; offsets=[]; data=bytearray()
    for x in range(w):
        offsets.append(8+w*4+len(data));y=0
        while y<h:
            if im.getpixel((x,y))==CLEAR:
                y+=1;continue
            top=y;column=[]
            while y<h and im.getpixel((x,y))!=CLEAR:
                column.append(im.getpixel((x,y)));y+=1
            data.extend([top,len(column),0,*column,0])
        data.append(255)
    return struct.pack('<hhhh',w,h,ox,oy)+struct.pack(f'<{w}I',*offsets)+data


def pack(lumps):
    body=bytearray();directory=bytearray()
    for name,content in lumps:
        directory.extend(struct.pack('<II8s',12+len(body),len(content),name.encode()))
        body.extend(content)
    return struct.pack('<4sII',b'PWAD',len(lumps),12+len(body))+body+directory


def render_preview(frames,pal,out):
    canvas=Image.new('RGB',(9*84,3*103),(31,30,27))
    for row,factory in enumerate(('MINI','HUSK','OMEG')):
        for col,f in enumerate('ABCDEFGHI'):
            sprite,(cx,base)=frames[factory+f+'0'];sprite=sprite.copy()
            sprite.putpalette(pal)
            alpha=sprite.point(lambda p:0 if p==CLEAR else 255,'L')
            canvas.paste(sprite.convert('RGB'),(84*col+42-cx,103*row+94-base),alpha)
    canvas.resize((canvas.width*2,canvas.height*2),Image.Resampling.NEAREST).save(out)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iwad',type=Path,help='local palette for --preview')
    parser.add_argument('output',type=Path)
    parser.add_argument('--preview',type=Path)
    args=parser.parse_args()
    frames={}
    for name,factory in [('OMEG',boss),('HUSK',medium),('MINI',small)]:
        for frame in 'ABCDEFGHI':
            frames[name+frame+'0']=shell_death(factory,frame) if frame in 'FGHI' else factory(frame)
    records=[('S_START',b'')]
    records.extend((name,encode(*art)) for name,art in frames.items())
    records.append(('S_END',b''))
    args.output.write_bytes(pack(records))
    if args.preview:
        if not args.iwad:
            parser.error('--preview requires --iwad')
        render_preview(frames,lumps(args.iwad)['PLAYPAL'][:768],args.preview)


if __name__=='__main__':
    main()
