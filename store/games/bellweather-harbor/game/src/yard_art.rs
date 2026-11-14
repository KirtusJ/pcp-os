//! Daylight at the Bellweather launch ways. All dimensions are in native pixels.
pub const YARD_W: usize = 360;
pub const YARD_H: usize = 112;

type Frame = [u32; YARD_W * YARD_H];
type Point = (i32, i32);
const TAR: u32 = 0x223e46;
const BLUE: u32 = 0x426672;
const STEEL: u32 = 0x62818a;
const BRASS: u32 = 0xd7a259;
const OAK: u32 = 0xa77954;
const BARK: u32 = 0x73594b;
const CORAL: u32 = 0xc9684d;

struct Paint<'a>(&'a mut Frame);

impl Paint<'_> {
    fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, ink: u32) {
        let left = x.clamp(0, YARD_W as i32) as usize;
        let right = (x + w).clamp(0, YARD_W as i32) as usize;
        if left >= right {
            return;
        }
        for row in y.max(0)..(y + h).min(YARD_H as i32) {
            let offset = row as usize * YARD_W;
            self.0[offset + left..offset + right].fill(ink);
        }
    }
    fn dot(&mut self, x: i32, y: i32, ink: u32) {
        if (x as u32) < YARD_W as u32 && (y as u32) < YARD_H as u32 {
            self.0[y as usize * YARD_W + x as usize] = ink;
        }
    }
    fn line(&mut self, a: Point, b: Point, ink: u32) {
        let (mut x, mut y) = a;
        let dx = (b.0 - x).abs();
        let dy = -(b.1 - y).abs();
        let sx = if x < b.0 { 1 } else { -1 };
        let sy = if y < b.1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.dot(x, y, ink);
            if (x, y) == b {
                break;
            }
            let twice = 2 * err;
            if twice >= dy {
                err += dy;
                x += sx;
            }
            if twice <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    fn poly(&mut self, ps: &[Point], ink: u32) {
        // The polygons are convex; intersect each edge with the pixel row.
        let top = ps.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let bottom = ps
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap_or(-1)
            .min(YARD_H as i32 - 1);
        for y in top..=bottom {
            let (mut left, mut right) = (i32::MAX, i32::MIN);
            let mut a = ps[ps.len() - 1];
            for &b in ps {
                if y >= a.1.min(b.1) && y <= a.1.max(b.1) {
                    if a.1 == b.1 {
                        left = left.min(a.0.min(b.0));
                        right = right.max(a.0.max(b.0));
                    } else {
                        let x = a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1);
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
                a = b;
            }
            if left <= right {
                self.bar(left, y, right - left + 1, 1, ink);
            }
        }
    }
}

fn mix(a: u32, b: u32, n: u32, d: u32) -> u32 {
    let channel =
        |shift: u32| ((((a >> shift) & 255) * (d - n) + ((b >> shift) & 255) * n) / d) << shift;
    channel(16) | channel(8) | channel(0)
}

fn noise(i: u32) -> u32 {
    let a = i.wrapping_mul(0x9e3779b9);
    let b = (a ^ (a >> 16)).wrapping_mul(0x85ebca6b);
    b ^ (b >> 13)
}

fn triangle(t: u32, n: u32) -> i32 {
    let phase = t % (2 * n);
    phase.min(2 * n - phase) as i32
}

fn cloud(p: &mut Paint, x: i32, y: i32, shade: u32, lit: u32) {
    p.bar(x, y, 31, 2, shade);
    p.bar(x + 4, y - 2, 21, 2, shade);
    p.bar(x + 8, y - 5, 10, 4, shade);
    p.bar(x + 11, y - 6, 7, 1, lit);
    p.bar(x + 7, y - 3, 6, 1, lit);
    p.bar(x + 15, y - 5, 5, 2, lit);
    p.bar(x + 18, y - 3, 7, 2, lit);
    p.bar(x + 24, y - 1, 5, 1, lit);
    p.bar(x - 5, y + 2, 21, 1, lit);
}

fn coast(p: &mut Paint, tick: u32) {
    for y in 0..67 {
        p.bar(
            0,
            y,
            360,
            1,
            mix(0x739ea9, 0xe3dfbd, (y / 3 * 3) as u32, 66),
        );
    }
    let drift = triangle(tick / 120, 5);
    cloud(p, 44 + drift, 14, 0xcad4c5, 0xe4e4cc);
    cloud(p, 147 + drift, 24, 0xdfdfca, 0xfcf0d6);
    cloud(p, 164 + drift, 26, 0xd3d6bd, 0xe7e4cb);
    cloud(p, 220 - drift, 14, 0xcbd6c9, 0xf3ead2);
    cloud(p, 308 + drift, 21, 0xd8dcc7, 0xf6ecd4);
    p.bar(91 + drift, 18, 25, 1, 0xc3d2cb);
    p.bar(247 + drift, 16, 18, 1, 0xe5e8d1);
    let ridge = [
        (0, 39),
        (25, 34),
        (53, 46),
        (66, 45),
        (84, 49),
        (100, 46),
        (114, 49),
        (135, 47),
        (150, 54),
        (161, 51),
        (175, 56),
        (190, 50),
        (205, 50),
        (219, 46),
        (225, 47),
        (241, 40),
        (248, 41),
        (267, 29),
        (273, 31),
        (284, 25),
        (293, 28),
        (303, 24),
        (322, 37),
        (334, 34),
        (350, 41),
        (360, 39),
    ];
    for segment in ridge.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        p.poly(&[a, b, (b.0, 64), (a.0, 64)], 0x9caeaa);
    }
    for &(summit, shoulder, base) in &[
        ((267, 29), (272, 32), (252, 48)),
        ((284, 25), (290, 27), (265, 48)),
        ((303, 24), (312, 29), (287, 48)),
    ] {
        p.poly(&[summit, shoulder, base], 0xc9ccba);
        p.line(summit, (base.0 + 3, base.1 - 4), 0xd9d7c0);
    }
    let near = [
        (0, 49),
        (16, 47),
        (42, 53),
        (63, 49),
        (84, 49),
        (109, 57),
        (129, 56),
        (153, 61),
        (179, 60),
        (199, 56),
        (212, 57),
        (230, 53),
        (258, 56),
        (274, 50),
        (298, 53),
        (317, 47),
        (336, 49),
        (360, 42),
    ];
    for segment in near.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        p.poly(&[a, b, (b.0, 66), (a.0, 66)], 0x7b9996);
        for x in (a.0..b.0).step_by(3) {
            let y = a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0);
            let v = noise(x as u32 + 17);
            let h = (v % 5) as i32;
            p.line((x, y), (x, y - h), 0x819e98);
            if h > 2 {
                p.bar(x - 1, y - 1, 3, 2, 0x819e98);
            }
        }
    }
    p.bar(0, 64, 360, 2, 0xb1b8a5);
    for y in 66..112 {
        p.bar(
            0,
            y,
            360,
            1,
            mix(0x88b1a8, 0x427d80, ((y - 66) / 2 * 2) as u32, 46),
        );
    }
    for i in 0..266 {
        let h = noise(i + 221);
        let x = ((h >> 9) % 380) as i32 - 12 + triangle(tick / (29 + (h & 15)), 4);
        let y = 66 + (h % 46) as i32;
        let w = 1 + ((h >> 17) % (3 + (y - 66) as u32 / 3)) as i32;
        let ink = match (h >> 7) % 5 {
            0 if x > 180 => 0xbad1b6,
            1 => 0x639895,
            2 => 0x82a99d,
            3 => 0x9ab7a3,
            _ => 0x629396,
        };
        p.bar(x, y, w, 1, ink);
    }
    // A striped daymark and a tiny fishing launch beyond the ways.
    p.bar(332, 50, 2, 14, 0x667f7c);
    p.poly(&[(330, 53), (333, 48), (336, 53)], 0x9a7963);
    p.bar(329, 63, 8, 2, 0x74877d);
    p.bar(319, 60, 12, 1, 0x95a295);
    p.poly(&[(223, 65), (237, 65), (233, 68), (225, 67)], 0x617977);
    p.bar(228, 62, 5, 3, 0xe6d5ad);
    p.bar(230, 60, 1, 2, 0x8d8c76);
}

