"""Hand-built orthographic meshes for Forklift Certified's Doom sprite PWAD.

Run with Python 3, Pillow and NumPy. No game files are needed to build the PWAD.
All colors are Doom PLAYPAL indices.
"""
import math
from pathlib import Path
import struct

import numpy as np
from PIL import Image, ImageDraw


CLEAR = 255
YELLOW = (225, 160, 161, 162, 163, 164)
AMBER = (226, 249, 162, 163, 164, 165)
ORANGE = (248, 215, 218, 220, 232, 234)
STEEL = (94, 98, 101, 104, 106, 108)
SILVER = (81, 86, 89, 93, 97, 101)
TIRE = (102, 105, 108, 109, 110, 8)
BLACK = (98, 103, 105, 108, 109, 8)
GREEN = (113, 116, 120, 121, 123, 125)
RED = (173, 176, 179, 182, 185, 190)
GUN = (134, 137, 139, 141, 143, 108)
COPPER = (210, 62, 64, 67, 70, 73)
CRIMSON = (25, 33, 36, 38, 41, 44)
WOOD = (62, 65, 66, 68, 70, 73)
WHITE = (224, 80, 81, 83, 84, 89)
PAINT = (166, 167, 75, 76, 77, 78)


class Model:
    def __init__(self):
        self.faces = []

    def face(self, points, color):
        self.faces.append((points, color))

    def box(self, left, right, mat):
        x,y,z = left
        X,Y,Z = right
        self.face([(x,y,Z),(X,y,Z),(X,Y,Z),(x,Y,Z)],mat[1])
        self.face([(X,y,z),(X,Y,z),(X,Y,Z),(X,y,Z)],mat[2])
        self.face([(x,Y,z),(x,y,z),(x,y,Z),(x,Y,Z)],mat[4])
        self.face([(x,y,z),(X,y,z),(X,y,Z),(x,y,Z)],mat[3])
        self.face([(X,Y,z),(x,Y,z),(x,Y,Z),(X,Y,Z)],mat[4])
        self.face([(x,y,z),(x,Y,z),(X,Y,z),(X,y,z)],mat[5])

    def tube(self, axis, start, end, u, v, radius, mat, count=16):
        def xyz(k, a, b):
            if axis=='x':return (k,a,b)
            if axis=='y':return (a,k,b)
            return (a,b,k)
        ring=[(u+radius*math.cos(i*math.tau/count),v+radius*math.sin(i*math.tau/count)) for i in range(count)]
        for i,(a,b) in enumerate(ring):
            aa,bb=ring[(i+1)%count]
            theta=(i+.5)*math.tau/count
            lit=math.cos(theta-.35) if axis=='z' else math.sin(theta+.4)
            shade=min(5,max(0,round(3-2.2*lit)))
            self.face([xyz(start,a,b),xyz(end,a,b),xyz(end,aa,bb),xyz(start,aa,bb)],mat[shade])
        self.face([xyz(start,a,b) for a,b in ring],mat[4 if axis!='z' else 5])
        self.face([xyz(end,a,b) for a,b in ring],mat[2 if axis!='z' else 0])


def panel(model, axis, plane, origin, polygon, color, sign=1):
    """Paint a polygon on a vertical panel, in local horizontal/vertical coordinates."""
    u0,v0=origin
    if axis=='x':
        model.face([(plane, u0+sign*u, v0+v) for u,v in polygon],color)
    else:
        model.face([(u0+sign*u, plane, v0+v) for u,v in polygon],color)


def painted_seven(model, axis, plane, center, base, scale, sign=1):
    pts=[(-4,7),(5,7),(5,5),(1,-1),(-2,-1),(-1,2),(2,5),(-4,5)]
    panel(model,axis,plane,(center,base),[(a*scale,b*scale) for a,b in pts],224,sign)


