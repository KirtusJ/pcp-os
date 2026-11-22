//! Pocket portraits for the residents' register. Coordinates refer to the native pixel grid.
pub const PORTRAIT_W: usize = 80;
pub const PORTRAIT_H: usize = 64;

type Frame = [u32; PORTRAIT_W * PORTRAIT_H];
type Point = (i32, i32);

const INK: u32 = 0x24353b;
const NIGHT: u32 = 0x334950;
const IVORY: u32 = 0xf0d7a6;
const BRASS: u32 = 0xd2a661;
const WOOD: u32 = 0x8d6b51;
const SCARLET: u32 = 0xad5444;

struct Paint<'a>(&'a mut Frame);
impl Paint<'_> {
    fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        let start = x.clamp(0, PORTRAIT_W as i32) as usize;
        let end = (x + w).clamp(0, PORTRAIT_W as i32) as usize;
        if start >= end {
            return;
        }
        for j in y.max(0)..(y + h).min(PORTRAIT_H as i32) {
            let row = j as usize * PORTRAIT_W;
            self.0[row + start..row + end].fill(c);
        }
    }
    fn line(&mut self, mut a: Point, b: Point, c: u32) {
        let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
        let (sx, sy) = (
            if a.0 < b.0 { 1 } else { -1 },
            if a.1 < b.1 { 1 } else { -1 },
        );
        let mut error = dx + dy;
        loop {
            self.bar(a.0, a.1, 1, 1, c);
            if a == b {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                a.0 += sx
            }
            if twice <= dx {
                error += dx;
                a.1 += sy
            }
        }
    }
    fn poly(&mut self, points: &[Point], c: u32) {
        // Sample at pixel centres, with top and left edges inclusive.
        let top = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let end = points
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap_or(0)
            .min(PORTRAIT_H as i32);
        for y in top..end {
            let mut spans = [0i32; 16];
            let mut n = 0;
            let mut a = points[points.len() - 1];
            for &b in points {
                if (a.1 > y) != (b.1 > y) {
                    let x = 256 * a.0 + 128 * (b.0 - a.0) * (2 * y + 1 - 2 * a.1) / (b.1 - a.1);
                    spans[n] = (x + 127).div_euclid(256);
                    n += 1;
                }
                a = b;
            }
            spans[..n].sort_unstable();
            for pair in spans[..n].chunks_exact(2) {
                self.bar(pair[0], y, pair[1] - pair[0], 1, c)
            }
        }
    }
    fn oval(&mut self, x: i32, y: i32, rx: i32, ry: i32, c: u32) {
        for j in -ry..=ry {
            let mut half = rx;
            while half * half * ry * ry + j * j * rx * rx > rx * rx * ry * ry {
                half -= 1
            }
            self.bar(x - half, y + j, half * 2 + 1, 1, c)
        }
    }
}