fn shed(p: &mut Paint) {
    // Sawmill with a sawtooth roof, glazed clerestory and a dark open door.
    p.poly(
        &[
            (0, 37),
            (8, 32),
            (8, 27),
            (31, 20),
            (31, 28),
            (56, 20),
            (56, 28),
            (79, 22),
            (85, 35),
            (85, 70),
            (0, 70),
        ],
        0x938d75,
    );
    p.poly(&[(0, 37), (10, 31), (80, 30), (85, 36)], 0x876c55);
    for x in [9, 33, 57] {
        p.poly(&[(x, 29), (x + 21, 21), (x + 21, 29)], 0x667d7b);
        for dx in [7, 14] {
            p.bar(x + dx, 28 - dx / 3, 1, 1 + dx / 3, 0xb5ab8b);
        }
        p.line((x - 2, 29), (x + 21, 20), TAR);
        p.line((x - 2, 28), (x + 21, 19), 0xbf9a74);
        p.bar(x + 20, 21, 2, 8, 0x5d6558);
    }
    p.bar(0, 35, 85, 3, 0x5b6053);
    for x in (0..83).step_by(4) {
        p.bar(x, 39, 1, 31, 0x707b69);
    }
    p.bar(0, 36, 84, 1, 0xd2a97c);
    p.bar(0, 48, 84, 1, 0x797f6a);
    for x in [3, 13, 23, 58, 68] {
        p.bar(x, 40, 8, 8, 0x647a72);
        p.bar(x + 1, 41, 6, 5, 0xacc1ae);
        p.bar(x + 2, 42, 4, 1, 0xc6c9aa);
        p.bar(x + 4, 40, 1, 7, 0x697c6f);
    }
    p.bar(34, 41, 21, 29, 0x716b55);
    p.bar(37, 44, 15, 26, TAR);
    p.bar(35, 43, 20, 1, 0xceab7a);
    p.bar(39, 45, 2, 21, 0x49615c);
    p.rect_boards(37, 64, 11, 2);
    p.bar(12, 53, 13, 13, 0xa1997b);
    p.line((13, 54), (23, 65), 0x6c7969);
    p.bar(2, 18, 6, 14, 0x927d64);
    p.bar(1, 17, 8, 2, 0x6e675b);
    p.bar(2, 21, 6, 1, 0xb09570);
}