def wheel(m,x,side,radius,phase):
    y=side*11
    m.tube('y',min(y,side*16),max(y,side*16),x,radius,radius,TIRE)
    # Raised treads cast their own silhouette in front, side, and rear views.
    for i in range(14):
        a=(i+phase*.5)*math.tau/14
        ring=[]
        for v,delta in [(side*16.3,-.10),(side*16.3,.10),(side*10.7,.14),(side*10.7,-.06)]:
            ring.append((x+(radius+.33)*math.cos(a+delta),v,radius+(radius+.33)*math.sin(a+delta)))
        m.face(ring,107 if i%3 else 105)
    cheek=side*16.55
    def disk(r,mat,dist):
        start,end=sorted((cheek,side*(16.55+dist)))
        m.tube('y',start,end,x,radius,r,mat,12)
    disk(radius*.72,BLACK,.2)
    disk(radius*.49,YELLOW,.55)
    disk(radius*.24,STEEL,.8)
    for n in range(4):
        a=(n+phase*.28)*math.pi/2
        u=x+radius*.34*math.cos(a)
        z=radius+radius*.34*math.sin(a)
        panel(m,'y',side*17.2,(u,z),[(-.55,-.55),(.55,-.55),(.55,.55),(-.55,.55)],89)


def driver(m,mode):
    # A compact brass drum bolted into the operator's seat, facing the mast (+X).
    m.tube('z',23,36,-3,0,6.2,YELLOW,18)
    m.tube('z',23,24.7,-3,0,6.65,GREEN)
    m.tube('z',34,35.6,-3,0,6.7,COPPER)
    m.tube('z',35.5,36.7,-3,0,6.6,YELLOW)
    for side in (-1,1):
        m.box((-6,min(side*6,side*6.42),25.3),(0,max(side*6,side*6.42),32),GREEN)
        painted_seven(m,'y',side*6.48,-3,26,.63,side)
    # Rakish eyebrows, gleaming eye and asymmetric enamel smile atop the drum.
    for yy in (-2.7,2.7):
        panel(m,'x',3.28,(yy,32.4),[(-1.7,-.5),(1.4,-.15),(1.3,1),(-1.7,.75)],8)
    panel(m,'x',3.32,(-2.7,32.4),[(0,0),(1,0),(1,.9),(0,.9)],225)
    panel(m,'x',3.4,(0,28.8),[(-3,.8),(-1,-1),(2,-.8),(3.3,.9),(.4,.1)],8)
    panel(m,'x',3.46,(0,29),[(-1.2,.15),(1.8,.25),(1.2,-.5),(-1,-.45)],224)
    # Lucky's upside-open horseshoe handle.
    m.box((-4,-3,36.3),(-2.5,-1.7,39.5),YELLOW)
    m.box((-4,1.7,36.3),(-2.5,3,40.4),YELLOW)
    m.box((-4,-3,36),(-2.5,3,37.1),AMBER)
    m.tube('y',-6.5,6.5,4.2,24,1.1,BLACK)
    m.box((.5,-5,25),(5,-3.6,27.5),YELLOW)
    m.box((.5,3.6,25),(5,5,27.5),YELLOW)


