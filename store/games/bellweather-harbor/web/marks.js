// Harbor sign painter. All coordinates address individual pixels on a 40 × 32 tile.
function paintMark(canvas, kind) {
  const width = 40, height = 32;
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, width, height);

  const ink = '#173646', shade = '#324850', steel = '#648993';
  const foam = '#fff1cc', cream = '#e5cf9c', wood = '#b36e4d', oak = '#87513f';
  const coral = '#e76d50', rose = '#fa9a6c', jade = '#41968c', mint = '#8ac2a5';
  const blue = '#44839c', brass = '#d59c42', gold = '#f6cd67';
  const pixel = (color, x, y, w = 1, h = 1) => {
    ctx.fillStyle = color;
    ctx.fillRect(x, y, w, h);
  };
  const line = (color, x, y, toX, toY) => {
    const dx = Math.abs(toX - x), dy = -Math.abs(toY - y);
    const sx = x < toX ? 1 : -1, sy = y < toY ? 1 : -1;
    let error = dx + dy;
    for (;;) {
      pixel(color, x, y);
      if (x === toX && y === toY) return;
      const twice = error * 2;
      if (twice >= dy) { error += dy; x += sx; }
      if (twice <= dx) { error += dx; y += sy; }
    }
  };
  // Fill at pixel centers so diagonal hulls are crisp at every display scale.
  const polygon = (color, points) => {
    const ys = points.map(point => point[1]);
    const firstY = Math.min.apply(null, ys), lastY = Math.max.apply(null, ys);
    for (let y = firstY; y < lastY; y++) {
      const intersections = [];
      for (let j = 0; j < points.length; j++) {
        const a = points[j], b = points[(j + 1) % points.length];
        if ((a[1] > y + 0.5) !== (b[1] > y + 0.5)) {
          intersections.push(a[0] + (y + 0.5 - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
        }
      }
      intersections.sort((a, b) => a - b);
      for (let i = 0; i < intersections.length; i += 2) {
        const left = Math.ceil(intersections[i] - 0.5);
        const right = Math.ceil(intersections[i + 1] - 0.5);
        if (right > left) pixel(color, left, y, right - left);
      }
    }
  };
  const ripples = () => {
    pixel(blue, 1, 25, 6); pixel(jade, 5, 27, 9);
    pixel(steel, 17, 28, 11); pixel(mint, 27, 26, 9); pixel(blue, 35, 25, 4);
  };

  switch (kind) {
    case 0: { // Clinker-built rowboat, with one oarsman in a red watch cap.
      ripples();
      polygon(ink, [[2,15],[10,18],[29,18],[38,13],[36,20],[29,25],[9,25],[4,21]]);
      polygon(wood, [[4,17],[10,19],[28,19],[36,16],[34,20],[27,23],[10,23],[6,20]]);
      pixel(cream, 10, 19, 18); line(cream, 28, 18, 34, 16);
      pixel(oak, 11, 22, 16); pixel(rose, 8, 21, 4);
      polygon(ink, [[14,11],[19,11],[21,14],[24,15],[24,18],[19,18],[17,16],[13,17],[12,15]]);
      pixel(brass, 14, 12, 4, 5); pixel(gold, 15, 12, 3, 2);
      pixel(ink, 15, 6, 5, 5); pixel(cream, 16, 8, 4, 3);
      pixel(ink, 18, 8); pixel(coral, 15, 6, 4, 2); pixel(coral, 14, 7, 2);
      line(ink, 12, 10, 30, 24); line(cream, 13, 10, 31, 24);
      polygon(ink, [[25,18],[28,19],[35,25],[35,27],[32,27],[25,21]]);
      line(brass, 27, 20, 33, 25); line(cream, 28, 20, 34, 25);
      pixel(rose, 19, 14, 3, 2);
      break;
    }
    case 1: { // A slatted wire pot beside its painted marker buoy.
      ripples();
      line(oak, 4, 23, 7, 24); line(oak, 7, 24, 12, 22);
      line(cream, 9, 22, 12, 18); line(wood, 11, 21, 16, 16);
      pixel(ink, 7, 4, 3, 14);
      pixel(coral, 7, 5, 2, 5); pixel(foam, 7, 8, 2, 2);
      polygon(ink, [[7,11],[10,11],[13,16],[12,20],[9,22],[5,21],[4,18],[5,14]]);
      pixel(cream, 6, 14, 5, 5); pixel(coral, 5, 17, 7, 3); pixel(foam, 6, 15, 2, 2);
      polygon(ink, [[12,15],[18,9],[31,9],[37,14],[36,24],[12,24],[10,20]]);
      polygon(wood, [[13,15],[19,11],[30,11],[34,14],[29,16],[13,16]]);
      polygon(jade, [[13,17],[29,17],[34,15],[34,22],[13,22]]);
      line(brass, 13, 15, 29, 15); line(cream, 17, 13, 32, 13);
      line(cream, 20, 11, 14, 15); line(oak, 25, 11, 19, 15);
      line(cream, 29, 11, 23, 15); line(ink, 30, 16, 30, 22);
      for (let x = 14; x <= 26; x += 4) {
        line(shade, x, 17, x + 3, 21); line(mint, x, 21, x + 3, 17);
      }
      pixel(wood, 13, 21, 16); pixel(cream, 13, 16, 1, 6);
      line(brass, 30, 21, 34, 20); pixel(brass, 34, 15, 1, 7);
      break;
    }
    case 2: { // Short jade-hulled harbor ferry with an open afterdeck.
      ripples();
      pixel(ink, 9, 13, 2, 6); pixel(ink, 13, 13, 1, 6);
      pixel(cream, 3, 15, 10); pixel(ink, 4, 16, 9); pixel(ink, 4, 17, 1, 2);
      polygon(ink, [[12,10],[28,10],[32,15],[32,19],[11,19]]);
      pixel(foam, 13, 12, 15, 6); polygon(cream, [[28,12],[31,15],[31,18],[28,18]]);
      pixel(blue, 14, 13, 3, 3); pixel(jade, 18, 13, 3, 3);
      pixel(blue, 22, 13, 3, 3); pixel(ink, 26, 13, 3, 3);
      pixel(ink, 10, 10, 21, 2); pixel(coral, 11, 10, 18);
      pixel(ink, 18, 6, 5, 4); pixel(wood, 19, 7, 3, 2);
      pixel(ink, 26, 4, 1, 6); pixel(coral, 27, 5, 5); pixel(rose, 27, 6, 3);
      polygon(ink, [[2,17],[32,17],[38,15],[36,21],[31,25],[8,25],[4,22]]);
      polygon(jade, [[4,19],[32,19],[36,17],[34,21],[30,23],[8,23]]);
      pixel(mint, 8, 22, 20); pixel(foam, 5, 18, 25);
      pixel(ink, 10, 19, 3, 4); pixel(shade, 11, 20);
      pixel(ink, 25, 19, 3, 4); pixel(shade, 26, 20);
      pixel(coral, 18, 17, 4, 4); pixel(foam, 19, 17, 2); pixel(foam, 19, 20, 2);
      pixel(ink, 19, 18, 2, 2);
      break;
    }
    case 3: { // Dockside gantry lifting a hull clear of its keel blocks.
      ripples();
      pixel(ink, 3, 3, 26, 3); pixel(brass, 5, 4, 20);
      polygon(ink, [[6,5],[25,5],[12,12],[11,23],[6,23]]);
      polygon(brass, [[8,5],[21,5],[10,10],[10,22],[8,22]]);
      line(oak, 9, 5, 9, 9); line(ink, 12, 5, 12, 8);
      line(ink, 16, 5, 16, 6); line(cream, 8, 9, 8, 21);
      pixel(ink, 25, 6, 1, 7); pixel(brass, 26, 6, 1, 6);
      line(ink, 25, 12, 20, 17); line(ink, 26, 12, 31, 17);
      polygon(ink, [[15,15],[21,17],[31,17],[37,14],[35,19],[32,21],[21,21],[17,19]]);
      polygon(coral, [[17,17],[21,18],[32,18],[35,16],[32,19],[21,19]]);
      pixel(rose, 21, 18, 10); pixel(steel, 23, 14, 5, 2); pixel(foam, 24, 14, 3);
      pixel(ink, 2, 22, 35, 4); pixel(wood, 3, 23, 33, 1);
      pixel(cream, 4, 22, 5); pixel(brass, 13, 22, 5); pixel(cream, 23, 22, 5);
      for (const x of [5, 15, 31]) { pixel(ink, x, 26, 3, 3); pixel(steel, x + 1, 26, 1, 2); }
      break;
    }
    case 4: { // A long ocean liner with twin raked funnels and rust-red boot stripe.
      ripples();
      pixel(steel, 6, 3, 4, 2); pixel(cream, 2, 2, 5, 1); pixel(steel, 12, 4, 3);
      for (const x of [14, 22]) {
        polygon(ink, [[x-2,5],[x+3,5],[x+5,12],[x,12]]);
        line(coral, x, 7, x + 1, 10); line(rose, x + 1, 7, x + 2, 10);
      }
      pixel(ink, 31, 6, 1, 9); line(steel, 31, 8, 35, 14);
      polygon(ink, [[6,13],[11,13],[11,11],[28,11],[31,14],[34,14],[35,19],[5,19]]);
      pixel(foam, 12, 12, 15, 2); pixel(cream, 7, 15, 25, 3);
      for (let x = 9; x < 31; x += 3) pixel(blue, x, 15, 2);
      for (let x = 13; x < 27; x += 3) pixel(shade, x, 12);
      polygon(ink, [[1,16],[8,18],[29,18],[39,14],[37,20],[31,25],[7,25],[3,21]]);
      polygon(coral, [[5,21],[12,22],[28,22],[36,19],[31,23],[8,23]]);
      pixel(foam, 8, 18, 20); line(cream, 30, 17, 37, 15);
      for (let x = 8; x < 28; x += 4) pixel(cream, x, 20);
      break;
    }
    case 5: { // Cast brass bell with an amber wooden handle.
      polygon(ink, [[17,1],[22,1],[24,3],[23,6],[22,7],[22,10],[26,12],[29,16],[30,21],[34,24],[34,27],[23,27],[22,30],[20,31],[17,30],[16,27],[5,27],[5,24],[9,21],[10,16],[12,12],[16,10],[17,8],[16,5],[15,4]]);
      pixel(wood, 17, 3, 5, 2); pixel(rose, 18, 2, 3);
      pixel(oak, 18, 5, 3, 5); pixel(brass, 18, 6, 1, 3);
      polygon(brass, [[16,11],[23,11],[26,13],[28,16],[29,22],[32,24],[7,24],[11,21],[12,16],[13,13]]);
      polygon(gold, [[16,12],[21,12],[17,15],[16,20],[13,22],[10,22],[13,18],[13,15]]);
      pixel(foam, 15, 14, 2, 4); pixel(cream, 13, 18, 2, 2);
      line(oak, 26, 15, 28, 21); pixel(oak, 10, 22, 19);
      pixel(gold, 7, 24, 25); pixel(wood, 7, 25, 25);
      pixel(brass, 18, 27, 4, 2); pixel(gold, 18, 27, 2);
      line(brass, 7, 8, 4, 5); pixel(brass, 3, 12, 3);
      line(brass, 31, 9, 34, 6); pixel(brass, 34, 13, 3);
      break;
    }
    case 6: { // Anchor seal and forked commissioning ribbons.
      polygon(ink, [[12,16],[21,19],[15,31],[12,27],[7,28]]);
      polygon(coral, [[13,18],[19,20],[15,28],[12,25],[10,26]]);
      polygon(ink, [[19,19],[27,16],[32,28],[27,27],[24,31]]);
      polygon(coral, [[21,20],[26,18],[29,26],[26,25],[24,28]]);
      polygon(ink, [[14,2],[25,2],[30,7],[30,16],[25,21],[14,21],[9,16],[9,7]]);
      polygon(brass, [[15,3],[24,3],[29,8],[29,15],[24,20],[15,20],[10,15],[10,8]]);
      polygon(gold, [[16,5],[23,5],[27,8],[27,14],[23,18],[16,18],[12,14],[12,8]]);
      pixel(oak, 18, 6, 4, 3); pixel(gold, 19, 7, 2); pixel(oak, 19, 9, 2, 7);
      pixel(oak, 16, 10, 8); line(oak, 15, 13, 19, 16); line(oak, 20, 16, 24, 13);
      pixel(oak, 15, 12, 2, 2); pixel(oak, 23, 12, 2, 2);
      break;
    }
  }
}

// Commemorative stamps from the harbor post office.
function paintPostmark(canvas, kind) {
  if (canvas.width !== 40) canvas.width = 40;
  if (canvas.height !== 32) canvas.height = 32;
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, 40, 32);
  if (!Number.isInteger(kind) || kind < 0 || kind > 4) return;

  const rect = (color, x, y, w = 1, h = 1) => {
    ctx.fillStyle = color;
    ctx.fillRect(x, y, w, h);
  };
  const stroke = (color, ax, ay, bx, by, weight = 1) => {
    const steps = Math.max(Math.abs(bx - ax), Math.abs(by - ay), 1);
    for (let i = 0; i <= steps; i++) {
      const x = Math.round(ax + (bx - ax) * i / steps);
      const y = Math.round(ay + (by - ay) * i / steps);
      rect(color, x, y, weight, weight);
    }
  };
  const ellipse = (color, cx, cy, rx, ry) => {
    for (let j = -ry; j <= ry; j++) {
      const dx = Math.floor(rx * Math.sqrt(1 - (j / ry) * (j / ry)));
      rect(color, cx - dx, cy + j, dx * 2 + 1);
    }
  };

  const paper = '#fff0bb', gum = '#bca575', ink = '#243c4c';
  // The projecting teeth have an offset gum shadow, including all four corners.
  rect(gum, 1, 2, 38, 28); rect(gum, 3, 30, 34);
  for (let x = 3; x <= 34; x += 5) {
    rect(gum, x, 1, 3); rect(gum, x, 30, 3, 2);
  }
  for (let y = 3; y <= 26; y += 5) { rect(gum, 0, y + 1, 2, 3); rect(gum, 38, y + 1, 2, 3); }
  rect(paper, 2, 2, 36, 27);
  for (let x = 3; x <= 34; x += 5) { rect(paper, x, 0, 3, 2); rect(paper, x, 29, 3, 2); }
  for (let y = 3; y <= 26; y += 5) { rect(paper, 0, y, 2, 3); rect(paper, 38, y, 2, 3); }

  const fields = ['#dd915d', '#558f83', '#567f9d', '#b86d52', '#384b68'];
  rect(ink, 4, 4, 32, 23); rect(fields[kind], 5, 5, 30, 21);
  if (kind === 0) {
    // A single oar across the sunrise and its reflection.
    const gold = '#ffce76', copper = '#935c49', sea = '#4e928d';
    ellipse(gold, 21, 13, 7, 7); rect('#efac69', 5, 15, 30, 2);
    rect(sea, 5, 18, 30, 8); rect(gold, 16, 18, 11); rect('#b4c599', 17, 20, 8);
    rect('#b4c599', 20, 22, 5); rect('#b4c599', 7, 20, 5); rect('#b4c599', 26, 24, 6);
    stroke(ink, 10, 23, 29, 7, 2); stroke(paper, 12, 21, 29, 7);
    stroke(ink, 9, 21, 15, 17, 3); stroke(copper, 10, 21, 15, 17, 2);
    stroke(gold, 10, 21, 14, 18);
  } else if (kind === 1) {
    // A red rock crab, with four walking legs on each side.
    const shell = '#ec7957', orange = '#ffa570', seagrass = '#a6bf99';
    for (const x of [7, 29]) { rect(seagrass, x, 22, 4); rect(seagrass, x + 1, 9, 2); }
    for (let side of [-1, 1]) {
      const sx = n => side === -1 ? n : 39 - n;
      for (let j = 0; j < 3; j++) {
        stroke(ink, sx(15), 17 + j, sx(10 - j), 18 + 2 * j);
        stroke(shell, sx(14), 17 + j, sx(9 - j), 17 + 2 * j);
      }
      stroke(ink, sx(16), 15, sx(10), 12, 2);
      stroke(orange, sx(15), 15, sx(10), 12);
      ellipse(ink, sx(10), 12, 3, 3); ellipse(shell, sx(10), 11, 3, 2);
      rect(orange, sx(8), 8, 1, 3); rect(shell, sx(12), 9, 1, 2);
    }
    ellipse(ink, 20, 17, 7, 5); ellipse(shell, 20, 16, 7, 4);
    rect(orange, 16, 14, 6); rect(paper, 16, 12, 2, 2); rect(paper, 22, 12, 2, 2);
    rect(ink, 17, 12); rect(ink, 22, 12);
  } else if (kind === 2) {
    // The little ferry crossing, seen through an eight-bolt porthole.
    const brass = '#d79e55', gold = '#f6cc7b', sea = '#477d86', ferry = '#264b59';
    ellipse(ink, 20, 15, 12, 10); ellipse(brass, 20, 15, 11, 10);
    ellipse(ink, 20, 15, 9, 8); ellipse('#a6c5bf', 20, 15, 8, 7);
    rect(sea, 13, 18, 15); rect(sea, 15, 20, 11); rect(paper, 17, 19, 6);
    rect(ferry, 13, 16, 15, 2); rect(ferry, 15, 18, 10);
    rect(paper, 17, 13, 7, 3); rect(ferry, 18, 14, 2); rect(ferry, 22, 14);
    rect('#cc6d52', 16, 12, 9); rect(ink, 20, 10, 2, 2);
    for (const [x, y] of [[20,6],[27,9],[30,15],[27,21],[20,24],[13,21],[10,15],[13,9]]) {
      rect(ink, x, y); rect(gold, x-1, y-1);
    }
    rect(gold, 6, 6, 3); rect(gold, 31, 24, 3);
  } else if (kind === 3) {
    // Crossed hammer and marlinspike on the shipwright's sunburst.
    const glow = '#e39865', brass = '#ecc474', metal = '#a5bdb1';
    for (const [x,y] of [[6,7],[15,5],[25,5],[33,7],[33,23],[25,24],[15,24],[6,23]]) {
      stroke(glow, 20, 15, x, y);
    }
    stroke(ink, 10, 7, 27, 22, 3); stroke(metal, 12, 9, 27, 22, 2);
    stroke(ink, 10, 22, 26, 7, 3); stroke(brass, 11, 22, 26, 8, 2);
    stroke(ink, 21, 7, 29, 12, 3); stroke(metal, 21, 7, 29, 12, 2);
    rect(metal, 20, 6, 5, 2); rect(ink, 29, 13, 2, 2); rect(paper, 26, 10, 2);
    rect(ink, 25, 21, 4, 3); rect(brass, 26, 21, 3, 2);
    rect(paper, 7, 14); rect(paper, 32, 16);
  } else {
    // A pair of liner funnels under a pale moon.
    const gold = '#f4d58e', red = '#df745c', copper = '#ab6654', night = fields[4];
    ellipse(gold, 29, 10, 4, 4); ellipse(night, 31, 8, 3, 3);
    [[8,11],[14,7],[23,8],[31,17],[7,19]].forEach(p => rect(paper, p[0], p[1]));
    rect('#798495', 7, 7, 3); rect('#798495', 9, 8, 4); rect('#798495', 17, 8, 3);
    for (const [x,y] of [[10,11],[20,13]]) {
      for (let i = 0; i < 10; i++) {
        const dx = Math.floor(i / 4);
        rect(ink, x + dx, y + i, 7); rect(i < 3 ? '#253a49' : red, x + dx + 1, y + i, 5);
        if (i >= 3) rect(copper, x + dx + 4, y + i, 2);
      }
      rect(paper, x + 1, y + 3, 4);
    }
    rect('#ddc4a2', 5, 21, 28, 2); rect(ink, 5, 23, 30, 3);
    for (let x = 7; x < 32; x += 4) rect(gold, x, 24);
  }
}

