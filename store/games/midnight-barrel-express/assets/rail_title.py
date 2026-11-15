"""The Midnight Barrel Express departure board, hand-lettered in palette indices."""
import math
from PIL import Image, ImageDraw

FONT={
'A':["01110","11011","11011","11111","11011","11011","11011"],
'B':["11110","11011","11011","11110","11011","11011","11110"],
'C':["01111","11000","11000","11000","11000","11000","01111"],
'D':["11110","11011","11011","11011","11011","11011","11110"],
'E':["11111","11000","11000","11110","11000","11000","11111"],
'F':["11111","11000","11000","11110","11000","11000","11000"],
'G':["01111","11000","11000","11011","11001","11001","01111"],
'H':["11011","11011","11011","11111","11011","11011","11011"],
'I':["11111","00100","00100","00100","00100","00100","11111"],
'L':["11000","11000","11000","11000","11000","11000","11111"],
'M':["11011","11111","11111","11011","11011","11011","11011"],
'N':["11001","11101","11101","11011","11011","11011","11001"],
'O':["01110","11011","11011","11011","11011","11011","01110"],
'P':["11110","11011","11011","11110","11000","11000","11000"],
'R':["11110","11011","11011","11110","11100","11010","11011"],
'S':["01111","11000","11000","01110","00011","00011","11110"],
'T':["11111","00100","00100","00100","00100","00100","00100"],
'U':["11011","11011","11011","11011","11011","11011","01110"],
'X':["11011","11011","01010","00100","01010","11011","11011"],
'Y':["11011","11011","01010","00100","00100","00100","00100"],
'0':["01110","10001","10011","10101","11001","10001","01110"],
'7':["11111","00001","00010","00010","00100","01000","01000"],
' ':["00000"]*7,
}

def letter(im,text,cx,y,sx,sy,colors,skew=0,track=1,shadow=0):
    w=(len(text)*(5*sx+track)-track)+skew
    x=cx-w//2
    d=ImageDraw.Draw(im)
    if shadow:
        letter(im,text,cx+shadow,y+shadow,sx,sy,(77,77,77),skew,track)
    for index,ch in enumerate(text):
        for line,row in enumerate(FONT[ch]):
            for dy in range(sy):
                offset=round(skew*(1-(line+dy/sy)/7))
                shade=colors[0] if line<3 else colors[1] if line<5 else colors[2]
                for k,b in enumerate(row):
                    if b=='1':
                        xx=x+index*(5*sx+track)+k*sx+offset
                        yy=y+line*sy+dy
                        d.line((xx,yy,xx+sx-1,yy),fill=shade)

def wheel(d,x,y,r):
    d.ellipse((x-r-1,y-r-1,x+r+1,y+r+1),fill=8,outline=162,width=2)
    d.ellipse((x-r+2,y-r+2,x+r-2,y+r-2),fill=243,outline=165,width=1)
    for i in range(6):
        a=i*math.tau/6
        d.line((x,y,x+round((r-3)*math.cos(a)),y+round((r-3)*math.sin(a))),fill=164,width=2)
    d.ellipse((x-2,y-2,x+2,y+2),fill=225)