fn nell(p: &mut Paint<'_>, watch: usize) {
    // Window on the second pier; the oilskin takes the opposite tone to the sea.
    const SKY: [u32; 4] = [0x738780, 0xb9b99a, 0xa28b76, 0x4b686c];
    p.0.fill(0x536867);
    p.bar(0, 0, 66, 39, SKY[watch % 4]);
    p.bar(0, 32, 64, 10, 0x617d79);
    for i in 0..9 {
        p.bar(2 + i * 7, 34 + (i * 7) % 5, 4, 1, 0x9daca0)
    }
    p.poly(
        &[
            (0, 31),
            (9, 27),
            (18, 30),
            (30, 26),
            (40, 29),
            (58, 25),
            (66, 30),
            (66, 33),
            (0, 33),
        ],
        0x768c81,
    );
    p.bar(3, 24, 3, 7, 0xe0c89a);
    p.bar(3, 21, 3, 3, SCARLET);
    p.bar(2, 21, 5, 1, INK);
    p.bar(58, 0, 22, 64, 0x6a6858);
    for x in (59..80).step_by(6) {
        p.bar(x, 0, 1, 64, 0x595e50);
        p.bar(x + 2, 0, 1, 27, 0x7b7965)
    }
    p.bar(0, 42, 80, 3, 0x4d5b51);
    // Broad storm collar and a double row of snap fastenings.
    p.poly(
        &[
            (1, 64),
            (3, 55),
            (8, 48),
            (17, 43),
            (28, 40),
            (46, 41),
            (59, 44),
            (64, 48),
            (68, 58),
            (72, 64),
        ],
        INK,
    );
    p.poly(
        &[
            (3, 64),
            (6, 55),
            (10, 50),
            (22, 44),
            (31, 42),
            (46, 43),
            (59, 46),
            (63, 50),
            (69, 64),
        ],
        0xbc8e46,
    );
    p.poly(
        &[
            (8, 64),
            (10, 54),
            (17, 48),
            (27, 44),
            (31, 47),
            (34, 51),
            (30, 64),
        ],
        0xd8a54e,
    );
    p.poly(
        &[(34, 64), (34, 48), (46, 43), (54, 45), (59, 50), (61, 64)],
        0xce9846,
    );
    p.poly(
        &[
            (26, 42),
            (28, 35),
            (38, 34),
            (47, 37),
            (46, 44),
            (39, 48),
            (33, 48),
        ],
        0x714c3e,
    );
    p.poly(
        &[(18, 44), (24, 39), (31, 42), (36, 49), (30, 54), (27, 50)],
        0xffcf70,
    );
    p.poly(
        &[(38, 48), (45, 40), (51, 41), (55, 45), (46, 51), (42, 55)],
        0xe5ac4e,
    );
    p.line((30, 54), (35, 51), 0x9c763e);
    p.line((43, 55), (53, 47), 0x936e40);
    p.line((37, 50), (37, 63), 0x946f42);
    for y in [54, 60] {
        p.bar(35, y, 2, 2, 0x57594a);
        p.bar(35, y, 1, 1, IVORY)
    }
    p.line((12, 52), (15, 61), 0xad8746);
    p.line((55, 57), (57, 63), 0xad8042);
    // Silvery coils spill out from beneath the deep brim.
    p.oval(33, 24, 17, 16, 0x4b4d48);
    for &(x, y) in &[
        (20, 19),
        (17, 23),
        (18, 28),
        (18, 32),
        (21, 36),
        (24, 38),
        (45, 24),
        (49, 22),
        (49, 28),
        (47, 33),
    ] {
        p.oval(x, y, 3, 3, 0x545953);
        p.oval(x - 1, y - 1, 2, 2, 0x83867a);
        p.bar(x - 1, y - 2, 2, 1, 0xa5a496);
        p.bar(x + 1, y + 1, 2, 1, 0x505953);
    }
    // Nell faces the light. Keep a deep shadow under the cap; the smile lifts the near cheek.
    p.poly(
        &[
            (23, 18),
            (30, 15),
            (41, 15),
            (48, 19),
            (49, 24),
            (48, 29),
            (51, 32),
            (49, 34),
            (48, 38),
            (44, 41),
            (38, 43),
            (31, 41),
            (26, 38),
            (23, 31),
        ],
        0x754d3e,
    );
    p.poly(
        &[
            (29, 20),
            (40, 17),
            (46, 20),
            (47, 25),
            (46, 28),
            (50, 32),
            (47, 34),
            (46, 38),
            (42, 40),
            (36, 41),
            (31, 39),
            (27, 35),
            (26, 29),
        ],
        0x986348,
    );
    p.poly(
        &[
            (32, 22),
            (39, 20),
            (44, 21),
            (46, 23),
            (45, 27),
            (44, 29),
            (48, 32),
            (44, 32),
            (41, 31),
            (39, 34),
            (36, 35),
            (31, 33),
            (29, 30),
        ],
        0xae7955,
    );
    p.poly(
        &[(28, 29), (31, 28), (35, 29), (37, 32), (33, 33), (29, 32)],
        0xb7805e,
    );
    p.poly(
        &[(42, 27), (44, 26), (45, 29), (48, 32), (44, 32), (42, 31)],
        0xc18c65,
    );
    p.oval(24, 29, 3, 5, 0x98674e);
    p.line((23, 29), (25, 28), 0x65483e);
    p.bar(24, 32, 1, 2, 0xb78765);
    p.bar(23, 34, 2, 2, BRASS);
    p.bar(23, 34, 1, 1, IVORY);
    p.line((29, 24), (34, 23), 0x5c493e);
    p.line((41, 22), (45, 22), 0x675044);
    p.line((30, 27), (33, 26), 0x5d453e);
    p.bar(33, 26, 1, 2, 0x382f31);
    p.bar(32, 26, 1, 1, 0xe3c199);
    p.bar(42, 25, 4, 1, 0x66483c);
    p.bar(44, 25, 1, 2, 0x382f31);
    p.bar(43, 25, 1, 1, 0xe0bc94);
    p.line((39, 31), (41, 32), 0x76513f);
    p.bar(45, 32, 2, 1, 0x6c4d3d);
    p.line((35, 35), (38, 37), 0x70473d);
    p.line((38, 37), (45, 35), 0x70473d);
    p.bar(38, 36, 5, 1, 0xdba47c);
    p.line((35, 39), (39, 40), 0xa67154);
    p.line((28, 29), (30, 30), 0x956247);
    p.bar(46, 28, 2, 1, 0xb07854);
    p.line((27, 27), (29, 28), 0x805943);
    p.line((29, 32), (33, 34), 0x94664c);
    p.line((36, 21), (39, 20), 0x95674d);
    // Flexible ochre sou'wester, flat in front and long over the curls.
    p.poly(
        &[
            (12, 23),
            (14, 19),
            (17, 17),
            (19, 11),
            (22, 7),
            (31, 4),
            (38, 5),
            (43, 8),
            (47, 14),
            (52, 15),
            (54, 17),
            (51, 19),
            (31, 19),
            (22, 21),
            (18, 25),
        ],
        0x665841,
    );
    p.poly(
        &[
            (14, 22),
            (17, 18),
            (19, 17),
            (21, 11),
            (24, 8),
            (31, 6),
            (37, 6),
            (42, 9),
            (46, 15),
            (52, 16),
            (51, 17),
            (31, 18),
            (21, 20),
            (17, 23),
        ],
        0xd4a653,
    );
    p.poly(
        &[
            (20, 16),
            (22, 11),
            (26, 8),
            (33, 7),
            (37, 8),
            (41, 12),
            (42, 15),
            (29, 16),
        ],
        0xe2b762,
    );
    p.line((22, 14), (26, 9), 0xf1ca76);
    p.line((26, 9), (31, 8), 0xf1ca76);
    p.line((18, 19), (44, 15), 0x9c7c48);
    p.line((30, 17), (50, 16), 0xeec476);
    // Wick lamp at her shoulder. The brass bail sits in her gloved hand.
    p.poly(
        &[
            (49, 64),
            (52, 57),
            (57, 53),
            (61, 50),
            (65, 50),
            (67, 53),
            (62, 57),
            (58, 60),
            (57, 64),
        ],
        0xbd914a,
    );
    p.poly(
        &[
            (58, 54),
            (59, 50),
            (60, 47),
            (62, 47),
            (62, 50),
            (65, 48),
            (68, 49),
            (69, 51),
            (64, 53),
            (61, 55),
        ],
        0x875c47,
    );
    p.line((61, 48), (61, 51), 0xbe8c68);
    p.line((63, 51), (67, 49), 0xb5815d);
    p.line((64, 51), (64, 44), 0xb5915d);
    p.line((64, 44), (67, 41), BRASS);
    p.line((67, 41), (71, 41), IVORY);
    p.line((71, 41), (74, 44), BRASS);
    p.line((74, 44), (74, 51), 0x947647);
    p.bar(66, 48, 7, 2, 0x735e46);
    p.bar(64, 50, 11, 2, BRASS);
    p.bar(66, 52, 7, 8, 0xe8ae60);
    p.bar(67, 53, 5, 6, 0xffcc78);
    p.bar(69, 53, 2, 7, 0xffe5a7);
    p.line((65, 52), (66, 59), 0x746346);
    p.line((73, 52), (72, 59), 0x8c704b);
    p.bar(64, 60, 11, 2, 0x8e7047);
    p.bar(63, 62, 13, 2, BRASS);
    p.bar(66, 62, 7, 1, IVORY);
}

