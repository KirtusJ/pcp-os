import math,random,struct
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import argparse
parser = argparse.ArgumentParser(description="Generate the title, signs, floor and interface patches in a local Doom 1.9 palette.")
parser.add_argument("iwad", type=Path)
parser.add_argument("output_wad", type=Path)
parser.add_argument("--preview", type=Path)
args = parser.parse_args()
data = args.iwad.read_bytes()
_, count, directory = struct.unpack_from("<4sII", data)
entries = (struct.unpack_from("<II8s", data, directory + 16*i) for i in range(count))
palette_start, palette_size = next((start, size) for start, size, name in entries if name.rstrip(bytes(1)) == b"PLAYPAL")
W = {b"PLAYPAL": data[palette_start:palette_start + palette_size]}
palette=Image.new('P',(1,1));palette.putpalette(W[b'PLAYPAL'][:768])
COND='/usr/share/fonts/opentype/urw-base35/NimbusSansNarrow-Bold.otf'
MONO='/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf'
def font(s,which=COND):return ImageFont.truetype(which,s)
def ink_text(pic,x,y,string,size,color,targetwidth=None,face=COND):
    f=font(size,face);box=f.getbbox(string);mask=Image.new('L',(box[2]-box[0],box[3]-box[1]));ImageDraw.Draw(mask).text((-box[0],-box[1]),string,fill=255,font=f)
    if targetwidth:mask=mask.resize((targetwidth,mask.height),Image.Resampling.LANCZOS)
    pic.paste(color,(x,y,x+mask.width,y+mask.height),mask)
def col_mix(a,b,t):return tuple(round(u+(v-u)*t) for u,v in zip(a,b))
def omega(draw,x,y,w,color,width=3):
    h=w*.89
    xy=[(x+int(px*w),y+int(py*h)) for px,py in [(-.48,.45),(-.16,.45),(-.16,.25),(-.34,.06),(-.39,-.13),(-.33,-.34),(-.18,-.48),(.18,-.48),(.33,-.34),(.39,-.13),(.34,.06),(.16,.25),(.16,.45),(.48,.45)]]
    draw.line(xy,fill=color,width=width)