// Small painted cargo labels for the exchange.
function paintCargo(canvas, good) {
  if (canvas.width !== 40) canvas.width = 40;
  if (canvas.height !== 32) canvas.height = 32;
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, 40, 32);
  const ink = '#193847', navy = '#355664', jade = '#579a90';
  const ivory = '#ffedbf', sand = '#d9bb84', gold = '#e8af50', pale = '#f7cf70';
  const umber = '#896347', walnut = '#9f6246', clay = '#c87e54', coral = '#e86e54';
  const rect = (color, x, y, w = 1, h = 1) => {
    ctx.fillStyle = color;
    ctx.fillRect(x, y, w, h);
  };
  const edge = (color, ax, ay, bx, by, size = 1) => {
    const steps = Math.max(Math.abs(ax - bx), Math.abs(ay - by), 1);
    for (let i = 0; i <= steps; i++) {
      const mix = i / steps;
      rect(color, Math.round(ax + (bx - ax) * mix), Math.round(ay + (by - ay) * mix), size, size);
    }
  };
  const face = (color, points) => {
    for (let y = 0; y < 32; y++) {
      const hits = [], scan = y + 0.5;
      for (let i = 0; i < points.length; i++) {
        const a = points[i], b = points[(i + 1) % points.length];
        if ((a[1] > scan) === (b[1] > scan)) continue;
        hits.push(a[0] + (b[0] - a[0]) * (scan - a[1]) / (b[1] - a[1]));
      }
      hits.sort((a, b) => a - b);
      for (let i = 0; i < hits.length; i += 2) {
        const x = Math.ceil(hits[i] - 0.5), end = Math.ceil(hits[i + 1] - 0.5);
        if (end > x) rect(color, x, y, end - x);
      }
    }
  };
  switch (good) {
    case 0: {
      // Two sailcloth sacks, with salt crystals stamped in merchant blue.
      const sack = (x, y, canvasColor, fold, bright) => {
        const shift = pts => pts.map(p => [x + p[0], y + p[1]]);
        const shape = [[6,0],[11,1],[14,0],[13,4],[12,5],[16,10],[18,16],[18,20],
          [15,23],[3,23],[0,20],[0,16],[2,10],[6,5],[4,2]];
        face(ink, shift(shape));
        face(canvasColor, shift([[6,2],[11,2],[12,1],[10,5],[14,10],[16,15],[16,20],
          [14,21],[4,21],[2,19],[2,16],[4,10],[8,5]]));
        face(fold, shift([[11,7],[14,11],[16,15],[16,20],[13,21],[6,21],[4,19],
          [10,20],[13,18],[14,15]]));
        edge(bright, x+5, y+10, x+3, y+16); edge(fold, x+7, y+7, x+6, y+10);
        rect(walnut, x+6,y+5,6); rect(gold,x+6,y+4,6);
        edge(umber,x+11,y+5,x+13,y+7);
      };
      sack(4,4,'#b4b7a1','#849a94','#e1ceb0');
      sack(17,6,'#ede0ba','#bbad8e',ivory);
      face(navy, [[23,20],[26,16],[29,20],[26,23]]);
      edge('#a5c2ad', 24,20,26,18); rect('#93b6a8',26,20,2); rect(ink,26,22);
      rect(sand, 10,26,3); rect(ivory,12,26); rect(ivory,15,27,2);
      break;
    }
    case 1: {
      // Landed crab peering over a split-willow basket.
      face(ink, [[4,18],[8,16],[31,16],[36,18],[33,26],[30,29],[9,29],[6,25]]);
      face(walnut, [[7,20],[33,20],[31,25],[29,27],[10,27],[8,24]]);
      for (let x = 9; x < 32; x += 4) {
        edge(gold, x,21,Math.min(x+2,31),26);
        edge(umber, x+1,26,Math.min(x+3,31),21);
      }
      rect(clay,9,24,22); rect(sand,11,26,18);
      for (const right of [false, true]) {
        const sx = x => right ? 39-x : x;
        for (let k = 0; k < 2; k++) {
          edge(ink,sx(15),15+k,sx(9-k*2),17+k);
          edge(coral,sx(15),14+k,sx(9-k*2),16+k);
        }
        edge(ink,sx(15),14,sx(8),12,2); edge(clay,sx(15),14,sx(8),11);
        const claw = [[3,7],[6,8],[8,10],[9,10],[10,6],[12,6],[12,12],[10,14],[6,14],[3,11]];
        face(ink,claw.map(p=>[sx(p[0]),p[1]]));
        face(coral, [[4,8],[7,10],[8,12],[10,11],[11,8],[11,12],[9,13],[6,12],[4,10]].map(p=>[sx(p[0]),p[1]]));
        rect('#ff9c6f',sx(6),10,1,2);
      }
      face(ink, [[11,13],[14,10],[25,10],[29,13],[27,17],[23,19],[15,19],[12,17]]);
      face(coral, [[13,13],[16,11],[24,11],[27,13],[26,16],[23,17],[16,17],[13,15]]);
      rect('#ffad78',15,12,8); rect('#bd5849',17,16,7);
      for (const x of [16,22]) {rect(ink,x,7,3,4); rect(ivory,x,7,2,2); rect(ink,x+1,7);}
      face(ink, [[4,17],[8,18],[31,18],[36,17],[35,21],[31,22],[8,22],[4,21]]);
      rect(gold, 6,19,27); rect('#f3ca87', 8,19,6); rect(clay, 8,20,24);
      break;
    }
    case 2: {
      // End grain faces the viewer, and two blue steel bands bind the courses.
      const plank = (x,y,length,top) => {
        const tip = [x+length,y-9];
        face(ink, [[x,y],[tip[0],tip[1]],[tip[0]+6,tip[1]+2],[tip[0]+6,tip[1]+6],
          [x+6,y+6],[x,y+4]]);
        face(top, [[x+1,y],[tip[0],tip[1]+1],[tip[0]+5,tip[1]+2],[x+5,y+2]]);
        face(walnut, [[x+6,y+2],[tip[0]+5,tip[1]+2],[tip[0]+5,tip[1]+5],[x+6,y+5]]);
        face(sand, [[x+1,y+1],[x+5,y+2],[x+5,y+5],[x+1,y+3]]);
        rect(umber,x+3,y+2); edge(umber,x+1,y+2,x+2,y+3);
      };
      for (const height of [0,4]) {
        for (let row=0;row<3;row++) {
          plank(3+row*5,20+row*2-height,17,[gold,'#d69760','#eab06e'][row]);
        }
      }
      for (const d of [6,12]) {
        face(navy, [[3+d,16-Math.round(d/2)],[6+d,15-Math.round(d/2)],
          [20+d,21-Math.round(d/2)],[17+d,23-Math.round(d/2)]]);
        const sy = 22-Math.round(d/2);
        face(ink, [[18+d,sy],[20+d,sy-1],[20+d,sy+6],[18+d,sy+7]]);
        edge(jade,4+d,16-Math.round(d/2),17+d,21-Math.round(d/2));
        rect('#98b6a7',18+d,sy+2);
      }
      break;
    }
    case 3: {
      // The exchange's two-pan dock scale on a graduated iron foot.
      rect(ink,18,5,4,20); rect(gold,19,7,2,16); rect(pale,19,8,1,12);
      face(ink,[[15,22],[24,22],[26,25],[31,25],[33,28],[32,29],[7,29],[7,27],[10,25],[14,25]]);
      rect(umber,15,24,10,2); rect(gold,16,23,8); rect(navy,9,26,22,2); rect(jade,11,26,18);
      for (const x of [9,30]) {
        edge(ink,x,9,x-4,18); edge(ink,x,9,x+4,18);
        edge(umber,x,10,x-3,17); edge(sand,x+1,11,x+3,17);
        face(ink, [[x-6,17],[x+7,17],[x+5,21],[x+2,23],[x-2,23],[x-5,21]]);
        face(gold, [[x-5,18],[x+5,18],[x+3,20],[x,21],[x-3,20]]);
        rect(pale,x-4,18,7);
      }
      face(ink,[[5,8],[10,6],[16,7],[20,5],[24,7],[31,6],[35,8],[34,10],[30,9],[24,10],[20,8],[15,10],[10,9],[6,10]]);
      edge(gold,7,8,10,7); edge(gold,10,7,16,8); edge(gold,16,8,20,6);
      edge(pale,20,6,24,8); edge(gold,24,8,31,7); edge(gold,31,7,33,8);
      rect(ink,18,2,4,5); rect(gold,19,3,2,3); rect(ivory,19,3);
      break;
    }
  }
}