def cab(m,mode):
    for side in (-1,1):
        a,b=sorted((side*11,side*12.5))
        m.box((-14,a,19),(-12,b,44),BLACK)
        m.box((7,a,19),(9,b,44),BLACK)
        m.box((-14,a,42.5),(9,b,44.4),BLACK)
        m.box((-15,a-.3,43.3),(10,b+.3,44.7),YELLOW)
    m.box((-15,-12.6,42.7),(-12.8,12.6,44.5),BLACK)
    m.box((8,-12.6,42.7),(10,12.6,44.5),BLACK)
    for yy in (-8,-4,0,4,8):
        m.box((-14,yy-.35,43.2),(9,yy+.35,44.05),STEEL)
    # The stolen launcher is mounted sideways off-centre, with two rusty straps.
    m.tube('x',-18,10,-4,48,3.8,GUN)
    m.tube('x',-18.6,-15.8,-4,48,4.5,STEEL)
    m.tube('x',8.6,14.3,-4,48,4.5,STEEL)
    for xx in (-11,3):
        m.tube('x',xx,xx+1,-4,48,4.15,COPPER)
        m.box((xx,-9,43),(xx+1.1,-8,48),COPPER)
    if mode=='D':
        m.tube('x',14.4,15.4,-4,48,3.5,ORANGE)
        for i in range(7):
            a=i*math.tau/7
            m.face([(14.5,-4+3*math.cos(a-.4),48+3*math.sin(a-.4)),
                    (18+(i%3)*2,-4+4*math.cos(a),48+4*math.sin(a)),
                    (14.5,-4+3*math.cos(a+.4),48+3*math.sin(a+.4))],160 if i%3 else 215)
        m.tube('x',15,19,-4,48,1.7,(225,)*6)
    else:
        m.tube('x',14.3,14.5,-4,48,3.2,TIRE)
        m.tube('x',14.51,14.61,-4,48,1.3,RED if mode=='C' else BLACK,10)
    m.box((-14,5,44.1),(-7,11,45.5),BLACK)
    m.tube('z',45.5,49,-10.5,8,2.7,ORANGE,12)
    m.tube('z',49,50,-10.5,8,1.9,YELLOW,10)
    yy=8+(-1.9 if mode in 'AC' else 1.9)
    m.box((-10.7,yy-.55,46),(-9.4,yy+.55,48.8),(225,)*6)


def forklift(mode):
    m=Model()
    wreck=mode in 'FGH'
    phase=int(mode in 'BD')
    # Chassis, counterweight, yellow wheel arches, rear lighting and bumper.
    m.box((-23,-13,6),(18,13,11),BLACK)
    m.box((-21,-12,10),(17,12,17.3),YELLOW)
    m.box((-24,-12.8,13),(-11,12.8,21.5),YELLOW)
    m.box((-24.8,-12,9),(-22.2,12,12.2),BLACK)
    m.box((-24, -10,17),(-23.2,10,20),AMBER)
    for side in (-1,1):
        y=side*12.25
        m.box((-21,min(y,side*14),13.7),(15,max(y,side*14),16.4),YELLOW)
        a,b=sorted((side*7.5,side*11.3))
        m.box((-24.6,a,15),(-24,b,17.6),RED)
        m.box((-24.8,a,16.5),(-24.65,b,17.15),(173,)*6)
        for u in (-19,-17,-15):
            panel(m,'y',side*12.84,(u,17.8),[(0,0),(.9,0),(1.5,2.2),(.5,2.2)],77)
        panel(m,'y',side*14.06,(-7,14.2),[(-6,-1),(6,-1),(5,5),(-5,5)],124)
        painted_seven(m,'y',side*14.13,-7,14.5,.69,side)
    m.box((-9,-8,17),(0,8,19),BLACK)
    m.box((-9,-6,18),(-7,6,29),TIRE)
    for side in (-1,1):
        wheel(m,-15,side,6.6,phase)
        wheel(m,12,side,8,phase)
    # Silver channels + moving carriage are foremost. Forks project well past the tires.
    mast_height=31 if wreck else 42
    for side in (-1,1):
        y=side*8.7
        m.box((15,y-1.35,7),(17.7,y+1.35,mast_height),BLACK)
        m.box((16.7,y-.75,8),(18.1,y+.75,mast_height-1),SILVER)
        m.box((18,y-.7,10),(18.55,y+.7,mast_height-3),STEEL)
        m.tube('z',8,mast_height-6,14,side*6.5,1.05,SILVER,10)
    m.box((15.5,-10,8),(17.7,10,10),BLACK)
    m.box((15.5,-10,mast_height-2),(17.7,10,mast_height),AMBER)
    fork_z=8 if mode in 'CD' else 3.4 if not wreck else 1.8
    m.box((18,-12,fork_z+4),(20,12,fork_z+7),STEEL)
    for side in (-1,1):
        y=side*8.7
        m.box((19,y-1.5,fork_z),(21.5,y+1.5,fork_z+10),SILVER)
        m.box((19,y-1.5,fork_z-.4),(35,y+1.5,fork_z+1.5),SILVER)
        m.face([(35,y-1.5,fork_z+1.5),(35,y+1.5,fork_z+1.5),
                (42,y+1.1,fork_z-.15),(42,y-1.1,fork_z-.15)],81)
        m.face([(35,y-1.5,fork_z-.4),(42,y-1.1,fork_z-.15),
                (35,y-1.5,fork_z+1.5)],98)
        if mode in 'CD':
            m.box((21,y-1.52,fork_z+6),(21.65,y+1.52,fork_z+8.3),ORANGE)
    if wreck:
        # Cab is empty: a crushed tilted cage and a muzzle hanging over the hood.
        tilt=Model()
        cab(tilt,'A')
        for pts,col in tilt.faces:
            m.face([(x+4+(z-20)*.12,y+(z-26)*.27,17+(z-17)*.6) for x,y,z in pts],col)
    else:
        cab(m,mode)
        driver(m,mode)
    if mode in 'CD':
        for side in (-1,1):
            y=side*10.3
            m.box((17.8,y-1.3,34),(18.9,y+1.3,36.6),(225,160,249,162,163,164))
    if mode=='E' or wreck:
        for k in range(3):
            x=-22+k*3
            panel(m,'y',-13.01,(x,16+k%2),[(0,3),(1,0),(3,1),(5,-1.5),(2,0),(1,-1)],108)
    return m


