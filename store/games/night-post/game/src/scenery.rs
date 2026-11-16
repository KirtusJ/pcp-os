use crate::{paint::*, triangle, W};

fn shaped(c: &mut Canvas, x: i32, y: i32, scale: i32, points: &[(i32, i32)], color: Color) {
    let mut out = [(0, 0); 32];
    for (slot, &(px, py)) in out.iter_mut().zip(points) {
        *slot = (x + px * scale, y + py * scale);
    }
    c.poly(&out[..points.len()], color);
}

// The courier is a kingfisher with a leather mail bag and a long red scarf.
pub fn bird(c: &mut Canvas, x: i32, y: i32, s: i32, t: i32) {
    let lift = triangle(t, 24) - 6;
    let mut part = |p: &[(i32, i32)], color| shaped(c, x, y, s, p, color);
    part(
        &[(-12, 1), (-29, -2), (-21, 4), (-30, 5), (-13, 6), (-6, 3)],
        0x384864,
    );
    part(&[(-11, 1), (-26, 0), (-15, 4), (-6, 3)], 0x60afbc);
    part(
        &[
            (-13, 2),
            (-8, -4),
            (2, -6),
            (7, -13),
            (13, -15),
            (21, -12),
            (24, -6),
            (19, -1),
            (16, 4),
            (9, 8),
            (0, 10),
            (-9, 7),
        ],
        INK,
    );
    part(
        &[
            (-13, 0),
            (-7, -5),
            (4, -6),
            (8, -13),
            (14, -14),
            (20, -12),
            (22, -6),
            (18, -2),
            (11, 1),
            (8, 5),
            (0, 8),
            (-8, 6),
        ],
        0x318690,
    );
    part(
        &[
            (4, -6),
            (8, -13),
            (14, -14),
            (20, -12),
            (21, -10),
            (13, -11),
            (9, -7),
        ],
        0x91d9ca,
    );
    part(
        &[
            (3, -1),
            (11, -4),
            (18, -5),
            (16, 1),
            (9, 7),
            (2, 8),
            (-2, 6),
        ],
        0xf69a55,
    );
    part(
        &[(8, -2), (16, -6), (21, -8), (21, -5), (16, -3), (11, 1)],
        PEARL,
    );
    part(&[(21, -8), (32, -7), (21, -4)], INK);
    part(&[(23, -8), (32, -7), (23, -6)], AMBER);
    part(&[(14, -10), (18, -11), (20, -9), (17, -7), (14, -8)], INK);
    part(&[(17, -10), (18, -10), (18, -9), (17, -9)], PEARL);
    part(
        &[(5, -3), (9, -4), (10, -1), (7, 2), (1, 1), (-4, -1)],
        CORAL,
    );
    let flutter = triangle(t + 4, 16) / 3;
    part(
        &[
            (3, -2),
            (-7, -3),
            (-15, -7 + flutter),
            (-24, -7),
            (-18, -9 + flutter),
            (-14, -10 + flutter),
            (-5, -5),
            (4, -4),
        ],
        0xf27d71,
    );
    part(
        &[
            (-9, 0),
            (-16, -4),
            (-22, -10 - lift),
            (-26, -17 - lift),
            (-18, -14 - lift),
            (-6, -10 - lift),
            (2, -2),
            (-2, 2),
        ],
        0x253a56,
    );
    part(
        &[
            (-11, -2),
            (-19, -7 - lift),
            (-24, -16 - lift),
            (-15, -12 - lift),
            (-6, -8 - lift),
            (1, -2),
            (-3, 1),
        ],
        0x479eaa,
    );
    part(
        &[
            (-13, -5 - lift),
            (-23, -15 - lift),
            (-15, -11 - lift),
            (-8, -8 - lift),
        ],
        0xa6e4d2,
    );
    part(
        &[(-7, 5), (-4, 3), (2, 6), (2, 10), (-4, 11), (-8, 8)],
        0x743e4a,
    );
    part(&[(-6, 5), (-4, 4), (1, 6), (-1, 8), (-6, 7)], 0xf3b167);
}

fn mountain(c: &mut Canvas, offset: i32, base: i32, step: i32, amp: i32, seed: u32, col: Color) {
    let ix = offset.div_euclid(step);
    let mut x = -offset.rem_euclid(step);
    let height = |i: i32| base - (noise((i as u32).wrapping_add(seed)) % amp as u32) as i32;
    let mut i = ix;
    while x < W as i32 {
        c.poly(
            &[
                (x, height(i)),
                (x + step, height(i + 1)),
                (x + step, 234),
                (x, 234),
            ],
            col,
        );
        x += step;
        i += 1;
    }
}

