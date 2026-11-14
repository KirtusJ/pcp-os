use crate::game::*;
use crate::ink::*;

struct Coast {
    sky: u32,
    haze: u32,
    far: u32,
    hill: u32,
    harbor: u32,
    sea: u32,
    foam: u32,
    cap: u32,
    soil: u32,
    sun: u32,
}
const COAST: [Coast; 3] = [
    Coast {
        sky: 0x7eb8bd,
        haze: 0xffcaa4,
        far: 0xc69e9e,
        hill: 0x938b93,
        harbor: 0x5a777d,
        sea: 0x518e90,
        foam: 0x9ac0ad,
        cap: 0x82bb94,
        soil: 0x945b55,
        sun: 0xffdf97,
    },
    Coast {
        sky: 0x99b6a1,
        haze: 0xffcc8a,
        far: 0xbdab7d,
        hill: 0x8f9777,
        harbor: 0x626f63,
        sea: 0x638e80,
        foam: 0xadc59a,
        cap: 0xb0c27a,
        soil: 0x876d4e,
        sun: 0xffe39c,
    },
    Coast {
        sky: 0x252c51,
        haze: 0xb88b9d,
        far: 0x8c7994,
        hill: 0x675e80,
        harbor: 0x434561,
        sea: 0x485b78,
        foam: 0x8290a4,
        cap: 0x899da8,
        soil: 0x736477,
        sun: 0xffe1a0,
    },
];
fn noise(i: i32) -> i32 {
    let mut n = i as u32;
    n = n.wrapping_mul(0x9e3779b9);
    n ^= n >> 16;
    n = n.wrapping_mul(0x85ebca6b);
    ((n ^ (n >> 13)) & 0x7fff) as i32
}
fn gullmark(f: &mut Frame, x: i32, y: i32, c: u32) {
    f.line(x - 4, y - 2, x, y, c);
    f.line(x, y, x + 3, y - 3, c)
}
fn clouds(f: &mut Frame, x: i32, y: i32, n: i32, c: u32) {
    f.rect(x, y, n, 2, c);
    f.rect(x + n / 5, y - 3, n / 2, 3, c);
    f.rect(x + n / 3, y - 6, n / 5, 3, c);
    f.rect(x - 8, y + 4, n / 2, 1, c)
}
fn skyline(f: &mut Frame, scroll: i32, y: i32, height: i32, step: i32, c: u32, seed: i32) {
    for x in (0..W).step_by(2) {
        let t = x + scroll + step * 5;
        let ix = t.div_euclid(step);
        let a = noise(ix + seed) % height;
        let b = noise(ix + seed + 1) % height;
        let v = y - a - (b - a) * t.rem_euclid(step) / step;
        f.rect(x, v, 2, y - v + 1, c)
    }
}
fn tower(f: &mut Frame, x: i32, y: i32, z: i32, lit: u32, light: u32, dark: u32, brick: u32) {
    for j in 0..27 {
        let w = 3 + j / 9;
        f.rect(
            x - w * z,
            y + (j - 27) * z,
            (2 * w + 1) * z,
            z,
            if (8..13).contains(&j) || (22..25).contains(&j) {
                brick
            } else {
                light
            },
        );
        f.rect(x + (w - 1) * z, y + (j - 27) * z, 2 * z, z, dark)
    }
    f.rect(x - z, y - 6 * z, 2 * z, 6 * z, dark);
    f.rect(x - 5 * z, y - 30 * z, 11 * z, 3 * z, dark);
    f.rect(x - 3 * z, y - 34 * z, 7 * z, 4 * z, lit);
    f.rect(x - 3 * z, y - 34 * z, z, 4 * z, dark);
    f.rect(x + 3 * z, y - 34 * z, z, 4 * z, dark);
    f.tri(x, y - 40 * z, 6 * z, 6 * z, dark);
    f.rect(x, y - 42 * z, z, 3 * z, dark);
}
fn scenery(f: &mut Frame, stage: usize, scroll: i32, t: u32) {
    let c = &COAST[stage];
    for y in (0..184).step_by(2) {
        f.rect(0, y, W, 2, mix(c.sky, c.haze, y, 184))
    }
    if stage == 2 {
        for i in 0..68 {
            let x = (noise(i * 19 + 7) - scroll / 12).rem_euclid(W);
            let y = noise(i * 31 + 21) % 120;
            f.rect(
                x,
                y,
                1 + ((i % 13 == 0) as i32),
                1,
                if i % 4 == 0 { PAPER } else { 0xc5b4c9 },
            )
        }
    }
    let sx = 389 - scroll / 18;
    let sy = 75;
    f.circ(sx, sy, 31, c.sun);
    f.rect(sx - 23, sy + 13, 54, 2, mix(c.haze, c.sun, 1, 2));
    for i in 0..9 {
        let x = (noise(i * 31 + 1) - scroll / 12 - (t / 30) as i32).rem_euclid(620) - 55;
        let y = 22 + noise(i * 29 + 19) % 83;
        clouds(
            f,
            x,
            y,
            20 + noise(i * 17 + 8) % 45,
            mix(c.haze, c.sun, 1, 3),
        )
    }
    skyline(f, scroll / 10, 168, 48, 41, c.far, 33);
    skyline(f, scroll / 6, 171, 32, 30, c.hill, 17);
    for i in -1..8 {
        let x = i * 112 - (scroll / 4).rem_euclid(112);
        let var = noise(i + (scroll / 4).div_euclid(112) + 7);
        let y = 168;
        if (i + (scroll / 4).div_euclid(112)).rem_euclid(5) == 4 {
            tower(f, x, y, 1, c.sun, c.hill, c.harbor, c.harbor)
        } else if (i + (scroll / 4).div_euclid(112)).rem_euclid(4) == 3 {
            // Dockside derrick with a suspended load and crossed tower braces.
            f.line(x - 5, y, x - 2, y - 22, c.harbor);
            f.line(x + 4, y, x - 2, y - 22, c.harbor);
            f.line(x - 2, y - 22, x + 18, y - 34, c.harbor);
            f.line(x - 2, y - 18, x + 18, y - 34, c.harbor);
            f.line(x + 16, y - 32, x + 16, y - 12, c.harbor);
            f.rect(x + 13, y - 13, 7, 5, c.harbor);
        } else if stage == 1 && var % 3 == 1 {
            for j in 0..8 {
                let xx = x + j * 3;
                let tip = y - 4 - noise(var + j) % 14;
                f.line(xx, y, xx - 2, tip, c.harbor);
                f.rect(xx - 3, tip - 2, 2, 4, c.harbor);
            }
        } else {
            for j in 0..4 {
                let xx = x + j * 8;
                let yy = y - 4 - (var / (j + 1)) % 10;
                f.rect(xx, yy, 9, y - yy, c.harbor);
                f.tri(xx + 4, yy - 4, 6, 4, c.harbor);
                f.rect(xx + 2, yy + 2, 1, 2, c.haze)
            }
        }
        f.line(x - 20, y - 5, x + 22, y - 5, c.harbor);
        for j in 0..4 {
            f.rect(x - 18 + j * 11, y - 5, 2, 10, c.harbor)
        }
    }
    for j in 0..4 {
        gullmark(
            f,
            ((j * 87 + 54 - scroll / 9 + (t / 9) as i32).rem_euclid(530)) - 8,
            43 + noise(j + 15) % 57,
            c.harbor,
        )
    }
    for y in (172..H).step_by(2) {
        f.rect(0, y, W, 2, mix(c.foam, c.sea, (y - 172).min(59), 59))
    }
    for i in 0..195 {
        let y = 174 + noise(i * 19 + 2) % 114;
        let x = (noise(7 + i * 43) - scroll / 3 + (t / (5 + (i % 4) as u32)) as i32)
            .rem_euclid(544)
            - 20;
        let len = 2 + noise(11 + i) % 17;
        let shimmer = (x - sx).abs() < 12 + (y - 172) / 3;
        f.rect(
            x,
            y,
            len,
            1,
            if shimmer {
                mix(c.foam, c.sun, 2, 3)
            } else {
                mix(c.sea, c.foam, 1 + (i % 3), 5)
            },
        )
    }
    let bx = (113 - scroll / 3 + (t / 24) as i32).rem_euclid(638) - 24;
    f.line(bx, 175, bx, 150, c.harbor);
    for j in 0..17 {
        f.rect(bx - 1 - j / 2, 153 + j, 1 + j / 2, 1, PAPER)
    }
    f.rect(bx - 12, 173, 26, 2, c.harbor);
    f.rect(bx - 9, 175, 20, 2, c.harbor);
}
// Original pixel sprite, drawn from the cap brim down to the webbed feet.
const BIRD: [&[u8]; 22] = [
    b"          nnnnn        ",
    b"        nnTTTTTnn      ",
    b"        nTTTTTtTnn     ",
    b"       nnnnGGnnnnnnn   ",
    b"       nWWWWWWWWn      ",
    b"      nWWWWWWWWWWn     ",
    b"      nWwWWWWWWIn      ",
    b"      nWWWWWWWWWWnaaa  ",
    b"      nwWWWWWWWWAaaaaa ",
    b"       nwwWWWWWnn      ",
    b"    nnnrRRRRRRRnn      ",
    b"  nnRRRnnTTTTTTRnn     ",
    b" nnRRnnnTtTTTTtRn      ",
    b" nnn  ntTttTTTTTRn     ",
    b" nnwwnttTTttTTTTbnn    ",
    b"  nwwwnttttTTTnbABn    ",
    b"   nnwwnnTTTTTnbBbn    ",
    b"     nnwwwwwwwnbbn     ",
    b"       nnwwwwwnnn      ",
    b"         AA  AA        ",
    b"        AAn  AAn       ",
    b"       aaan aaaan      ",
];
fn bird(f: &mut Frame, x: i32, foot: i32, z: i32, face: i32, wing: bool, step: bool) {
    let left = x - 11 * z;
    for (y, row) in BIRD.iter().enumerate() {
        for (i, b) in row.iter().enumerate() {
            let c = match b {
                b'n' | b'I' => INK,
                b'W' => 0xfff1d0,
                b'w' => 0xc6d0bc,
                b'T' => 0x307076,
                b't' => 0x45918e,
                b'G' => GOLD,
                b'R' => RED,
                b'r' => 0x9e4643,
                b'A' => 0xe78c4a,
                b'a' => 0xffb65c,
                b'b' => 0x815448,
                b'B' => 0xd1975f,
                _ => continue,
            };
            let col = if face >= 0 { i as i32 } else { 22 - i as i32 };
            let stride = if step && y >= 19 {
                if i < 12 {
                    -1
                } else {
                    1
                }
            } else {
                0
            };
            f.rect(
                left + (col + stride) * z,
                foot + (y as i32 - 22) * z,
                z,
                z,
                c,
            )
        }
    }
    if wing {
        let wx = x - face * 7 * z;
        for j in 0..5 {
            f.rect(
                wx - j * z * face,
                foot - (10 + j) * z,
                (4 + j / 2) * z,
                z,
                if j == 4 { INK } else { 0xe3dfc8 },
            )
        }
    }
}
fn sparkle(f: &mut Frame, x: i32, y: i32, n: i32, c: u32) {
    f.rect(x - n, y, 2 * n + 1, 1, c);
    f.rect(x, y - n, 1, 2 * n + 1, c)
}
fn stamp(f: &mut Frame, x: i32, y: i32, z: i32) {
    f.rect(x - 5 * z, y - 6 * z, 11 * z, 13 * z, INK);
    f.rect(x - 4 * z, y - 6 * z, 9 * z, 12 * z, PAPER);
    f.rect(x - 5 * z, y - 5 * z, 11 * z, 10 * z, PAPER);
    for i in 0..4 {
        f.rect(x - 5 * z, y + (-4 + i * 3) * z, z, z, INK);
        f.rect(x + 5 * z, y + (-4 + i * 3) * z, z, z, INK)
    }
    f.rect(x - 3 * z, y - 4 * z, 7 * z, 8 * z, RED);
    f.rect(x - 2 * z, y - z, 5 * z, 2 * z, GOLD);
    f.rect(x - z, y - 2 * z, z, z, PAPER);
    f.rect(x + z, y - 2 * z, z, z, PAPER);
}
fn jetty(f: &mut Frame, x: i32, y: i32, w: i32) {
    for i in (4..w).step_by(25) {
        f.rect(x + i, y + 5, 4, 40, 0x6c6666);
        f.rect(x + i + 1, y + 6, 1, 36, 0xb19579);
        f.line(x + i, y + 24, x + i + 22, y + 6, 0x80766c)
    }
    f.rect(x, y, w, 7, INK);
    f.rect(x, y, w, 3, 0xeab682);
    f.rect(x + 1, y + 3, w - 2, 2, 0xac795f);
    for i in (3..w).step_by(12) {
        f.rect(x + i, y, 1, 4, 0x946d5b)
    }
}
fn ground(f: &mut Frame, x: i32, y: i32, w: i32, seed: i32, c: &Coast) {
    f.rect(x, y, w, H - y, INK);
    f.rect(x + 2, y + 4, w - 4, H - y, c.soil);
    for row in 0..8 {
        for i in 0..w / 17 + 1 {
            let xx = x + i * 17 + (row % 2) * 7;
            let yy = y + 12 + row * 10;
            let a = noise(i + seed + row * 13);
            if xx + 11 < x + w && a % 5 != 0 {
                f.rect(xx + 3, yy, 7 + a % 4, 2, mix(c.soil, INK, 1, 4));
                f.rect(xx + 2, yy + 2, 2, 2, mix(c.soil, PAPER, 1, 6));
            }
        }
    }
    f.rect(x, y, w, 2, 0xe9d1a1);
    f.rect(x, y + 2, w, 4, c.cap);
    for i in (0..w - 3).step_by(7) {
        let n = noise(seed + i);
        f.rect(x + i + 1, y + 5, 2 + n % 4, 2 + n % 5, c.cap);
        if n % 4 == 0 {
            f.rect(x + i, y - 2, 1, 3, c.cap);
            f.rect(x + i + 2, y - 3, 1, 3, c.cap)
        }
    }
}
fn crab(f: &mut Frame, x: i32, y: i32, t: u32) {
    let a = (t / 6 % 2) as i32;
    for side in [-1, 1] {
        for j in 0..3 {
            let xx = x + side * (4 + j * 2);
            f.line(xx, y - 4, xx + side * (2 + a), y - 1 - j % 2, INK)
        }
        f.rect(x + side * 9 - 2, y - 9 + a, 4, 4, RED);
        f.rect(x + side * 10 - 2, y - 11 + a, 2, 3, GOLD)
    }
    f.rect(x - 7, y - 7, 15, 6, INK);
    f.rect(x - 6, y - 8, 13, 6, RED);
    f.rect(x - 4, y - 9, 2, 3, PAPER);
    f.rect(x + 3, y - 9, 2, 3, PAPER);
    f.rect(x - 3, y - 9, 1, 1, INK);
    f.rect(x + 4, y - 9, 1, 1, INK);
    f.rect(x - 3, y - 5, 6, 1, 0xff935e);
}
fn mail(f: &mut Frame, x: i32, y: i32, on: bool) {
    f.rect(x - 1, y - 23, 3, 23, INK);
    f.rect(x - 7, y - 26, 14, 10, INK);
    f.rect(x - 6, y - 25, 12, 7, if on { 0x549b88 } else { RED });
    f.rect(x - 3, y - 23, 7, 1, PAPER);
    f.rect(x + 4, y - 30, 1, 7, INK);
    f.rect(x + 5, y - 30, 5, 3, if on { GOLD } else { PAPER })
}
fn play(f: &mut Frame, g: &Game) {
    let r = g.route();
    let c = &COAST[g.stage];
    let cam = g.scroll;
    scenery(f, g.stage, cam, g.tick);
    let tx = r.length - cam;
    tower(f, tx + 4, FLOOR, 2, c.sun, PAPER, BLUE, RED);
    for t in r.spans {
        if t.x - cam < W && t.end - cam > 0 {
            ground(f, t.x - cam, t.y, t.end - t.x, t.x, c)
        }
    }
    for t in r.decks {
        if t.x - cam < W && t.end - cam > 0 {
            jetty(f, t.x - cam, t.y, t.end - t.x)
        }
    }
    for &sx in r.springs {
        let x = sx - cam;
        let y = r
            .spans
            .iter()
            .find(|s| sx >= s.x && sx < s.end)
            .map_or(FLOOR, |s| s.y);
        f.rect(x - 8, y - 3, 16, 3, INK);
        f.line(x - 6, y - 4, x + 6, y - 6, PAPER);
        f.line(x + 6, y - 6, x - 5, y - 8, PAPER);
        f.rect(x - 9, y - 11, 18, 3, RED);
        f.rect(x - 7, y - 12, 14, 1, GOLD)
    }
    mail(f, r.checkpoint - cam, FLOOR, g.checkpoint);
    for (i, &(x, y)) in r.stamps.iter().enumerate() {
        if g.mask & (1 << i) == 0 && x - cam > (-10) && x - cam < W + 10 {
            let b = ((g.tick / 8 + i as u32) % 4) as i32;
            stamp(f, x - cam, y + (b - 2).abs() - 1, 1)
        }
    }
    for (i, &(_, _, y)) in r.crabs.iter().enumerate() {
        if g.dead & (1 << i) == 0 {
            crab(f, g.crab_x(i) - cam, y, g.tick)
        }
    }
    let (x, y, n) = g.collected_fx;
    if n > 0 {
        let d = (17 - n as i32) / 2;
        sparkle(f, x - cam - 3 - d, y - 4 - d, 2, GOLD);
        sparkle(f, x - cam + 5 + d, y - 6 - d, 2, PAPER)
    }
    bird(
        f,
        g.pos() - cam,
        g.foot(),
        1,
        g.face,
        !g.grounded,
        (g.tick / 5) % 2 == 1 && g.vx != 0,
    );
    f.rect(8, 8, 16 + r.name.len() as i32 * 6 + 20, 18, INK);
    f.rect(10, 10, 3, 14, RED);
    f.num(17, 14, (g.stage + 1) as u32, 1, GOLD);
    f.text(34, 14, r.name, 1, PAPER);
    f.rect(W - 81, 8, 73, 18, INK);
    stamp(f, W - 70, 17, 1);
    f.num(W - 60, 14, g.mask.count_ones(), 1, PAPER);
    f.text(W - 45, 14, "/", 1, GOLD);
    f.num(W - 35, 14, r.stamps.len() as u32, 1, PAPER);
    // A route strip shows the remaining walk and the red postbox halfway marker.
    let bar = 76;
    let bx = W - 90 - bar;
    let cy = 17;
    f.rect(bx - 2, cy - 3, bar + 4, 7, INK);
    f.rect(bx, cy, bar, 1, 0x839394);
    f.rect(bx, cy, (bar * g.pos() / r.length).clamp(0, bar), 1, GOLD);
    f.rect(
        bx + r.checkpoint * bar / r.length,
        cy - 2,
        2,
        5,
        if g.checkpoint { GOLD } else { RED },
    );
    f.rect(
        bx + (bar * g.pos() / r.length).clamp(0, bar - 2),
        cy - 1,
        2,
        3,
        PAPER,
    );
    if g.notice > 0 {
        f.rect(189, 31, 133, 14, INK);
        f.text(205, 35, "CHECKPOINT SAVED", 1, PAPER)
    }
}
fn panel(f: &mut Frame, x: i32, y: i32, w: i32, h: i32) {
    f.rect(x + 3, y + 4, w, h, INK);
    f.rect(x, y, w, h, PAPER);
    f.rect(x + 2, y + 2, w - 4, h - 4, 0x263d48)
}
fn title(f: &mut Frame, g: &Game) {
    scenery(f, 0, 0, g.tick);
    jetty(f, 279, 224, 245);
    tower(f, 480, 174, 1, GOLD, PAPER, BLUE, RED);
    bird(f, 382, 224, 5, 1, false, false);
    f.rect(23, 22, 145, 13, BLUE);
    f.text(30, 25, "COASTAL POSTAL SERVICE", 1, PAPER);
    for (s, y) in [("GULLPOST", 46), ("EXPRESS", 86)] {
        f.text(25, y + 4, s, 5, INK);
        f.text(23, y, s, 5, if y == 46 { PAPER } else { GOLD })
    }
    panel(f, 23, 151, 217, 51);
    f.text(34, 161, "THREE LIGHTHOUSES. ONE MAILBAG.", 1, PAPER);
    f.rect(34, 177, 82, 15, RED);
    f.text(41, 181, "SPACE / ENTER", 1, PAPER);
    f.text(124, 181, "START ROUTE", 1, GOLD);
    for i in 0..3 {
        stamp(f, 31 + i * 18, 219, 1)
    }
    f.text(81, 216, "39 POSTMARKS", 1, INK);
    f.rect(0, 261, W, 27, INK);
    f.text(16, 272, "ARROWS / AD  MOVE", 1, PAPER);
    f.text(150, 272, "SPACE  JUMP / AIR FLAP", 1, PAPER);
    f.text(322, 272, "P  PAUSE", 1, PAPER);
    f.text(408, 272, "ESC  EXIT", 1, PAPER);
}
pub fn draw(f: &mut Frame, g: &Game) {
    if g.mode == Mode::Cover {
        title(f, g);
        return;
    }
    play(f, g);
    match g.mode {
        Mode::Pause => {
            panel(f, 136, 95, 240, 81);
            f.text(168, 110, "ON A BREAK", 3, PAPER);
            f.text(180, 152, "P / SPACE TO RESUME", 1, GOLD)
        }
        Mode::Delivery => {
            panel(f, 115, 77, 282, 112);
            f.text(131, 89, "LIGHTHOUSE POST", 1, GOLD);
            f.text(130, 107, "DELIVERED", 4, PAPER);
            stamp(f, 139, 156, 1);
            f.num(153, 152, g.mask.count_ones(), 1, GOLD);
            f.text(170, 152, "POSTMARKS", 1, PAPER);
            f.text(
                130,
                170,
                if g.stage < 2 {
                    "SPACE / ENTER  NEXT ROUTE"
                } else {
                    "SPACE / ENTER  OPEN ALBUM"
                },
                1,
                GOLD,
            )
        }
        Mode::Home => {
            panel(f, 112, 68, 288, 129);
            f.text(127, 81, "COASTAL POSTAL SERVICE", 1, GOLD);
            f.text(126, 99, "MAIL CALL", 4, PAPER);
            for (i, r) in ROUTES.iter().enumerate() {
                let x = 128 + i as i32 * 86;
                stamp(f, x + 7, 150, 1);
                f.num(x + 18, 146, g.album[i] as u32, 1, PAPER);
                f.text(x, 165, r.stamp, 1, GOLD)
            }
            f.text(126, 181, "SPACE / ENTER  PLAY AGAIN", 1, PAPER)
        }
        _ => {}
    }
}
