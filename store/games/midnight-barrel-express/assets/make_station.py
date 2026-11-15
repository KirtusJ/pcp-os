"""The Midnight Express arrival ticket and cab instruments, drawn in the IWAD palette."""
from pathlib import Path
import argparse,math,random,struct
from PIL import Image, ImageDraw, ImageFont

FONTS=Path('/usr/share/fonts/opentype/urw-base35')
def ink(im,x,y,words,size,color,face='NimbusSansNarrow-Bold.otf',width=None):
    font=ImageFont.truetype(str(FONTS/face),size)
    l,t,r,b=font.getbbox(words);m=Image.new('L',(r-l,b-t))
    ImageDraw.Draw(m).text((-l,-t),words,font=font,fill=255)
    if width:m=m.resize((width,m.height),Image.Resampling.LANCZOS)
    im.paste(color,(x,y,x+m.width,y+m.height),m)

def arrival():
    rng=random.Random(77);im=Image.new('RGB',(320,200),'#0c2022');d=ImageDraw.Draw(im)
    for y in range(200):
        for x in range(320):
            v=12+int((180-y)*.08)+rng.randrange(6)
            im.putpixel((x,y),(v,max(0,v+8),v+12))
    d.rectangle((6,6,313,193),outline='#98734a');d.rectangle((9,9,310,190),outline='#504b3b')
    d.rectangle((16,16,303,127),fill='#10272c',outline='#d6af74')
    for x in range(20,300):
        h=int(93+5*math.sin(x*.083)+3*math.cos(x*.24))
        d.line((x,h,x,120),fill='#233a39')
    d.ellipse((239,22,271,54),fill='#e2c58e')
    for _ in range(62):
        x,y=rng.randint(22,294),rng.randint(21,91);d.point((x,y),fill='#a99a79')
    # A brass diner clock over the empty platform, and the engine finally at rest.
    d.rectangle((17,106,302,125),fill='#252620')
    for y in (110,117,121):d.line((19,y,300,y),fill='#6c6347')
    for x in range(25,300,22):d.line((x,118,x-5,124),fill='#433a2b',width=2)
    for x in (55,96,142,186,223):
        d.rectangle((x-13,78,x+12,102),fill='#142020',outline='#5a5742')
        for k in (-9,-2,5):d.rectangle((x+k,82,x+k+3,88),fill='#edbd69')
        for k in (-7,6):d.ellipse((x+k-3,100,x+k+3,106),fill='#171a18',outline='#9b7846')
    d.rounded_rectangle((236,81,281,101),radius=8,fill='#424b3d',outline='#e4b46c')
    d.rectangle((232,71,248,99),fill='#334337',outline='#cfaa70')
    d.rectangle((236,75,244,83),fill='#eac67a')
    d.polygon([(256,81),(257,72),(254,69),(254,67),(267,67),(267,69),(264,73),(265,81)],fill='#151c19',outline='#b4854e')
    d.line((245,98,279,98),fill='#d59b53')
    for x in (241,251,262,273):
        d.ellipse((x-4,98,x+4,106),fill='#22251d',outline='#b28a50')
    d.line((241,102,275,102),fill='#dec184')
    d.ellipse((276,84,282,90),fill='#ffe3a1')
    ink(im,26,23,'PHOBOS RAILWAY',11,'#d6ba85',width=101)
    ink(im,24,38,'END OF THE LINE',32,'#f5dca9',width=197)
    d.line((24,67,218,67),fill='#ab8e59')
    ink(im,25,70,'DEIMOS JUNCTION',9,'#b5a47d',width=102)
    d.rectangle((16,132,303,178),fill='#d7b887')
    d.line((162,136,162,175),fill='#91794f')
    for x,y,s in [(24,139,'STOP POSITION'),(173,139,'BRAKE ATTEMPTS')]:
        ink(im,x,y,s,10,'#494b37',width=80)
    ink(im,25,183,'MIDNIGHT BARREL EXPRESS',8,'#c4ab7d',width=103)
    return im

def panel():
    rng=random.Random(77);im=Image.new('RGB',(320,53),'#1e2927');d=ImageDraw.Draw(im)
    d.line((0,0,319,0),fill='#f0be7a');d.line((0,1,319,1),fill='#855f37')
    for x in (2,99,219,317):
        d.line((x,4,x,49),fill='#424941')
    for x in (6,313):
        for y in (6,47):d.point((x,y),fill='#9b7950')
    # Brass speedometer dial. Zero is seven o'clock; full speed is five.
    cx,cy=49,33
    d.ellipse((cx-27,cy-27,cx+27,cy+27),fill='#c69b61',outline='#efd2a0')
    d.ellipse((cx-24,cy-24,cx+24,cy+24),fill='#25302a',outline='#5a5037')
    for k in range(19):
        ang=math.radians(145+k*(250/18))
        lo=18 if k%3==0 else 20
        x0=cx+round(lo*math.cos(ang));y0=cy+round(lo*math.sin(ang))
        x1=cx+round(22*math.cos(ang));y1=cy+round(22*math.sin(ang))
        d.line((x0,y0,x1,y1),fill='#e0bc7e' if k<16 else '#d96945')
    d.rectangle((13,45,85,52),fill='#202c27')
    ink(im,33,45,'KM/H',8,'#d8be8e',width=32)
    d.rounded_rectangle((105,7,212,45),radius=3,fill='#101d1a',outline='#8e7959')
    ink(im,110,10,'SIGNAL DISTANCE',8,'#b6956e',width=76)
    d.rounded_rectangle((223,7,308,45),radius=3,fill='#101d1a',outline='#8e7959')
    ink(im,230,10,'BRAKE VALVE',8,'#b6956e',width=60)
    return im

def indexed(im, pal):
    sw=Image.new('P',(1,1));sw.putpalette(pal)
    return im.quantize(palette=sw,dither=Image.Dither.NONE)

def patch(pic):
    w,h=pic.size;data=pic.tobytes();body=bytearray();offsets=[]
    for x in range(w):
        offsets.append(8+4*w+len(body));body.extend((0,h,0));body.extend(data[x::w]);body.extend((0,255))
    return struct.pack('<4h',w,h,0,0)+struct.pack('<'+'I'*w,*offsets)+body

def pack(resources):
    raw=bytearray();directory=bytearray()
    for key,im in resources:
        data=patch(im);directory.extend(struct.pack('<ii8s',12+len(raw),len(data),key));raw.extend(data)
    return struct.pack('<4sii',b'PWAD',len(resources),12+len(raw))+raw+directory

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('iwad',type=Path);ap.add_argument('output',type=Path);ap.add_argument('--preview',type=Path)
    args=ap.parse_args();b=args.iwad.read_bytes();n,o=struct.unpack_from('<II',b,4)
    pal=next(b[p:p+768] for p,s,k in struct.iter_unpack('<II8s',b[o:o+16*n]) if k.rstrip(b'\0')==b'PLAYPAL')
    ticket=indexed(arrival(),pal);dash=indexed(panel(),pal);args.output.write_bytes(pack([(b'RLTICKET',ticket),(b'RLPANEL',dash)]))
    if args.preview:ticket.save(args.preview)