pub fn pine(c: &mut Canvas, x: i32, y: i32, h: i32, col: Color) {
    let w = (h / 4).max(2);
    c.rect(x - 1, y - h, 2, h, col);
    for (t, b, f) in [(0, 5, 2), (2, 8, 3), (4, 10, 4)] {
        c.poly(
            &[
                (x, y - h + h * t / 12),
                (x + w * f / 4, y - h + h * b / 12),
                (x - w * f / 4, y - h + h * b / 12),
            ],
            col,
        );
    }
}

pub fn sky(c: &mut Canvas, d: i32, t: i32) {
    for y in 0..235 {
        let col = if y < 118 {
            blend(0x262744, 0x695674, y, 118)
        } else if y < 190 {
            blend(0x695674, 0xce8586, y - 118, 72)
        } else {
            blend(0xce8586, 0xf1ae8c, y - 190, 45)
        };
        c.rect(0, y, W as i32, 1, col);
    }
    for i in 0u32..63 {
        let k = noise(i + 29);
        let x = ((k % 532) as i32 - d / 36).rem_euclid(532) - 10;
        let y = ((k >> 10) % 136) as i32 + 4;
        c.dot(
            x,
            y,
            if (i as i32 + t / 22) % 7 == 0 {
                PEARL
            } else {
                0xaf95aa
            },
        );
        if i % 14 == 0 {
            c.line(x - 2, y, x + 2, y, 0xb59aab);
            c.line(x, y - 2, x, y + 2, 0xd5b5b6);
        }
    }
    let mx = 374 - d / 90;
    c.disc(mx, 79, 44, 0x977a8a);
    c.disc(mx, 79, 42, 0xe9baaa);
    c.disc(mx - 1, 77, 40, 0xffd9ae);
    for &(ox, oy, len) in &[
        (-29, -11, 9),
        (10, -25, 8),
        (15, -22, 3),
        (4, 12, 12),
        (-20, 21, 6),
        (-35, -2, 5),
        (26, 8, 8),
    ] {
        c.rect(mx + ox, 79 + oy, len, 1, 0xf1c8a4);
    }
    // Long streaks cross the moon and dissolve into the dusk.
    for &(x, y, w, col) in &[
        (318, 97, 55, 0xa08294),
        (329, 95, 26, 0xb08b9c),
        (381, 108, 86, 0x9b8096),
        (14, 132, 70, 0x94718a),
        (75, 134, 31, 0x94718a),
        (195, 77, 37, 0x79667f),
        (202, 75, 17, 0x79667f),
        (399, 42, 57, 0x755e79),
        (417, 40, 19, 0x755e79),
        (197, 145, 110, 0x98798d),
    ] {
        let x = (x - d / 20).rem_euclid(560) - 8;
        c.rect(x, y, w, 2, col);
    }
    mountain(c, d / 8, 201, 38, 39, 92, 0x9a7e90);
    mountain(c, d / 5, 218, 32, 45, 46, 0x776780);
    mountain(c, d / 3, 229, 43, 38, 771, 0x574d6c);
    for i in -1..60 {
        let shift = d / 3;
        let cell = shift.div_euclid(10) + i;
        let k = noise((cell as u32).wrapping_add(7001));
        let x = i * 10 - shift.rem_euclid(10) + (k % 7) as i32;
        pine(c, x, 232, 8 + ((k >> 8) % 14) as i32, 0x48465f);
    }
    for y in 228..288 {
        c.rect(0, y, W as i32, 1, blend(0x736a87, 0x242d4a, y - 228, 60));
    }
    for i in 0u32..170 {
        let k = noise(i + 118);
        let y = 230 + (k % 58) as i32;
        let x = (((k >> 12) % 558) as i32 - d / (6 - (y - 230) / 13).max(2)).rem_euclid(558) - 22;
        let spread = (y - 227) * 2 + 8;
        let moonlight = (x - mx).abs() < spread / 2 && k % 4 != 0;
        let col = if moonlight {
            blend(0xf0ad98, 0x8c7a92, y - 230, 58)
        } else {
            blend(0xa68aa0, 0x4c4d6a, y - 230, 58)
        };
        c.rect(
            x + triangle(t / 4 + (k % 30) as i32, 16) / 4,
            y,
            3 + ((k >> 22) % 17) as i32,
            1,
            col,
        );
    }
}