def loco(d):
    # An inconvenient design decision by management: the boiler has a bung.
    # Fortunately the very small conductor still fits under the station clock.
    for x,y in [(81,96),(94,102),(76,107)]:
        d.line((23,y,x,y),fill=164 if y==96 else 242,width=2)
    # Ivory plumes embedded in the night sky, broad enough to survive a 320px screen.
    d.polygon([(193,60),(188,56),(178,56),(174,53),(163,53),(154,49),(136,49),(126,45),
               (145,43),(159,45),(173,45),(184,49),(191,51),(195,56)],fill=135)
    d.polygon([(187,52),(177,51),(173,48),(159,48),(151,45),(136,45),(148,42),(159,42),
               (171,45),(177,44),(185,47),(189,50)],fill=50)
    d.polygon([(220,80),(277,63),(290,64),(228,87)],fill=76)
    d.polygon([(224,81),(282,68),(282,70),(225,85)],fill=163)
    # Art Deco streamlining and a brass drum boiler in angular three-quarter profile.
    d.polygon([(89,85),(92,64),(99,59),(119,58),(130,62),(131,87)],fill=8,outline=162,width=2)
    d.polygon([(90,61),(96,57),(121,55),(134,59),(133,62),(91,65)],fill=162)
    d.line((96,58,120,56,130,58),fill=225,width=2)
    d.polygon([(99,67),(109,66),(108,78),(97,80)],fill=206)
    d.polygon([(113,65),(123,65),(123,77),(112,78)],fill=166)
    d.line((113,75,121,71),fill=225)
    d.polygon([(103,86),(101,82),(107,81),(112,84),(114,82),(117,83),(116,89)],fill=163)
    d.polygon([(85,84),(108,80),(172,72),(208,75),(225,83),(225,94),(214,102),(130,109),(86,100)],fill=8)
    d.polygon([(91,84),(121,77),(166,69),(204,71),(218,79),(222,87),(217,94),(128,103),(89,96)],fill=164)
    d.polygon([(111,81),(166,71),(200,71),(207,74),(158,79),(116,87),(93,89)],fill=161)
    d.polygon([(91,92),(147,83),(202,76),(210,78),(205,81),(144,89),(90,97)],fill=246)
    d.line((105,93,150,85,207,79),fill=225,width=2)
    d.polygon([(129,88),(166,82),(195,80),(198,88),(143,97),(108,99)],fill=242)
    # Barrel hoops cross the swept paintwork. A big black pilot leads the train.
    for xx,yy in [(139,77),(162,73),(189,72)]:
        d.line([(xx,yy),(xx+4,yy+3),(xx+7,yy+12)],fill=76,width=3)
        d.line([(xx,yy),(xx+3,yy+2),(xx+6,yy+10)],fill=162,width=2)
    d.ellipse((197,72,219,94),fill=75,outline=162,width=2)
    d.ellipse((200,74,217,90),fill=242,outline=164)
    d.ellipse((202,73,213,83),fill=225,outline=162,width=2)
    d.line((205,75,210,75),fill=224,width=2)
    d.polygon([(212,91),(223,90),(241,102),(224,105),(207,102)],fill=75,outline=162)
    for x in range(217,231,4):d.line((218+(x-217)//2,94,x+4,101),fill=163)
    d.polygon([(180,71),(181,62),(178,60),(177,56),(196,54),(198,58),(194,62),(194,70)],fill=8,outline=164)
    d.line((179,56,195,55),fill=225,width=2)
    d.polygon([(152,74),(153,67),(159,63),(163,65),(166,72)],fill=77,outline=162)
    d.line((155,67,159,64,162,65),fill=225)
    d.polygon([(79,99),(92,96),(133,101),(214,92),(214,100),(132,112),(81,106)],fill=8)
    for x,y,r in [(102,102,7),(121,103,9),(145,102,9),(172,99,9),(195,96,7)]:wheel(d,x,y,r)
    d.line((119,105,147,103,171,101,193,97),fill=77,width=5)
    d.line((119,104,147,102,171,100,193,96),fill=225,width=2)
    d.line((63,116,216,105,263,104),fill=164,width=2)
    d.line((70,119,227,108,273,107),fill=162)

def board():
    im=Image.new('P',(320,200),245);d=ImageDraw.Draw(im)
    for y in range(8,193,4):d.line((7,y,312,y),fill=244)
    for side in (-1,1):
        # Pressed-brass stepped corners and converging rails.
        def xy(x,y):return (x,y) if side<0 else (319-x,y)
        d.line([xy(5,39),xy(5,15),xy(15,5),xy(100,5)],fill=162,width=2)
        d.line([xy(10,34),xy(10,17),xy(18,10),xy(78,10)],fill=75)
        d.line([xy(5,163),xy(5,185),xy(14,193),xy(95,193)],fill=163,width=2)
        for z in range(3):d.line([xy(16,106+z*4),xy(36+z*9,106+z*4)],fill=164)
        for row in range(4):
            d.line([xy(50,64+row*4),xy(72+row*8,64+row*4)],fill=242)
    # The moon is part of the line's old enamel crest.
    d.ellipse((127,39,220,122),fill=8,outline=76,width=2)
    d.ellipse((131,42,217,117),fill=243,outline=164,width=2)
    d.ellipse((136,46,212,113),fill=242,outline=205)
    for i in range(26):
        x=21+(i*43)%275;y=42+(i*29)%56
        if (x-174)**2/1600+(y-81)**2/1100>1.1:
            d.point((x,y),fill=161 if i%3==0 else 136)
    for x,y in [(65,51),(242,50),(266,87),(35,81)]:
        d.line((x-3,y,x+3,y),fill=162);d.line((x,y-4,x,y+4),fill=162);d.point((x,y),fill=225)
    letter(im,'PHOBOS TRANSIT',160,11,1,1,(162,162,164),0,2)
    letter(im,'MIDNIGHT',160,23,3,2,(225,162,163),2,3,1)
    loco(d)
    d.line((16,127,303,127),fill=76)
    letter(im,'BARREL',160,124,5,4,(225,161,162),5,4,3)
    for x in (12,250):
        for j in range(3):
            length=38-j*7
            a=x if x<100 else 307-length
            d.line((a,138+j*4,a+length,138+j*4),fill=163 if j==0 else 75,width=2)
    letter(im,'EXPRESS',160,158,4,3,(225,195,196),4,4,2)
    for x in (64,256):d.line((x-11,186,x+11,186),fill=163)
    letter(im,'NIGHT FREIGHT',160,184,1,1,(162,162,164),0,2)
    return im