impl Paint<'_> {
    fn rect_boards(&mut self, x: i32, y: i32, w: i32, n: i32) {
        for i in 0..n {
            let offset = if i % 2 == 0 { 0 } else { 2 };
            self.bar(x + offset, y - 3 * i, w, 3, BARK);
            self.bar(x + offset, y - 3 * i, w - 1, 1, 0xc29c70);
            self.bar(x + w - 5 + offset, y - 3 * i + 1, 2, 1, BRASS);
        }
    }
}

fn davit(p: &mut Paint, stage: u16) {
    // Tall travelling sheerlegs, pulley cheek plates and a double-purchase hook.
    p.poly(&[(50, 70), (55, 70), (61, 21), (58, 21)], 0x7d7e68);
    p.poly(&[(73, 70), (78, 70), (64, 18), (61, 18)], 0x707661);
    p.line((54, 68), (59, 26), 0xd3b57a);
    p.line((59, 22), (76, 68), 0xbca370);
    p.line((55, 56), (74, 66), 0x777e69);
    p.line((55, 66), (70, 52), 0x737c68);
    p.poly(&[(56, 25), (59, 20), (124, 5), (124, 8)], 0x757b69);
    p.line((57, 21), (124, 5), 0xd8b374);
    p.line((58, 18), (125, 4), 0x8e947d);
    for i in 0..9 {
        let x = 60 + i * 7;
        let y = 19 - i * 7 / 4;
        p.line((x, y + 3), (x + 5, y - 1), 0x747d6d);
    }
    p.line((55, 59), (62, 11), 0x667c76);
    p.line((62, 11), (124, 5), 0x6f827a);
    p.bar(123, 4, 4, 3, 0x485f5b);
    p.dot(124, 5, 0xd7be83);
    let hook = if stage > 0 { 22 } else { 30 };
    p.bar(124, 7, 1, hook - 7, 0x788b7f);
    p.bar(126, 7, 1, hook - 7, 0x9aa18b);
    p.bar(123, hook - 1, 5, 4, TAR);
    p.dot(125, hook, BRASS);
    p.line((126, hook + 3), (123, hook + 5), TAR);
    p.dot(122, hook + 4, TAR);
    if (350..780).contains(&stage) {
        p.line((124, hook + 5), (117, hook + 14), 0x807967);
        p.line((124, hook + 5), (137, hook + 14), 0x807967);
        p.bar(114, hook + 14, 29, 2, OAK);
        p.bar(115, hook + 14, 27, 1, 0xd2a473);
    }
}