// Boatbuilder's plates. The Pilgrim's gaff rig, Juniper's raked stack and
// Northstar's lantern tower form different silhouettes even at ledger size.
function paintHull(canvas, kind) {
  if (canvas.width !== 40) canvas.width = 40;
  if (canvas.height !== 32) canvas.height = 32;
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, 40, 32);
  const ink = '#193544', indigo = '#335367', chalk = '#fff0c7', canvasTan = '#dbb988';
  const gold = '#dfaa54', pine = '#4c8b80', copper = '#b96e4b', coral = '#e67a58';
  const rect = (color, x, y, w = 1, h = 1) => {
    ctx.fillStyle = color;
    ctx.fillRect(x, y, w, h);
  };
  const stroke = (color, x, y, x2, y2) => {
    const steps = Math.max(Math.abs(x2 - x), Math.abs(y2 - y), 1);
    for (let s = 0; s <= steps; s++) {
      rect(color, x + Math.round((x2 - x) * s / steps), y + Math.round((y2 - y) * s / steps));
    }
  };
  // One color per region keeps the sails and deck edges sharp at fractional CSS scales.
  const polygon = (color, points) => {
    const ys = points.map(p => p[1]);
    for (let y = Math.min(...ys); y < Math.max(...ys); y++) {
      const slice = y + 0.5, cuts = [];
      for (let i = 0; i < points.length; i++) {
        const p = points[i], q = points[(i+1)%points.length];
        if ((p[1] > slice) !== (q[1] > slice)) {
          cuts.push(p[0] + (slice-p[1])*(q[0]-p[0])/(q[1]-p[1]));
        }
      }
      cuts.sort((a,b)=>a-b);
      for (let i = 0; i < cuts.length; i += 2) {
        const a = Math.ceil(cuts[i]-0.5), b = Math.ceil(cuts[i+1]-0.5);
        if (b > a) rect(color, a, y, b-a);
      }
    }
  };
  const wake = () => {
    rect('#58949a', 3,27,7); rect('#87afa3', 10,29,8);
    rect('#548b8e', 22,28,8); rect('#99bbb1', 31,26,6);
  };
  switch (kind) {
    case 0: {
      wake();
      rect(ink,16,3,2,21); rect(gold,17,5,1,16);
      rect(coral,17,3,6); rect(copper,17,4,4);
      // Gaff mainsail to starboard of the mast, with the jib out on the bowsprit.
      polygon(ink, [[17,11],[25,5],[29,19],[28,21],[17,21]]);
      polygon(chalk, [[18,11],[24,7],[28,19],[18,20]]);
      polygon(canvasTan, [[24,9],[27,18],[20,19],[27,19]]);
      stroke(canvasTan,18,15,25,11); stroke(gold,17,10,25,5);
      polygon(ink, [[15,6],[16,20],[3,21]]);
      polygon(canvasTan, [[15,8],[15,19],[5,20]]);
      polygon(chalk, [[14,10],[13,17],[6,19]]);
      stroke(chalk,2,22,14,6); stroke(gold,2,22,8,23);
      polygon(ink, [[3,21],[13,22],[30,21],[35,19],[34,23],[29,26],[12,27],[6,25]]);
      polygon(pine, [[6,23],[14,24],[29,23],[33,21],[30,24],[13,25]]);
      rect(chalk,12,22,10); stroke(canvasTan,24,22,32,20);
      rect(copper,12,25,13); rect(gold,16,25,10);
      break;
    }
    case 1: {
      wake();
      // Juniper's cream wheelhouse, raked funnel and single mail pennant.
      rect(ink,29,5,1,15); rect(gold,29,7,1,10);
      rect(coral,30,5,5); rect(copper,30,6,3);
      stroke(canvasTan,29,7,35,18);
      polygon(ink, [[10,5],[15,5],[18,15],[13,15]]);
      polygon(coral, [[11,7],[15,7],[17,14],[13,14]]);
      rect(canvasTan,12,9,4); rect('#a7b5a5',6,4,4); rect('#bfc2a9',3,3,4);
      rect(ink,18,9,8,6); rect(chalk,19,10,6,3); rect(indigo,20,10,2,2);
      rect(indigo,23,10,2,2); rect(ink,17,8,10,2);
      rect(ink,7,13,21,6); rect(chalk,8,14,19,4);
      for (let x=9;x<26;x+=4) rect(indigo,x,15,2,2);
      polygon(ink, [[1,17],[8,19],[30,19],[38,16],[37,21],[32,25],[8,25],[4,22]]);
      polygon(copper, [[4,20],[9,21],[31,20],[36,18],[34,21],[31,23],[8,23]]);
      rect(canvasTan,7,20,22); stroke(gold,30,20,36,17);
      for (let x=10;x<32;x+=4) rect(ink,x,21,2);
      rect(chalk,10,23,19);
      break;
    }
    case 2: {
      wake();
      // Open iron tower and gallery keep the all-round mast light visible.
      rect(ink,8,10,1,11); stroke(canvasTan,8,10,4,19);
      rect(coral,9,10,3); rect(ink,31,11,1,10); stroke(canvasTan,31,11,35,20);
      stroke(ink,18,11,15,21); stroke(chalk,19,11,17,19);
      stroke(ink,22,11,25,21); stroke(chalk,21,11,24,19);
      stroke(indigo,18,15,23,18); stroke(copper,19,16,22,12);
      rect(ink,17,4,7,7); rect(chalk,18,6,5,3); rect(gold,18,8,5);
      rect(copper,20,6,1,3); rect(ink,15,9,11,2); rect(canvasTan,16,9,9);
      polygon(ink,[[16,5],[19,2],[22,2],[25,5]]);
      rect(coral,18,4,5); rect(chalk,20,1,1,2);
      stroke(gold,9,4,12,5); rect(chalk,7,7,5); stroke(gold,28,5,31,3); rect(chalk,29,7,5);
      rect(ink,9,17,7,4); rect(chalk,10,18,5,2); rect(indigo,11,18,2);
      rect(ink,25,18,5,3); rect(canvasTan,26,19,3);
      polygon(ink, [[2,19],[9,20],[29,20],[38,18],[36,23],[32,26],[9,26],[5,23]]);
      polygon(copper, [[4,21],[9,22],[30,22],[36,20],[34,23],[30,24],[9,24]]);
      rect(coral,9,22,20); rect(chalk,14,22,1,2);rect(chalk,16,22,2);
      rect(chalk,19,22); rect(chalk,21,22,2); rect(chalk,24,22);
      break;
    }
    case 3: {
      // Unrolled lines plan, timber board, carpenter's square and yellow pencil.
      polygon(ink, [[2,8],[30,3],[38,8],[37,24],[8,30],[2,25]]);
      polygon('#875b47', [[3,9],[30,5],[36,9],[36,23],[9,28],[4,25]]);
      stroke(gold,8,26,35,21); stroke(copper,6,25,22,22);
      polygon(ink, [[4,8],[29,4],[34,6],[34,21],[9,26],[5,24]]);
      polygon('#3f7286', [[5,9],[29,5],[32,7],[32,20],[9,24],[6,23]]);
      for (let x=9; x<=27; x+=6) stroke('#538594',x,10-Math.floor((x-9)/6),x+1,22-Math.floor((x-9)/6));
      stroke('#538594',7,16,31,12); stroke('#538594',8,20,31,16);
      stroke(chalk,9,16,26,12); stroke(chalk,9,16,13,18); stroke(chalk,13,18,23,16);
      stroke(chalk,23,16,26,12); stroke(canvasTan,15,14,15,11); stroke(canvasTan,15,11,21,12);
      stroke('#a4c4bd',10,21,16,20); rect('#a4c4bd',23,18,3);
      stroke(ink,29,5,32,20); stroke(canvasTan,30,5,33,19); stroke(chalk,31,5,34,19);
      stroke(ink,17,23,32,27); stroke(gold,19,22,33,26); stroke(chalk,17,22,18,22);
      rect(ink,16,21); rect(coral,32,25,3,2); rect(canvasTan,31,25);
      break;
    }
  }
}

