"""Draw the train crew and the emergency brake in the original Doom palette.

Run with Python 3 and Pillow. Indexed colors are deliberate; there are no
borrowed sprites, game-file dependencies, or external fonts.
"""
from pathlib import Path
import math
import struct
from PIL import Image, ImageDraw

CLEAR=255
BLACK=8
STEEL=(105,100,94,89,94,99,104,108)
GOLD=(76,165,163,162,161,162,163,164,165,166,77)
BLUE=(245,242,207,205,204,205,207,242,244)
GREEN=(126,124,121,118,116,120,123,125)
CLARET=(46,43,39,35,32,35,38,41,45)

def new():
    return Image.new('P',(72,66),CLEAR)

def dome(d,cx,y,w,h,fill,shine,shadow):
    x=cx-w//2
    d.ellipse((x,y,x+w-1,y+h-1),fill=BLACK)
    d.ellipse((x+1,y+1,x+w-2,y+h-2),fill=fill)
    d.arc((x+1,y+1,x+w-2,y+h-2),182,298,fill=shine)
    d.arc((x+1,y+1,x+w-2,y+h-2),10,143,fill=shadow)

def barrel(d,cx,y,w,h,ramp,curve=2):
    x=cx-w//2
    for k in range(w):
        u=k/(w-1)
        yy=y+round(curve*(1-(2*u-1)**2))
        v=ramp[min(len(ramp)-1,int(u*len(ramp)))]
        d.line((x+k,yy,x+k,yy+h-1),fill=v if 0<k<w-1 else BLACK)
        d.point((x+k,yy+h-1),fill=BLACK)

def shoe(d,x,y,w):
    d.polygon([(x+2,y-3),(x+w-3,y-3),(x+w-2,y-1),(x+w,y),(x+w,y+2),(x,y+2),(x-1,y)],fill=BLACK)
    d.line((x+1,y,x+w-2,y),fill=100)
    d.line((x+3,y-1,x+w-3,y-1),fill=134)
    d.point((x+2,y),fill=87)

def watch(d,x,y,large=False):
    r=4 if large else 3
    d.arc((x-1,y-r-3,x+2,y-r),165,375,fill=162)
    d.ellipse((x-r-1,y-r,x+r+1,y+r+1),fill=76)
    d.ellipse((x-r,y-r,x+r,y+r),fill=162)
    d.ellipse((x-r+1,y-r+1,x+r-1,y+r-1),fill=225)
    d.line((x,y-2,x,y,x+2,y+1),fill=BLACK)
    d.point((x-1,y-r),fill=224)

def chain(d,points):
    d.line(points,fill=75,width=2)
    d.line([(x,y-1) for x,y in points],fill=163)
    for x,y in points[::2]:d.point((x,y-1),fill=225)

def gloves(d,x,y,side):
    d.ellipse((x-2,y-2,x+2,y+2),fill=BLACK)
    d.ellipse((x-2,y-2,x+1,y+1),fill=83)
    d.point((x-1,y-2),fill=224)
    d.point((x+side*2,y),fill=131)

def whistle(d,x,y):
    d.polygon([(x,y),(x+5,y-1),(x+6,y-3),(x+10,y-3),(x+10,y),(x+7,y+2),(x+3,y+1),(x,y+1)],fill=75)
    d.line((x+1,y,x+6,y-1,x+7,y-2,x+10,y-2),fill=160)
    d.point((x+8,y-1),fill=78)

def peaked_cap(d,cx,y,tilt):
    # The winged badge stays pinned to the cap through every timetable emergency.
    pts=[(-16,6),(-15,2),(-10,-1),(-1,-3),(9,-2),(15,0),(17,3),(16,7),(11,9),(-10,10)]
    def xy(v):
        x,h=v
        return cx+x,round(y+h+tilt*x/12)
    d.polygon(list(map(xy,pts)),fill=BLACK)
    d.polygon(list(map(xy,[(-15,4),(-10,0),(1,-2),(10,-1),(15,2),(14,5),(7,7),(-13,7)])),fill=207)
    d.polygon(list(map(xy,[(-13,4),(-7,1),(3,-1),(10,0),(7,2),(-2,3),(-12,6)])),fill=204)
    d.line(list(map(xy,[(-14,3),(-7,0),(2,-2),(9,-1)])),fill=195)
    d.line(list(map(xy,[(-12,7),(0,8),(13,5)])),fill=163,width=2)
    d.line(list(map(xy,[(-11,7),(-4,8),(4,7)])),fill=225)
    d.polygon(list(map(xy,[(-12,8),(0,9),(10,7),(15,7),(11,10),(4,12),(-8,11),(-13,10)])),fill=BLACK)
    d.line(list(map(xy,[(-8,11),(1,11),(10,9)])),fill=101)
    bx,by=xy((0,4))
    d.line((bx-6,by-1,bx-3,by+1,bx+3,by+1,bx+6,by-1),fill=162,width=2)
    d.line((bx-5,by-1,bx-2,by,bx+2,by,bx+5,by-1),fill=225)
    d.polygon([(bx,by-3),(bx+2,by-1),(bx,by+3),(bx-2,by)],fill=160)
    d.point((bx,by-1),fill=224)

