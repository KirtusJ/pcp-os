//! Cached architecture, lamp reflections and rink markings for the night arena.
use super::arena::{cam, focal, project, TILT};
use crate::{H, W};
use flux_rink::{Settings, GOAL, LENGTH, WIDTH};
use heliograph::assets::sin;
use pcp_game_guest::{ink::Frame, rgb};
const GOLD: u32 = 0xff9f56;
const MINT: u32 = 0x68f0de;
fn hash(x: i32, y: i32) -> u32 {
    let mut n = (x as u32).wrapping_mul(0x85ebca6b) ^ (y as u32).wrapping_mul(0xc2b2ae35);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^ (n >> 15)
}
fn add(c: u32, r: i32, g: i32, b: i32) -> u32 {
    let ch = |s: u32, v: i32| (((c >> s) & 255) as i32 + v).clamp(0, 255) as u32;
    (ch(16, r) << 16) | (ch(8, g) << 8) | ch(0, b)
}
fn line(f: &mut Frame, a: (i32, i32), b: (i32, i32), c: u32, w: i32) {
    let dx = (b.0 - a.0).abs();
    let dy = (b.1 - a.1).abs();
    let n = dx.max(dy).max(1);
    for i in 0..=n {
        f.rect(
            a.0 + (b.0 - a.0) * i / n,
            a.1 + (b.1 - a.1) * i / n,
            w,
            w,
            c,
        );
    }
}
fn circle(f: &mut Frame, x: i32, y: i32, rx: i32, ry: i32, c: u32) {
    for yy in -ry..=ry {
        let mut xx = rx;
        while xx > 0 && xx * xx * ry * ry + yy * yy * rx * rx > rx * rx * ry * ry {
            xx -= 1;
        }
        f.rect(x - xx, y + yy, 2 * xx + 1, 1, c);
    }
}
fn board(f: &mut Frame, x: i32, w: i32, label: &str, c: u32) {
    f.rect(x, 69, w, 13, 0x111a2c);
    f.rect(x, 82, w, 2, c);
    f.text(x + 5, 72, label, 1, c);
}
fn atmosphere(f: &mut Frame) {
    for y in 0..H {
        for x in 0..W {
            let edge =
                ((x - W / 2).abs() * 27 / (W / 2) + (y - H / 2).abs() * 16 / (H / 2)).min(52);
            let mut c = rgb(0x2d304a, 0x101c31, y, H);
            let warm = (150 - (x - 94).abs() - (y - 40).abs()).max(0) / 8;
            let cool = (180 - (x - 551).abs() - (y - 42).abs()).max(0) / 9;
            c = add(
                c,
                warm + 4 - edge / 4,
                warm / 3 + cool / 2 - edge / 4,
                cool + 6 - edge / 4,
            );
            f.px[(y * W + x) as usize] = c;
        }
    }
    for layer in 0..2 {
        for i in 0..48 {
            let h = (hash(i, layer) % 22) as i32 + 7;
            let x = i * 15 - 20 + layer * 8;
            let base = 54 + layer * 8;
            let c = if layer == 0 { 0x32394d } else { 0x222f43 };
            f.rect(x, base - h, 11 + ((hash(i, 7) % 4) as i32), h, c);
            for yy in (base - h + 3..base - 1).step_by(4) {
                for xx in (x + 2..x + 10).step_by(4) {
                    if hash(xx, yy) % 5 == 0 {
                        f.rect(xx, yy, 2, 1, 0x6a6873);
                    }
                }
            }
        }
    }
    // Back-wall spectators sit in staggered rows behind the advertisement rail.
    for y in 0..4 {
        let base = 49 + y * 6;
        f.rect(0, base + 3, W, 2, 0x212c40);
        for j in 0..118 {
            let x = j * 6 + (y % 2) * 3;
            let n = hash(j, y);
            if n % 5 != 0 {
                f.rect(
                    x,
                    base - 1,
                    2,
                    2,
                    [0x677484, 0x778487, 0x646b83, 0xa58a79][(n % 4) as usize],
                );
                f.rect(
                    x - 1,
                    base + 1,
                    4,
                    2,
                    [0x405067, 0x665579, 0x326269, 0x684e52][((n >> 4) % 4) as usize],
                );
            }
        }
    }
    for x in (0..W + 60).step_by(64) {
        line(f, (x - 28, 1), (x + 16, 29), 0x101b2a, 3);
        line(f, (x + 16, 29), (x + 38, 1), 0x121f31, 3);
    }
    f.rect(0, 1, W, 3, 0x152536);
    f.rect(0, 29, W, 2, 0x293d4b);
    for (j, x) in [38, 158, 278, 400, 518, 611].iter().enumerate() {
        let tint = if j < 3 { 0xffc998 } else { 0xadfff3 };
        f.rect(*x - 13, 25, 26, 5, 0x495563);
        f.rect(*x - 11, 27, 22, 2, tint);
    }
    // Broad falloff and narrow cores make each ceiling light read as a continuous source.
    for y in 31..135 {
        let dy = y - 29;
        for (i, &lamp) in [48, 167, 486, 598].iter().enumerate() {
            let axis = lamp + if i < 2 { dy / 2 } else { -dy / 2 };
            let width = 4 + dy / 4;
            for x in (axis - width).max(0)..=(axis + width).min(W - 1) {
                let v = (width - (x - axis).abs()) * (140 - y) / (width * 8);
                let k = (y * W + x) as usize;
                f.px[k] = if i < 2 {
                    add(f.px[k], v, v * 4 / 5, v / 2)
                } else {
                    add(f.px[k], v / 2, v, v)
                };
            }
        }
    }
    for i in 0..8 {
        board(
            f,
            i * 82,
            78,
            ["NORTH DOCK", "FLUX  /  05", "NIGHT CUP", "PORT ARENA"][i as usize % 4],
            if i % 2 == 0 { 0xc88f6e } else { 0x75b4b4 },
        );
    }
    for x in (0..W).step_by(26) {
        line(f, (x, 92), (x - 30, H), 0x1e2b3f, 1);
    }
}
fn surface(x: i32, z: i32, detail: bool) -> u32 {
    let ax = x.abs();
    let az = z.abs();
    let left = z < 0;
    let team = if left { GOLD } else { MINT };
    let n = (hash(x / 7, z / 7) % 8) as i32;
    let edge = (WIDTH - ax).min(LENGTH - az);
    let mut c = rgb(
        0x234454,
        0x617d85,
        (x + WIDTH).clamp(0, 2 * WIDTH),
        2 * WIDTH,
    );
    let lamp = (320 - ((z + 1170) - (x / 3)).abs()).max(0) / 15
        + (170 - ((z - 690) - (x / 3)).abs()).max(0) / 10;
    let end = ((az - 1050).max(0) / 19).min(55);
    let fall = (170 - edge).clamp(0, 170) / 12;
    let v = if detail { lamp + n - 4 } else { 0 };
    c = add(
        c,
        v + if left { end * 2 } else { -end / 4 } - fall,
        v + if left { end / 2 } else { end } - fall,
        v + if left { -end / 7 } else { end } - fall,
    );
    if detail {
        let g = (x + z / 4 + 8843).rem_euclid(263);
        if g < 6 {
            c = rgb(c, 0xa4d8d3, 1, 6);
        }
        let scratch = (x * 7 + z * 3 + (z / 340) * 29).rem_euclid(601);
        if scratch < 6 && hash(z / 185, x / 83) % 3 == 0 {
            c = add(c, 21, 22, 23);
        }
        let reflected =
            (45 - (z + (x / 4) + 980).abs()).max(0) + (65 - (z + (x / 4) - 670).abs()).max(0);
        if x > 180 {
            let amount = reflected * (x - 180) / 2600;
            c = add(c, amount, amount, amount);
        }
    }
    let disk = x * x + z * z;
    let arc = |sq: i32, r: i32, w: i32| sq >= (r - w) * (r - w) && sq <= (r + w) * (r + w);
    // Centre crest: a cut brass compass, two inset rings and a diagonal lightning stroke.
    if disk < 345 * 345 {
        c = rgb(c, 0x203b4b, 2, 3);
    }
    if arc(disk, 353, 9) || arc(disk, 323, 4) {
        c = 0xc3ad86;
    }
    if disk < 212 * 212 {
        let bolt = [
            (-30, -184),
            (88, -184),
            (21, -38),
            (114, -38),
            (-60, 179),
            (-12, 42),
            (-112, 42),
        ];
        let mut inside = false;
        let mut a = bolt[6];
        for b in bolt {
            if (a.1 > z) != (b.1 > z) && x < a.0 + (z - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                inside = !inside;
            }
            a = b;
        }
        if inside {
            c = 0xf3c282;
        }
    }
    if (az < 11 && disk > 367 * 367) || (az - 928).abs() < 9 {
        c = if az < 11 {
            0xd6e4db
        } else {
            rgb(c, team, 4, 5)
        };
    }
    let goal = x * x + (az - LENGTH) * (az - LENGTH);
    if goal < 503 * 503 {
        c = rgb(c, team, 1, 5);
    }
    if arc(goal, 503, 13) {
        c = team;
    }
    for zone in [-1, 1] {
        let dd = (x - zone * 710) * (x - zone * 710) + (az - 1130) * (az - 1130);
        if arc(dd, 180, 7) {
            c = rgb(c, team, 3, 4);
        }
        if dd < 29 * 29 {
            c = team;
        }
    }
    if edge < 0 {
        let band = -edge;
        c = if band < 16 {
            0xf4ede2
        } else if band < 37 {
            team
        } else if band < 54 {
            0x222b37
        } else if band < 78 {
            rgb(team, 0x273a49, 2, 3)
        } else {
            0x223143
        };
        if az > LENGTH && ax < GOAL {
            c = if band < 32 {
                team
            } else if (x / 32 + z / 27).rem_euclid(2) == 0 {
                0x2a4456
            } else {
                0x172b3b
            };
        }
        if detail && band > 55 && (x + z).rem_euclid(215) < 15 {
            c = 0x9eaeb2;
        }
    }
    c
}
pub fn bake(p: &mut [u32], s: &Settings) {
    let f = &mut Frame {
        px: p,
        pitch: W as usize,
    };
    atmosphere(f);
    let focal = focal(s);
    for y in 0..H {
        let dy = H / 2 - y;
        let den = focal * sin(TILT) - dy * sin(TILT + 256);
        if den <= 0 {
            continue;
        }
        let x = ((dy as i64 * 7200 - 155 * focal as i64) * 16384 / den as i64) as i32;
        if x.abs() > WIDTH + 104 {
            continue;
        }
        let dep = cam(x, 0)[2];
        let extent = (LENGTH + 112) * focal / dep;
        for col in (W / 2 - extent).max(0)..=(W / 2 + extent).min(W - 1) {
            let z = (col - W / 2) * dep / focal;
            f.px[(y * W + col) as usize] = surface(x, z, s.detail);
        }
    }
    // The near apron has its own vertical thickness, fasteners and a lower kick rail.
    let a = project(-WIDTH - 98, -LENGTH - 108, focal);
    let b = project(-WIDTH - 98, LENGTH + 108, focal);
    for dy in 1..11 {
        line(
            f,
            (a.0, a.1 + dy),
            (b.0, b.1 + dy),
            rgb(0x526373, 0x172739, dy, 11),
            1,
        );
    }
    for x in (a.0 + 7..b.0 - 7).step_by(19) {
        f.rect(x, a.1 + 4, 2, 2, 0x8b9491);
    }
    for side in [-1, 1] {
        let c = if side < 0 { GOLD } else { MINT };
        let a = project(-GOAL, side * (LENGTH + 66), focal);
        let b = project(GOAL, side * (LENGTH + 66), focal);
        let ra = (a.0 + side * 8, a.1 - 11);
        let rb = (b.0 + side * 7, b.1 - 9);
        for j in 0..13 {
            let t = |a: i32, b: i32| a + (b - a) * j / 12;
            line(
                f,
                (t(a.0, b.0), t(a.1, b.1)),
                (t(ra.0, rb.0), t(ra.1, rb.1)),
                0x8b9a99,
                1,
            );
        }
        for j in 1..4 {
            line(
                f,
                (a.0 + (ra.0 - a.0) * j / 4, a.1 + (ra.1 - a.1) * j / 4),
                (b.0 + (rb.0 - b.0) * j / 4, b.1 + (rb.1 - b.1) * j / 4),
                0x526c75,
                1,
            );
        }
        line(f, a, ra, c, 2);
        line(f, b, rb, c, 2);
        line(f, ra, rb, c, 2);
    }
}
pub fn under(f: &mut Frame, x: i32, y: i32, r: i32, c: u32, detail: bool) {
    if detail {
        for yy in -r / 3..=r / 2 {
            for xx in -r * 2..=r * 2 {
                let d = xx * xx / 4 + yy * yy * 3;
                let n = r * r - d;
                if n > 0 {
                    let k = (y + yy + 4) * W + x + xx;
                    if k >= 0 && k < W * H {
                        let k = k as usize;
                        f.px[k] = rgb(f.px[k], c, n * 40 / (r * r), 160);
                    }
                }
            }
        }
    }
    circle(f, x + 2, y + 2, r, r / 4, 0x294653);
    circle(f, x + 1, y + 1, r * 3 / 4, r / 5, 0x182e3a);
}