fn ways(p: &mut Paint) {
    p.poly(
        &[
            (0, 65),
            (60, 64),
            (100, 70),
            (144, 79),
            (188, 88),
            (145, 94),
            (0, 94),
        ],
        0xa99f7e,
    );
    p.poly(
        &[(0, 83), (66, 77), (144, 84), (177, 90), (76, 95), (0, 99)],
        0x8c866b,
    );
    for i in 0..110 {
        let n = noise(i + 15);
        let x = (n % 152) as i32;
        let y = 66 + ((n >> 12) % 22) as i32;
        if y >= 66 + x / 9 && y < 91 {
            p.bar(x, y, 1 + ((n >> 8) % 3) as i32, 1, 0xbdb18e);
        }
    }
    // Rails run downhill to the right. Their ends disappear just below the surface.
    let rail = |x: i32| 64 + (x - 12) / 7;
    for x in (15..220).step_by(9) {
        let y = rail(x);
        p.poly(
            &[(x, y), (x + 3, y), (x - 5, y + 14), (x - 9, y + 14)],
            0x695f51,
        );
        p.line((x + 1, y), (x - 7, y + 13), 0xa78c69);
        p.dot(x - 5, y + 10, TAR);
    }
    for off in [2, 11] {
        p.line((8, 64 + off), (229, 95 + off), BARK);
        p.line((8, 65 + off), (228, 96 + off), BARK);
        p.line((8, 63 + off), (229, 94 + off), 0xc5a676);
        p.line((8, 64 + off), (229, 95 + off), 0x9a9d85);
    }
    for x in [95, 113, 132, 152, 169] {
        let y = rail(x);
        p.poly(&[(x - 6, y + 8), (x + 6, y + 10), (x + 1, y - 3)], 0x846c50);
        p.line((x - 5, y + 8), (x + 1, y - 3), 0xc19c6a);
    }
    p.bar(216, 97, 13, 1, 0x91b1a4);
    p.bar(207, 104, 20, 1, 0x92b2a3);
    p.bar(215, 106, 9, 1, 0xc3c9aa);
    // Quay stonework and an old workshop mooring.
    p.poly(
        &[(0, 93), (35, 87), (70, 89), (88, 97), (34, 109), (0, 110)],
        0x918b73,
    );
    p.poly(
        &[(0, 104), (34, 100), (87, 94), (87, 99), (35, 110), (0, 112)],
        0x586a61,
    );
    p.line((0, 102), (84, 93), 0xc4ac83);
    for x in [8, 22, 39, 53, 65, 77] {
        let y = 103 - x / 9;
        p.line((x, y), (x + 2, y + 4), 0x465d59);
    }
    p.bar(16, 91, 5, 8, TAR);
    p.bar(14, 90, 9, 3, 0x52685e);
    p.bar(16, 90, 5, 1, 0xc8b18a);
    p.line((18, 94), (32, 98), 0x94825c);
    p.line((32, 98), (45, 93), 0x94825c);
    p.rect_boards(2, 84, 27, 3);
    p.rect_boards(37, 87, 22, 2);
    // Striped boundary marker on the water side.
    p.bar(248, 76, 2, 8, 0x526e68);
    p.bar(247, 78, 4, 3, CORAL);
    p.bar(247, 81, 5, 2, 0xc2ac80);
    p.bar(245, 85, 7, 1, 0x76a29b);
}

fn worker(p: &mut Paint, x: i32, y: i32, hammer: bool, tick: u32) {
    p.bar(x - 2, y + 1, 6, 1, 0x797967);
    p.bar(x - 2, y - 4, 2, 5, TAR);
    p.bar(x + 1, y - 4, 2, 5, BLUE);
    p.bar(x - 2, y - 7, 5, 4, if hammer { CORAL } else { 0x607e7b });
    p.bar(x - 1, y - 10, 3, 3, 0xd6a078);
    p.bar(x - 2, y - 11, 5, 2, if hammer { BRASS } else { TAR });
    p.dot(x + 2, y - 8, 0xdbad83);
    if hammer {
        let lift = triangle(tick / 3, 4);
        p.line((x + 2, y - 6), (x + 5, y - 7 - lift), 0xd5b088);
        p.line((x + 5, y - 7 - lift), (x + 7, y - 10 - lift), BARK);
        p.bar(x + 6, y - 11 - lift, 4, 2, BLUE);
    } else {
        p.line((x + 2, y - 7), (x + 5, y - 4), 0xa39a7b);
    }
}

