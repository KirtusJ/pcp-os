// Bellweather Fish Exchange: one painted scene on a 360 × 96 pixel grid.
function paintExchange(canvas, tide = 0, stock = [], kind = 0) {
  const width = 360, height = 96;
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  const context = canvas.getContext('2d');
  const rect = (x, y, w, h, color) => {
    context.fillStyle = color;
    context.fillRect(x, y, w, h);
  };
  const line = (x, y, x2, y2, color) => {
    const dx = Math.abs(x2 - x), dy = -Math.abs(y2 - y);
    const stepX = x < x2 ? 1 : -1, stepY = y < y2 ? 1 : -1;
    let error = dx + dy;
    for (;;) {
      rect(x, y, 1, 1, color);
      if (x === x2 && y === y2) break;
      const twice = 2 * error;
      if (twice >= dy) { error += dy; x += stepX; }
      if (twice <= dx) { error += dx; y += stepY; }
    }
  };
  const polygon = (points, color) => {
    const ys = points.map(p => p[1]);
    for (let y = Math.max(0, Math.min(...ys)); y < Math.min(height, Math.max(...ys)); y++) {
      const cuts = [];
      let a = points[points.length - 1];
      for (const b of points) {
        const scan = y + 0.5;
        if ((a[1] > scan) !== (b[1] > scan)) {
          cuts.push(a[0] + (b[0] - a[0]) * (scan - a[1]) / (b[1] - a[1]));
        }
        a = b;
      }
      cuts.sort((left, right) => left - right);
      for (let i = 0; i + 1 < cuts.length; i += 2) {
        const left = Math.ceil(cuts[i] - 0.5), right = Math.ceil(cuts[i+1] - 0.5);
        rect(left, y, right - left, 1, color);
      }
    }
  };
  const ink = '#203b40', tar = '#2c4546', shade = '#3c504c', pine = '#695d4d';
  const amber = '#c7955e', cream = '#efcd95', gold = '#f3b968';
  const skin = '#d99a6e', rust = '#af5743', teal = '#597971';
  const phase = ((Number(tide) | 0) % 4 + 4) % 4;
  const inventory = i => Math.max(0, Number(stock && stock[i]) || 0);
  const sky = ['#e6bb95','#efd1a8','#f0c29a','#c5bcac'][phase];
  const water = ['#65938d','#79a59b','#88aba0','#749994'][phase];
  const light = ['#a08861','#b89968','#bea06d','#928466'][phase];
  const seaLevel = [48,45,42,45][phase];

  // Salt-grey boards below the sawtooth roof. A few warmer planks catch the lamps.
  rect(0,0,width,height,shade);
  for (let x = 0; x < width; x += 7) {
    const board = ['#53615a','#62675c','#535f56','#59645a','#4b5a55'][(x / 7) % 5];
    rect(x,8,6,59,board);
    rect(x,9,1,54,'#73796a');
    rect(x+6,9,1,56,'#394f4d');
    const y = 17 + (x * 3 % 36);
    rect(x+3,y,1,5,'#4d5a52');
    rect(x+2,58,1,1,ink);
  }
  rect(0,38,360,3,'#4b5148');
  rect(0,38,360,1,'#8c826a');
  rect(0,63,360,5,'#374b48');

  // Open saltwater loading bay. The coast, lamp glints and waterline move with the tide.
  rect(91,15,114,54,sky);
  rect(91,31,114,5,['#edc99a','#f5d5a0','#f2d7a3','#d9c7a6'][phase]);
  rect(91,36,114,17,'#e1c49d');
  const coast = [[91,39],[101,37],[109,38],[120,35],[128,37],[141,39],
    [153,38],[165,41],[176,39],[190,39],[205,36],[205,53],[91,53]];
  polygon(coast,'#b8af94');
  polygon([[91,42],[101,42],[111,39],[120,40],[134,44],[145,42],
    [158,44],[176,42],[191,42],[205,38],[205,52],[91,52]],'#969f8c');
  rect(91,seaLevel,114,69-seaLevel,water);
  for (let i = 0; i < 67; i++) {
    const x = 94 + (i*41 + phase*3) % 103;
    const y = seaLevel + 1 + (i*13 + i*i) % (67-seaLevel);
    rect(x,y,2 + i%5,1,i%4 ? '#639089' : '#d2c7a3');
  }
  // An early fishing smack, obscured in part by the edge of the open door.
  const boatY = seaLevel + 6;
  rect(147,boatY-16,1,16,'#5f6961');
  polygon([[145,boatY-14],[145,boatY-2],[137,boatY-2]], '#f4ddae');
  polygon([[149,boatY-11],[154,boatY-2],[149,boatY-2]], '#c69975');
  polygon([[135,boatY],[158,boatY-1],[155,boatY+2],[139,boatY+3]],'#56665c');
  rect(141,boatY+4,11,1,'#b5b499');
  line(147,boatY-15,158,boatY-1,'#acaa90');
  // Pilings and access stage frame the morning view.
  rect(180,47,23,2,'#82775e'); rect(181,49,21,2,'#687366');
  rect(184,49,2,12,'#778276'); rect(196,49,2,11,'#778276');
  line(185,57,196,50,'#8a8b75');
  line(181,60,187,60,'#a8ab91');
  line(168,33,170,32,'#7f8170'); line(171,33,173,31,'#7f8170');

  // Wharves are damp, even with both doors wide open. The reflection crosses
  // the direction of the floorboards; its length and angle change with the water.
  rect(0,67,360,29,'#6c6956');
  polygon([[92,68],[204,68],[239+phase*3,96],[58+phase*6,96]],light);
  for (const [y, dark] of [[69,'#606354'],[74,'#656250'],[82,'#625f4e'],[92,'#585a4d']]) {
    rect(0,y,360,1,dark);
    for (let x = (y*6)%47-15; x < width; x += 48) {
      rect(x,y+1,14,1,'#9a8968');
      rect(x+28,y-2,1,2,dark);
    }
  }
  polygon([[98,68],[105,68],[75+phase*6,96],[65+phase*6,96]],'#8b7e60');
  for (let i = 0; i < 15; i++) {
    const x = 96 + (i*29 + phase*3) % 107;
    const y = 70 + (i*7) % 24;
    rect(x,y,1 + (i*11)%9,1,i%3 === 0 ? '#cbbb8b' : '#a79d73');
  }
  // Sliding-door track, thick jambs, crossbraces, tie bolts and mortises.
  rect(82,11,7,57,ink); rect(83,13,2,53,'#84745b');
  rect(89,15,4,53,'#927b5c'); rect(92,17,1,50,'#e3b77e');
  rect(204,12,8,57,ink); rect(204,15,2,51,'#e7b980'); rect(209,14,2,53,pine);
  rect(80,10,134,5,ink); rect(83,14,128,1,'#b18a61');
  rect(90,11,115,1,'#8c8069');
  [86,109,140,173,208].forEach(x => rect(x,11,1,1,'#b4a686'));
  for (const x of [0,73,214,318,354]) {
    rect(x,9,6,58,tar); rect(x+1,9,2,56,'#7d725b');
    rect(x+4,12,1,52,'#4e5044');
    rect(x,64,7,4,shade); rect(x+2,61,1,1,cream);
  }
  rect(0,0,360,9,ink); rect(0,7,360,4,'#514f43'); rect(0,10,360,1,'#928169');
  for (const [x,span] of [[0,22],[76,18],[211,22],[300,20],[355,-21]]) {
    polygon([[x,8],[x+span,8],[x+(span>0?2:-2),24],[x,24]],'#414c46');
    line(x+(span>0?3:-3),22,x+span,9,'#9b8868');
    rect(x+(span>0?7:-7),12,2,1,'#b09a74');
  }
  // Block and tackle for the loading door.
  rect(124,8,6,5,tar); rect(125,9,3,3,'#a48d6a');
  rect(126,12,1,12,'#857960'); rect(128,12,1,10,'#a1916e');
  line(126,24,128,24,'#756f59'); rect(128,22,1,2,'#756f59');

  // Painted glass panes behind the counter.
  rect(235,16,51,24,tar); rect(237,18,47,20,'#97a398');
  for (let i = 0; i < 3; i++) {
    const x = 238+i*15;
    rect(x,19,13,7,'#b5ba9d');
    polygon([[x,26],[x+5,20],[x+11,20],[x+4,26]],'#ccd0ac');
    rect(x,29,13,7,'#849d8d');
    rect(x+1,33,10,2,'#9aaa96');
    rect(x+13,17,2,22,tar);
  }
  rect(237,27,47,2,shade); rect(234,39,53,2,'#a79877');

  const lamp = (x, y) => {
    rect(x,1,1,y-2,ink);
    rect(x-1,y-2,3,2,'#9c8a66');
    polygon([[x-3,y],[x+3,y],[x+8,y+4],[x-8,y+4]],ink);
    line(x-7,y+4,x+7,y+4,'#c69b62');
    rect(x-3,y+5,6,2,gold); rect(x-2,y+5,4,1,'#fff0bd');
    rect(x-1,y+7,2,1,'#eac785');
  };
  lamp(36,20); lamp(225,19); lamp(306,14);

  // Racks of shell hooks, coiled line, and the chalk tally board.
  rect(3,31,26,2,'#9b8668'); rect(3,33,25,1,ink);
  for (let i=0; i<4; i++) {
    const x = 5+i*6;
    rect(x,34,1,4+i%2,tar); rect(x-1,38+i%2,2,1,'#bcb397');
  }
  rect(39,30,28,15,ink); rect(41,31,25,13,'#547068'); rect(39,44,28,2,'#9a825e');
  for (const [x,y] of [[44,35],[47,35],[50,35],[57,39],[60,39]]) {
    line(x,y+2,x+1,y,'#c2c8ad');
  }
  rect(294,28,20,19,'#46554f'); rect(295,28,18,1,'#ae946e');
  for (let i=0; i<3; i++) {
    const x=298+i*6;
    rect(x,29,1,9+i*2,'#b79c71'); rect(x-1,37+i*2,3,1,'#ceb086');
    rect(x+1,35+i*2,1,2,'#ac9874');
  }

  // Landing net and its hand-bent frame hang beside the sash.
  line(333,29,344,14,'#b99c73');
  line(334,30,345,15,'#766d55');
  const hoop = [[327,24],[333,25],[336,28],[335,31],[330,34],[324,33],[321,29],[322,26],[327,24]];
  for (let i=1;i<hoop.length;i++) line(...hoop[i-1],...hoop[i],'#b1ab86');
  line(323,28,328,33,'#918f73'); line(325,26,332,32,'#989577');
  line(329,26,334,30,'#989577'); line(326,32,332,26,'#929178');
  line(322,30,327,25,'#929178'); line(331,33,335,29,'#929178');

  // Folded salt sacks, tied necks, side gussets and the sewn seam.
  const sack = (x, y, pale) => {
    const body = pale ? '#dfbe88' : '#bba47d', lit = pale ? '#f4d8a3' : '#d8c49a';
    polygon([[x+3,y-14],[x+7,y-16],[x+10,y-16],[x+14,y-13],[x+17,y-4],
      [x+16,y],[x+2,y+1],[x,y-2],[x+1,y-9]],'#68644f');
    polygon([[x+3,y-13],[x+7,y-15],[x+11,y-15],[x+14,y-11],[x+15,y-3],
      [x+13,y-1],[x+3,y],[x+1,y-3]],body);
    polygon([[x+3,y-10],[x+6,y-13],[x+7,y-6],[x+3,y-2]],lit);
    rect(x+5,y-18,6,2,body); rect(x+6,y-16,5,1,'#8c7152');
    rect(x+7,y-14,4,1,lit); line(x+13,y-10,x+14,y-3,'#9b835f');
    rect(x+3,y-2,8,1,lit); line(x+7,y-8,x+9,y-9,'#b1966d');
  };
  rect(1,62,79,6,tar); rect(2,61,76,2,amber); rect(5,68,4,6,pine);
  for (const [x,y] of [[5,61],[23,61],[42,61],[59,61]]) sack(x,y,false);
  if (inventory(0)>0) { sack(17,49,true); sack(38,49,false); }
  // A salt shuttleman leans on his gaff at the doorway.
  rect(104,65,20,2,'#787758');
  rect(116,35,1,33,'#887655'); line(116,35,119,32,'#7b7762');
  polygon([[103,47],[106,45],[111,46],[114,54],[109,56],[103,54]],shade);
  polygon([[103,52],[110,52],[110,58],[107,66],[105,66],[106,58],[103,65],[100,65]],tar);
  rect(105,40,4,5,'#b38764'); rect(103,39,7,2,ink); rect(104,38,4,2,teal);
  line(110,47,113,51,pine); rect(113,50,4,2,'#a69070'); rect(116,49,1,2,skin);
  rect(98,65,6,1,ink); rect(105,65,5,1,ink);

  // Fish exchange desk. A swinging beam scale and open ledger anchor the silhouette.
  const scale = (x,y) => {
    rect(x-4,y-1,9,2,tar); rect(x-1,y-17,2,17,'#a68b5a');
    rect(x-8,y-17,17,1,ink); rect(x-1,y-19,2,2,gold);
    for (const [pan,dy] of [[x-7,1],[x+7,0]]) {
      line(pan,y-16,pan-4,y-9+dy,'#c3b18c'); line(pan,y-16,pan+4,y-9+dy,'#998b6d');
      polygon([[pan-5,y-9+dy],[pan+5,y-9+dy],[pan+3,y-7+dy],[pan-2,y-7+dy]],'#ba9161');
    }
    rect(x-3,y-1,7,1,'#d0ae77');
  };
  // The clerk turns away from the window to weigh a delivery.
  polygon([[260,48],[264,45],[269,45],[274,49],[273,63],[259,63]],'#809084');
  polygon([[261,48],[264,51],[268,52],[270,47],[272,50],[272,64],[260,64]],rust);
  rect(264,43,5,4,skin); rect(263,38,7,6,pine); rect(263,38,7,2,'#b6a790');
  rect(262,40,5,3,'#dfa278'); rect(261,42,3,2,skin); rect(266,43,2,2,'#986d52');
  rect(263,37,6,2,'#666562'); rect(264,36,4,1,'#c8bdb0');
  rect(265,40,1,1,ink); rect(262,43,2,1,'#bb8265');
  polygon([[261,47],[263,49],[261,55],[252,55],[251,53],[258,52]],'#95a498');
  rect(250,53,4,2,skin); line(251,52,248,48,ink);
  line(270,54,267,57,'#ca7252'); rect(266,57,4,1,'#dfa57a');
  rect(218,61,67,11,tar); rect(220,64,64,8,'#846e54');
  for (let i=0; i<9; i++) rect(222+7*i,65,1,6,'#a08a67');
  rect(216,59,71,3,'#a98459'); rect(216,59,71,1,'#dfb881');
  rect(219,72,4,9,shade); rect(280,72,4,8,shade);
  scale(230,58);
  polygon([[243,57],[252,56],[256,58],[251,59],[242,59]],'#eed6a7');
  line(250,57,252,58,'#aa9272');
  rect(275,55,5,4,teal); rect(275,54,5,1,cream); rect(280,56,1,2,teal);

  // Oak timber with different bundles marked by one, two or three axe cuts.
  const timber = (x, y, w, n) => {
    rect(x,y,w,7,'#5e5848'); rect(x+1,y+1,w-5,4,'#b78558');
    rect(x+2,y+1,w-7,1,'#d5a372'); rect(x+w-5,y+1,5,5,'#d7ab78');
    rect(x+w-4,y+2,3,3,'#a57b54'); rect(x+w-3,y+3,1,1,'#e4b987');
    rect(x+Math.floor(w/3),y,2,6,'#6b6c5b');
    for (let i=0; i<n; i++) line(x+4+i*3,y+4,x+5+i*3,y+2,'#e7bc84');
    rect(x+w-14,y+4,6,1,'#926e50');
  };
  timber(301,63,40,1); timber(317,69,43,3); timber(305,57,45,2);
  timber(313,51,39,3);
  if (inventory(2)>0) { timber(321,45,39,2); timber(329,39,30,1); }
  line(320,52,320,66,tar); line(342,52,342,70,'#776e55');

  // A broad fish basket has individual split-cane stitches, a dark oval rim,
  // and a pair of blue crabs with contrasting claws.
  function crab(x, y, warm) {
    const shell = warm ? '#b86b50' : '#5a8984';
    rect(x+2,y-2,4,1,tar); rect(x+1,y-1,6,3,shell);
    rect(x+2,y-1,3,1,warm ? '#e49b6a' : '#a6b9a1');
    line(x+1,y+1,x-2,y-1,shell); line(x+6,y+1,x+9,y-2,shell);
    rect(x-3,y-3,2,2,shell); rect(x+9,y-4,2,2,shell);
    rect(x-3,y-4,1,1,cream); rect(x+10,y-5,1,1,cream);
    line(x+1,y+2,x-1,y+3,tar); line(x+6,y+2,x+8,y+3,tar);
  }
  const basket = (x, y, w, full) => {
    const h = Math.floor(w/2);
    polygon([[x,y-h],[x+2,y-h-2],[x+w-3,y-h-2],[x+w,y-h],[x+w-3,y-1],
      [x+w-6,y],[x+4,y],[x+2,y-2]],'#675e47');
    if (full) { crab(x+5,y-h-2,false); if (w>17) crab(x+w-11,y-h-2,true); }
    polygon([[x+1,y-h+1],[x+w-1,y-h+1],[x+w-4,y-2],[x+4,y-1]],'#aa8755');
    for (let row=1;row<h-2;row+=3) {
      const inset = 2+Math.floor(row/5);
      rect(x+inset,y-h+row,w-inset*2,1,'#d3af72');
      for (let k=inset+1+(row%2); k<w-inset; k+=4) rect(x+k,y-h+row,1,2,'#826b49');
    }
    line(x,y-h,x+w-1,y-h,'#d7b781'); rect(x+3,y-1,w-7,1,'#8a714d');
    line(x+2,y-h-2,x+w-4,y-h-2,'#bd9d6c');
  };
  basket(166,74,18,true); basket(190,80,19,true);
  if (inventory(1)>0) basket(181,68,15,true);

  // Near the threshold the objects are larger, and their cast shadows touch the viewer.
  polygon([[0,87],[17,83],[50,84],[69,88],[47,91],[0,92]],'#545b4e');
  sack(1,89,true); sack(18,87,true); sack(35,85,false);
  if (inventory(0)>2) sack(13,73,true);
  basket(144,88,24,true);
  timber(306,79,52,2); timber(312,86,55,3);
  if (inventory(2)>2) timber(316,73,42,1);
  // A small brass clip picks out the selected lot.
  if (kind===0) { rect(31,77,3,2,gold); rect(32,77,1,1,cream); }
  if (kind===1) { rect(162,82,3,2,gold); rect(163,82,1,1,cream); }
  if (kind===2) { rect(340,82,3,2,gold); rect(341,82,1,1,cream); }
}
