use crate::{H, W};
use pcp_game_guest::{digits, ink::Frame, rgb};
use sunspur_rally::{bend, Race, FINISH, GATES};
const SKY: i32 = 100;
fn mountain(x: i32, seed: i32, step: i32) -> i32 {
    let x = x + 18000;
    let at = |i: i32| {
        let v = (i as u32)
            .wrapping_mul(1664525)
            .wrapping_add(seed as u32 * 3119);
        ((v ^ (v >> 11)).wrapping_mul(2246822519) >> 26) as i32
    };
    let i = x.div_euclid(step);
    let f = x.rem_euclid(step);
    let a = at(i);
    a + (at(i + 1) - a) * f / step
}
pub fn sky(p: &mut [u32]) {
    for y in 0..H {
        for x in 0..W {
            let sun = (x - 295) * (x - 295) + (y - 53) * (y - 53);
            let mut c = rgb(0x637e99, 0xf5b985, y.min(SKY), SKY);
            if sun < 450 {
                c = 0xffe4a6;
            } else if sun < 680 {
                c = rgb(0xffd39a, c, sun - 450, 230);
            }
            let far = 57 + mountain(x, 77, 49) / 2;
            let mid = 73 + mountain(x, 19, 33) / 3;
            let near = 82 + mountain(x, 31, 21) / 4;
            if y >= far {
                c = 0xad7790;
            }
            if y >= mid {
                c = if y < (mid + 5) { 0xca8c76 } else { 0xaa6e68 };
            }
            if y >= near {
                c = 0xce997b;
            }
            if y >= SKY {
                c = 0xc78857;
            }
            p[(y * W + x) as usize] = c;
        }
    }
}
fn line(f: &mut Frame, x: i32, y: i32, w: i32, c: u32) {
    f.rect(x, y, w, 1, c)
}
// Scanline polygon fill for the original rear-view rally coupe.
fn poly(f: &mut Frame, p: &[(i32, i32)], c: u32) {
    let ymin = p.iter().map(|p| p.1).min().unwrap();
    let ymax = p.iter().map(|p| p.1).max().unwrap();
    for y in ymin..ymax {
        let mut l = 1000;
        let mut r = -1000;
        let mut a = p[p.len() - 1];
        for &b in p {
            let (lo, hi) = if a.1 <= b.1 { (a, b) } else { (b, a) };
            if y >= lo.1 && y < hi.1 {
                let x = lo.0 + (y - lo.1) * (hi.0 - lo.0) / (hi.1 - lo.1);
                l = l.min(x);
                r = r.max(x);
            }
            a = b;
        }
        if r >= l {
            line(f, l, y, r - l + 1, c);
        }
    }
}
fn coupe(f: &mut Frame, x: i32, y: i32, brake: bool) {
    let mut shape = |pts: &[(i32, i32)], c| {
        let mut a = [(0, 0); 14];
        for (k, &(i, j)) in pts.iter().enumerate() {
            a[k] = (x + i, y + j);
        }
        poly(f, &a[..pts.len()], c);
    };
    shape(
        &[(-48, 0), (-38, -5), (38, -5), (49, 0), (40, 4), (-39, 4)],
        0x353540,
    );
    shape(&[(-40, -12), (-30, -12), (-30, 0), (-40, 0)], 0x21222b);
    shape(&[(29, -12), (40, -12), (40, 0), (29, 0)], 0x21222b);
    shape(
        &[
            (-44, -9),
            (-44, -20),
            (-34, -25),
            (-26, -40),
            (-18, -44),
            (19, -44),
            (27, -40),
            (34, -25),
            (44, -19),
            (44, -9),
            (37, -5),
            (-37, -5),
        ],
        0x153637,
    );
    shape(
        &[
            (-41, -20),
            (-31, -25),
            (-24, -38),
            (-17, -42),
            (18, -42),
            (25, -38),
            (31, -25),
            (41, -20),
            (42, -14),
            (-41, -14),
        ],
        0x4bbab0,
    );
    shape(
        &[
            (-29, -26),
            (-22, -38),
            (-16, -40),
            (17, -40),
            (23, -37),
            (29, -26),
        ],
        0x192f3c,
    );
    shape(&[(-24, -28), (-18, -37), (11, -37), (-2, -28)], 0x69888e);
    shape(
        &[
            (-40, -20),
            (-29, -25),
            (29, -25),
            (41, -20),
            (36, -18),
            (-37, -18),
        ],
        0x97c5ac,
    );
    shape(&[(-9, -25), (-3, -25), (-4, -14), (-11, -14)], 0xf4b54c);
    shape(&[(1, -25), (6, -25), (9, -14), (3, -14)], 0xf4b54c);
    f.rect(x - 39, y - 17, 78, 7, 0x214448);
    f.rect(
        x - 36,
        y - 15,
        16,
        3,
        if brake { 0xffdd88 } else { 0xfb6b49 },
    );
    f.rect(
        x + 20,
        y - 15,
        16,
        3,
        if brake { 0xffdd88 } else { 0xfb6b49 },
    );
    f.rect(x - 7, y - 13, 14, 4, 0xecd59d);
    f.rect(x - 37, y - 7, 74, 2, 0x88969b);
}
fn center(r: &Race, d: i32) -> i32 {
    let cam = r.z - 600;
    let slope = bend(cam + 256) - bend(cam - 256);
    W / 2 + (bend(cam + d) - bend(cam) - slope * d / 512 - r.offset * 3 / 4) * 252 / d
}
pub fn draw(p: &mut [u32], base: &[u32], r: &Race, brake: bool, cover: bool, demo: bool) {
    p.copy_from_slice(base);
    let mut f = Frame {
        px: p,
        pitch: W as usize,
    };
    for y in SKY + 1..H {
        let d = 42000 / (y - SKY);
        let wz = r.z - 600 + d;
        let x = center(r, d);
        let half = (460 * 252 / d).max(1);
        let band = (wz / 350) & 1;
        let shoulder = (half / 11).max(1);
        line(&mut f, 0, y, W, if band == 0 { 0xd29661 } else { 0xd69c65 });
        line(
            &mut f,
            x - half - shoulder,
            y,
            2 * (half + shoulder) + 1,
            if band == 0 { 0xf1ca93 } else { 0xaf634b },
        );
        line(
            &mut f,
            x - half,
            y,
            2 * half + 1,
            if band == 0 { 0x56565c } else { 0x58585f },
        );
        if wz.rem_euclid(700) < 350 {
            line(&mut f, x - half / 2, y, (half / 64).max(1), 0xcac1a8);
            line(&mut f, x + half / 2, y, (half / 64).max(1), 0xcac1a8);
        }
    }
    for i in (1..=13).rev() {
        let z = (r.z / 950 + i) * 950;
        let d = z - r.z + 600;
        let y = SKY + 42000 / d;
        let side = if (z / 950) % 2 == 0 { 1 } else { -1 };
        let x = center(r, d) + side * (650 + (z / 950 % 4) * 50) * 252 / d;
        let h = (300 * 252 / d).max(2);
        f.rect(x, y - h, (h / 6).max(1), h, 0x4c644d);
        f.rect(x - h / 4, y - h * 3 / 4, h / 3, (h / 7).max(1), 0x4c644d);
        f.rect(x - h / 4, y - h * 7 / 8, (h / 8).max(1), h / 4, 0x4c644d);
    }
    for &(z, lane) in GATES.iter().rev() {
        let d = z - r.z + 600;
        if (430..12500).contains(&d) {
            let y = SKY + 42000 / d;
            let h = (320 * 252 / d).max(2);
            let x = center(r, d) + lane * 252 / d;
            let span = 150 * 252 / d;
            for side in [-1, 1] {
                let xx = x + side * span;
                f.rect(xx, y - h, (h / 10).max(1), h, 0x134b50);
                f.rect(xx, y - h, (h / 4).max(1), (h / 3).max(1), 0xf9df8f);
            }
        }
    }
    coupe(&mut f, W / 2 + r.offset / 10, H - 12, brake);
    if cover || r.finished {
        f.rect(12, 14, 209, 66, 0x25363b);
        f.rect(12, 14, 3, 66, 0xf9b94e);
        f.text(
            21,
            21,
            if r.finished {
                "FINISH LINE"
            } else {
                "SUNSPUR RALLY"
            },
            2,
            0xffdd9e,
        );
        if r.finished {
            let mut b = [0; 10];
            f.text(22, 46, "ADJUSTED SECONDS", 1, 0xe2e1cf);
            f.text(130, 46, digits(r.score() / 60, &mut b), 1, 0xffd791);
            f.text(22, 62, "ENTER  RACE AGAIN", 1, 0xcad5c4);
        } else {
            f.text(22, 44, "CINDER PASS / FIVE GATES", 1, 0xe2e1cf);
            f.text(22, 62, "ENTER  RACE     F2  DEMO", 1, 0xcad5c4);
        }
    } else {
        f.rect(8, 8, 110, 31, 0x25363b);
        f.text(14, 13, if demo { "DEMO / KMH" } else { "KMH" }, 1, 0xf1cf89);
        let mut b = [0; 10];
        f.text(
            75,
            13,
            digits((r.speed * 216 / 100) as u32, &mut b),
            1,
            0xffffff,
        );
        f.text(14, 26, "GATES", 1, 0xccd4c4);
        f.text(75, 26, digits(r.taken.count_ones(), &mut b), 1, 0xffffff);
        f.rect(126, 11, 132, 3, 0x655b58);
        f.rect(126, 11, r.z * 132 / FINISH, 3, 0xffcf82);
    }
}