fn scaffold(p: &mut Paint, tick: u32, stage: u16) {
    let end = if stage >= 450 { 181 } else { 151 };
    for x in (83..end).step_by(17) {
        let y = 49;
        let foot = 73 + (x - 83) / 7;
        p.bar(x, y - 4, 2, foot - y + 4, 0x8d8a73);
        p.bar(x + 1, y - 4, 1, foot - y + 4, 0xd8c398);
        p.line((x + 2, foot - 2), (x + 15, y + 7), 0x908a6f);
        p.line((x + 2, y + 7), (x + 15, foot - 2), 0x9a9175);
    }
    p.bar(79, 53, end - 78, 2, 0xb39a70);
    p.bar(80, 53, end - 80, 1, 0xe5c28a);
    p.bar(82, 47, end - 82, 1, 0x8c8c73);
    p.line((90, 71), (97, 53), 0x928065);
    p.line((94, 71), (101, 53), 0x928065);
    for i in 0..5 {
        p.line((90 + i, 70 - 3 * i), (94 + i, 70 - 3 * i), 0xc0a47a);
    }
    if stage > 290 {
        worker(p, 101, 52, true, tick);
    }
}

struct Ship<'p, 'a> {
    p: &'p mut Paint<'a>,
    x: i32,
    y: i32,
    // Zero on the water, 1000 for the one-in-seven gradient of the launch rails.
    tilt: i32,
}
impl Ship<'_, '_> {
    fn point(&self, x: i32, y: i32) -> Point {
        (self.x + x, self.y + y + x * self.tilt / 7000)
    }
    fn line(&mut self, a: Point, b: Point, color: u32) {
        self.p
            .line(self.point(a.0, a.1), self.point(b.0, b.1), color);
    }
    fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        for i in 0..w {
            let (xx, yy) = self.point(x + i, y);
            self.p.bar(xx, yy, 1, h, color);
        }
    }
    fn poly(&mut self, const_points: &[Point], color: u32) {
        // Ship pieces contain at most eight points.
        let mut points = [(0, 0); 8];
        for (dest, &(x, y)) in points.iter_mut().zip(const_points) {
            *dest = self.point(x, y);
        }
        self.p.poly(&points[..const_points.len()], color);
    }
    fn flag(&mut self, x: i32, y: i32, tick: u32) {
        let dy = triangle(tick / 6, 1);
        self.poly(
            &[(x, y), (x + 7, y + dy), (x + 6, y + dy + 1), (x, y + 3)],
            CORAL,
        );
        self.line((x, y), (x + 3, y), 0xedac79);
    }
}

fn envelope(x: i32, w: i32, kind: usize) -> (i32, i32) {
    let scaled = x * 100 / w;
    let deck = -12 - (12 - scaled).max(0) / 6 - (scaled - 78).max(0) / 6;
    let base = match scaled {
        0..=6 => -10 + scaled,
        7..=17 => -4 + (scaled - 6) * 4 / 11,
        18..=76 => 1,
        77..=90 => 1 - (scaled - 76) / 3,
        _ => -3 - (scaled - 90) * 11 / 10,
    };
    let extra = if kind == 0 { 0 } else { 2 };
    (deck - extra, (base + extra).min(-1 + (100 - scaled) * 2))
}