GLYPHS={"0":["111","101","101","101","111"],"7":["111","001","010","010","100"]}


def cargo_label(m,axis,plane,sign):
    panel(m,axis,plane,(0,0),[(-10,14),(10,14),(10,29),(-10,29)],77,sign)
    bias=.08*(1 if plane>=0 else -1)
    panel(m,axis,plane+bias,(0,0),[(-9,15),(9,15),(9,28),(-9,28)],232,sign)
    for digit,base in zip("07",(-6.8,1.2)):
        for row,pattern in enumerate(GLYPHS[digit]):
            for c,bit in enumerate(pattern):
                if bit=='1':
                    u=base+c*2
                    v=25.6-row*2.3
                    panel(m,axis,plane+bias*2,(sign*u,v),[(0,-2.35),(2.05,-2.35),(2.05,0),(0,0)],225,sign)
    for u in (-8.3,7.4):
        panel(m,axis,plane+bias*2,(u,15.8),[(0,0),(1,0),(1,1),(0,1)],164,sign)


def cargo(frame):
    m=Model()
    for x in (-13,0,13):
        m.box((x-2,-17,0),(x+2,17,4),WOOD)
    for y in (-14,-5,5,14):m.box((-18,y-2,3),(18,y+2,5.5),WOOD)
    if frame in 'AB':
        m.box((-16,-16,5.5),(16,16,35.5),CRIMSON)
        for x in (-16.2,16.2):
            for y in (-16.2,16.2):
                m.box((x-1.2,y-1.2,6),(x+1.2,y+1.2,36),CRIMSON)
        for z in (7.7,31.5,35.4):
            m.box((-16.8,-16.8,z),(16.8,16.8,z+1.1),ORANGE if z==31.5 else CRIMSON)
        for axis in 'xy':
            for side in (-1,1):
                plane=side*16.1
                cargo_label(m,axis,plane,-side if axis=='x' else side)
                for u in (-13,13):
                    panel(m,axis,plane,(u,0),[(-.6,8),(.6,8),(.6,30),(-.6,30)],43)
                    panel(m,axis,plane+side*.02,(u,0),[(-.5,12),(.6,12),(.6,13),(-.5,13)],94)
                for i in range(8):
                    u=-15+i*4
                    panel(m,axis,side*16.86,(u,0),[(0,32),(1.6,32),(3.2,34.7),(1.6,34.7)],78)
                if frame=='B':
                    for ox in (-7,4):
                        panel(m,axis,side*16.7,(ox,0),[(-1,35),(1,35),(-2,29),(1,27),(-2,22),(-.5,27),(-4,29)],8)
        for y in (-9,9):
            m.box((-16.2,y-.8,36.5),(16.2,y+.8,37.4),BLACK)
        if frame=='B':
            for j,(pts,col) in enumerate(m.faces):
                m.faces[j]=([(x+(z-4)*.055,y-(z-4)*.06,z) for x,y,z in pts],col)
    else:
        m.box((-12,-12,5),(13,13,8.4),CRIMSON)
        # Folded shell panels have exposed bright red backs and buckled yellow trim.
        factor=1 if frame=='C' else .28
        for side in (-1,1):
            for axis in ('x','y'):
                v=side*(21 if frame=='C' else 24)
                def pt(u,w,z):
                    return (w,u,z) if axis=='x' else (u,w,z)
                base=side*10
                panelpts=[pt(-13,base,7),pt(13,base,7),pt(11,v,7+21*factor),
                          pt(5,v-side,7+17*factor),pt(-3,v+side*1.6,7+23*factor),pt(-12,v,7+17*factor)]
                m.face(panelpts,35 if side<0 else 38)
                m.face([pt(-11,v,7+18*factor),pt(11,v,7+20*factor),
                        pt(11,v-side*.6,7+18*factor),pt(-11,v-side*.6,7+16*factor)],164)
        # Detonated crate leaves two ugly bands and a broken numbered face plate.
        for yy in (-8,8):
            m.box((-12,yy,8),(12,yy+1.6,10),BLACK)
        m.face([(-9,-19,6),(6,-20,7),(8,-11,11),(-6,-10,12)],232)
        for i in range(3):
            m.face([(-4+i,-15,9.3),(4,-15,9.3),(4,-14,9.9),(-4+i,-14,9.9)],225)
    return m