def crew(frame):
    im=new();d=ImageDraw.Draw(im)
    cx,base=35,58
    sit=frame in 'GH'
    bob={'A':0,'B':-1,'C':1,'D':-1,'E':-2,'F':-3,'G':5,'H':5}[frame]
    y=15+bob
    for side,x in ((-1,cx-11),(1,cx+3)):
        lift=(-1 if frame=='B' else 1 if frame=='C' else 0)*side
        if sit:x+=side*4
        d.rectangle((x+3,y+33,x+6,base-2+lift),fill=108)
        d.line((x+4,y+35,x+4,base-3+lift),fill=101)
        shoe(d,x,base-2+lift,10 if sit else 9)
    barrel(d,cx,y+8,25,25,GOLD)
    barrel(d,cx,y+19,25,13,BLUE)
    # Open formal waistcoat, brass lapels and an hourglass watch chain.
    d.polygon([(cx-9,y+18),(cx-3,y+20),(cx,y+27),(cx-4,y+25)],fill=163)
    d.polygon([(cx+9,y+18),(cx+2,y+20),(cx,y+27),(cx+5,y+25)],fill=162)
    d.polygon([(cx-3,y+20),(cx+3,y+20),(cx,y+25)],fill=225)
    d.line((cx,y+21,cx,y+25),fill=250 if frame=='H' else 181)
    for z in (26,29):
        d.point((cx,y+z),fill=160)
    barrel(d,cx,y+31,27,4,GOLD)
    # Deep green glazed visor under the navy peak.
    d.polygon([(cx-11,y+12),(cx-4,y+13),(cx+9,y+11),(cx+8,y+16),(cx-1,y+18),(cx-10,y+16)],fill=125)
    d.line((cx-9,y+13,cx-4,y+14),fill=117)
    d.line((cx+4,y+13,cx+8,y+12),fill=112)
    if frame in 'EFG':
        d.ellipse((cx-7,y+14,cx-4,y+17),fill=225)
        d.ellipse((cx+3,y+13,cx+6,y+16),fill=225)
        d.point((cx-5,y+15),fill=BLACK);d.point((cx+4,y+14),fill=BLACK)
        d.ellipse((cx-2,y+17,cx+3,y+21),fill=77)
    else:
        d.line((cx-7,y+15,cx-5,y+14,cx-3,y+15),fill=BLACK)
        d.line((cx+3,y+14,cx+5,y+13,cx+7,y+14),fill=BLACK)
        d.line((cx-4,y+17,cx-1,y+19,cx+4,y+18,cx+7,y+16),fill=75,width=2)
        d.line((cx-2,y+18,cx+3,y+17),fill=225)
    # Sleeves, with high hands and a watch on the loose in the panic sequence.
    lx,ly=cx-14,y+26
    rx,ry=cx+14,y+27
    if frame=='B':lx-=1;ly-=3
    if frame=='C':rx+=1;ry-=3
    if frame=='D':rx,ry=cx+8,y+17
    if frame in 'EF':lx,ly=cx-18,y+12;rx,ry=cx+20,y+7
    if frame=='G':lx,ly=cx-17,y+28;rx,ry=cx+15,y+16
    if frame=='H':lx,ly=cx-14,y+25;rx,ry=cx+15,y+17
    for side,x,yy in ((-1,lx,ly),(1,rx,ry)):
        xx=cx+side*11
        points=[(xx,y+21),(xx+side*1,y+25),(x,yy)]
        d.line(points,fill=BLACK,width=5);d.line(points,fill=205,width=3)
        d.point((x-side,yy+1),fill=162);gloves(d,x,yy,side)
    if frame=='D':
        whistle(d,cx+5,y+17)
        for xx,dy in ((cx+18,0),(cx+21,-3),(cx+22,2)):
            d.line((xx,y+15+dy,xx+2,y+14+dy),fill=225)
    elif frame in 'EFG':
        if frame=='F':
            chain(d,[(rx,ry),(rx+2,ry-3),(rx-1,ry-6)])
            watch(d,rx+1,ry-12,True)
            d.arc((rx-11,ry-14,rx-2,ry-2),113,234,fill=161)
        elif frame=='E':
            chain(d,[(rx,ry),(rx+3,ry+3),(rx+1,ry+7)])
            watch(d,rx+1,ry+9)
        else:
            watch(d,cx+23,base-3,True)
    else:
        chain(d,[(cx-9,y+24),(cx-7,y+27),(cx-4,y+28),(cx-2,y+27)])
        watch(d,cx-10,y+27)
    captilt={'B':-1,'C':1,'D':-1,'E':-2,'F':-3,'G':2,'H':1}.get(frame,0)
    peaked_cap(d,cx,y+(0 if frame!='F' else -1),captilt)
    if frame=='H': # A single finger acknowledges the now-quite-reasonable speed.
        d.line((rx+1,ry,rx+2,ry-3),fill=225,width=2)
    return im,(cx,base)