fn hull(s: &mut Ship, kind: usize, progress: i32) {
    let w = [83, 101, 111][kind];
    let fraction = ((progress - 160) * 1000 / 490).clamp(0, 1000);
    let (paint, under) = match kind {
        0 => (0x38626a, 0xa4664e),
        1 => (0x416e69, 0x8a5344),
        _ => (0xbe654e, 0x883f39),
    };
    // Fit keel and frames from stern to stem. Each rib has a second edge on its far side.
    let keel_length = (w * progress / 80).clamp(8, w);
    for x in 0..=keel_length {
        let (_, b) = envelope(x, w, kind);
        s.bar(x, b, 1, 2, BARK);
        s.bar(x, b, 1, 1, 0xd0a06b);
    }
    let rib_limit = (w * (progress - 35) / 165).clamp(0, w);
    for x in (2..rib_limit).step_by(6) {
        let (deck, base) = envelope(x, w, kind);
        let lean = if x < w / 4 {
            -2
        } else if x > w * 3 / 4 {
            3
        } else {
            1
        };
        s.line((x + lean - 1, deck + 1), (x, base), 0x735e50);
        s.line((x + lean - 2, deck + 1), (x - 1, base), 0xc49865);
        s.line((x + lean - 2, deck + 1), (x + lean + 1, deck + 1), 0xe1b27b);
    }
    for x in 0..=w {
        let (deck, base) = envelope(x, w, kind);
        let height = (base - deck + 1) * fraction / 1000;
        if height <= 0 {
            continue;
        }
        for y in base - height + 1..=base {
            let col = if progress < 615 {
                if (y + 27) % 3 == 0 {
                    0x936e50
                } else {
                    0xc39263
                }
            } else if y >= -2 {
                under
            } else if y == deck + 2 {
                if kind == 2 {
                    0xeab07d
                } else {
                    0xeac496
                }
            } else if y == deck + 3 {
                if kind == 2 {
                    0xaa5747
                } else {
                    0x69877b
                }
            } else {
                paint
            };
            s.bar(x, y, 1, 1, col);
        }
    }
    if progress >= 590 {
        for x in 0..=w {
            let (deck, _) = envelope(x, w, kind);
            s.bar(x, deck - 1, 1, 1, 0xe1ad77);
            s.bar(x, deck, 1, 1, if kind == 2 { 0x805547 } else { TAR });
        }
    }
    if progress >= 655 {
        for x in (12..w - 9).step_by(if kind == 0 { 14 } else { 8 }) {
            let (deck, _) = envelope(x, w, kind);
            s.bar(x, deck + 5, 2, 2, TAR);
            s.bar(x, deck + 5, 1, 1, 0xd7bc88);
        }
    }
}

fn cutter(s: &mut Ship, progress: i32, tick: u32) {
    if progress < 605 {
        return;
    }
    s.poly(&[(24, -13), (26, -19), (45, -19), (49, -13)], 0x99795a);
    s.bar(27, -19, 17, 2, 0xe8cb97);
    s.bar(30, -16, 6, 3, TAR);
    s.bar(39, -16, 4, 3, BLUE);
    s.bar(13, -17, 2, 4, BARK);
    s.line((73, -14), (93, -20), 0x927351);
    if progress < 705 {
        return;
    }
    let mast_height = ((progress - 705) * 45 / 125).clamp(0, 45);
    s.bar(51, -14 - mast_height, 2, mast_height, 0xcdaa77);
    s.bar(51, -14 - mast_height, 1, mast_height, 0x8d7458);
    if progress < 830 {
        return;
    }
    s.line((52, -58), (92, -20), 0x7d8175);
    s.line((51, -57), (13, -15), 0x848a79);
    s.line((51, -50), (28, -49), 0x998064);
    s.line((16, -19), (50, -26), 0x8d775a);
    let drop = ((progress - 835) * 100 / 100).clamp(0, 100);
    if drop > 0 {
        let foot = -49 + 30 * drop / 100;
        let leech = 28 - 13 * drop / 100;
        s.poly(
            &[(29, -48), (50, -49), (50, foot - 6), (leech, foot)],
            0xf2dfb4,
        );
        s.line((29, -48), (leech, foot), 0xc1ad89);
        s.line((leech, foot), (50, foot - 6), 0xccb796);
        for (xb, yb) in [(23, -29), (20, -23)] {
            if foot > yb {
                s.line((xb, yb), (49, yb - 7), 0xd4c09b);
            }
        }
        let jib = -54 + 31 * drop / 100;
        s.poly(
            &[(54, -55), (55, jib), (57 + 27 * drop / 100, jib + 2)],
            0xf8e7be,
        );
        s.line((55, jib), (57 + 27 * drop / 100, jib + 2), 0xcaba98);
    }
    if progress >= 960 {
        s.flag(53, -59, tick);
    }
}

