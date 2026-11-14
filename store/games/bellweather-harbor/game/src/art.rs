pub const ART_W: usize = 480;
pub const ART_H: usize = 192;

type Point = (i32, i32);
type Color = u32;
type Frame = [Color; ART_W * ART_H];

const TAR: Color = 0x203e45;
const INK: Color = 0x304b4d;
const DEEP: Color = 0x345955;
const PAINT: Color = 0xad5040;
const RED: Color = 0xd36c4e;
const GOLD: Color = 0xf3b65f;
const IVORY: Color = 0xffe0aa;
const SLATE: Color = 0x627b73;

struct Canvas<'a>(&'a mut Frame);

impl Canvas<'_> {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let x0 = x.max(0) as usize;
        let x1 = (x + w).clamp(0, ART_W as i32) as usize;
        if x0 >= x1 {
            return;
        }
        let y1 = (y + h).min(ART_H as i32);
        for row in y.max(0)..y1 {
            let start = row as usize * ART_W;
            self.0[start + x0..start + x1].fill(color);
        }
    }

    fn pixel(&mut self, x: i32, y: i32, color: Color) {
        if (x as u32) < ART_W as u32 && (y as u32) < ART_H as u32 {
            self.0[y as usize * ART_W + x as usize] = color;
        }
    }

    fn line(&mut self, mut a: Point, b: Point, color: Color) {
        let dx = (b.0 - a.0).abs();
        let dy = -(b.1 - a.1).abs();
        let sx = if a.0 < b.0 { 1 } else { -1 };
        let sy = if a.1 < b.1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.pixel(a.0, a.1, color);
            if a == b {
                break;
            }
            let twice = err * 2;
            if twice >= dy {
                err += dy;
                a.0 += sx;
            }
            if twice <= dx {
                err += dx;
                a.1 += sy;
            }
        }
    }

    // All polygons here are convex. One clipped horizontal span fills each row.
    fn shape(&mut self, points: &[Point], color: Color) {
        let lo = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let hi = points
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap_or(-1)
            .min(ART_H as i32 - 1);
        for y in lo..=hi {
            let mut left = i32::MAX;
            let mut right = i32::MIN;
            let mut prev = points[points.len() - 1];
            for &next in points {
                if y >= prev.1.min(next.1) && y <= prev.1.max(next.1) {
                    if prev.1 == next.1 {
                        left = left.min(prev.0.min(next.0));
                        right = right.max(prev.0.max(next.0));
                    } else {
                        let x = prev.0 + (next.0 - prev.0) * (y - prev.1) / (next.1 - prev.1);
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
                prev = next;
            }
            if left <= right {
                self.rect(left, y, right - left + 1, 1, color);
            }
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        let mut half = r;
        for dy in 0..=r {
            while half * half + dy * dy > r * r {
                half -= 1;
            }
            self.rect(cx - half, cy - dy, half * 2 + 1, 1, color);
            if dy > 0 {
                self.rect(cx - half, cy + dy, half * 2 + 1, 1, color);
            }
        }
    }
}

fn noise(n: u32) -> u32 {
    let mut v = n.wrapping_mul(0x9e3779b9);
    v ^= v >> 16;
    v = v.wrapping_mul(0x85ebca6b);
    v ^ (v >> 13)
}

fn triangle(t: u32, max: u32) -> i32 {
    let n = t % (max * 2);
    n.min(max * 2 - n) as i32
}

fn between(a: Color, b: Color, n: u32, d: u32) -> Color {
    let ch = |s: u32| ((((a >> s) & 255) * (d - n) + ((b >> s) & 255) * n) / d) << s;
    ch(16) | ch(8) | ch(0)
}

fn cloud(c: &mut Canvas, x: i32, y: i32, length: i32, seed: u32, dark: Color, lit: Color) {
    let mut at = 0;
    let mut i = 0;
    while at < length {
        let hash = noise(seed + i);
        let w = (8 + (hash & 23) as i32).min(length - at);
        let h = 1 + ((hash >> 8) % 5) as i32;
        c.rect(x + at, y - h, w, h + 1, dark);
        c.rect(x + at + 2, y + 1, (w - 5).max(1), 1, lit);
        at += w;
        i += 1;
    }
}

fn sky(c: &mut Canvas, tick: u32) {
    for y in 0..106 {
        let band = (y / 3 * 3) as u32;
        let color = if band < 54 {
            between(0x89949a, 0xe4ab91, band, 54)
        } else {
            between(0xe4ab91, 0xffce87, band - 54, 52)
        };
        c.rect(0, y, ART_W as i32, 1, color);
    }
    c.disc(168, 64, 27, 0xffcf85);
    c.disc(168, 64, 25, 0xffdf99);
    c.disc(168, 64, 22, 0xffe3a2);
    let drift = triangle(tick / 165, 8);
    cloud(c, -15 + drift, 17, 91, 22, 0xc5a49e, 0xe1b2a1);
    cloud(c, 67 + drift, 24, 37, 55, 0xe0b0a2, 0xedbfab);
    cloud(c, 112 - drift, 11, 58, 11, 0xaaa3a3, 0xbba9a6);
    cloud(c, 186 + drift, 32, 61, 310, 0xe1b19e, 0xf1c1a5);
    cloud(c, 251 - drift, 21, 104, 117, 0xc7aaa2, 0xe1b2a1);
    cloud(c, 369 + drift, 13, 62, 71, 0xaaa2a2, 0xb7a6a5);
    c.rect(134 + drift, 72, 39, 2, 0xf9c991);
    c.rect(164 + drift, 75, 44, 1, 0xfacb90);
    c.rect(48 + drift, 62, 30, 1, 0xf2c098);
    c.rect(281 + drift, 66, 55, 1, 0xf8c897);
    // Two headlands recede through the salt haze.
    let far = [
        (0, 70),
        (24, 76),
        (49, 69),
        (68, 75),
        (100, 84),
        (124, 83),
        (147, 90),
        (169, 88),
        (196, 93),
        (216, 88),
        (237, 87),
        (256, 80),
        (282, 85),
        (305, 77),
        (324, 79),
        (348, 72),
        (365, 77),
        (381, 75),
        (409, 69),
        (427, 72),
        (445, 69),
        (463, 74),
        (480, 73),
    ];
    for ab in far.windows(2) {
        c.shape(&[ab[0], ab[1], (ab[1].0, 105), (ab[0].0, 105)], 0xc2a293);
    }
    let near = [
        (0, 81),
        (32, 84),
        (52, 83),
        (76, 87),
        (103, 92),
        (124, 94),
        (155, 100),
        (192, 100),
        (217, 96),
        (248, 98),
        (274, 91),
        (305, 92),
        (325, 86),
        (350, 90),
        (383, 82),
        (409, 86),
        (435, 77),
        (454, 82),
        (480, 76),
    ];
    for ab in near.windows(2) {
        c.shape(&[ab[0], ab[1], (ab[1].0, 105), (ab[0].0, 105)], 0xa79a8b);
    }
    // The spire gives the far shore a readable scale.
    for i in 0..43 {
        let h = noise(i + 142);
        let x = 190 + (h % 225) as i32;
        let y = 94 - ((h >> 8) % 6) as i32;
        let w = 3 + ((h >> 16) % 8) as i32;
        c.rect(x, y, w, 104 - y, 0x998c80);
        c.line((x - 1, y), (x + w / 2, y - 2), 0x8d857c);
        c.line((x + w / 2, y - 2), (x + w, y), 0x8d857c);
    }
    c.rect(261, 86, 5, 12, 0x94897f);
    c.shape(&[(260, 85), (263, 77), (266, 85)], 0x8b827c);
    c.rect(0, 102, 480, 3, 0xc6b198);
}

fn sea(c: &mut Canvas, tick: u32) {
    let bright = [0xd8ba93, 0xd1b595, 0xc8ae91, 0xe2ba8d];
    let shadow = [0x809b93, 0x72928c, 0x698b87, 0x648983];
    for y in 105..192 {
        let t = ((y - 105) / 3 * 3) as u32;
        let col = if t <= 33 {
            between(0x9dada0, 0x78998e, t, 33)
        } else {
            between(0x78998e, 0x416e6d, t - 33, 54)
        };
        c.rect(0, y, 480, 1, col);
    }
    for i in 0..620 {
        let h = noise(i + 781);
        let y = 105 + ((h >> 9) % 87) as i32;
        let z = y - 105;
        let speed = 23 + (h & 31);
        let x = ((h >> 17) % 510) as i32 - 15 + triangle(tick / speed, 5) - 2;
        let span = 1 + ((h >> 5) % (4 + z as u32 / 6)) as i32;
        let shimmer = ((x - 168).abs() < 13 + z * 3 / 4 + ((h >> 2) % 29) as i32) && (h & 7) > 1;
        let idx = ((h >> 1) & 3) as usize;
        let col = if shimmer {
            bright[idx]
        } else if y > 145 && h & 1 == 0 {
            between(shadow[idx], DEEP, (z - 40) as u32, 70)
        } else {
            shadow[idx]
        };
        c.rect(x, y, span, 1, col);
    }
    for i in 0..63 {
        let h = noise(i + 188);
        let y = 105 + (h % 66) as i32;
        let radius = 9 + (y - 105) / 2;
        let x = 168 - radius + ((h >> 14) % (2 * radius) as u32) as i32;
        let on = ((tick / 15 + (h & 31)) % 20) < 14;
        if on {
            c.rect(x, y, 1 + ((h >> 9) % 8) as i32, 1, 0xf3ca95);
        }
    }
}

fn gull(c: &mut Canvas, x: i32, y: i32, phase: u32, col: Color) {
    let tip = match phase % 5 {
        0 | 1 => -2,
        2 | 4 => -1,
        _ => 1,
    };
    c.line((x - 4, y + tip), (x - 2, y - 1), col);
    c.line((x - 2, y - 1), (x, y + 1), col);
    c.line((x, y + 1), (x + 2, y - 1), col);
    c.line((x + 2, y - 1), (x + 4, y + tip), col);
}

fn light_station(c: &mut Canvas, tick: u32) {
    // Exposed strata at the foot of the light.
    let edge = [
        (0, 79),
        (13, 77),
        (25, 79),
        (34, 77),
        (47, 80),
        (54, 83),
        (65, 84),
        (72, 87),
        (81, 88),
        (91, 94),
        (99, 95),
        (105, 99),
        (117, 101),
        (124, 105),
    ];
    for ab in edge.windows(2) {
        c.shape(&[ab[0], ab[1], (ab[1].0, 105), (ab[0].0, 105)], 0x536d66);
        c.line(ab[0], ab[1], 0x878974);
    }
    for (x, y, w) in [
        (0, 82, 14),
        (21, 84, 18),
        (5, 89, 10),
        (36, 87, 11),
        (49, 90, 14),
        (16, 95, 18),
        (34, 94, 12),
        (60, 94, 11),
        (80, 98, 7),
        (0, 100, 27),
        (44, 99, 11),
        (91, 102, 13),
    ] {
        c.rect(x, y, w, 1, 0x839082);
        c.line((x + w, y), (x + w + 2, y + 2), 0x728477);
    }
    for (x, y, w) in [
        (0, 103, 21),
        (17, 105, 19),
        (42, 104, 29),
        (69, 107, 22),
        (94, 105, 17),
    ] {
        c.rect(x, y, w, 1, 0x435e5c);
        c.rect(x + 3, y + 3, w / 2, 1, 0x889f90);
    }
    // Keeper's red-roofed cottage.
    c.rect(1, 69, 22, 12, 0xddbb8c);
    c.shape(&[(0, 69), (6, 62), (21, 62), (26, 69)], 0x854b41);
    c.rect(5, 65, 16, 1, 0xb96c52);
    c.rect(15, 58, 3, 6, 0x75574b);
    c.rect(3, 72, 3, 4, INK);
    c.rect(10, 72, 3, 4, INK);
    c.rect(17, 73, 4, 7, 0x655e51);
    // Bellweather Light, with a dark seaward face, gallery and Fresnel lamp.
    c.shape(&[(32, 47), (40, 47), (45, 80), (28, 80)], 0xf1d4a4);
    c.shape(&[(37, 47), (40, 47), (45, 80), (39, 80)], 0xcaac89);
    c.shape(&[(31, 58), (41, 58), (42, 65), (30, 65)], 0xb55747);
    c.shape(&[(38, 58), (41, 58), (42, 65), (38, 65)], 0x9b564a);
    c.rect(34, 70, 3, 5, 0x726d60);
    c.rect(35, 51, 2, 3, 0x696f65);
    c.rect(27, 79, 19, 2, 0x7a7562);
    c.rect(29, 45, 14, 2, TAR);
    c.rect(31, 36, 10, 8, 0xd2ab72);
    c.rect(32, 37, 7, 5, 0xffdc83);
    c.rect(34, 37, 2, 5, if tick % 245 < 38 { 0xffffd0 } else { IVORY });
    c.rect(36, 36, 1, 7, 0x837658);
    c.rect(30, 36, 2, 8, INK);
    c.rect(40, 36, 2, 8, INK);
    c.shape(&[(28, 35), (36, 30), (43, 35)], 0x4b5651);
    c.rect(35, 27, 1, 4, INK);
    c.rect(28, 42, 16, 1, INK);
    for x in [28, 31, 40, 43] {
        c.rect(x, 42, 1, 4, INK);
    }
    c.rect(28, 45, 16, 1, 0x354b47);
    // Winding fence on the headland.
    for i in 0..6 {
        let x = 48 + i * 5;
        let y = 79 + i;
        c.rect(x, y, 1, 5, 0x576553);
        if i < 5 {
            c.line((x, y + 1), (x + 5, y + 2), 0xac9c7a);
        }
    }
}

fn boathouse(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    // y is the foundation, the gable faces left.
    let roof = y - h;
    c.rect(x, roof, w, h, 0xb79873);
    c.rect(x + 14, roof, w - 14, h, 0x9f886d);
    for i in (x + 2..x + w).step_by(4) {
        c.rect(i, roof, 1, h, 0x927f66);
    }
    c.shape(
        &[(x - 2, roof), (x + 7, roof - 8), (x + 16, roof)],
        0xd6b68b,
    );
    c.shape(
        &[
            (x + 7, roof - 8),
            (x + w - 7, roof - 8),
            (x + w + 2, roof),
            (x + 16, roof),
        ],
        0x765751,
    );
    for i in (x + 13..x + w - 7).step_by(5) {
        c.line((i, roof - 7), (i + 7, roof - 1), 0x906c5d);
    }
    c.line((x + 7, roof - 8), (x + w - 7, roof - 8), 0xba8568);
    c.line((x - 2, roof), (x + 7, roof - 8), INK);
    c.line((x + 7, roof - 8), (x + 16, roof), INK);
    c.rect(x + 15, roof, w - 12, 2, INK);
    c.rect(x + 5, y - 8, 6, 8, 0x4c6058);
    c.rect(x + 7, y - 7, 1, 7, 0x938d72);
    for xx in (x + 19..x + w - 3).step_by(9) {
        c.rect(xx - 1, roof + 3, 6, 6, 0xd4b08a);
        c.rect(xx, roof + 3, 4, 5, INK);
        c.rect(xx, roof + 4, 3, 2, 0xe3b980);
        c.rect(xx + 2, roof + 3, 1, 5, 0x776e59);
    }
}

fn crane(c: &mut Canvas, tick: u32, busy: bool) {
    let sway = if busy { triangle(tick / 13, 3) } else { 0 };
    // Iron portal legs share a concrete sill with the machine house.
    c.shape(&[(406, 77), (411, 77), (406, 110), (400, 110)], INK);
    c.shape(&[(420, 77), (424, 77), (431, 110), (425, 110)], INK);
    c.line((407, 82), (425, 103), SLATE);
    c.line((423, 82), (405, 103), SLATE);
    c.rect(400, 108, 32, 2, TAR);
    c.rect(404, 76, 21, 5, TAR);
    c.rect(411, 58, 5, 19, INK);
    c.line((413, 60), (414, 75), 0xa88c68);
    c.rect(399, 69, 25, 8, 0x645f51);
    c.rect(399, 69, 24, 2, 0xc68e65);
    c.rect(399, 71, 7, 4, 0x8a6b53);
    c.rect(419, 59, 10, 11, 0x846c53);
    c.rect(420, 60, 7, 5, 0xd3ab78);
    c.rect(424, 60, 1, 5, INK);
    c.rect(423, 67, 5, 1, 0x9d805f);
    c.rect(418, 57, 12, 2, INK);
    // Triangular lattice jib and rear counterweight.
    c.line((412, 70), (368, 33), TAR);
    c.line((415, 65), (368, 29), INK);
    c.line((368, 29), (368, 33), INK);
    for i in 0..7 {
        let x = 368 + 6 * i;
        let y = 32 + 5 * i;
        c.line((x, y), (x + 6, y + 1), 0x52645d);
        c.line((x + 6, y + 1), (x + 6, y + 5), 0x3e5751);
    }
    c.line((368, 29), (415, 65), 0xab8e69);
    c.line((369, 30), (421, 48), 0x5d6660);
    c.line((421, 48), (428, 70), INK);
    c.line((421, 48), (415, 66), INK);
    c.line((421, 48), (398, 68), 0x6e7365);
    c.rect(367, 29, 4, 3, INK);
    c.line((368, 33), (368 + sway, 79 + sway), 0x797c69);
    c.rect(367 + sway, 80 + sway, 3, 3, INK);
    c.line((369 + sway, 83 + sway), (366 + sway, 86 + sway), INK);
    if busy {
        c.line((367 + sway, 84 + sway), (362 + sway, 89 + sway), INK);
        c.line((367 + sway, 84 + sway), (372 + sway, 89 + sway), INK);
        crate_box(c, 361 + sway, 89 + sway, 12, 8);
    }
}

fn crate_box(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    c.rect(x, y, w, h, 0x9a7353);
    c.rect(x, y, w, 1, 0xe0aa75);
    c.rect(x, y, 1, h, 0xc09669);
    c.rect(x + w - 2, y + 1, 2, h - 1, 0x745c49);
    c.rect(x, y + h - 2, w, 2, 0x785e4a);
    c.line((x + 2, y + 1), (x + w - 4, y + h - 3), 0xc39a6c);
    c.line((x + 2, y + h - 3), (x + w - 4, y + 1), 0xb58a61);
}

fn worker(c: &mut Canvas, x: i32, feet: i32, coat: Color, foreman: bool) {
    c.rect(x, feet - 7, 3, 2, if foreman { GOLD } else { 0x744f41 });
    c.rect(x, feet - 5, 2, 2, 0xdeae80);
    c.rect(x - 1, feet - 3, 4, 3, coat);
    c.rect(x - 1, feet, 1, 2, TAR);
    c.rect(x + 1, feet, 1, 2, TAR);
}

fn yard(c: &mut Canvas, tick: u32, units: [u32; 5], managed: u8, running: u8) {
    // Wool sheds, roller doors and timber piles.
    for (x, y, w) in [
        (354, 113, 56),
        (400, 116, 74),
        (427, 120, 62),
        (390, 124, 40),
        (409, 129, 27),
        (443, 132, 52),
        (457, 138, 28),
    ] {
        c.rect(x, y, w, 1, 0x6c8680);
        c.rect(x + 3, y + 2, w / 2, 1, 0x6c8379);
    }
    boathouse(c, 431, 105, 49, 21);
    boathouse(c, 382, 107, 22, 12);
    c.rect(457, 66, 5, 13, 0x866d60);
    c.rect(456, 65, 7, 2, 0xb88970);
    c.rect(440, 97, 14, 8, 0x465b54);
    for y in [98, 100, 102] {
        c.rect(440, y, 14, 1, 0x899279);
    }
    crane(c, tick, units[3] > 0 && running & 8 != 0);
    c.rect(355, 109, 125, 2, 0xc69b73);
    c.rect(356, 111, 124, 5, 0x465c56);
    for x in (360..480).step_by(13) {
        c.rect(x, 114, 3, 6, 0x4c635d);
        c.line((x + 3, 116), (x + 11, 113), 0x4c635d);
        c.rect(x - 1, 120, 5, 1, 0x819184);
    }
    for (x, y, w, h) in [
        (431, 102, 9, 7),
        (441, 103, 8, 6),
        (431, 97, 7, 5),
        (464, 102, 11, 7),
    ] {
        crate_box(c, x, y, w, h);
    }
    if units[3] > 0 {
        // The rebuild has ribbed hull plating and a ways frame.
        for x in (340..400).step_by(8) {
            c.line((x, 117), (x + 3, 108), 0x9d8062);
        }
        c.shape(
            &[
                (334, 101),
                (346, 104),
                (391, 104),
                (399, 99),
                (392, 110),
                (349, 114),
            ],
            0x6e554c,
        );
        c.line((334, 101), (346, 104), 0xe4a16c);
        c.line((347, 104), (391, 104), 0xc79973);
        for x in (350..391).step_by(6) {
            c.line((x, 105), (x + 2, 110), 0x917863);
        }
        worker(c, 396, 106, RED, managed & 8 != 0);
        if running & 8 != 0 && tick % 58 < 14 {
            for (dx, dy) in [(0, 0), (-2, 1), (2, -2), (4, 1)] {
                c.pixel(393 + dx, 104 + dy, IVORY);
            }
        }
    }
}

fn pennant(c: &mut Canvas, x: i32, y: i32, tick: u32) {
    let dy = if tick / 9 % 3 == 0 { 1 } else { 0 };
    c.shape(&[(x, y), (x + 7, y + dy), (x, y + 3)], RED);
}

fn hull(c: &mut Canvas, x: i32, y: i32, w: i32, depth: i32, top: Color, bottom: Color) {
    c.shape(
        &[
            (x, y),
            (x + w, y),
            (x + w - depth - 1, y + depth),
            (x + depth, y + depth),
        ],
        bottom,
    );
    c.rect(x + 1, y, w - 1, 2, top);
    c.line((x + depth, y + depth), (x + w - depth - 1, y + depth), TAR);
}

fn liner(c: &mut Canvas, tick: u32, working: bool, foreman: bool) {
    let x = 79 + if working { triangle(tick / 8, 54) } else { 0 };
    let y = 111;
    c.rect(x + 7, y + 7, 81, 1, 0x71867f);
    c.rect(x + 15, y + 9, 68, 1, 0x829489);
    c.rect(x + 9, y - 4, 71, 4, 0xf5d7a7);
    c.rect(x + 18, y - 8, 54, 4, 0xe8c8a0);
    c.rect(x + 25, y - 11, 41, 3, 0xf1d3a4);
    c.rect(x + 21, y - 9, 49, 1, 0x7d8270);
    for xx in (12..79).step_by(4) {
        c.rect(x + xx, y - 3, 2, 1, INK);
    }
    for xx in (22..69).step_by(4) {
        c.rect(x + xx, y - 7, 2, 2, 0x6e7f74);
    }
    for xx in [32, 49] {
        c.shape(
            &[
                (x + xx, y - 11),
                (x + xx + 6, y - 11),
                (x + xx + 4, y - 21),
                (x + xx - 1, y - 21),
            ],
            RED,
        );
        c.rect(x + xx - 1, y - 21, 6, 3, TAR);
    }
    if working {
        for i in 0..9 {
            let life = ((tick / 6 + i * 4) % 37) as i32;
            let sz = 1 + life / 10;
            let col = if life < 11 { 0x8e877d } else { 0xb09a86 };
            c.rect(x + 35 - life, y - 23 - life / 4, sz + 2, sz, col);
        }
    }
    c.rect(x + 79, y - 18, 1, 18, SLATE);
    pennant(c, x + 79, y - 18, tick);
    c.line((x + 80, y - 16), (x + 90, y), SLATE);
    hull(c, x, y, 94, 6, INK, 0x704b43);
    for xx in (13..83).step_by(7) {
        c.pixel(x + xx, y + 3, 0xf9c38a);
    }
    if foreman {
        c.rect(x + 64, y - 10, 3, 2, GOLD);
    }
}

fn ferry(c: &mut Canvas, tick: u32, working: bool, foreman: bool) {
    let x = 235 + if working { triangle(tick / 5, 54) } else { 30 };
    let y = 133 + (tick / 54 % 2) as i32;
    for (dx, dy, w) in [(-7, 5, 53), (-1, 8, 39), (4, 10, 23)] {
        c.rect(x + dx, y + dy, w, 1, 0x698883);
    }
    if working {
        c.rect(x - 13, y + 4, 18, 1, 0xe2c49c);
        c.rect(x - 23, y + 6, 14, 1, 0xc2b89b);
    }
    c.rect(x + 8, y - 6, 36, 6, IVORY);
    c.rect(x + 17, y - 11, 22, 5, 0xe5c79b);
    c.rect(x + 15, y - 12, 25, 2, INK);
    for xx in [11, 17, 23, 29, 35, 40] {
        c.rect(x + xx, y - 5, 3, 3, 0x426764);
    }
    c.rect(x + 30, y - 10, 7, 3, 0x617e75);
    c.rect(x + 21, y - 10, 6, 2, 0xb69e77);
    c.rect(x + 27, y - 17, 3, 5, RED);
    c.rect(x + 27, y - 18, 3, 2, TAR);
    c.rect(x + 10, y - 15, 1, 10, SLATE);
    pennant(c, x + 10, y - 15, tick);
    hull(c, x, y, 52, 6, 0x506960, PAINT);
    c.rect(x + 5, y + 2, 42, 1, IVORY);
    if foreman {
        c.pixel(x + 34, y - 9, GOLD);
    }
}

fn rowing(c: &mut Canvas, tick: u32, working: bool, foreman: bool, quantity: u32) {
    let x = if working {
        112 + triangle(tick / 3, 90)
    } else {
        126
    };
    let y = 148 + (tick / 45 % 2) as i32;
    if quantity >= 10 {
        c.rect(103, 132, 15, 1, 0xdab48d);
        hull(c, 101, 133, 22, 3, 0x926d56, INK);
        c.line((110, 131), (121, 137), 0x7b735a);
    }
    c.rect(x + 1, y + 5, 32, 1, 0x5f8076);
    c.rect(x + 6, y + 8, 19, 1, 0x738c7f);
    if working {
        c.rect(x - 6, y + 4, 13, 1, 0xd8bc96);
    }
    let oar = if working {
        triangle(tick / 3, 8) - 4
    } else {
        -2
    };
    c.line((x + 14, y - 2), (x + 5 - oar, y - 4), 0x926e51);
    c.rect(x + 12, y - 8, 4, 2, 0x65594e);
    c.rect(x + 12, y - 9, 3, 1, if foreman { GOLD } else { PAINT });
    c.rect(x + 13, y - 6, 3, 2, 0xf4c999);
    c.shape(
        &[
            (x + 11, y - 4),
            (x + 14, y - 5),
            (x + 16, y - 1),
            (x + 10, y - 1),
        ],
        RED,
    );
    c.rect(x + 14, y - 3, 4, 1, IVORY);
    c.rect(x + 5, y - 1, 24, 2, 0xce9e70);
    hull(c, x, y, 34, 4, 0x755748, TAR);
    c.rect(x + 5, y + 2, 24, 1, 0xa87753);
    c.line((x + 15, y - 3), (x + 24 + oar, y + 4), 0xe6bc83);
    c.line((x + 24 + oar, y + 4), (x + 27 + oar, y + 5), IVORY);
}

fn lobster_pots(c: &mut Canvas, tick: u32, managed: bool, working: bool) {
    for (i, (x, y)) in [(202, 153), (215, 149), (225, 155), (237, 150)]
        .into_iter()
        .enumerate()
    {
        let y = y + ((tick / 28 + i as u32) % 2) as i32;
        c.rect(x - 2, y + 2, 7, 1, 0x68857b);
        c.rect(x, y - 2, 3, 4, RED);
        c.rect(x, y - 2, 3, 1, IVORY);
    }
    c.rect(244, 161, 22, 1, 0x4f7571);
    hull(c, 241, 155, 27, 4, 0x738977, INK);
    crate_box(c, 245, 150, 9, 5);
    worker(c, 258, 153, if working { RED } else { SLATE }, managed);
    c.line((263, 154), (265, 146), INK);
    c.line((265, 146), (268, 153), 0x9e9474);
}

fn tug(c: &mut Canvas, tick: u32) {
    // A riveted harbor tug rides at its moorings.
    let y = 160 + (tick / 49 % 2) as i32;
    for (x, dy, w) in [
        (343, 8, 79),
        (346, 10, 69),
        (359, 12, 59),
        (354, 15, 20),
        (384, 14, 28),
    ] {
        c.rect(x, y + dy, w, 1, 0x4e726b);
    }
    c.rect(362, y - 15, 40, 14, 0xe2bd8d);
    c.rect(362, y - 15, 6, 14, 0xb39571);
    c.rect(361, y - 16, 43, 2, 0x6a5c4c);
    c.rect(367, y - 25, 25, 9, 0xf3d3a0);
    c.rect(366, y - 26, 28, 2, TAR);
    c.rect(368, y - 27, 24, 1, 0xb96c50);
    for x in [370, 375, 380, 385] {
        c.rect(x, y - 23, 3, 5, 0x3f625d);
        c.pixel(x, y - 23, 0xa6ac8e);
    }
    c.rect(362, y - 12, 5, 4, 0x647b6d);
    c.rect(370, y - 12, 5, 4, 0x5d756b);
    c.rect(381, y - 12, 6, 10, 0x805f49);
    c.rect(382, y - 11, 4, 4, 0x486960);
    c.pixel(385, y - 5, IVORY);
    c.rect(393, y - 11, 6, 3, INK);
    c.rect(398, y - 32, 8, 16, 0xc36e50);
    c.rect(398, y - 32, 2, 16, 0xd88a60);
    c.rect(398, y - 30, 8, 3, 0xebc796);
    c.rect(397, y - 34, 10, 4, TAR);
    c.rect(398, y - 34, 9, 1, 0x667568);
    c.line((406, y - 17), (408, y - 33), INK);
    c.rect(407, y - 34, 3, 2, 0x8e7862);
    c.rect(352, y - 35, 2, 34, 0x705e4a);
    pennant(c, 353, y - 35, tick);
    c.line((352, y - 31), (337, y - 2), 0x626d60);
    c.line((354, y - 29), (367, y - 10), 0x726f5d);
    // Rail stanchions and a low stern winch.
    c.rect(338, y - 5, 24, 1, 0x687766);
    for x in [338, 345, 353, 360] {
        c.rect(x, y - 5, 1, 5, INK);
    }
    for x in [406, 414, 423] {
        c.rect(x, y - 6, 1, 6, INK);
    }
    c.rect(405, y - 6, 18, 1, INK);
    c.rect(411, y - 4, 7, 4, 0x977752);
    c.shape(
        &[
            (334, y - 2),
            (352, y),
            (410, y),
            (429, y - 5),
            (423, y + 3),
            (410, y + 9),
            (349, y + 8),
        ],
        TAR,
    );
    c.shape(
        &[
            (344, y + 4),
            (412, y + 5),
            (423, y + 1),
            (418, y + 6),
            (409, y + 9),
            (349, y + 8),
        ],
        0x9b5042,
    );
    c.line((334, y - 2), (351, y), 0xe4bb85);
    c.line((351, y), (410, y), 0xe4bb85);
    c.line((410, y), (429, y - 5), 0xe4bb85);
    c.line((350, y + 9), (410, y + 10), 0x4d6b62);
    for x in [353, 358, 363, 392, 398, 404] {
        c.pixel(x, y + 2, 0xb2a181);
    }
    for x in [348, 373, 404] {
        c.disc(x, y + 1, 2, 0x243e3f);
        c.pixel(x, y + 1, 0xa88d69);
    }
    c.line((425, y - 4), (447, y - 6), 0x635e4f);
}

fn shore_pier(c: &mut Canvas, tick: u32, units: [u32; 5], managed: u8, sel: usize) {
    // Foreshortened planks, diagonal bracing and foreground bollards.
    for &(x, y) in &[(9, 164), (29, 155), (56, 147), (78, 138)] {
        c.rect(x, y, 4, 24, 0x344f4d);
        c.rect(x + 1, y + 2, 1, 17, 0x667262);
        c.line((x + 3, y + 20), (x + 22, y + 1), 0x45615a);
        c.rect(x - 1, y + 25, 10, 1, 0x567c72);
    }
    c.shape(&[(0, 151), (74, 126), (95, 134), (0, 173)], 0xb79369);
    c.shape(&[(0, 173), (95, 134), (95, 139), (0, 180)], 0x504e42);
    c.line((0, 172), (95, 134), 0xecbb83);
    for i in 0..18 {
        let x = 88 - i * 6;
        let y = 136 + i * 39 / 16;
        c.line((x, y), (x - 19, y - 8), 0x826e52);
        c.line((x - 1, y), (x - 16, y - 6), 0xd3aa76);
    }
    c.line((0, 151), (74, 126), 0xd6b182);
    for (x, y) in [(10, 165), (36, 155), (64, 144), (89, 134)] {
        c.rect(x - 2, y - 7, 5, 9, 0x685c47);
        c.rect(x - 1, y - 6, 1, 6, 0x9e8461);
        c.rect(x - 3, y - 8, 6, 2, 0xc9a57b);
    }
    c.line((12, 160), (21, 160), 0x856f50);
    c.line((21, 160), (36, 151), 0x856f50);
    c.line((38, 151), (49, 150), 0x856f50);
    c.line((49, 150), (64, 140), 0x856f50);
    crate_box(c, 13, 143, 13, 9);
    crate_box(c, 22, 140, 10, 8);
    crate_box(c, 15, 138, 9, 6);
    c.rect(41, 139, 6, 6, 0x815c44);
    c.rect(41, 139, 6, 1, IVORY);
    c.rect(41, 143, 6, 1, INK);
    if units[0] > 0 {
        worker(c, 67, 132, 0x57756c, managed & 1 != 0);
        if sel == 0 || managed & 1 != 0 {
            c.pixel(68, 125, GOLD);
        }
    }
    // An iron ladder on the short landing.
    c.line((74, 144), (74, 156), INK);
    c.line((78, 142), (78, 154), INK);
    for y in [146, 149, 152] {
        c.line((74, y), (78, y - 2), SLATE);
    }

    // Quayside signal post, its gantry and a life ring.
    c.shape(&[(439, 153), (480, 146), (480, 160), (434, 169)], 0xb2926b);
    c.shape(&[(434, 169), (480, 160), (480, 169), (434, 177)], 0x625d4a);
    for x in (438..486).step_by(6) {
        let y = 168 - (x - 434) / 5;
        c.line((x, y), (x + 3, y - 14), 0x8c7759);
    }
    c.line((434, 169), (479, 160), 0xd4aa74);
    c.rect(444, 168, 5, 18, DEEP);
    c.rect(445, 171, 1, 14, 0x83826a);
    c.rect(469, 164, 6, 18, DEEP);
    c.line((448, 180), (469, 167), 0x48635b);
    crate_box(c, 461, 146, 12, 10);
    crate_box(c, 470, 142, 12, 11);
    c.rect(448, 125, 3, 38, TAR);
    c.rect(449, 128, 1, 32, 0xa49370);
    c.rect(446, 124, 7, 2, INK);
    c.rect(447, 119, 5, 5, 0xe2b36f);
    c.rect(448, 120, 3, 3, IVORY);
    c.rect(446, 118, 7, 2, INK);
    c.rect(448, 116, 3, 2, INK);
    c.disc(448, 166, 4, RED);
    c.rect(447, 162, 3, 2, IVORY);
    c.rect(444, 165, 2, 2, IVORY);
    c.rect(447, 169, 3, 2, IVORY);
    c.rect(451, 165, 2, 2, IVORY);
    c.disc(448, 166, 2, 0x5f5c4a);
    if units[2] > 0 {
        worker(c, 457, 153, RED, managed & 4 != 0);
    }

    // Two small navigation marks define the channel.
    for (x, y) in [(75, 115), (313, 111)] {
        let bob = (tick / 60 % 2) as i32;
        c.rect(x - 2, y + bob, 5, 2, 0x925f4d);
        c.rect(x, y - 5 + bob, 1, 5, INK);
        c.pixel(x, y - 6 + bob, 0xffcd85);
    }
}

// The Night Post kingfisher carries the same red scarf and mail satchel
// as the pilot on the open-water route. Only the wings and scarf flutter.
fn mail_pilot(c: &mut Canvas, tick: u32) {
    let phase = tick % 750;
    if phase >= 150 {
        return;
    }
    let x = -12 + phase as i32 * 504 / 150;
    let y = 47 + triangle(phase / 3, 4);
    let wing = [-7, -4, 0, 4, 6, 3, 0, -4][(phase / 2 % 8) as usize];
    let blue = 0x347e81;
    let glint = 0x8cc9b4;
    // Wedge-shaped tail, stocky body, pointed bill, broad white collar.
    c.shape(
        &[(x - 9, y), (x - 4, y), (x - 2, y + 1), (x - 6, y + 2)],
        INK,
    );
    c.line((x - 8, y), (x - 4, y + 1), blue);
    c.shape(
        &[
            (x - 5, y),
            (x - 1, y - 2),
            (x + 3, y - 2),
            (x + 5, y),
            (x + 1, y + 4),
            (x - 3, y + 3),
        ],
        INK,
    );
    c.shape(
        &[
            (x - 4, y),
            (x - 1, y - 2),
            (x + 4, y - 2),
            (x + 2, y + 1),
            (x - 2, y + 2),
        ],
        blue,
    );
    c.shape(
        &[(x - 1, y + 1), (x + 5, y - 1), (x + 3, y + 2), (x, y + 3)],
        0xdb8655,
    );
    c.shape(
        &[
            (x + 1, y - 3),
            (x + 3, y - 6),
            (x + 6, y - 6),
            (x + 8, y - 4),
            (x + 7, y - 2),
            (x + 3, y),
        ],
        blue,
    );
    c.line((x + 3, y - 6), (x + 6, y - 6), glint);
    c.line((x + 4, y - 1), (x + 7, y - 3), IVORY);
    c.rect(x + 6, y - 4, 2, 1, TAR);
    c.line((x + 8, y - 3), (x + 13, y - 3), TAR);
    c.rect(x + 8, y - 2, 3, 1, 0xd3985f);
    // One moving scarf tail remains visible above the wing.
    let scarf = triangle(phase / 3, 2);
    c.line((x + 2, y - 1), (x - 5, y - 3), RED);
    c.line((x - 5, y - 3), (x - 10, y - 2 - scarf), PAINT);
    c.shape(
        &[
            (x, y),
            (x - 4, y - 1),
            (x - 8, y + wing),
            (x - 5, y + wing - 1),
        ],
        INK,
    );
    c.line((x - 6, y + wing), (x - 2, y - 1), blue);
    c.pixel(x - 6, y + wing, glint);
    // Cream envelope and its oxblood leather pouch.
    c.line((x + 2, y), (x - 1, y + 4), 0x77523e);
    c.rect(x - 4, y + 3, 6, 4, 0x784938);
    c.rect(x - 3, y + 2, 4, 2, IVORY);
    c.rect(x - 3, y + 5, 4, 1, 0xb37a51);
    c.pixel(x - 1, y + 4, GOLD);
}

/// Paint the harbor into an RGB framebuffer. The clock is in 1/30-second ticks.
/// Shop bits follow the rowboat, pots, ferry, dry dock and liner, in that order.
pub fn draw_harbor(
    buf: &mut [u32; ART_W * ART_H],
    tick: u32,
    units: [u32; 5],
    managed: u8,
    running: u8,
    selected: usize,
) {
    let mut c = Canvas(buf);
    sky(&mut c, tick);
    sea(&mut c, tick);
    light_station(&mut c, tick);
    if units[4] > 0 {
        liner(&mut c, tick, running & 16 != 0, managed & 16 != 0);
    } else {
        // A small gaff sail, silhouetted against the bright channel.
        let x = 181 + triangle(tick / 20, 9);
        c.rect(x, 92, 1, 16, 0x8c8270);
        c.shape(&[(x - 1, 92), (x - 1, 105), (x - 9, 105)], 0xf8d8a7);
        c.shape(&[(x + 2, 96), (x + 6, 105), (x + 2, 105)], 0xc38e70);
        hull(&mut c, x - 10, 107, 18, 2, 0x847b65, INK);
        c.rect(x - 6, 111, 12, 1, 0xc3ab89);
    }
    yard(&mut c, tick, units, managed, running);
    if units[2] > 0 {
        ferry(&mut c, tick, running & 4 != 0, managed & 4 != 0);
    }
    if units[0] > 0 {
        rowing(&mut c, tick, running & 1 != 0, managed & 1 != 0, units[0]);
    }
    if units[1] > 0 {
        lobster_pots(&mut c, tick, managed & 2 != 0, running & 2 != 0);
    }
    tug(&mut c, tick);
    shore_pier(&mut c, tick, units, managed, selected);
    for (i, (x, y)) in [(91, 39), (104, 33), (116, 42), (284, 43), (294, 39)]
        .into_iter()
        .enumerate()
    {
        let drift = triangle(tick / (9 + 2 * i as u32), 14) - 7;
        let bob = triangle(tick / (14 + i as u32), 2);
        gull(
            &mut c,
            x + drift,
            y + bob,
            (tick / 8 + i as u32) % 5,
            if i < 3 { INK } else { 0x7b7a6e },
        );
    }
    mail_pilot(&mut c, tick);
}

/// Launched hulls return to the harbor, in the order recorded by the shipyard.
pub fn draw_home_fleet(buf: &mut [u32; ART_W * ART_H], tick: u32, completed: u8) {
    let mut c = Canvas(buf);
    if completed & 4 != 0 {
        // Northstar keeps a fixed station at the mouth of the channel.
        let x = 315;
        let y = 108 + (tick / 72 % 2) as i32;
        c.rect(x - 5, y + 4, 13, 1, 0x86978a);
        c.line((x - 2, y - 1), (x, y - 6), SLATE);
        c.line((x + 3, y - 1), (x + 1, y - 6), SLATE);
        c.rect(x - 1, y - 8, 4, 3, 0xaa8b70);
        c.rect(x, y - 9, 2, 1, PAINT);
        if tick % 106 < 17 {
            c.rect(x - 2, y - 7, 6, 1, 0xedc48d);
            c.rect(x, y - 8, 2, 2, 0xffe4ad);
        }
        c.rect(x - 5, y - 2, 3, 2, 0xd3bba0);
        hull(&mut c, x - 8, y, 16, 2, 0xaa6856, 0x775f55);
        c.pixel(x + 3, y, IVORY);
    }
    if completed & 1 != 0 {
        // Pilgrim carries her ochre gaff sail on a short reach across the bay.
        let x = 205 + triangle(tick / 13, 15);
        let y = 130 + (tick / 62 % 2) as i32;
        c.rect(x - 12, y + 3, 13, 1, 0xb1ab8e);
        c.rect(x, y - 13, 1, 13, 0x736d5b);
        c.shape(
            &[
                (x + 1, y - 8),
                (x + 5, y - 11),
                (x + 8, y - 2),
                (x + 1, y - 1),
            ],
            0xd6a465,
        );
        c.line((x + 1, y - 8), (x + 5, y - 11), 0x9a7958);
        c.shape(&[(x - 1, y - 10), (x - 2, y - 2), (x - 7, y - 2)], 0xe9c99b);
        hull(&mut c, x - 9, y, 18, 2, 0x8b7861, INK);
    }
    if completed & 2 != 0 {
        // Juniper's raked funnel and oxblood mail hull pass below the liner's lane.
        let x = 146 + triangle(tick / 17, 16);
        let y = 131 + (tick / 55 % 2) as i32;
        c.rect(x - 13, y + 4, 13, 1, 0xc0b393);
        c.rect(x + 6, y - 9, 1, 9, SLATE);
        c.rect(x + 7, y - 9, 3, 1, RED);
        c.line((x + 6, y - 7), (x + 10, y - 1), 0xb1a68e);
        c.line((x - 3, y - 8), (x - 2, y - 4), PAINT);
        c.line((x - 2, y - 8), (x - 1, y - 4), RED);
        c.rect(x - 3, y - 8, 2, 1, INK);
        c.rect(x + 1, y - 5, 4, 2, 0xe9cca2);
        c.rect(x - 7, y - 3, 13, 3, 0xd6bb94);
        for dx in [-5, -2, 1, 4] {
            c.pixel(x + dx, y - 2, SLATE);
        }
        hull(&mut c, x - 10, y, 21, 2, 0xb47258, 0x8f6656);
        c.rect(x - 6, y, 10, 1, 0xcbab82);
    }
}

/// The room behind the quay counter changes hands at the end of The Second Manifest.
pub fn draw_quay_room(buf: &mut [u32; ART_W * ART_H], tick: u32, ending: crate::manifest::Ending) {
    use crate::manifest::Ending;
    if ending == Ending::None {
        return;
    }
    // The rent-free room occupies the bottom of the big shed beside the crane.
    let mut c = Canvas(buf);
    let x = 431;
    c.rect(x + 1, 94, 40, 14, 0x8c7b60);
    c.rect(x + 3, 95, 36, 11, TAR);
    c.rect(x + 4, 96, 34, 9, 0x53645b);
    c.rect(x + 6, 98, 30, 6, 0x495a50);
    c.rect(x + 2, 106, 38, 2, 0xd9a776);
    c.rect(x + 1, 108, 39, 2, 0x725b48);
    c.rect(x + 2, 110, 39, 1, 0xa78c67);
    match ending {
        Ending::Print => {
            // The green iron platen sits left; fresh broadsheets air on a line.
            c.shape(&[(x, 94), (x + 4, 90), (x + 38, 90), (x + 41, 94)], DEEP);
            c.line((x + 3, 91), (x + 38, 91), 0x739584);
            c.rect(x, 94, 42, 2, 0x314e49);
            c.rect(x + 1, 95, 39, 1, 0xafb59b);
            c.rect(x + 4, 96, 1, 10, 0x987b59);
            c.rect(x + 6, 97, 2, 8, 0x253f42);
            c.rect(x + 8, 97, 7, 2, 0x729c89);
            c.rect(x + 12, 98, 2, 4, 0x354d46);
            let press = triangle(tick / 9, 2);
            c.rect(x + 9, 100 + press, 7, 2, 0x263f42);
            c.rect(x + 8, 104, 10, 1, IVORY);
            c.line((x + 13, 99), (x + 19, 96 + press), 0xc89a66);
            worker(&mut c, x + 18, 105, 0x66867b, false);
            c.line((x + 17, 102), (x + 16, 100 + press), 0xd2a779);
            c.line((x + 22, 98), (x + 38, 96), 0xac9d7f);
            let fold = triangle(tick / 12, 2);
            for (k, y) in [(23, 99), (29, 98), (35, 98)] {
                c.shape(
                    &[
                        (x + k, y),
                        (x + k + 4, y),
                        (x + k + 3 + fold, y + 5),
                        (x + k, y + 4),
                    ],
                    0xe7d9b3,
                );
                c.rect(x + k + 1, y + 1, 3, 1, 0x6e796b);
                c.rect(x + k + 1, y + 3, 2, 1, 0x9c9c86);
                c.pixel(x + k + 2, y - 1, 0xb99059);
            }
            // The customer at the counter takes the printer's latest copy.
            let hand = triangle(tick / 5, 4);
            worker(&mut c, x + 25, 109, 0x7b695b, false);
            c.rect(x + 19 + hand, 103, 5, 3, IVORY);
            c.rect(x + 20 + hand, 104, 3, 1, 0x889587);
        }
        Ending::Kitchen => {
            // The red enamel canopy opens toward the pier, past the stove flue.
            c.rect(x + 2, 86, 3, 10, 0x695e50);
            c.rect(x + 1, 85, 5, 2, TAR);
            c.shape(
                &[(x - 1, 94), (x + 3, 90), (x + 37, 90), (x + 42, 94)],
                PAINT,
            );
            c.line((x + 4, 90), (x + 36, 90), 0xe99a68);
            for off in [7, 16, 25, 34] {
                c.line((x + off, 91), (x + off - 1, 94), 0xdf916b);
            }
            c.rect(x - 1, 94, 43, 2, 0xab503e);
            c.rect(x, 96, 41, 1, 0xe5ba89);
            c.rect(x + 6, 101, 9, 5, INK);
            c.rect(x + 8, 103, 5, 2, PAINT);
            c.pixel(x + 10, 103, GOLD);
            c.shape(
                &[(x + 4, 99), (x + 16, 99), (x + 13, 102), (x + 7, 102)],
                0x273f42,
            );
            c.rect(x + 5, 99, 11, 1, 0xd09e63);
            c.rect(x + 7, 98, 7, 1, 0xa7a98c);
            for n in 0..3 {
                let age = (tick / 4 + n * 4) % 12;
                let drift = triangle((tick / 5 + n * 3) % 10, 2);
                let xx = x + 2 + drift + age as i32 / 4;
                let yy = 84 - age as i32;
                c.rect(
                    xx,
                    yy,
                    2 + age as i32 / 5,
                    1,
                    if age > 7 { 0xc3aa8d } else { 0xd6c5a2 },
                );
            }
            worker(&mut c, x + 20, 105, 0xd9d0ad, false);
            c.rect(x + 19, 97, 4, 2, IVORY);
            c.rect(x + 20, 96, 2, 1, 0xf0d5ac);
            worker(&mut c, x + 32, 109, 0x74887a, false);
            let reach = triangle(tick / 5, 6);
            c.line((x + 21, 102), (x + 23 + reach, 103), 0xdaae82);
            c.rect(x + 22 + reach, 102, 5, 1, IVORY);
            c.rect(x + 23 + reach, 103, 3, 1, 0xc99a71);
            c.rect(x + 34, 104, 5, 1, IVORY);
            c.rect(x + 35, 105, 3, 1, 0xae8164);
        }
        Ending::None => {}
    }
}

mod watch;
pub use watch::{draw_harbor_watch, quay_contact};

mod tea;
pub use tea::draw_tea_committee;