def porter(frame):
    im=new();d=ImageDraw.Draw(im);cx,base=35,58
    y=23+int(frame=='B')
    for x in (cx-10,cx+2):
        d.rectangle((x+3,y+24,x+6,base-2),fill=106)
        d.point((x+4,base-5),fill=96)
        shoe(d,x,base-2,9)
    barrel(d,cx,y+3,25,25,CLARET)
    dome(d,cx,y,26,8,38,29,43)
    barrel(d,cx,y+21,27,5,STEEL)
    for side in (-1,1):
        x=cx+side*11
        d.polygon([(x,y+9),(x+side*2,y+10),(x-side*5,y+21),(x-side*8,y+21)],fill=163)
        d.line((x,y+10,x-side*6,y+20),fill=224)
    barrel(d,cx,y+8,24,3,STEEL)
    # The porter wears a miner's lamp, a rolled red bandanna, and a thick oilskin.
    d.ellipse((cx-10,y-4,cx+10,y+9),fill=BLACK)
    d.ellipse((cx-9,y-3,cx+9,y+7),fill=38)
    d.arc((cx-7,y-3,cx+8,y+8),172,304,fill=31)
    d.line((cx-12,y+4,cx+10,y+3),fill=76,width=3)
    d.line((cx-11,y+3,cx+10,y+2),fill=164)
    d.ellipse((cx-4,y-3,cx+3,y+4),fill=75)
    d.ellipse((cx-3,y-2,cx+2,y+3),fill=163)
    d.ellipse((cx-2,y-1,cx+1,y+2),fill=225)
    d.line((cx-7,y+7,cx-3,y+8),fill=BLACK,width=2)
    d.line((cx+3,y+8,cx+7,y+6),fill=BLACK,width=2)
    d.line((cx-5,y+12,cx+5,y+11),fill=75,width=2)
    d.line((cx-4,y+12,cx+1,y+12),fill=225)
    d.polygon([(cx-12,y+12),(cx-16,y+15),(cx-13,y+16),(cx-17,y+19),(cx-12,y+19),(cx-9,y+13)],fill=178)
    # One stubby signal pistol; one shot means the ticket window really is closed.
    gun_y=y+(14 if frame=='C' else 18)
    d.line((cx+8,y+14,cx+14,gun_y+2),fill=39,width=4)
    d.rectangle((cx+10,gun_y,cx+18,gun_y+3),fill=BLACK)
    d.line((cx+11,gun_y,cx+18,gun_y),fill=94)
    if frame=='C':
        d.polygon([(cx+18,gun_y+1),(cx+22,gun_y-3),(cx+21,gun_y),(cx+26,gun_y),(cx+22,gun_y+2),(cx+23,gun_y+5)],fill=216)
        d.line((cx+19,gun_y+1,cx+22,gun_y+1),fill=225)
    if frame=='D':
        d.line((cx-5,y+9,cx-1,y+13,cx-3,y+17),fill=160)
        d.line((cx+12,y+1,cx+16,y-2),fill=162)
    if frame in 'EF':
        area=mask(im).getbbox();im=im.crop(area)
        im=im.rotate(-35 if frame=='E' else -87,Image.Resampling.NEAREST,expand=True,fillcolor=CLEAR)
        im=im.crop(mask(im).getbbox())
        return im,(im.width//2,im.height+(2 if frame=='E' else 0))
    return im,(cx,base)

def brake(frame):
    im=new();d=ImageDraw.Draw(im);cx,base=35,58
    d.polygon([(cx-12,53),(cx+7,51),(cx+13,53),(cx+13,57),(cx-8,59),(cx-13,57)],fill=BLACK)
    d.polygon([(cx-12,53),(cx+7,51),(cx+13,53),(cx-7,56)],fill=99)
    for x,y in ((cx-10,54),(cx+8,53),(cx-7,56),(cx+9,55)):d.point((x,y),fill=82)
    d.polygon([(cx-8,52),(cx-8,33),(cx-3,27),(cx+5,27),(cx+9,32),(cx+9,52),(cx-1,55)],fill=BLACK)
    d.polygon([(cx-7,51),(cx-7,34),(cx-2,29),(cx+3,29),(cx+5,34),(cx+5,51),(cx-2,53)],fill=123)
    d.polygon([(cx+6,33),(cx+8,32),(cx+8,51),(cx+6,52)],fill=125)
    d.line((cx-7,34,cx-3,29,cx+3,29),fill=162,width=2)
    d.arc((cx-9,16,cx+13,52),176,348,fill=76,width=4)
    d.arc((cx-9,16,cx+13,52),176,348,fill=162,width=2)
    for j in range(7):
        a=math.radians(188+j*22)
        x=cx+2+round(10*math.cos(a));y=34+round(17*math.sin(a))
        d.point((x,y),fill=225)
    pivot=(cx,36);grip={'A':(cx-9,12),'B':(cx,8),'C':(cx+15,23)}[frame]
    d.line((pivot,grip),fill=76,width=5);d.line((pivot,grip),fill=163,width=3)
    d.line(((pivot[0]-1,pivot[1]-1),(grip[0]-1,grip[1])),fill=225)
    x,y=grip
    d.rounded_rectangle((x-5,y-3,x+5,y+3),2,fill=44,outline=BLACK)
    d.line((x-3,y-2,x+4,y-2),fill=172,width=2)
    d.ellipse((cx-4,32,cx+4,40),fill=75,outline=BLACK)
    d.ellipse((cx-3,33,cx+3,39),fill=162)
    d.line((cx-1,34,cx+1,37),fill=97)
    d.rectangle((cx-5,43,cx+3,49),fill=76)
    d.rectangle((cx-4,44,cx+2,48),fill=183 if frame!='C' else 120)
    d.line((cx-2,45,cx,45),fill=225);d.point((cx-1,47),fill=225)
    return im,(cx,base)

def mask(image):
    return Image.frombytes('L',image.size,image.tobytes()).point(lambda p:0 if p==CLEAR else 255)

def patch(image,origin=None):
    if origin is not None:
        box=mask(image).getbbox()
        image=image.crop(box)
        left,top=origin[0]-box[0],origin[1]-box[1]
    else:left=top=0
    w,h=image.size;columns=bytearray();offsets=[]
    for x in range(w):
        offsets.append(8+w*4+len(columns));y=0
        while y<h:
            if origin is not None and image.getpixel((x,y))==CLEAR:
                y+=1;continue
            first=y;pixels=bytearray()
            while y<h and (origin is None or image.getpixel((x,y))!=CLEAR):
                pixels.append(image.getpixel((x,y)));y+=1
            columns.extend((first,len(pixels),0));columns.extend(pixels);columns.append(0)
        columns.append(255)
    return struct.pack('<4h',w,h,left,top)+struct.pack(f'<{w}I',*offsets)+columns

def build(path):
    from rail_title import board
    lumps=[('S_START',b'')]
    for prefix,frames,paint in [('COND','ABCDEFGH',crew),('BRAK','ABC',brake),('BRKM','ABCDEF',porter)]:
        for frame in frames:lumps.append((prefix+frame+'0',patch(*paint(frame))))
    lumps.extend([('S_END',b''),('RLTITLE',patch(board()))])
    data=bytearray(12);table=bytearray()
    for name,payload in lumps:
        table.extend(struct.pack('<II8s',len(data),len(payload),name.encode()));data.extend(payload)
    struct.pack_into('<4sII',data,0,b'PWAD',len(lumps),len(data))
    path.write_bytes(data+table)

if __name__=='__main__':
    build(Path(__file__).with_name('rail-sprites.wad'))