fn packet(s: &mut Ship, progress: i32, tick: u32) {
    if progress < 600 {
        return;
    }
    let cabin_h = ((progress - 600) * 11 / 100).clamp(1, 11);
    s.bar(14, -15 - cabin_h, 61, cabin_h, 0xe1c59b);
    s.bar(17, -15 - cabin_h, 53, 1, 0xf5dfb9);
    if progress < 690 {
        return;
    }
    for x in (17..71).step_by(7) {
        s.bar(x, -23, 5, 5, 0x517577);
        s.bar(x + 1, -22, 3, 1, 0xb4c2a3);
    }
    s.bar(12, -27, 67, 2, TAR);
    s.bar(13, -28, 66, 1, 0xd1a676);
    if progress < 750 {
        return;
    }
    s.bar(44, -35, 21, 7, 0xeee0b7);
    s.bar(46, -33, 6, 4, BLUE);
    s.bar(55, -33, 7, 4, BLUE);
    s.bar(42, -36, 24, 2, TAR);
    s.poly(&[(32, -28), (39, -28), (35, -45), (28, -45)], 0xc97855);
    s.bar(28, -46, 7, 4, TAR);
    s.bar(30, -40, 6, 2, 0xe2b488);
    if progress < 830 {
        return;
    }
    s.bar(77, -45, 1, 29, 0x88785c);
    s.line((77, -43), (95, -17), 0x939a84);
    s.line((77, -43), (65, -20), 0xa19c83);
    s.line((4, -17), (13, -17), STEEL);
    s.line((77, -18), (94, -18), STEEL);
    for x in [4, 9, 77, 82, 88, 94] {
        s.bar(x, -18, 1, 5, STEEL);
    }
    s.poly(&[(16, -29), (29, -29), (25, -26), (19, -26)], 0xad7957);
    if progress >= 960 {
        s.flag(78, -45, tick);
        for i in 0..6 {
            let age = ((tick / 8 + i * 5) % 33) as i32;
            let y = -49 - age / 5;
            s.bar(
                30 - age,
                y,
                2 + age / 7,
                1 + age / 10,
                if age < 10 { 0x95a69c } else { 0xb2bdac },
            );
        }
    }
}

fn lightship(s: &mut Ship, progress: i32, tick: u32) {
    if progress < 580 {
        return;
    }
    s.bar(17, -23, 25, 8, 0xe6d3a9);
    s.bar(15, -24, 29, 2, TAR);
    s.bar(20, -21, 5, 4, BLUE);
    s.bar(29, -22, 7, 6, 0x94785c);
    s.bar(78, -22, 17, 7, 0xebd9b0);
    s.bar(77, -23, 19, 2, TAR);
    s.bar(83, -20, 6, 3, BLUE);
    if progress < 675 {
        return;
    }
    let tower = ((progress - 675) * 25 / 115).clamp(1, 25);
    s.poly(
        &[(49, -15), (65, -15), (61, -15 - tower), (55, -15 - tower)],
        0xd9ab78,
    );
    s.poly(
        &[(59, -15), (65, -15), (61, -15 - tower), (59, -15 - tower)],
        0xb38b67,
    );
    if tower >= 13 {
        s.poly(&[(52, -26), (63, -26), (62, -32), (53, -32)], CORAL);
    }
    s.bar(56, -24, 3, 4, TAR);
    if progress < 790 {
        return;
    }
    s.bar(50, -41, 16, 2, TAR);
    s.bar(51, -45, 1, 5, BLUE);
    s.bar(65, -45, 1, 5, BLUE);
    s.bar(51, -45, 15, 1, BLUE);
    s.bar(53, -52, 10, 11, 0x526a68);
    s.bar(55, -50, 6, 7, 0xebc776);
    s.bar(56, -49, 4, 5, 0xffedb1);
    s.bar(58, -51, 1, 9, 0x8b7e5c);
    s.bar(52, -52, 12, 2, TAR);
    s.poly(&[(50, -52), (57, -57), (60, -57), (65, -52)], 0x5b645c);
    s.bar(57, -59, 2, 3, TAR);
    if progress < 890 {
        return;
    }
    s.bar(13, -42, 1, 28, OAK);
    s.line((13, -41), (2, -16), 0x879381);
    s.line((14, -39), (52, -32), 0x8b9682);
    s.line((63, -38), (96, -17), 0x8b9682);
    for x in (39..78).step_by(6) {
        s.bar(x, -18, 1, 5, STEEL);
    }
    s.bar(40, -18, 38, 1, STEEL);
    if progress >= 960 {
        s.flag(14, -42, tick);
        let glint = triangle(tick, 70);
        if glint < 22 {
            s.bar(58, -49, 2, 3, 0xffffd8);
            let size = (22 - glint) / 4;
            if size > 0 {
                s.bar(64, -48, size, 1, 0xe1dca7);
                s.bar(53 - size, -48, size, 1, 0xe2dead);
            }
        }
    }
}

