// Interiors of the two working buildings at Bellweather. One logical pixel is one paint stroke.
function paintWorks(canvas, kind, phase) {
  const W=360,H=96;
  if(canvas.width!==W)canvas.width=W;
  if(canvas.height!==H)canvas.height=H;
  const ctx=canvas.getContext('2d'), active=phase>0;
  const iron='#29494e', dark='#223a3e', wood='#826148', oak='#b48b5a', cream='#efcf96';
  const brass='#dcad65', rust='#ac5942', sea='#7eaaa2', light='#d4cbb0';
  function r(x,y,w,h,c){ctx.fillStyle=c;ctx.fillRect(x|0,y|0,w|0,h|0)}
  function dot(x,y,c){r(x,y,1,1,c)}
  function line(x,y,X,Y,c){
    x=Math.round(x);y=Math.round(y);X=Math.round(X);Y=Math.round(Y);
    const dx=Math.abs(X-x),dy=-Math.abs(Y-y),sx=x<X?1:-1,sy=y<Y?1:-1;
    let e=dx+dy;
    for(;;){dot(x,y,c);if(x===X&&y===Y)break;const a=2*e;if(a>=dy){e+=dy;x+=sx}if(a<=dx){e+=dx;y+=sy}}
  }
  function ellipse(x,y,rx,ry,c){
    for(let dy=-ry;dy<=ry;dy++){const half=Math.round(rx*Math.sqrt(Math.max(0,1-dy*dy/(ry*ry))));r(x-half,y+dy,2*half+1,1,c)}
  }
  function board(x,y,w,h,tone){
    r(x,y,w,h,dark);r(x+1,y+1,w-2,h-2,tone);
    line(x+2,y+1,x+w-3,y+1,oak);dot(x+3,y+2,dark);dot(x+w-5,y+2,dark);
  }
  function lamp(x,y){
    r(x,3,1,y-3,dark);r(x-2,y-1,5,2,dark);r(x-4,y+1,9,1,'#415847');r(x-6,y+2,13,2,'#5b6e55');r(x-5,y+4,11,1,'#e8b566');r(x-2,y+5,5,2,'#ffe2a0')
  }
  function windowBay(x,w){
    r(x-4,11,w+8,47,'#273d40');r(x-2,12,w+4,44,'#ab9169');
    r(x,15,w,36,'#ced3b8');r(x,31,w,12,'#92b1a4');r(x,40,w,12,sea);
    for(let k=0;k<w;k++){const a=31+Math.round(Math.sin(k*.087)*3+Math.cos(k*.19)*2);r(x+k,a,1,38-a,'#90a79a')}
    for(let i=0;i<14;i++){const q=(13+i*23)%w;r(x+q,42+i%7,Math.min(2+i%6,w-q),1,'#c3c8ab')}
    const sail=x+Math.floor(w*.51);
    r(sail,24,1,18,'#687771');for(let j=0;j<12;j++)r(sail+1,26+j,1+j*.65,1,'#edd8af');
    r(sail-6,42,19,2,'#655e4c');r(sail-3,44,13,1,'#958968');
    for(let k=0;k<=w;k+=Math.floor(w/3))r(x+k,14,2,38,'#4e655e');
    r(x,34,w,2,'#6b7f72');r(x-4,52,w+8,3,'#6d6752');r(x-3,53,w+7,1,oak);
  }
  function kettle(x,y){
    r(x-3,28,6,20,'#657d73');r(x-2,27,11,2,'#939a7e');r(x+7,24,3,5,'#9fa68a');
    r(x-12,y+3,25,20,iron);ellipse(x,y+4,12,4,'#a1a48a');r(x-9,y+7,20,13,'#8c9d85');
    r(x-6,y+7,2,13,'#b3b49b');r(x+6,y+7,3,14,'#687c70');ellipse(x,y+20,10,3,'#6d8071');
    r(x-12,y+23,25,7,dark);r(x-7,y+24,14,3,'#9a583c');r(x-4,y+24,6,2,active?'#e39a57':'#916447');
    r(x-10,y+30,3,3,dark);r(x+7,y+30,3,3,dark);
    if(active){for(let s=0;s<3;s++){const rise=(phase/40+s*5)%18;const drift=Math.round(Math.sin((rise+s)*.6)*2);r(x+7+drift-s,26-rise,3+s,2,'#bdbda2');r(x+8+drift-s,25-rise,2,1,'#d3c9a9')}}
  }
  function can(x,y){
    r(x,y+1,6,7,dark);r(x+1,y+1,4,6,'#c9af80');r(x+1,y+3,4,2,'#a45c42');r(x+1,y,4,1,'#c3ccc0');r(x+2,y+2,1,4,'#ead8ad')
  }
  function rope(x,y){
    for(let i=0;i<4;i++){ellipse(x,y+i*2,12-i,3,'#6e5843');ellipse(x,y-1+i*2,11-i,2,'#c69b61');ellipse(x,y-1+i*2,6-i,1,wood)}
  }
  function worker(x,y,apron,lean,tool){
    const reach=active ? Math.round(2*Math.sin(phase*Math.PI/100)) : 0;
    r(x-4,y+12,4,12,iron);r(x+2,y+12,4,12,iron);r(x-6,y+23,6,2,dark);r(x+2,y+23,7,2,dark);
    r(x-5,y+1,12,12,'#b6bb9f');r(x-2,y,8,17,apron);r(x-5,y+14,11,3,apron);
    r(x+lean-3,y-10,8,9,'#bc9573');r(x+lean+4,y-6,2,3,'#d6ab7a');r(x+lean,y-8,4,6,'#dab88a');
    r(x+lean-4,y-12,8,4,'#435954');r(x+lean-5,y-9,12,2,'#596f62');dot(x+lean+3,y-5,dark);
    line(x+4,y+2,x+8,y+7,'#bcbba2');line(x+5,y+2,x+8,y+6,'#ccc8ac');
    line(x+8,y+7,x+14,y+4+reach,'#d4ac7b');line(x+9,y+8,x+14,y+5+reach,'#aa865f');
    if(tool){line(x+14,y+5+reach,x+18,y-2+reach,oak);r(x+15,y-4+reach,7,3,'#73847c')}
  }
  // The daylight opening and the heavy lintel tie both interiors to the same waterfront.
  r(0,0,W,H,kind?'#666f61':'#69796a');
  for(let x=0;x<W;x+=13){r(x,4,1,61,kind?'#4f5d53':'#53675b');r(x+2,7,1,54,kind?'#828677':'#809080')}
  r(0,59,W,5,'#49594d');r(0,64,W,32,kind?'#8d775b':'#90907a');
  for(let y=67;y<H;y+=5){r(0,y,W,1,kind?'#74694f':'#757b69');for(let x=(y*9)%40;x<W;x+=47)r(x,y-3,1,3,'#767966')}
  r(0,0,W,5,dark);r(0,5,W,2,'#706f5a');
  if(kind===0){
    windowBay(124,79);lamp(108,19);lamp(261,16);
    // Brick furnace beneath the steam retorts; through-pipe follows the ceiling.
    r(16,12,78,44,'#7e6954');
    for(let y=13;y<55;y+=5){r(16,y,78,1,'#a78b6c');for(let x=19+(y%2)*6;x<94;x+=13)r(x,y+1,1,4,'#a58a6b')}
    r(12,17,88,3,'#476258');r(13,17,85,1,'#a9a48a');r(13,19,3,24,'#4b655b');
    kettle(37,45);kettle(71,44);
    r(228,20,57,2,dark);r(229,21,55,2,wood);r(235,23,2,6,iron);r(277,23,2,6,iron);
    for(let j=0;j<6;j++)can(232+j*8,12);
    r(300,9,47,44,'#3f584e');for(let k=0;k<3;k++){r(302,19+k*12,43,2,oak);for(let j=0;j<5;j++)can(304+j*8,12+k*12)}
    worker(154,53,rust,1,false);worker(236,56,'#566d69',2,false);
    board(104,66,117,5,'#a79266');r(110,71,4,16,wood);r(210,71,4,16,wood);
    // Fresh crab at one end, a line of closed tins at the other.
    for(let j=0;j<3;j++){
      const x=115+j*8,y=63-j%2;ellipse(x,y,4,2,rust);r(x-6,y-2,2,2,'#d88c5e');r(x+5,y-3,2,3,'#ca7852');line(x-3,y+2,x-5,y+3,rust)
    }
    for(let j=0;j<4;j++)can(171+j*9,58);
    board(223,72,84,5,'#939682');r(229,77,4,12,wood);r(299,77,4,12,wood);
    const moved=active?Math.floor(phase/85)%9:0;for(let j=0;j<7;j++)can(226+j*10+moved,65);
    for(let j=0;j<3;j++)board(311+j*2,77-j*6,34,6,'#a68961');
  }else{
    windowBay(13,92);lamp(125,17);lamp(276,19);
    r(146,15,99,34,'#3f5551');r(148,17,95,30,'#6f7560');
    for(let i=0;i<7;i++){
      const x=154+i*13;r(x,18,2,4,iron);
      if(i%2){line(x,21,x+4,34,oak);r(x+1,32,8,3,'#adb0a0')}else{r(x-1,22,3,14,'#465e59');r(x-1,20,3,5,'#c69b67')}
    }
    // Iron stove, forge hood and an anvil with a tapered horn.
    r(295,7,11,22,iron);r(298,8,2,20,'#72837a');
    for(let y=0;y<12;y++)r(291-y,26+y,18+2*y,1,'#475c5a');
    r(278,39,47,4,dark);r(284,44,34,16,'#785e4c');r(288,47,25,10,dark);
    r(290,51,21,5,active?'#d37a43':'#955b42');r(295,50,8,5,active?'#f0bc6f':'#be8151');
    board(278,60,50,6,wood);r(282,66,4,15,dark);r(320,66,4,16,dark);
    worker(234,49,'#915b40',1,true);
    r(244,60,28,5,iron);for(let j=0;j<5;j++)r(242-j,60+j,6+j,1,'#798d82');r(254,65,7,5,iron);r(248,70,20,3,iron);
    board(252,73,14,12,wood);
    if(active&&phase%166<52){for(let j=0;j<4;j++){const s=Math.floor((phase%166)/8);dot(250+s+j*2,57-j-s,'#f3cc76')}}
    board(78,64,111,6,'#a48d66');r(85,70,5,18,wood);r(178,70,4,18,wood);
    for(let i=0;i<4;i++){r(92+i*9,61,7,2,'#73867b');r(95+i*9,58,2,5,'#b3b5a0')}
    rope(165,56);rope(35,80);rope(53,84);
    for(let i=0;i<4;i++){board(8+i*3,70-i*3,67,4,'#b08959');r(17+i*3,70-i*3,2,3,'#526458')}
    for(let i=0;i<3;i++){board(303+i*2,84-i*6,39,7,'#a68a60');r(312+i*2,84-i*6,2,6,'#526259')}
  }
  r(0,0,5,H,'#344b45');r(355,0,5,H,'#334941');
}
