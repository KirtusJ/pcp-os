// Three linocut-sized views of the port after the late shift.
function drawManifestArt(canvas, chapter, lead = -1, ending = 0, phase = 0) {
  if (!canvas) return;
  const W = 320, H = 112;
  if (canvas.width !== W) canvas.width = W;
  if (canvas.height !== H) canvas.height = H;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const ink = '#16262c', pitch = '#101e25', dusk = '#273c44', steel = '#40585b';
  const halftone = '#586966', pale = '#d4caa7', cream = '#ffdda0', gold = '#ddaa65';
  const wood = '#85684f', oak = '#ae865f', rust = '#ad5d48', brick = '#614740';
  const frame = Math.floor(phase) % 3;
  function box(x, y, w, h, c) {
    ctx.fillStyle = c;
    ctx.fillRect(Math.round(x), Math.round(y), Math.round(w), Math.round(h));
  }
  function line(x, y, tx, ty, c) {
    const dx = Math.abs(tx - x), dy = -Math.abs(ty - y);
    const sx = x < tx ? 1 : -1, sy = y < ty ? 1 : -1;
    let err = dx + dy;
    while (true) {
      box(x, y, 1, 1, c);
      if (x === tx && y === ty) break;
      const e = 2 * err;
      if (e >= dy) { err += dy; x += sx; }
      if (e <= dx) { err += dx; y += sy; }
    }
  }
  function poly(points, c) {
    const ys = points.map(p => p[1]);
    for (let y = Math.max(0, Math.min(...ys)); y < Math.min(H, Math.max(...ys)); y++) {
      const cuts = [];
      let a = points[points.length-1];
      for (const b of points) {
        if ((a[1] > y + .5) !== (b[1] > y + .5))
          cuts.push(a[0]+(b[0]-a[0])*(y+.5-a[1])/(b[1]-a[1]));
        a = b;
      }
      cuts.sort((a, b) => a-b);
      for (let i=0; i+1<cuts.length; i+=2) {
        const left = Math.round(cuts[i]), right = Math.round(cuts[i+1]);
        box(left, y, right-left, 1, c);
      }
    }
  }
  function oval(x, y, rx, ry, c) {
    for (let j = -ry; j <= ry; j++) {
      const radius = Math.floor(rx*Math.sqrt(1-(j*j)/(ry*ry)));
      box(x-radius,y+j,2*radius+1,1,c);
    }
  }
  function glow(x, y, rx, ry, shades) {
    for (let i=0; i<shades.length; i++) {
      const scale = 1 - .74*i/(shades.length-1);
      oval(x,y,Math.round(rx*scale),Math.round(ry*scale),shades[i]);
    }
  }
  function skyline(x, y, scale) {
    for (let i = 0; i < 9; i++) {
      const w = (9+i%3*2)*scale, left = x+i*11*scale, top = y-(3+i*7%6)*scale;
      box(left, top, w, y-top+1, steel);
      poly([[left-2,top],[left+w/2,top-4],[left+w+1,top]], steel);
      box(left+2,top+1,1,2,i%3 ? '#768682' : gold);
    }
  }
  function lamp(x,y) {
    box(x-2,y-4,5,4,ink);box(x-4,y-1,9,2,'#947954');
    box(x-3,y+1,7,8,'#765f4b');box(x-2,y+1,5,7,gold);
    box(x-1,y+2,3,5,frame === 1 ? '#ffebad' : cream);
    box(x-4,y+8,9,2,ink);box(x-2,y+10,5,1,wood);
  }
  function planks(x,y,w,h) {
    box(x,y,w,h,'#675d50');
    for(let iy=y;iy<y+h;iy+=5) {
      box(x,iy,w,1,'#3e4844');
      for(let ix=x+((iy*11)%17);ix<x+w-2;ix+=26) {
        box(ix,iy+2,Math.min(x+w-ix,6+(ix+iy)%9),1,'#81705b');
        box(ix+1,iy+3,1,1,'#384441');
      }
    }
  }
  function brickwork(x,y,w,h) {
    box(x,y,w,h,'#574e48');
    for(let iy=y;iy<y+h;iy+=6) {
      for(let ix=x-9*((iy-y)/6%2);ix<x+w;ix+=18) {
        const left=Math.max(x,ix), right=Math.min(x+w,ix+17);
        if(right<=left)continue;
        const choices=['#69564b','#6f594d','#66524a','#60544c'];
        box(left,iy+1,right-left,4,choices[Math.abs(ix*3+iy)%4]);
      }
    }
  }
  function warehouseCrate(x,y,w,h) {
    box(x,y,w,h,ink); box(x+1,y+1,w-2,h-2,wood);
    for(let k=3;k<w-2;k+=5)box(x+k,y+1,1,h-2,'#605649');
    line(x+2,y+h-3,x+w-3,y+2,oak);line(x+2,y+h-2,x+w-3,y+3,'#98795a');
    box(x+1,y+1,w-2,2,oak);box(x+1,y+h-3,w-2,2,'#725e4c');
  }
  function person(x, y, coat, scarf, facing = 1) {
    const at = (dx,dy,w,h,c) => box(facing>0 ? x+dx : x-dx-w+1,y+dy,w,h,c);
    at(-4,11,4,12,ink);at(2,12,4,11,dusk);at(-5,22,6,2,pitch);at(2,22,7,2,pitch);
    at(-4,-2,9,6,coat);at(-5,0,12,12,coat);at(-6,12,14,3,coat);
    at(-5,0,2,10,'#435754');at(0,-1,5,2,scarf);at(2,1,2,5,scarf);
    at(-3,-9,7,8,'#ae8767');at(1,-8,3,6,'#dbb185');at(4,-6,2,2,'#dab38b');at(2,-6,1,1,ink);
    at(-4,-12,7,2,ink);at(-5,-10,9,3,'#354a4d');at(-5,-8,11,2,ink);
    at(5,5,4,6,coat);at(8,7,6,2,'#be9670');at(12,6,3,2,'#d6ac7f');
  }
  function mug(x,y,c) {
    box(x,y,4,4,c);box(x+4,y,2,1,c);box(x+5,y+1,1,2,c);box(x+4,y+3,1,1,c);
    box(x,y,4,1,cream);
  }
  function sheet(x,y,w,h,c) {
    poly([[x,y+1],[x+w-2,y],[x+w,y+h-1],[x+2,y+h]],c);
    for(let j=2;j<h-1;j+=3)box(x+3,y+j,Math.max(1,w-6-(j%4)),1,'#8c8a72');
  }
  function tin(x,y) {
    box(x,y,5,8,steel);box(x+1,y+1,3,6,'#a69472');box(x+1,y+3,3,2,rust);
    box(x+1,y+1,1,4,'#ddc597');box(x,y,5,1,'#c7c7a9');
  }
  function tide() {
    // A cold moon and sparse amber windows put a human scale on the horizon.
    box(0,0,W,H,'#263c49');box(0,28,W,19,'#344b56');box(0,44,W,18,'#465e63');
    for(let i=0;i<38;i++) {
      const x=4+i*37%300,y=2+i*13%33;
      box(x,y,i%11===0?2:1,1,i%3?'#667c7c':'#a0a999');
    }
    glow(51,22,21,13,['#2d4350','#374e58','#4a6268']);
    oval(51,22,9,9,'#c4cbb4');oval(53,20,6,6,'#cad1bb');
    box(43,19,8,1,'#b8c4b2');box(54,24,3,1,'#acbbae');
    for(let i=0;i<10;i++) {
      const x=i*33-11,y=16+(i*13)%20;
      box(x,y,18+i%8,1,'#566a70');box(x+7,y-1,23,1,'#4a606a');
    }
    poly([[0,53],[12,50],[23,51],[40,44],[64,51],[83,46],[100,49],[118,44],[141,49],[152,46],[168,52],[185,49],[201,52],[220,47],[241,48],[282,44],[320,49],[320,63],[0,63]],'#52696a');
    skyline(4,61,1);skyline(131,60,1);
    box(0,61,320,51,'#334d55');box(0,61,320,2,'#758780');
    for(let i=0;i<146;i++){
      const x=(i*71+4*(i%3)+frame*(i%2))%316, y=63+(i*17%47), w=2+i*i%11;
      box(x,y,w,1,['#456469','#567474','#6c8580','#2e464e'][i%4]);
    }
    for(let i=0;i<22;i++){
      const y=63+i*2, offset=Math.round(Math.sin(i*2+frame)*i*.65);
      box(52+offset-i/4,y,3+i%7,1,i%4?'#8d9d91':'#b8baa0');
    }
    // Far-channel lightship and lighthouse.
    poly([[18,58],[38,58],[35,61],[22,61]],dusk);box(26,53,6,4,steel);line(29,47,29,58,ink);
    box(31,54,2,1,gold);box(82,42,4,15,'#879389');box(82,48,4,3,'#625f58');
    box(81,40,6,3,steel);box(82,40,4,2,cream);poly([[80,39],[84,36],[87,39]],dusk);
    // Diagonals from the jetty lead directly to the pool of lamplight.
    poly([[0,94],[160,70],[320,68],[320,112],[0,112]],'#5e5b53');
    for(let i=0;i<15;i++) {
      const near=-100+i*33;
      line(near,111,Math.min(370,near+77),74,'#494d48');
      line(near+1,111,Math.min(371,near+78),74,'#756b58');
    }
    poly([[0,91],[156,68],[169,69],[0,97]],'#ac8b65');
    line(0,96,158,72,ink);
    for(const [x,y] of [[7,91],[43,85],[83,79],[116,75],[145,71]]){
      box(x,y-10,3,13,'#3c4746');box(x,y-11,3,2,'#9d9376');
    }
    line(8,83,44,78,'#657573');line(44,78,84,72,'#657573');line(84,72,117,68,'#657573');
    // The hut has a wedged-open sash and two panes of amber light.
    box(176,12,125,66,'#614e47');
    for(let i=0;i<14;i++) {
      const x=178+i*8;box(x,13,7,62,['#665448','#71614f','#5b5047'][i%3]);
      box(x+6,16,1,59,'#474943');
      box(x+2,20+i*7%19,1,10,'#786650');
    }
    poly([[157,27],[189,3],[281,3],[309,19],[306,24],[199,17],[173,33]],ink);
    poly([[159,26],[188,5],[280,5],[305,19],[195,13],[170,30]],'#475654');
    for(let i=0;i<12;i++)line(173+i*10,22-i%7,185+i*9,8,'#52635d');
    line(171,30,197,14,'#a08b6c');line(197,14,307,21,'#7e7563');
    box(193,25,69,40,ink);box(195,27,65,36,oak);box(197,29,61,32,'#c08f59');
    box(198,30,13,30,'#745c4d');box(248,30,9,30,'#9c744e');
    glow(225,43,24,14,['#c7965f','#d8a565','#eab674']);
    for(let k=0;k<3;k++) {
      box(202,34+k*7,13,2,'#735e4b');box(203+k%2,29+k*7,3,5,'#a58159');
      box(207,30+k*7,3,4,'#947151');
    }
    // A clerk in profile on the far side of the pane.
    poly([[226,56],[227,48],[230,44],[235,44],[240,48],[243,57]],'#5b5748');
    box(230,38,6,8,'#ac7956');box(233,38,4,5,'#d4a575');box(236,41,2,2,'#cd9e6d');
    box(230,36,6,3,'#4b4c43');box(229,38,4,2,'#595448');
    line(237,49,242,54,'#746251');line(242,54,248,52,'#bb8e61');
    box(196,54,62,3,'#775c48');box(196,60,63,2,wood);box(218,28,2,34,'#776246');
    poly([[192,63],[261,60],[267,63],[195,67]],oak);
    sheet(235,60,13,5,cream);box(243,62,3,1,rust);
    box(269,28,21,47,ink);box(272,31,15,43,'#655a48');
    box(274,34,11,11,'#354c4c');line(275,42,283,35,'#58706b');box(282,55,2,2,gold);
    box(182,23,2,9,ink);line(182,24,174,31,ink);line(174,31,174,35,ink);lamp(174,37);
    // Footprints and a little light on wet boards below the sill.
    for(let i=0;i<18;i++){
      const x=163+i*13%33, y=73+i*7%10;
      box(x,y,2+i%5,1,i%3?'#a18b6b':'#c2a27b');
    }
    for(let i=0;i<35;i++){
      const x=15+i*37%295, y=86+i*19%22;
      box(x,y,2+i%8,1,i%3?'#756c5c':'#918069');
    }
    box(172,76,4,7,'#5c5c50');box(168,80,12,1,'#cec09a');
    person(153,62,'#465550','#ac7051');sheet(165,67,8,4,pale);
    warehouseCrate(269,73,25,14);warehouseCrate(286,79,27,15);
    // The foreground mooring and cable frame the watch hut without hiding the clerk.
    box(10,80,7,32,pitch);box(11,80,2,30,'#344647');oval(13,80,5,2,'#53635e');
    line(0,94,12,87,pitch);line(15,87,40,93,pitch);line(40,93,67,87,pitch);
    box(66,81,5,15,ink);oval(68,81,3,1,'#69786a');
    poly([[0,0],[11,0],[11,3],[7,3],[7,34],[4,34],[4,62],[0,62]],pitch);
    line(8,1,102,8,ink);line(102,8,164,1,ink);
  }
  function kitchen() {
    brickwork(0,0,W,84);
    poly([[0,0],[74,0],[40,15],[21,32],[0,34]],dusk);
    // A low basement window admits a few cool strips of street light.
    box(25,16,65,27,ink);box(28,18,59,22,'#819b93');
    for(let i=0;i<5;i++){
      const x=30+i*11, y=36-i%3*2;
      poly([[x,y],[x+4,y-3],[x+9,y]],steel);box(x,y,9,41-y,steel);
      box(x+2,y+1,2,2,i%3?'#99a397':gold);
    }
    box(22,40,70,5,'#4e5147');
    for(let i=0;i<5;i++)box(30+i*13,17,2,24,dusk);
    box(26,28,61,2,halftone);box(22,42,70,3,'#a39273');
    poly([[29,45],[87,45],[142,84],[76,84]],'#726a59');
    for(let i=0;i<5;i++)line(30+i*13,45,79+i*13,81,'#8e8066');
    // Pans and saucepans hung by their handles.
    box(116,18,62,3,'#443e37');box(117,18,59,1,'#a29070');
    for(let i=0;i<4;i++) {
      const x=122+i*15, dy=i%2*4;
      box(x,20,1,8+dy,'#b4a281');oval(x,30+dy,4,5,'#353c39');
      box(x-2,28+dy,1,4,'#787866');
    }
    box(201,19,79,3,'#3d4039');box(201,18,80,1,oak);
    for(let i=0;i<9;i++)tin(204+i*8,10);
    box(202,40,76,3,'#594a3c');box(203,39,75,1,oak);
    for(let i=0;i<6;i++) { const x=207+i*11; oval(x,36,4,2,pale);box(x-4,35,9,2,'#a2a087'); }
    box(0,82,W,30,'#5a594d');
    for(let i=0;i<13;i++)line(-28+i*32,111,22+i*24,82,'#444d44');
    for(const y of [86,94,105])box(0,y,320,1,'#41493f');
    // Stove door, kettle, ladle and the red enamel lid hanging from a brick jamb.
    box(53,0,13,36,ink);box(56,0,7,35,'#525b53');box(61,2,2,32,'#77796a');
    poly([[54,33],[63,33],[72,47],[66,50],[53,39]],'#474e46');
    box(41,61,73,24,ink);box(44,82,5,9,ink);box(105,82,5,9,ink);
    box(42,63,70,18,'#9a5345');box(44,64,67,3,'#bd6d50');box(45,69,41,11,'#864c42');
    box(50,69,28,10,'#392f2e');box(53,71,21,6,'#bc6845');
    poly([[54,77],[56,72],[59,73],[61,69],[64,74],[66,71],[71,77]],'#eea655');
    for(let i=0;i<4;i++)box(54+i*5,70,2,8,'#57453b');
    box(90,69,17,9,'#69493d');box(92,70,12,1,gold);box(102,74,2,2,pale);
    box(39,59,77,4,ink);box(40,59,74,1,'#a99c77');
    box(52,51,29,8,'#536762');oval(66,51,14,3,'#909a85');box(52,51,3,7,'#afaa8b');
    oval(66,49,16,2,'#6c7d71');box(64,46,5,3,ink);
    box(83,47,20,11,'#8e4b40');oval(92,47,11,3,rust);box(83,49,2,6,'#d98661');
    oval(93,46,8,2,'#c77654');box(91,43,4,3,ink);line(100,46,106,37,oak);
    for(let i=0;i<3;i++) {
      const x=55+i*17, y=37+(i+frame)%3;
      line(x,y+8,x+3,y+4,'#92917b');line(x+3,y+4,x+1,y+1,'#92917b');
    }
    oval(177,43,8,8,'#403a35');oval(177,42,7,7,rust);oval(176,41,5,5,'#c17c58');
    box(175,41,4,2,'#624436');
    person(143,57,'#c1b898','#b65e48',-1);
    // Head cloth and apron strings.
    poly([[138,44],[144,43],[148,47],[144,48],[138,47]],'#c5b699');
    box(142,46,4,1,'#d5c8ab');line(145,60,147,67,'#b49474');
    box(132,65,2,6,'#927350');oval(133,71,3,2,'#a7aa91');
    person(226,59,'#45605c','#a36e51',-1);
    box(122,76,150,5,'#443f37');box(123,75,148,2,'#b18c66');
    for(const x of [127,195,261]){box(x,81,4,19,'#575248');line(x,92,x+10,81,'#735e4c');}
    for(let i=0;i<6;i++){
      const x=148+i*10,y=72+i%2;
      oval(x,y,4,2,'#d8cab0');box(x-4,y-2,9,2,'#8e9b8c');box(x-3,y-2,6,1,'#c09a62');
    }
    sheet(236,72,11,4,pale);mug(252,71,'#b8bc9e');
    // In the doorway, empty chairs and a coat hung up for the next shift.
    box(287,3,29,80,dusk);box(290,6,22,73,'#374c4b');box(288,82,31,3,wood);
    line(297,18,302,18,oak);poly([[297,19],[303,19],[308,36],[305,37],[305,49],[294,49],[293,34],[290,35]],'#75614d');
    for(let i=0;i<2;i++){box(277+i*14,69,2,21,ink);box(277+i*14,69,11,2,wood);box(277+i*14,80,12,3,wood);}
    // Tin bins and a folded dish cloth in the near corner.
    for(let i=0;i<2;i++){
      const x=4+i*12,y=81+i*3;
      box(x,y,10,13,ink);box(x+1,y+1,8,11,'#68756a');box(x+2,y+2,1,9,'#9aa08a');
      oval(x+5,y,5,2,'#8a9583');oval(x+5,y,3,1,dusk);
    }
    poly([[124,75],[136,75],[140,76],[136,77],[134,84],[126,83]],'#c3b79a');
    line(126,76,125,82,'#566c66');line(129,77,128,83,'#738780');
    box(0,0,320,4,'#333d37');box(0,4,320,2,'#83705a');
  }
  function press() {
    box(0,0,W,H,'#4f5b53');
    for(let i=0;i<35;i++) {
      const x=i*10;box(x,2,9,78,['#4d5950','#556056','#505c52'][i%3]);
      box(x+8,4,1,71,'#414f47');
    }
    // On the rack, narrow columns of newly set type.
    box(10,14,54,48,ink);box(12,16,50,44,'#8e7959');
    for(let row=0;row<5;row++)for(let col=0;col<9;col++) {
      const x=14+col*5,y=18+row*8;
      box(x,y,4,6,'#424b45');box(x+1,y+2,1,3,'#888b76');box(x+2,y+1+col%2,1,2,'#a09e82');
    }
    // The far window has a railway clock and a rain-streaked waterfront.
    box(164,10,75,43,ink);box(168,13,67,36,'#50696b');box(168,31,67,14,'#6c8380');
    for(let i=0;i<6;i++){const x=170+i*11;box(x,36-i%3*2,9,10,dusk);box(x+2,39,2,2,gold);}
    box(167,45,68,4,'#617776');box(169,47,62,1,'#a4a898');
    for(const x of [187,213])box(x,12,2,39,'#827b63');
    box(166,30,70,2,'#7a7561');box(161,51,81,4,wood);
    oval(278,22,10,10,ink);oval(278,22,8,8,'#d2c3a0');
    for(const [x,y] of [[278,15],[278,29],[271,22],[285,22]])box(x,y,1,1,brick);
    line(278,22,274,19,brick);line(278,22,281,17,dusk);
    // Telegrams drying overhead. Their curled edges catch the lamp.
    line(242,5,318,14,ink);
    for(let i=0;i<4;i++){
      const x=248+i*16,y=7+i*2;
      poly([[x,y],[x+11,y+1],[x+9,y+10],[x-2,y+8]],['#b8b499','#c5bea1'][i%2]);
      box(x+2,y-1,2,4,wood);
    }
    planks(0,80,320,32);
    // Cast-iron platen press. The big spoked flywheel is seen from the side.
    poly([[72,90],[79,78],[86,55],[91,55],[91,83],[110,83],[119,56],[125,56],[122,81],[131,90]],ink);
    box(70,89,63,3,'#444f49');
    poly([[80,64],[93,33],[118,32],[126,40],[119,43],[111,39],[98,40],[90,66]],ink);
    line(84,60,94,36,'#81887a');line(95,35,115,35,'#92947e');
    oval(85,31,6,6,ink);oval(85,31,3,3,halftone);box(88,30,19,3,dusk);
    poly([[99,50],[123,45],[131,54],[108,60]],'#928b73');poly([[99,49],[121,45],[124,47],[102,52]],pale);
    poly([[91,63],[123,57],[132,59],[124,63],[94,68]],'#56665d');
    line(95,65,126,59,'#a6a389');
    oval(76,66,16,17,ink);oval(76,66,13,14,'#56645b');oval(76,66,11,12,'#455249');
    for(const [x,y] of [[76,52],[88,58],[89,72],[76,80],[64,73],[63,60]]) {
      line(76,66,x,y,ink);line(77,66,x+1,y,'#84907e');
    }
    oval(76,66,3,3,ink);box(75,64,2,2,'#b1a686');line(77,66,88,70,ink);box(86,68,4,3,wood);
    person(149,56,'#738276','#a7654e',-1);
    box(145,45,6,2,'#ae9e7a');line(134,62,129,59,pale);
    // Gas light shades one proofreader's bench.
    line(207,0,207,14,'#273a36');poly([[204,14],[210,14],[218,20],[195,20]],'#2b4744');
    box(196,20,21,1,gold);box(203,21,7,2,cream);
    box(169,69,87,4,oak);box(167,72,90,4,'#51463a');box(173,76,5,19,wood);box(246,76,5,18,wood);
    sheet(174,62,20,7,pale);sheet(191,64,18,6,'#dac89e');mug(243,63,'#a3ad96');
    // The sounder: two coils, brass bridge, and key lever in profile.
    box(212,66,24,2,'#654d3c');box(214,59,4,7,ink);box(221,59,4,7,ink);
    for(let i=0;i<3;i++){box(214,60+i*2,4,1,'#ad784e');box(221,60+i*2,4,1,'#b58759');}
    box(213,57,14,2,oak);box(227,56,2,10,ink);line(229,63,236,61,gold);box(235,60,4,2,ink);
    line(236,67,241,70,ink);line(241,70,253,66,ink);line(253,66,261,54,ink);line(261,54,319,52,ink);
    box(260,57,48,23,ink);box(262,59,44,19,wood);
    for(let i=0;i<3;i++){box(263,60+i*6,42,4,'#9a805e');box(280,61+i*6,7,1,ink);}
    sheet(265,54,25,5,'#aeb098');
    // A wire hamper under the press holds misprinted sheets.
    poly([[18,83],[46,83],[41,98],[24,98]],dusk);
    for (let i=0; i<4; i++)sheet(19+i*3,82-i%3,15,4,'#afa887');
    for(let i=0;i<5;i++)line(20+i*5,85,24+i*4,97,'#788475');
    box(18,84,28,2,'#808675');
    box(0,0,320,4,ink);box(0,4,320,1,'#91836a');
  }
  function closeup(caseIndex, item) {
    // A small evidence mount leaves the whole scene readable behind it.
    const x=243,y=61,w=70,h=47;
    box(x+2,y+3,w,h,ink);box(x,y,w,h,'#c3b18d');box(x+1,y+1,w-2,h-2,'#e2d2ae');
    box(x+3,y+3,w-6,h-6,wood);box(x+4,y+4,w-8,h-8,'#52635b');
    for(let i=0;i<8;i++)line(x+5+i*7,y+40,x+12+i*7,y+34,'#5c6a5e');
    // Silver split pin and scuffed paper corners.
    box(x+2,y+2,4,1,'#f0dfb8');box(x+w-5,y+h-3,4,1,'#baab8d');
    const left=x+13,top=y+8;
    const docket=(px,py,pw,ph) => {
      sheet(px+1,py+2,pw,ph,'#394943');sheet(px,py,pw,ph,'#e3cb9e');
      box(px+3,py+2,pw-8,1,rust);
    };
    if(caseIndex===0) {
      if(item===0){
        docket(left+5,top+1,33,30);oval(left+19,top+3,16,3,'#b99b74');
        oval(left+20,top+2,16,2,'#dfc59c');box(left+33,top+1,2,2,wood);
        for(let i=0;i<6;i++)box(left+27,top+7+i*3,4,1,'#89735e');
        box(left+12,top+9,1,17,'#ac8066');
      } else if(item===1){
        docket(left+15,top+1,29,30);lamp(left+11,top+14);
        oval(left+35,top+24,3,3,rust);box(left+34,top+24,2,1,'#d6ae81');
      } else if(item===2){
        // An inverted carbon impression alongside its smudged original.
        poly([[left,top+7],[left+25,top+1],[left+31,top+23],[left+5,top+29]],'#243a43');
        poly([[left+14,top+5],[left+40,top+3],[left+44,top+26],[left+17,top+29]],'#d6c6a4');
        for(let i=0;i<5;i++){
          box(left+19+(i%2),top+8+i*3,17-i%3*3,1,'#777c70');
          box(left+6+i%2,top+9+i*3,5+i%3,1,'#63777a');
        }
        box(left+34,top+17,2,6,'#61706c');box(left+31,top+17,4,1,'#61706c');
        box(left+30,top+23,8,1,'#61706c');
      } else {
        docket(left+1,top+5,31,23);
        oval(left+35,top+17,8,8,'#2f413e');oval(left+35,top+17,6,6,'#c5b791');
        box(left+34,top+6,3,3,gold);line(left+35,top+17,left+35,top+12,ink);
        line(left+35,top+17,left+38,top+19,ink);box(left+7,top+23,8,1,'#67736a');
      }
    } else if(caseIndex===1){
      if(item===0){
        oval(left+22,top+22,20,7,ink);oval(left+21,top+19,20,8,'#874c3f');
        oval(left+21,top+17,19,8,'#c47353');oval(left+21,top+16,16,6,rust);
        box(left+15,top+10,11,4,'#4e4237');box(left+17,top+10,7,1,oak);
        line(left+5,top+16,left+11,top+13,'#f4bf87');
      }else if(item===1){
        for(let j=0;j<2;j++){
          const ox=left+2+j*6, oy=top+5+j*8;
          box(ox+1,oy+1,31,13,wood);box(ox,oy,31,13,j?'#c5a278':'#e6c7a0');
          box(ox+3,oy+2,24,1,rust);box(ox+4,oy+6,11,1,halftone);
          for(const dx of [21,26]){oval(ox+dx,oy+8,2,2,'#596658');box(ox+dx,oy+7,1,1,ink);}
        }
      }else if(item===2){
        poly([[left+1,top+6],[left+19,top+3],[left+24,top+5],[left+40,top+3],[left+43,top+27],[left+24,top+29],[left+4,top+30]],'#6d5141');
        sheet(left+2,top+4,18,23,'#cdb998');sheet(left+22,top+4,18,22,'#dfca9f');
        line(left+20,top+4,left+23,top+27,wood);box(left+8,top+8,8,1,rust);
      }else{
        docket(left+3,top+2,31,28);
        for(let i=0;i<4;i++) {box(left+9+i*2,top+9,1,7,'#65685b');}
        line(left+7,top+14,left+16,top+9,'#65685b');
        box(left+24,top+15,15,10,'#c6a77e');box(left+28,top+18,5,1,rust);
        box(left+27,top+20,2,2,'#52635b');box(left+34,top+20,2,2,'#52635b');
      }
    }else{
      if(item===0){
        for(let i=0;i<3;i++){
          const ox=left+[2,8,0][i], oy=top+3+i*9;
          sheet(ox,oy,36,6,'#e2cda5');
          for(let j=0;j<7;j++){box(ox+3+j*4,oy+2,1+j%3,1,ink);}
        }
      }else if(item===1){
        docket(left+6,top+1,35,29);box(left+10,top+5,25,4,ink);
        for(let j=0;j<3;j++)box(left+10+j*8,top+13,1,8,'#948a70');
        line(left+12,top+26,left+27,top+24,rust);line(left+27,top+24,left+32,top+19,rust);
      }else if(item===2){
        docket(left+3,top+3,37,25);
        for(let i=0;i<3;i++) {
          box(left+8,top+9+i*5,3,2,ink);box(left+15,top+9+i*5,7-i*2,2,ink);
          box(left+25,top+9+i*5,3+i,2,i===2?rust:ink);
        }
      }else{
        docket(left+4,top+2,35,29);line(left+28,top+5,left+29,top+28,'#aa806a');
        for(let i=0;i<5;i++)box(left+31,top+7+i*3,4,1,'#686b5e');
        line(left+10,top+23,left+13,top+18,rust);line(left+13,top+18,left+13,top+23,rust);
        line(left+13,top+23,left+22,top+21,rust);
      }
    }
  }
  if(chapter===1)kitchen();
  else if(chapter===2)press();
  else tide();
  if(ending===1 && chapter===2) {
    for(let i=0;i<3;i++)sheet(174+i*5,65-i,17,5,'#e5cda1');
  }
  if(ending===2 && chapter===1) {
    for(let i=0;i<4;i++)mug(149+i*11,81,'#c9c7a7');
  }
  if(Number.isInteger(lead) && lead>=0 && lead<4)closeup(chapter,lead);
}