fn swash(p: &mut Paint, x: i32, y: i32, age: i32) {
    let height = (210 - (age - 200).abs()).max(0) / 13;
    for i in 0..17 {
        let h = noise(i + 502);
        let side = if i % 2 == 0 { -1 } else { 1 };
        let spread = 2 + ((h >> 9) % 9) as i32 + age / 30;
        let xx = x + side * spread;
        let rise = height * (3 + (h % 8) as i32) / 11;
        p.bar(xx, y - rise, 1 + ((h >> 4) % 3) as i32, 2, 0xe8dfb7);
        p.dot(xx - side, y - rise - 2, 0xf1e9cb);
        p.bar(xx, y, 3, 1, 0xa7c8b1);
    }
}

fn vessel(p: &mut Paint, tick: u32, kind: usize, done: bool, stage: u16) {
    let w = [83, 101, 111][kind];
    let launching = (1001..=2000).contains(&stage);
    let slide = if launching {
        i32::from(stage - 1000)
    } else if done {
        1000
    } else {
        0
    };
    let eased = slide * slide * (3000 - 2 * slide) / 1_000_000;
    let start_x = [96, 88, 85][kind];
    let end_x = [232, 226, 214][kind];
    let start_y = 67;
    let end_y = 91;
    let x = start_x + (end_x - start_x) * eased / 1000;
    let y = start_y + (end_y - start_y) * eased / 1000;
    let tilt = 1000 - ((slide - 280) * 1000 / 520).clamp(0, 1000);
    let progress = if done || launching {
        1000
    } else {
        i32::from(stage).clamp(0, 1000)
    };
    let rocking = if slide >= 970 {
        triangle(tick / 19, 2) - 1
    } else if launching && slide > 650 {
        triangle(slide as u32 / 22, 3) - 1
    } else {
        0
    };
    if slide > 550 {
        for i in 0..19 {
            let n = noise(i + 55);
            let offset = ((n >> 9) % (w as u32 - 5)) as i32;
            p.bar(
                x + offset,
                y + 4 + (n % 10) as i32,
                3 + ((n >> 5) % 9) as i32,
                1,
                if i % 3 == 0 { 0x80a495 } else { 0x64928a },
            );
        }
    }
    let mut ship = Ship {
        p,
        x,
        y: y + rocking,
        tilt,
    };
    if progress == 0 {
        // A new elm keel lies across the waiting launch blocks.
        ship.bar(3, 0, w - 9, 2, 0x916d50);
        ship.bar(5, 0, w - 14, 1, 0xd7ad79);
        ship.bar(12, -2, 4, 3, BARK);
        ship.bar(w - 25, -2, 4, 3, BARK);
        return;
    }
    hull(&mut ship, kind, progress);
    match kind {
        0 => cutter(&mut ship, progress, tick),
        1 => packet(&mut ship, progress, tick),
        _ => lightship(&mut ship, progress, tick),
    }
    if launching && (500..950).contains(&slide) {
        swash(ship.p, x + w - 3, y + rocking, slide - 500);
        if slide > 680 {
            swash(ship.p, x + 9, y + rocking + 2, slide - 680);
        }
    }
    if slide > 850 {
        ship.p.bar(x + 9, y + rocking + 3, w - 20, 1, 0x8cae9d);
        ship.p.bar(x + w - 8, y + rocking - 1, 8, 1, 0xc4c9a8);
    }
}

/// Draw the selected hull on the ways or afloat. Completion is a count of ships.
/// Construction uses 1..=1000, launch uses 1001..=2000. Time runs at 30 Hz.
pub fn draw_yard(
    buf: &mut [u32; YARD_W * YARD_H],
    tick: u32,
    selected: usize,
    completed: u8,
    stage: u16,
) {
    let kind = selected.min(2);
    let ready = kind < usize::from(completed.min(3));
    let on_ways = !ready && stage <= 1000;
    let mut p = Paint(buf);
    coast(&mut p, tick);
    shed(&mut p);
    davit(&mut p, if on_ways { stage } else { 0 });
    ways(&mut p);
    if on_ways && stage > 80 {
        scaffold(&mut p, tick, stage);
    }
    worker(&mut p, 75, 74, on_ways && stage > 0, tick);
    vessel(&mut p, tick, kind, ready, stage);
    // A pair of water birds crosses the distant entrance.
    for (i, (x, y)) in [(197, 20), (206, 17)].into_iter().enumerate() {
        let x = x + triangle(tick / (12 + i as u32), 7);
        let tip = if (tick / 12 + i as u32) % 3 == 0 {
            -1
        } else {
            0
        };
        p.line((x - 2, y + tip), (x, y + 1), 0x758d87);
        p.line((x, y + 1), (x + 2, y + tip), 0x758d87);
    }
}