fn ada(p: &mut Paint<'_>, watch: usize) {
    // The cool enamel pots stand out against the brick kitchen wall.
    const WALL: [u32; 4] = [0x948573, 0x9f8c78, 0x907660, 0x71675c];
    p.0.fill(WALL[watch % 4]);
    for y in (3..49).step_by(7) {
        p.bar(0, y, 80, 1, 0x746c5f);
        for x in (y % 3 * 5..80).step_by(17) {
            p.bar(x, y + 1, 1, 6, 0x746c5f)
        }
    }
    p.bar(57, 28, 23, 3, 0x605c51);
    p.bar(60, 27, 20, 1, BRASS);
    for (x, y) in [(62, 21), (70, 17), (75, 21)] {
        p.bar(x, y, 6, 27 - y, 0x506963);
        p.bar(x + 1, y, 4, 1, 0xb1b8a1);
        p.bar(x + 1, y + 2, 1, 3, 0x87a18d)
    }
    p.bar(0, 46, 80, 3, WOOD);
    p.bar(0, 49, 80, 15, 0x626554);
    // High white shoulders, stout elbows and a striped apron on an angled body.
    p.poly(
        &[
            (12, 64),
            (15, 52),
            (25, 45),
            (36, 41),
            (52, 39),
            (63, 42),
            (68, 49),
            (72, 64),
        ],
        INK,
    );
    p.poly(
        &[
            (13, 64),
            (17, 53),
            (28, 46),
            (39, 43),
            (54, 41),
            (63, 45),
            (66, 50),
            (70, 64),
        ],
        0xb8b59b,
    );
    p.poly(
        &[(15, 64), (18, 54), (28, 48), (35, 48), (31, 55), (29, 64)],
        0xdccbac,
    );
    p.poly(
        &[
            (30, 64),
            (32, 52),
            (38, 46),
            (52, 44),
            (58, 46),
            (59, 55),
            (57, 64),
        ],
        0x7e897a,
    );
    p.poly(
        &[(35, 64), (37, 53), (40, 46), (46, 46), (43, 53), (43, 64)],
        0xa3a38c,
    );
    p.poly(
        &[(52, 46), (55, 44), (59, 45), (58, 52), (54, 64), (51, 64)],
        0xd2c2a0,
    );
    p.line((35, 53), (34, 60), 0xcabda0);
    p.line((63, 50), (62, 57), 0x898d7c);
    // Copper bun drawn behind the head; one corkscrew refuses the pins.
    p.oval(56, 18, 8, 10, 0x684943);
    p.oval(57, 17, 6, 8, 0x995f43);
    p.oval(58, 17, 4, 5, 0xb5784e);
    p.line((56, 12), (60, 13), 0xcf925e);
    p.line((55, 21), (59, 23), 0x704c40);
    p.line((52, 17), (62, 14), 0xd3ab70);
    p.line((56, 25), (59, 30), 0x794f3f);
    p.line((59, 30), (58, 34), 0x976047);
    p.poly(
        &[
            (34, 25),
            (32, 14),
            (37, 7),
            (44, 5),
            (53, 8),
            (57, 15),
            (55, 27),
            (53, 32),
            (46, 35),
        ],
        0x82523e,
    );
    // Her nose, cheek and square chin read in profile, turned to the serving hatch.
    p.poly(
        &[
            (34, 15),
            (39, 11),
            (50, 12),
            (55, 19),
            (54, 30),
            (51, 34),
            (53, 40),
            (48, 44),
            (39, 44),
            (38, 40),
            (34, 39),
            (30, 35),
            (29, 31),
            (25, 30),
            (28, 26),
            (30, 23),
            (30, 18),
        ],
        0xa37256,
    );
    p.poly(
        &[
            (32, 18),
            (38, 13),
            (48, 15),
            (52, 19),
            (52, 26),
            (49, 31),
            (46, 36),
            (42, 39),
            (36, 38),
            (32, 35),
            (31, 31),
            (27, 30),
            (30, 26),
        ],
        0xd19a74,
    );
    p.poly(
        &[
            (33, 19),
            (39, 15),
            (45, 16),
            (45, 22),
            (41, 26),
            (37, 28),
            (34, 32),
            (30, 29),
            (32, 25),
        ],
        0xe3b18a,
    );
    p.oval(51, 26, 3, 4, 0xbd8665);
    p.line((51, 25), (53, 24), 0x895e4c);
    p.bar(51, 30, 2, 2, BRASS);
    p.line((36, 32), (42, 33), 0xae7359);
    p.line((32, 34), (38, 35), 0x8c584b);
    p.bar(35, 34, 4, 1, 0xead2ac);
    p.line((40, 35), (42, 33), 0x99624c);
    p.line((37, 38), (40, 38), 0xbb8466);
    p.bar(30, 29, 3, 1, 0xab775a);
    p.line((41, 29), (44, 28), 0xc88a68);
    // The two distinct lenses and glints are large enough to read at one native pixel.
    for &(x, y) in &[(31, 24), (41, 23)] {
        p.oval(x, y, 5, 4, 0x9a694c);
        p.oval(x, y, 4, 3, 0xe8ba88);
        p.bar(x - 3, y + 1, 6, 1, 0xc4a78b);
        p.bar(x - 2, y, 5, 1, 0x735548);
        p.bar(x - 1, y, 1, 2, 0x453a37);
        p.bar(x + 1, y - 2, 2, 1, 0xf9d5a3);
    }
    p.line((35, 23), (37, 22), 0x8a5e43);
    p.line((45, 22), (52, 20), 0xcc995f);
    p.bar(36, 20, 4, 1, 0x8d6044);
    // Hair combed back into the bun. A few bright arcs stand in for individual curls.
    p.poly(
        &[
            (30, 21),
            (28, 17),
            (30, 11),
            (35, 7),
            (42, 5),
            (50, 7),
            (55, 12),
            (56, 17),
            (53, 21),
            (52, 25),
            (49, 22),
            (49, 17),
            (46, 16),
            (45, 13),
            (39, 15),
            (37, 17),
            (35, 16),
            (32, 19),
        ],
        0x935d42,
    );
    p.poly(
        &[
            (30, 16),
            (33, 11),
            (39, 8),
            (46, 7),
            (50, 9),
            (45, 9),
            (38, 12),
            (35, 14),
        ],
        0xb57b52,
    );
    p.line((35, 12), (42, 9), 0xcf9160);
    p.line((43, 12), (48, 10), 0xc3895a);
    p.line((49, 13), (52, 16), 0xc28a58);
    p.line((51, 19), (53, 17), 0x6f4e3e);
    // An apron knot plus the red neckerchief, tied slightly to one side.
    p.poly(
        &[
            (35, 39),
            (41, 40),
            (48, 38),
            (53, 35),
            (56, 39),
            (53, 43),
            (43, 48),
            (37, 45),
        ],
        0x883f37,
    );
    p.poly(
        &[
            (35, 39),
            (42, 41),
            (48, 39),
            (52, 37),
            (55, 38),
            (50, 42),
            (43, 44),
            (38, 44),
        ],
        0xc5684b,
    );
    p.poly(
        &[(41, 43), (46, 43), (48, 46), (44, 49), (41, 47)],
        0xdb805b,
    );
    p.poly(&[(43, 47), (45, 47), (43, 57), (40, 53)], SCARLET);
    p.poly(
        &[(46, 46), (48, 46), (51, 52), (48, 52), (44, 49)],
        0x994b3c,
    );
    // A tarnished broad-bowled ladle held upright at the shoulder.
    p.line((19, 53), (14, 30), 0x665c4d);
    p.line((20, 53), (15, 30), 0xaa9a7b);
    p.oval(13, 27, 4, 5, 0x6b776c);
    p.oval(12, 26, 3, 4, 0xa3aa90);
    p.line((11, 24), (11, 27), 0xe5d3ad);
    p.poly(
        &[
            (16, 62),
            (15, 56),
            (16, 49),
            (19, 47),
            (22, 48),
            (21, 52),
            (22, 55),
            (21, 60),
        ],
        0xc78f6a,
    );
    p.line((17, 49), (20, 49), 0xe4b18a);
    p.line((17, 52), (20, 51), 0xa86f56);
    // Part of an enamelled soup pot catches the low light at the bottom of the frame.
    p.oval(10, 60, 12, 3, 0x6d4b40);
    p.bar(-1, 59, 25, 5, 0xaa5945);
    p.oval(10, 59, 12, 2, 0xe0a275);
    p.oval(10, 58, 9, 2, 0xb66b4d);
    p.bar(8, 55, 4, 2, INK);
    p.line((1, 60), (4, 60), IVORY);
}