pub fn cloud(c: &mut Canvas, x: i32, y: i32, t: i32) {
    if !(-60..570).contains(&x) {
        return;
    }
    for (ox, oy, r) in [
        (-22, 0, 10),
        (-12, -6, 12),
        (2, -8, 16),
        (16, -3, 12),
        (25, 1, 9),
    ] {
        c.disc(x + ox, y + oy, r, 0x867795);
        c.disc(x + ox, y + oy + 2, r, 0x4d4764);
    }
    c.rect(x - 26, y + 2, 56, 6, 0x4d4764);
    for i in 0..8 {
        let dx = (i * 17 + t / 4).rem_euclid(53) - 27;
        let dy = (i * 9 + t).rem_euclid(15) + 8;
        c.line(x + dx, y + dy, x + dx - 2, y + dy + 4, 0xa6a9c6);
    }
    c.line(x + 4, y + 2, x, y + 9, AMBER);
    c.line(x, y + 9, x + 4, y + 9, AMBER);
    c.line(x + 4, y + 9, x, y + 16, AMBER);
}

pub fn post(c: &mut Canvas, x: i32, y: i32, id: usize, lit: bool, t: i32) {
    if !(-100..612).contains(&x) {
        return;
    }
    let base = 239;
    c.poly(
        &[
            (x - 89, base + 1),
            (x - 74, 232),
            (x - 53, 229),
            (x - 42, 221),
            (x - 25, 220),
            (x - 17, 213),
            (x + 14, 213),
            (x + 31, 222),
            (x + 47, 224),
            (x + 62, 231),
            (x + 75, 234),
            (x + 86, base + 2),
        ],
        0x2e324d,
    );
    c.line(x - 70, 233, x - 51, 230, 0x877187);
    c.line(x + 44, 227, x + 57, 231, 0x84718a);
    for (ox, h) in [(-51, 22), (-44, 36), (40, 29), (49, 18), (56, 14)] {
        pine(c, x + ox, 233, h, 0x34394e);
    }
    c.rect(x - 7, y - 1, 15, base - y, 0xc0a49a);
    c.rect(x + 3, y - 1, 5, base - y, 0x927b84);
    c.rect(x - 7, y + 13, 15, 5, 0xa96868);
    c.rect(x - 7, y + 14, 10, 3, 0xcb7771);
    c.rect(x - 2, base - 9, 5, 9, INK);
    c.rect(x - 4, y + 6, 3, 5, INK);
    c.rect(x - 4, y - 8, 9, 7, if lit { JADE } else { AMBER });
    c.rect(x, y - 8, 1, 7, 0x6a525b);
    c.poly(&[(x - 8, y - 8), (x, y - 15), (x + 8, y - 8)], INK);
    c.rect(x - 10, y - 1, 21, 2, INK);
    // The net is where the falling letter crosses the station's roofline.
    for k in -22..=22 {
        let sy = y + 3 - (k * k / 130);
        c.dot(x + k, sy, if lit { JADE } else { AMBER });
        if k % 5 == 0 {
            c.line(x + k, sy, x + k + 2, sy - 2, 0xe7b899);
        }
    }
    for k in [-23, 23] {
        c.rect(x + k, y - 7, 1, 10, INK);
        c.dot(x + k, y - 8, PEARL);
    }
    if !lit {
        let dy = triangle(t / 3, 14) / 3;
        c.line(x - 3, y - 25 - dy, x, y - 22 - dy, AMBER);
        c.line(x, y - 22 - dy, x + 3, y - 25 - dy, AMBER);
    }
    // Glints and a small numbered station plate.
    c.rect(x + 10, 231, 11, 8, INK);
    c.text(x + 13, 231, ["1", "2", "3"][id], 1, PEARL);
    for j in 0..3 {
        c.rect(x - 44 + j * 8, 243 + j * 2, 61 - j * 13, 1, 0x54516b);
    }
}

pub fn envelope(c: &mut Canvas, x: i32, y: i32, s: i32) {
    c.rect(x - 5 * s, y - 3 * s, 11 * s, 7 * s, INK);
    c.rect(x - 4 * s, y - 2 * s, 9 * s, 5 * s, PEARL);
    c.line(x - 4 * s, y - 2 * s, x, y + s, 0xb07872);
    c.line(x, y + s, x + 4 * s, y - 2 * s, 0xb07872);
    c.dot(x, y + s, CORAL);
}