def drum(pic,cx,base,w,h,col,style,seed):
    rng=random.Random(seed);d=ImageDraw.Draw(pic)
    # The light falls left of centre on the curved lacquer and rust.
    for v in range(h):
        r=(v+.5)/h
        taper=.79+.20*math.sin(math.pi*r)**.55
        half=w*taper/2
        for x in range(math.ceil(cx-half),math.floor(cx+half)+1):
            u=(x-cx)/half
            lit=math.sqrt(max(0,1-u*u))*.65+max(-u,0)*.34
            shine=.23*math.exp(-((u+.45)/.16)**2)
            m=(.16+lit*.82+shine)*(1-.27*r)+rng.uniform(-.065,.065)
            if min(abs(r-a) for a in (.11,.20,.72,.86))<max(.016,1/h):
                brass=(229,157,52);m=(.27+lit*.75+shine)*(.68 if int(r*h)%3==0 else 1)
                rgb=tuple(min(255,int(v*m)) for v in brass)
            else:rgb=tuple(min(255,max(0,int(v*m))) for v in col)
            d.point((x,base-h+v),fill=rgb)
    dx=int(w*.42);cy=base-h+round(h*.025);ry=max(2,round(h*.065))
    d.ellipse((cx-dx,cy-ry,cx+dx,cy+ry),fill='#1a1715',outline='#eda442',width=max(1,w//70))
    d.arc((cx-dx+2,cy-ry+1,cx+dx-3,cy+ry-1),12,166,fill='#f4be5e',width=1)
    for px in (-.38,.35):
        for py in (.2,.72,.86):
            xx=int(cx+w*px);yy=int(base-h+h*py);d.point((xx,yy),fill='#f8c077')
    eye=base-h+int(h*.36);cx=int(cx)
    if style=='omega':
        omega(d,cx,int(base-h+h*.50),int(w*.46),'#20140c',max(3,w//11))
        omega(d,cx,int(base-h+h*.49),int(w*.46),'#ffe092',max(2,w//16))
        d.line((cx-int(w*.28),base-int(h*.27),cx+int(w*.26),base-int(h*.27)),fill='#191812',width=2)
    else:
        for sign in (-1,1):
            p=[(cx+int(sign*w*.33),eye-int(.05*h)),(cx+int(sign*w*.1),eye+int(.01*h)),(cx+int(sign*w*.14),eye+max(2,int(.07*h)))]
            d.polygon(p,fill='#e8f4a1')
        y=base-h+int(.57*h);d.line([(cx-int(w*.25),y-2),(cx-int(w*.15),y+2),(cx+int(w*.15),y+2),(cx+int(w*.25),y-2)],fill='#1a190f',width=2)
    if style=='omega':
        x0=cx-int(w*.22);x1=cx+int(w*.22)
        d.polygon([(x0,cy),(x0-int(w*.06),cy-int(h*.15)),(cx-int(w*.11),cy-int(h*.08)),(cx,cy-int(h*.22)),(cx+int(w*.11),cy-int(h*.08)),(x1+int(w*.06),cy-int(h*.15)),(x1,cy)],fill='#c9892f',outline='#f6cb66')
def poster(end=False):
    rng=random.Random(27 if end else 21);img=Image.new('RGB',(320,200));d=ImageDraw.Draw(img)
    for y in range(200):
        for x in range(320):
            light=max(0,1-math.hypot((x-240)/172,(y-78)/105))
            noise=rng.uniform(-2.5,2.5)
            img.putpixel((x,y),tuple(max(0,int(c+noise)) for c in col_mix((16,21,24),(125,66,30),light*.88)))
    for cx,cy in [(218,86),(255,86),(298,86)]:
        d.line((cx,0,cx,cy),width=2,fill='#393632')
        for yy in range(7,cy,12):d.point((cx+1,yy),fill='#877653')
    d.ellipse((181,33,310,161),outline='#705936',width=1)
    d.ellipse((185,37,306,157),outline='#483723',width=1)
    for j in range(9):
        x=160+j*22;hh=rng.randrange(12,44);d.rectangle((x,139-hh,x+8,157),fill='#292b29')
        for yy in range(139-hh+3,135,4):d.line((x+1,yy,x+6,yy),fill='#73492a')
    d.polygon([(0,146),(320,129),(320,199),(0,199)],fill='#191b19')
    for j in range(6):
        y=int(132+j*j*2.25);d.line((0,y+18,320,y),fill='#3a3530')
    for j in range(-4,8):d.line((230,131,j*80,200),fill='#332e27')
    for j in range(122):
        x=rng.randrange(2,318);y=rng.randrange(12,174)
        if rng.randrange(5)==0:d.point((x,y),fill='#a67839')
    if end:
        for cx,cy,w,h,color in [(170,152,43,39,(144,89,55)),(281,136,58,52,(161,76,43))]:
            drum(img,cx,cy,w,h,color,'omega',cx)
            d.polygon([(cx-w//2,cy-h-8),(cx+w//2,cy-h-8),(cx+w//2,cy-h//2+4),(cx+w//4,cy-h//2-2),(cx,cy-h//2+6),(cx-w//4,cy-h//2-3),(cx-w//2,cy-h//2+2)],fill='#232424')
        drum(img,229,154,37,46,(125,145,62),'mini',51)
        words=[('ONE',26,47,131),('MORE',64,46,136),('INSIDE.',100,46,154)]
    else:
        drum(img,248,149,114,116,(180,69,38),'omega',15)
        drum(img,207,155,47,62,(152,135,65),'mini',16)
        drum(img,289,160,30,38,(103,129,54),'mini',17)
        words=[('ULTRA',23,47,147),('OMEGA',60,52,161),('BARREL',103,52,166)]
    for s,y,fs,w in words:
        ink_text(img,14,y+2,s,fs,'#101214',w);ink_text(img,13,y,s,fs,'#ead2a0' if s not in ('OMEGA','MORE') else '#f4a747',w)
    d.line((14,13,305,13),fill='#947342')
    d.line((14,170,305,170),fill='#947342')
    ink_text(img,15,177,'THE LAST SHELL' if end else 'A DOOM SHAREWARE CAMPAIGN',9,'#d3b886',None,MONO)
    omega(d,299,182,12,'#da9749',2)
    return img
def sign(which):
    im=Image.new('RGB',(64,64));d=ImageDraw.Draw(im)
    for y in range(64):
        for x in range(64):
            a=((x*11+y*13+(x*y)%7)%12)/11
            im.putpixel((x,y),col_mix((37,43,40),(66,69,56),a*.6+(1-y/63)*.4))
    d.rectangle((1,1,62,62),outline='#161c18');d.rectangle((3,3,60,60),outline='#a38c5a')
    if which==0:
        d.rectangle((5,8,58,42),fill='#24271e')
        for cx,w in [(18,15),(35,10),(48,6)]:
            h=w+4; d.rounded_rectangle((cx-w//2,36-h,cx+w//2,36),radius=2,fill='#dbae5c');d.line((cx-w//2,32,cx+w//2,32),fill='#745422')
        ink_text(im,7,45,'SHELL WORKS',11,'#f3cf82',49)
    elif which==1:
        d.rectangle((6,6,57,49),fill='#292722')
        omega(d,32,27,35,'#ffc369',5)
        ink_text(im,10,51,'PRESS  08',8,'#deb56a',44,MONO)
    else:
        for x in range(-60,90,18):
            pts=[(x,18),(x+9,18),(x-15,46),(x-24,46)]
            d.polygon(pts,fill='#a07d3f')
        d.rectangle((5,20,58,43),outline='#3d392a')
    for x in (4,59):
        for y in (4,59): d.point((x,y),fill='#e5d599')
    return im
def floor():
    im=Image.new('RGB',(64,64));d=ImageDraw.Draw(im)
    for y in range(64):
        for x in range(64):
            p=(x+y)%23
            hue=(102,83,45) if (x<6 or y<6) and p<11 else (47,51,50)
            d.point((x,y),fill=tuple(max(0,v+(x*3+y*5+x*y)%11-5) for v in hue))
    d.rectangle((0,0,63,63),outline='#1b2020');d.rectangle((7,7,59,59),outline='#393e3b')
    for x in (9,57):
        for y in (9,57):d.point((x,y),fill='#989279')
    return im
def patch(im):
    alpha=im.getchannel('A') if im.mode=='RGBA' else None
    idx=im.convert('RGB').quantize(palette=palette,dither=Image.Dither.NONE)
    w,h=im.size;posts=bytearray();offset=[]
    for x in range(w):
        offset.append(8+4*w+len(posts));y=0
        while y<h:
            if alpha and alpha.getpixel((x,y))==0:y+=1;continue
            y0=y;data=bytearray()
            while y<h and (alpha is None or alpha.getpixel((x,y))):
                data.append(idx.getpixel((x,y)));y+=1
            posts.extend([y0,len(data),0]);posts.extend(data);posts.append(0)
        posts.append(255)
    return struct.pack('<4h',w,h,0,0)+struct.pack('<'+'I'*w,*offset)+posts
def label(s,w,h,size):
    img=Image.new('RGBA',(w,h));f=font(size)
    m=Image.new('L',(330,90));d=ImageDraw.Draw(m);d.text((0,0),s,font=f,fill=255,stroke_width=0);m=m.crop(m.getbbox());m.thumbnail((w-2,h-2))
    x=0;y=0
    img.paste('#804724',(x+1,y+1,x+1+m.width,y+1+m.height),m)
    img.paste('#f8c972',(x,y,x+m.width,y+m.height),m);return img
names=['The Loading Bay','Pressure Test','Split Shift','Control Room','Phobos Shell Works','The Core Line','Dispatch','The Last Press','The Spare Parts Depot']
title=poster();ending=poster(True)
logo=Image.new('RGBA',(123,60));logo.alpha_composite(label('ULTRA OMEGA',123,25,32),(0,0));logo.alpha_composite(label('BARREL',123,34,51),(0,24))
ls=[(b'TITLEPIC',patch(title)),(b'HELP2',patch(ending)),(b'M_DOOM',patch(logo)),(b'M_EPI1',patch(label('Ultra Omega Barrel',210,15,22)))]
for i,n in enumerate(names):ls.append((f'WILV0{i}'.encode(),patch(label(n.upper(),min(240,len(n)*11),15,21))))
for i,n in enumerate([b'W111_2',b'W111_3',b'W112_1']):ls.append((n,patch(sign(i))))
ls.append((b'FLOOR0_1',floor().quantize(palette=palette,dither=Image.Dither.NONE).tobytes()))
body=bytearray();directory=bytearray()
for k,v in ls:directory+=struct.pack('<II8s',12+len(body),len(v),k);body+=v
args.output_wad.write_bytes(struct.pack('<4sII',b'PWAD',len(ls),12+len(body))+body+directory)
if args.preview:
    title.quantize(palette=palette,dither=Image.Dither.NONE).resize((640,400),Image.Resampling.NEAREST).save(args.preview)