fn kit(p: &mut Paint<'_>, watch: usize) {
    // The postboat's sail and a rack of pigeonholes.
    const DAWN: [u32; 4] = [0xb2bba2, 0xbaac91, 0xb58e76, 0x72868a];
    p.0.fill(DAWN[watch % 4]);
    p.bar(0, 32, 80, 4, 0x81968b);
    p.bar(0, 36, 80, 9, 0x678680);
    p.line((11, 12), (11, 37), 0x655e52);
    p.poly(&[(10, 15), (10, 34), (0, 34)], 0xe3d2a9);
    p.poly(&[(1, 36), (19, 36), (15, 39), (5, 39)], 0x4f6260);
    p.bar(0, 43, 80, 3, WOOD);
    p.bar(59, 0, 21, 64, 0x6e6757);
    for y in (8..64).step_by(12) {
        p.bar(62, y, 16, 8, 0x4c554b);
        p.bar(65, y + 5, 11, 3, 0xbbb293);
        p.line((70, y + 7), (74, y + 5), 0x948b72);
        p.bar(59, y + 9, 21, 2, 0x997e59);
    }
    // Lanky figure in a pea coat. The diagonal mailbag strap gives the pose some lean.
    p.poly(
        &[
            (5, 64),
            (9, 55),
            (21, 47),
            (36, 43),
            (42, 36),
            (51, 38),
            (52, 44),
            (59, 48),
            (63, 56),
            (65, 64),
        ],
        INK,
    );
    p.poly(
        &[
            (6, 64),
            (12, 55),
            (22, 48),
            (37, 44),
            (44, 43),
            (50, 43),
            (57, 49),
            (60, 54),
            (63, 64),
        ],
        0x4c6c70,
    );
    p.poly(
        &[(10, 64), (14, 57), (23, 51), (30, 49), (29, 55), (22, 64)],
        0x6d8883,
    );
    p.poly(
        &[(43, 46), (51, 44), (58, 49), (62, 64), (48, 64)],
        0x456368,
    );
    p.poly(
        &[(36, 45), (31, 46), (34, 54), (38, 59), (42, 49)],
        0x6f9391,
    );
    p.poly(
        &[(40, 49), (47, 43), (53, 44), (49, 50), (43, 54)],
        0x334f59,
    );
    p.poly(
        &[
            (33, 42),
            (37, 37),
            (40, 35),
            (48, 32),
            (48, 41),
            (51, 43),
            (45, 48),
            (40, 50),
            (36, 49),
        ],
        0xaf785b,
    );
    p.poly(
        &[(39, 41), (45, 36), (47, 36), (47, 42), (43, 46), (40, 47)],
        0xdca278,
    );
    p.poly(
        &[(33, 45), (37, 43), (40, 47), (43, 50), (40, 53), (37, 52)],
        0x739b90,
    );
    p.line((38, 50), (40, 51), 0xb7b69a);
    p.line((43, 54), (44, 63), 0x34505a);
    p.bar(45, 58, 2, 2, BRASS);
    p.poly(
        &[(23, 48), (27, 46), (52, 62), (54, 64), (46, 64)],
        0x675444,
    );
    p.poly(&[(24, 48), (27, 47), (52, 64), (48, 64)], 0xae855a);
    p.poly(&[(33, 53), (37, 55), (35, 59), (31, 56)], 0xd6ab69);
    p.line((33, 55), (35, 56), 0x6f6251);
    // Freckles, a clipped curl, and the tip of an upturned nose catch the morning sun.
    p.poly(
        &[
            (28, 18),
            (38, 15),
            (46, 16),
            (52, 21),
            (54, 26),
            (54, 29),
            (59, 31),
            (57, 34),
            (54, 34),
            (53, 38),
            (48, 41),
            (43, 40),
            (40, 37),
            (36, 35),
            (32, 33),
            (28, 27),
        ],
        0xb67b5c,
    );
    p.poly(
        &[
            (32, 18),
            (42, 16),
            (48, 19),
            (52, 23),
            (52, 28),
            (54, 30),
            (58, 31),
            (55, 33),
            (53, 33),
            (52, 37),
            (48, 39),
            (44, 38),
            (40, 35),
            (36, 33),
            (33, 29),
        ],
        0xdeaa81,
    );
    p.poly(
        &[
            (40, 20),
            (46, 19),
            (49, 22),
            (50, 26),
            (46, 27),
            (45, 30),
            (40, 31),
            (37, 28),
        ],
        0xedbc91,
    );
    p.poly(
        &[(49, 28), (51, 26), (53, 30), (57, 31), (54, 32), (51, 31)],
        0xf3c398,
    );
    p.line((47, 24), (51, 24), 0x7f5c4d);
    p.line((41, 25), (44, 24), 0x87634d);
    p.bar(49, 27, 4, 1, 0x5c4a43);
    p.bar(51, 27, 1, 2, INK);
    p.bar(50, 27, 1, 1, IVORY);
    p.bar(41, 27, 3, 1, 0x745547);
    p.bar(43, 27, 1, 2, INK);
    for &(x, y) in &[(44, 30), (46, 31), (48, 30), (50, 30), (52, 31)] {
        p.bar(x, y, 1, 1, 0xaa7457)
    }
    p.line((49, 35), (53, 35), 0x9c6655);
    p.line((47, 34), (49, 35), 0xaf7860);
    p.bar(49, 36, 2, 1, 0xf0bd93);
    p.line((43, 37), (46, 39), 0xc38c6c);
    p.oval(34, 29, 3, 4, 0xc18b68);
    p.line((33, 28), (35, 27), 0x99694f);
    p.bar(34, 32, 1, 2, 0xe4b78a);
    p.poly(
        &[
            (26, 16),
            (36, 12),
            (45, 13),
            (48, 17),
            (46, 21),
            (42, 22),
            (40, 24),
            (39, 22),
            (36, 24),
            (36, 27),
            (33, 27),
            (31, 31),
            (32, 35),
            (29, 37),
            (28, 34),
            (25, 36),
            (26, 31),
            (24, 32),
            (25, 26),
            (23, 23),
        ],
        0x694f44,
    );
    p.line((28, 25), (28, 30), 0x927157);
    p.line((30, 22), (32, 19), 0xa17a58);
    p.line((35, 21), (38, 19), 0x907353);
    // The cap sweeps forward into a hard, projecting navy peak.
    p.poly(
        &[
            (19, 19),
            (19, 15),
            (22, 11),
            (29, 7),
            (40, 6),
            (48, 8),
            (54, 13),
            (53, 15),
            (59, 15),
            (62, 17),
            (55, 20),
            (43, 21),
            (36, 20),
            (26, 24),
            (23, 23),
        ],
        INK,
    );
    p.poly(
        &[
            (20, 18),
            (21, 14),
            (26, 10),
            (32, 8),
            (42, 8),
            (48, 10),
            (52, 13),
            (45, 15),
            (34, 16),
            (26, 20),
            (23, 21),
        ],
        0x3c5968,
    );
    p.poly(
        &[
            (23, 15),
            (26, 11),
            (34, 8),
            (40, 8),
            (35, 10),
            (30, 12),
            (27, 16),
        ],
        0x5c7780,
    );
    p.poly(
        &[
            (27, 19),
            (35, 16),
            (47, 14),
            (53, 14),
            (52, 16),
            (40, 18),
            (29, 22),
        ],
        0x2e4450,
    );
    p.poly(
        &[(42, 18), (51, 15), (58, 16), (60, 17), (53, 19), (44, 20)],
        0x4c656d,
    );
    p.line((43, 20), (56, 18), 0x243841);
    p.bar(27, 18, 2, 2, BRASS);
    // One kingfisher feather tucked into the hatband.
    p.poly(
        &[
            (27, 19),
            (23, 14),
            (20, 11),
            (18, 6),
            (18, 3),
            (21, 6),
            (23, 8),
            (25, 13),
        ],
        0x396966,
    );
    p.poly(
        &[(26, 17), (22, 12), (19, 7), (19, 5), (22, 8), (24, 12)],
        0x78aba0,
    );
    p.line((20, 6), (24, 14), 0x9bc5ad);
    p.bar(24, 14, 2, 2, 0xc68651);
    // The oat-coloured mailbag carries its own brass catch.
    p.poly(
        &[(47, 64), (47, 61), (53, 57), (69, 53), (75, 56), (78, 64)],
        0x72573f,
    );
    p.poly(
        &[(49, 64), (49, 61), (54, 58), (69, 55), (73, 57), (76, 64)],
        0x986e48,
    );
    p.poly(
        &[(49, 61), (52, 59), (68, 55), (73, 57), (72, 60), (54, 63)],
        0xb78d5c,
    );
    p.line((54, 62), (70, 58), 0xcca777);
    p.bar(61, 59, 4, 3, BRASS);
    p.bar(62, 59, 2, 1, IVORY);
    p.bar(62, 60, 1, 1, 0x806243);
}

/// Paint the full canvas. Resident indices are Nell, Ada and Kit, in register order.
pub fn draw_neighbor(buf: &mut [u32; PORTRAIT_W * PORTRAIT_H], resident: usize, watch: usize) {
    let mut p = Paint(buf);
    match resident {
        0 => nell(&mut p, watch),
        1 => ada(&mut p, watch),
        2 => kit(&mut p, watch),
        _ => p.0.fill(NIGHT),
    }
}