def render(model, angle, frame, is_cargo=False):
    w,h=116,96
    cx,cy=58,80
    ox,oy=math.cos(angle),math.sin(angle)
    tilt=math.radians(12)
    lo,hi=math.sin(tilt),math.cos(tilt)
    # Depth-tested polygon rasterization keeps both fork tines and the visibly
    # asymmetrical launcher correctly occluded from all eight rotations.
    points=[];lengths=[];colors=[]
    for pts,c in model.faces:
        points.extend(pts)
        lengths.append(len(pts))
        colors.append(c)
    xyz=np.array(points,np.float32)
    side=xyz[:,0]*oy-xyz[:,1]*ox
    near=xyz[:,0]*ox+xyz[:,1]*oy
    p=np.column_stack((cx+side,cy+near*lo-xyz[:,2]*hi,near*hi+xyz[:,2]*lo))
    im=np.full((h,w),CLEAR,np.uint8)
    depth=np.full((h,w),-1000,np.float32)
    start=0
    for n,color in zip(lengths,colors):
        poly=p[start:start+n]
        start+=n
        x0=max(0,int(math.floor(poly[:,0].min())))
        y0=max(0,int(math.floor(poly[:,1].min())))
        x1=min(w,int(math.ceil(poly[:,0].max()))+1)
        y1=min(h,int(math.ceil(poly[:,1].max()))+1)
        if x1<=x0 or y1<=y0:continue
        anchor=poly[0]
        gx,gy=np.meshgrid(np.arange(x0,x1,dtype=np.float32)+.5,
                          np.arange(y0,y1,dtype=np.float32)+.5)
        gx-=anchor[0];gy-=anchor[1]
        for j in range(1,n-1):
            a=poly[j]-anchor
            b=poly[j+1]-anchor
            area=a[0]*b[1]-a[1]*b[0]
            if abs(area)<1e-5:continue
            u=(gx*b[1]-gy*b[0])/area
            v=(gy*a[0]-gx*a[1])/area
            z=anchor[2]+u*a[2]+v*b[2]
            sub=depth[y0:y1,x0:x1]
            take=(u>=-.001)&(v>=-.001)&(u+v<=1.001)&(z>=sub-.002)
            sub[take]=z[take]
            im[y0:y1,x0:x1][take]=color
    out=Image.frombytes("P",(w,h),im.tobytes())
    d=ImageDraw.Draw(out)
    def screen(x,y,z):
        return round(cx+x*oy-y*ox),round(cy+(x*ox+y*oy)*lo-z*hi)
    if not is_cargo and frame in 'CEFGH':
        if frame=='C':
            bx,by=screen(-10.5,8,49)
            d.line((bx-5,by,bx+5,by),fill=163);d.line((bx,by-4,bx,by+3),fill=162)
            d.line((bx-2,by,bx+2,by),fill=225);d.point((bx,by-1),fill=225)
        else:
            for i in range(4 if frame in 'EG' else 2):
                x,y=screen(-11+i%2*2,3,32+i*3)
                x+=(-1 if i%2 else 1)*(1+i)
                c=[98,101,105,107][i]
                if frame!='F':
                    d.ellipse((x-2-i//2,y-2,x+1+i//2,y+1),fill=c)
                    d.point((x-1,y-1),fill=c-1)
    if is_cargo and frame=='C':
        for x,y,z in [(-15,-11,26),(8,15,25),(15,-4,34),(-9,7,29)]:
            px,py=screen(x,y,z)
            d.polygon([(px-2,py),(px,py-2),(px+2,py+1),(px-1,py+1)],fill=217)
            d.point((px-1,py),fill=160)
    bbox=Image.fromarray(np.where(np.asarray(out)!=CLEAR,255,0).astype(np.uint8)).getbbox()
    return out.crop(bbox),(cx-bbox[0],cy-bbox[1])


def patch(im,anchor):
    w,h=im.size
    payload=bytearray()
    offsets=[]
    for x in range(w):
        offsets.append(8+w*4+len(payload))
        y=0
        while y<h:
            if im.getpixel((x,y))==CLEAR:
                y+=1
                continue
            start=y
            data=bytearray()
            while y<h and im.getpixel((x,y))!=CLEAR:
                data.append(im.getpixel((x,y)))
                y+=1
            payload.extend((start,len(data),0))
            payload.extend(data)
            payload.append(0)
        payload.append(255)
    return struct.pack("<4h",w,h,*anchor)+struct.pack(f"<{w}I",*offsets)+payload


def images():
    for frame in 'ABCDEFGH':
        model=forklift(frame)
        for rot in (range(1,9) if frame<'F' else [0]):
            angle=(rot-1)*math.tau/8 if rot else math.radians(42)
            im,anchor=render(model,angle,frame)
            yield f'FORK{frame}{rot}',im,anchor
    for frame in 'ABCD':
        im,anchor=render(cargo(frame),math.radians(33),frame,True)
        yield f'CARG{frame}0',im,anchor


def build(file):
    lumps=[('S_START',b'')]
    for name,img,origin in images():
        lumps.append((name,patch(img,origin)))
    lumps.append(('S_END',b''))
    data=bytearray(12);directory=bytearray()
    for name,src in lumps:
        directory.extend(struct.pack('<II8s',len(data),len(src),name.encode()))
        data.extend(src)
    struct.pack_into('<4sII',data,0,b'PWAD',len(lumps),len(data))
    file.write_bytes(data+directory)


if __name__=='__main__':
    build(Path(__file__).with_name('fork-sprites.wad'))
