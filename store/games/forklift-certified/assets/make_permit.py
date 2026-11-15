"""Draw the Phobos freight desk's operator permit in the original game palette."""
import argparse
import math
from pathlib import Path
import random
import struct
from PIL import Image, ImageDraw, ImageFont

FONTS=Path('/usr/share/fonts/opentype/urw-base35')
def ink(im,x,y,words,size,color,face='NimbusSansNarrow-Bold.otf',width=None):
    font=ImageFont.truetype(str(FONTS/face),size)
    l,t,r,b=font.getbbox(words)
    mask=Image.new('L',(r-l,b-t))
    ImageDraw.Draw(mask).text((-l,-t),words,font=font,fill=255)
    if width:mask=mask.resize((width,mask.height),Image.Resampling.LANCZOS)
    im.paste(color,(x,y,x+mask.width,y+mask.height),mask)

def picture():
    rng=random.Random(7007)
    im=Image.new('RGB',(320,200),'#202725');d=ImageDraw.Draw(im)
    for y in range(200):
        for x in range(320):
            n=rng.randrange(0,6)
            v=25+((x-y)//17)%3
            im.putpixel((x,y),(v+n,v+4+n,v+3+n))
    d.rounded_rectangle((10,11,311,177),radius=6,fill='#121615')
    d.rounded_rectangle((8,8,309,174),radius=6,fill='#caab7c',outline='#f2ddac')
    d.rounded_rectangle((12,12,305,170),radius=4,outline='#7c7355')
    for _ in range(13500):
        x=rng.randrange(15,304);y=rng.randrange(15,168)
        d.point((x,y),fill=rng.choice(['#ceaf84','#cdb287','#c9ae80','#c5a77a']))
    for x in (19,298):
        for y in (19,163):
            d.ellipse((x-2,y-2,x+2,y+2),fill='#4f543f')
            d.line((x-1,y+1,x+1,y-1),fill='#ab9569')
    ink(im,26,18,'PHOBOS PORT AUTHORITY',12,'#2d3934',width=134)
    ink(im,226,19,'FREIGHT DIV. 07',8,'#4e513f','NimbusMonoPS-Bold.otf',68)
    d.rectangle((19,34,298,55),fill='#2a3e39')
    ink(im,25,37,'FORKLIFT OPERATOR PERMIT',20,'#eed6a3',width=237)
    # ID portrait. The barrel insisted on wearing the hard hat in his photograph.
    d.rectangle((22,62,98,143),fill='#3f4942')
    for y in range(66,140,8):d.line((24,y,96,y),fill='#586055')
    for x in range(34,85):
        u=(x-59)/25
        for y in range(88,135):
            if (abs(u)>.94 and y in (88,89,132,133,134)):continue
            m=max(0,.8*math.sqrt(max(0,1-u*u))-.22*u)
            if 91<y<96 or 124<y<129:im.putpixel((x,y),(int(56+90*m),int(65+88*m),int(39+65*m)))
            else:im.putpixel((x,y),(int(85+134*m),int(58+108*m),int(23+49*m)))
    d.polygon([(32,88),(36,83),(39,80),(40,75),(54,70),(70,71),(79,75),(82,82),(88,85),(88,88)],fill='#c38b2d',outline='#5a4528')
    d.polygon([(43,81),(47,74),(59,71),(58,79)],fill='#edd479')
    d.line((33,88,86,88),fill='#f1cd68',width=2)
    d.line([(42,100),(47,103),(51,101)],fill='#241f19',width=2)
    d.polygon([(65,100),(77,96),(73,103),(65,105)],fill='#353428')
    d.point((70,101),fill='#e8e4b4')
    ink(im,52,109,'7',17,'#e7dfae','NimbusSans-BoldItalic.otf',12)
    d.line([(46,132),(55,134),(71,131)],fill='#282a24',width=2)
    ink(im,22,148,'BADGE NO. 0007',8,'#4b513e','NimbusMonoPS-Bold.otf',78)
    ink(im,109,64,'LICENSED TO',8,'#525b41')
    ink(im,108,75,'LUCKY SEVEN',24,'#26342d',width=150)
    ink(im,109,99,'CAUSE',8,'#626145')
    ink(im,109,110,'THREE BAYS OF PROPERTY DAMAGE',10,'#393b2b',width=162)
    # Hand-stamped cancelation lies over the card rather than covering the name.
    stamp=Image.new('RGBA',(151,39));sd=ImageDraw.Draw(stamp)
    sd.rounded_rectangle((1,1,149,37),3,outline='#8c281f',width=3)
    sd.rectangle((5,5,145,33),outline='#a33926')
    ink(stamp,10,7,'REVOKED',30,'#8c281f',width=128)
    stamp=stamp.rotate(5,Image.Resampling.BICUBIC,expand=True)
    im.paste(stamp,(139,120),stamp)
    # Plastic clip and short reading stripe.
    d.rounded_rectangle((147,11,168,15),2,fill='#4f5140')
    return im

def in_palette(im, raw):
    count,off=struct.unpack_from('<II',raw,4)
    palette=next(raw[pos:pos+768] for pos,n,k in struct.iter_unpack('<II8s',raw[off:off+count*16]) if k.rstrip(b'\0')==b'PLAYPAL')
    tbl=Image.new('P',(1,1));tbl.putpalette(palette)
    pic=im.quantize(palette=tbl,dither=Image.Dither.NONE)
    w,h=pic.size;data=pic.tobytes();body=bytearray();offsets=[]
    for x in range(w):
        offsets.append(8+4*w+len(body));body.extend((0,h,0));body.extend(data[x::w]);body.extend((0,255))
    patch=struct.pack('<4h',w,h,0,0)+struct.pack('<'+'I'*w,*offsets)+body
    return struct.pack('<4sII',b'PWAD',1,12+len(patch))+patch+struct.pack('<II8s',12,len(patch),b'FKPERMIT'),pic

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('original',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--preview',type=Path)
    args=parser.parse_args();data,pic=in_palette(picture(),args.original.read_bytes())
    args.output.write_bytes(data)
    if args.preview:pic.save(args.preview)