// A keepsake drawn at double the slipway scene's native resolution.
function drawShipCertificate(canvas, scene, i, name, role, bonus) {
  canvas.width = 816;
  canvas.height = 506;
  const ctx = canvas.getContext('2d');
  const ink = '#254c4d', wornInk = '#52716a', gold = '#ac854d';
  const vellum = '#f4e8ca', pale = '#fff4d8';
  const serial = 'BWH-0' + (i + 1);

  function rule(x, y, endX, endY, color, weight = 1) {
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = weight;
    ctx.moveTo(x, y);
    ctx.lineTo(endX, endY);
    ctx.stroke();
  }
  function frame(x, y, w, h, color, weight = 1) {
    ctx.lineWidth = weight;
    ctx.strokeStyle = color;
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
  }
  function label(text, x, y, font, color = ink, align = 'left') {
    ctx.font = font;
    ctx.fillStyle = color;
    ctx.textAlign = align;
    ctx.fillText(text, x, y);
  }
  // Typeset small-cap labels by hand so tracking also works in offline browsers.
  function caps(text, x, y, size, color, tracking = 1.25, align = 'left') {
    ctx.font = 'bold ' + size + 'px Arial, sans-serif';
    ctx.fillStyle = color;
    ctx.textAlign = 'left';
    const widths = Array.from(text, ch => ctx.measureText(ch).width);
    const span = widths.reduce((a,b) => a + b, 0) + tracking * Math.max(0, widths.length - 1);
    let pen = align === 'right' ? x - span : align === 'center' ? x - span / 2 : x;
    Array.from(text).forEach((ch, j) => { ctx.fillText(ch, pen, y); pen += widths[j] + tracking; });
  }
  function diamond(x, y, size, color) {
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.moveTo(x, y-size); ctx.lineTo(x+size, y); ctx.lineTo(x, y+size); ctx.lineTo(x-size, y);
    ctx.closePath(); ctx.fill();
  }

  ctx.fillStyle = vellum;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  // Deterministic flecks suggest paper fibres and carry across duplicate prints.
  let fiber = 8933;
  for (let n = 0; n < 7200; n++) {
    fiber = (Math.imul(fiber, 1664525) + 1013904223) >>> 0;
    const x = fiber % canvas.width;
    fiber = (Math.imul(fiber, 1664525) + 1013904223) >>> 0;
    const y = fiber % canvas.height;
    ctx.fillStyle = n % 3 === 0 ? '#e8dbbb' : '#f9efd3';
    ctx.fillRect(x, y, n % 9 === 0 ? 2 : 1, 1);
  }
  frame(11, 11, 793, 483, wornInk);
  frame(15, 15, 785, 475, ink, 2);
  frame(19, 19, 777, 467, gold);
  for (const [x,y,sx,sy] of [[19,19,1,1],[797,19,-1,1],[19,487,1,-1],[797,487,-1,-1]]) {
    rule(x + sx*2, y+sy*8, x + sx*8, y+sy*2, gold);
    rule(x + sx*2, y+sy*14, x + sx*14, y+sy*2, gold);
    diamond(x+sx*7, y+sy*7, 2, ink);
  }

  caps('BELLWEATHER SHIPYARD', 48, 53, 12, ink, 2.4);
  caps('FLEET REGISTER', 644, 53, 10, wornInk, 1.25, 'right');
  rule(659, 37, 659, 58, gold);
  label(serial, 768, 54, 'bold 17px "Courier New", monospace', ink, 'right');
  rule(48,67,768,67,wornInk);
  rule(48,70,768,70,gold);

  label(name, 46, 122, 'bold 49px Georgia, "Times New Roman", serif');
  label('Certificate of launch', 768, 103, 'italic 23px Georgia, "Times New Roman", serif', ink, 'right');
  caps(role.toUpperCase(), 768, 122, 10, wornInk, 1.8, 'right');

  // The launch portrait is the live shipyard painting at a true 2× pixel scale.
  ctx.fillStyle = pale;
  ctx.fillRect(44, 139, 728, 232);
  frame(43, 138, 729, 233, gold);
  frame(46, 141, 723, 227, ink);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(scene, 48, 143, 720, 224);

  caps('LAUNCH RECORD', 48, 400, 10, wornInk, 1.7);
  label('Entered in the Bellweather fleet register', 48, 430, '19px Georgia, "Times New Roman", serif');
  rule(48,444,402,444,gold);
  for (const x of [48,402]) diamond(x,444,2.5,gold);

  // The seal has a milled rim, concentric dies and an anchor in raised relief.
  const cx = 493, cy = 424;
  function disk(x,y,r,fill) {
    ctx.beginPath(); ctx.arc(x,y,r,0,Math.PI*2); ctx.fillStyle=fill; ctx.fill();
  }
  disk(cx+1,cy+2,35,'#b79d64');
  ctx.beginPath();
  for (let t=0;t<96;t++) {
    const a=t*Math.PI/48, r=t%2===0?36:33.5;
    const x=cx+Math.cos(a)*r, y=cy+Math.sin(a)*r;
    if (t===0) ctx.moveTo(x,y); else ctx.lineTo(x,y);
  }
  ctx.closePath(); ctx.fillStyle='#ceaf69'; ctx.fill();
  disk(cx,cy,31.7,'#947146'); disk(cx,cy-0.5,30.5,'#eed495');
  disk(cx,cy,28.4,'#c7a45d'); disk(cx,cy,26.2,'#dec387');
  for(let n=0;n<40;n++){
    const a=n*Math.PI/20;
    rule(cx+27*Math.cos(a),cy+27*Math.sin(a),cx+29*Math.cos(a),cy+29*Math.sin(a),'#9f814c',0.8);
  }
  const anchor = (dx,dy,color) => {
    ctx.save(); ctx.translate(cx+dx,cy+dy);
    ctx.beginPath();ctx.arc(0,-13,3,0,Math.PI*2);ctx.lineWidth=2.2;ctx.strokeStyle=color;ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(0,-10);ctx.lineTo(0,15);
    ctx.moveTo(-7,-5);ctx.lineTo(7,-5);
    ctx.moveTo(-13,3);ctx.quadraticCurveTo(-12,12,0,15);ctx.quadraticCurveTo(12,12,13,3);
    ctx.moveTo(-14,8);ctx.lineTo(-13,2);ctx.lineTo(-9,6);
    ctx.moveTo(14,8);ctx.lineTo(13,2);ctx.lineTo(9,6);
    ctx.stroke();ctx.restore();
  };
  anchor(1,1,'#f3dba2'); anchor(0,0,'#857046');

  label('Bo, Shipwright', 663, 427, 'italic 27px Georgia, "Times New Roman", serif', ink, 'center');
  rule(566,438,757,438,wornInk);
  caps('BUILDER’S SIGNATURE', 662, 454, 9, wornInk, 1.55, 'center');
}
