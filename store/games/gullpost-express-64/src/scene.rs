use crate::{
    game::*,
    ink::{mix, Frame},
    raster::{Camera, Painter},
};
const CREAM: u32 = 0xffedcf;
const GOLD: u32 = 0xffc76c;
const INK: u32 = 0x243b4b;
const CORAL: u32 = 0xc85042;
const TEAL: u32 = 0x346f70;
fn lit(c: u32, k: i32) -> u32 {
    let part = |s: u32| (((c >> s) & 255) as i32 * k / 256).min(255) as u32;
    (part(16) << 16) | (part(8) << 8) | part(0)
}
fn hash(n: i32) -> i32 {
    let mut x = n as u32;
    x = x.wrapping_mul(0x9e3779b9);
    ((x ^ (x >> 15)) & 32767) as i32
}
fn add(a: V, x: i32, y: i32, z: i32) -> V {
    v(a.x + x, a.y + y, a.z + z)
}
fn turn(p: V, a: i32, l: V) -> V {
    add(
        p,
        (l.x * cos(a) + l.z * sin(a)) / 256,
        l.y,
        (l.z * cos(a) - l.x * sin(a)) / 256,
    )
}
fn sky(f: &mut Frame, c: Camera, t: u32) {
    let line = c.horizon().clamp(8, H - 8);
    let heading = c.yaw().rem_euclid(256) * 5;
    let time = t;
    for y in 0..line {
        f.rect(0, y, W, 1, mix(0x77aeb7, 0xf5d8ad, y, line))
    }
    let sun_x = (423 - heading).rem_euclid(1280) - 30;
    let sun_y = line - 32;
    f.circ(sun_x, sun_y, 20, 0xffe2a6);
    f.rect(sun_x - 17, sun_y + 9, 39, 1, 0xfbdaa8);
    for i in 0..12 {
        let x =
            (i * 99 + hash(i) % 55 - heading + ((time / 37) % 1280) as i32).rem_euclid(1280) - 60;
        let y = line - 9 - hash(4 + i) % 47;
        let w = 18 + hash(i + 34) % 35;
        f.rect(x, y, w, 2, 0xffe9ca);
        f.rect(x + 6, y - 3, w / 2, 3, 0xffe9ca);
        f.rect(x + w / 3, y - 5, 8, 2, 0xffe9ca);
    }
    for layer in 0..2 {
        for x in 0..W {
            let phase = (x + heading).rem_euclid(1280);
            let h = (sin(phase * 2 / 5 + layer * 77) + 256) / 50
                + (sin(phase + 63 + layer * 9) + 256) / 95;
            let y = line - h + layer * 5;
            f.rect(
                x,
                y,
                1,
                line - y + 3,
                if layer == 0 { 0xa5aaa0 } else { 0x809a90 },
            )
        }
    }
    for y in line..H {
        f.rect(0, y, W, 1, mix(0x9dc6b4, 0x3b8687, y - line, H - line))
    }
    for i in 0..858 {
        let y = line + 2 + hash(i * 13 + 51) % (H - line - 2);
        let x = (hash(i * 47 + 2) - heading + ((time / (5 + i % 5) as u32) % 1280) as i32)
            .rem_euclid(1280)
            - 20;
        let w = 1 + (y - line) / 28 + hash(i + 17) % 9;
        let glint = (x - sun_x).abs() < 7 + (y - line) / 4;
        f.rect(
            x,
            y,
            w,
            1,
            if glint {
                0xefce92
            } else {
                mix(0x64a29d, 0xc5d7b6, i % 5, 6)
            },
        );
    }
}
fn prism(p: &mut Painter, o: V, a: i32, hx: i32, hh: i32, hz: i32, c: u32) {
    let b = [
        turn(o, a, v(-hx, 0, -hz)),
        turn(o, a, v(hx, 0, -hz)),
        turn(o, a, v(hx, 0, hz)),
        turn(o, a, v(-hx, 0, hz)),
    ];
    let t = b.map(|q| add(q, 0, hh, 0));
    p.poly(&t, lit(c, 268));
    p.poly(&b, lit(c, 145));
    for i in 0..4 {
        let j = (i + 1) % 4;
        p.poly(&[b[i], b[j], t[j], t[i]], lit(c, [212, 184, 164, 236][i]))
    }
}
fn cylinder(p: &mut Painter, o: V, y0: i32, y1: i32, r0: i32, r1: i32, c: u32) {
    let mut top = [V::default(); 8];
    let mut bottom = [V::default(); 8];
    for i in 0..8 {
        let theta = i as i32 * 32 + 16;
        let x = sin(theta);
        let z = cos(theta);
        let nx = sin(theta + 32);
        let nz = cos(theta + 32);
        let a = add(o, r0 * x / 256, y0, r0 * z / 256);
        let b = add(o, r0 * nx / 256, y0, r0 * nz / 256);
        let t = add(o, r1 * x / 256, y1, r1 * z / 256);
        let u = add(o, r1 * nx / 256, y1, r1 * nz / 256);
        top[i] = t;
        bottom[i] = a;
        p.poly(
            &[a, b, u, t],
            lit(c, [198, 205, 225, 254, 264, 248, 224, 204][i]),
        )
    }
    if r0 > 0 {
        p.poly(&bottom, lit(c, 145));
    }
    if r1 > 0 {
        p.poly(&top, lit(c, 269))
    }
}
fn turf(p: &mut Painter) {
    let shore = |i: usize, scale: i32, y: i32| {
        let (x, z) = SHORE[i % SHORE.len()];
        v(x * scale, y, z * scale)
    };
    for i in 0..SHORE.len() {
        let j = (i + 1) % SHORE.len();
        let a = shore(i, 64, TERRAIN);
        let b = shore(j, 64, TERRAIN);
        let c = shore(i, 68, 35);
        let d = shore(j, 68, 35);
        let e = shore(i, 69, 9);
        let f = shore(j, 69, 9);
        let l = shore(i, 74, -43);
        let r = shore(j, 74, -43);
        p.poly(
            &[v(0, TERRAIN, 0), a, b],
            lit(0x8fa970, 247 + (i as i32 * 9) % 14),
        );
        p.poly(&[a, b, d, c], lit(0xae885f, 222 + (i as i32 * 19) % 33));
        p.poly(&[c, d, f, e], lit(0xb2775c, 219 + (i as i32 * 7) % 28));
        p.poly(&[e, f, r, l], 0xe0bd89);
        let u = shore(i, 76, -44);
        let w = shore(j, 76, -44);
        p.poly(&[l, r, w, u], 0xd6d9b9);
    }
    // Packed-earth path from the pier to the shuttered boathouse and lighthouse.
    let trail = [
        (0, -523, 73),
        (-18, -395, 71),
        (11, -263, 65),
        (-12, 0, 62),
        (-100, 264, 65),
        (-186, 393, 74),
    ];
    for seg in trail.windows(2) {
        let (x, z, w) = seg[0];
        let (a, b, n) = seg[1];
        p.poly(
            &[
                v(x - w, 82, z),
                v(x + w, 82, z),
                v(a + n, 82, b),
                v(a - n, 82, b),
            ],
            0xc4b285,
        )
    }
    // This is one solid with one grass cap. Two nearly coincident top faces shimmer at grazing angles.
    let terrace = [
        v(TERRACE.l, TERRACE.top, TERRACE.a),
        v(TERRACE.r, TERRACE.top, TERRACE.a),
        v(TERRACE.r, TERRACE.top, TERRACE.b),
        v(TERRACE.l, TERRACE.top, TERRACE.b),
    ];
    p.poly(&terrace, 0xa6b67b);
    for i in 0..4 {
        let a = terrace[i];
        let b = terrace[(i + 1) % 4];
        p.poly(
            &[v(a.x, 81, a.z), v(b.x, 81, b.z), b, a],
            lit(0xc6aa81, [212, 184, 164, 236][i]),
        );
    }
    for i in 0..46 {
        let x = hash(i * 23 + 19) % 1360 - 680;
        let z = hash(i * 47 + 2) % 1490 - 570;
        if surface(x, z, 999) != TERRAIN || x.abs() < 78 {
            continue;
        }
        p.poly(
            &[
                v(x, 83, z),
                v(x + 7, 83, z + 2),
                v(x + 3, 91 + i % 8, z + 1),
            ],
            0x738e60,
        );
        if i % 3 == 0 {
            prism(p, v(x + 13, 81, z - 6), i * 13, 3, 8, 3, 0xe8c17b)
        }
    }
}
fn piers(p: &mut Painter) {
    for block in PIERS {
        let x = (block.l + block.r) / 2;
        let z = (block.a + block.b) / 2;
        let hz = (block.b - block.a) / 2;
        prism(p, v(x, 42, z), 0, 145, 22, hz, 0xaa7957);
        for (i, z) in (block.a + 3..block.b).step_by(34).enumerate() {
            let end = (z + 31).min(block.b);
            prism(
                p,
                v(0, 64, (z + end) / 2),
                0,
                145,
                2,
                (end - z) / 2,
                [0xc89a72, 0xd9aa7c, 0xc7976c, 0xd2a078][i % 4],
            );
        }
        for (i, z) in (block.a + 14..block.b).step_by(148).enumerate() {
            for x in [-142, 142] {
                prism(p, v(x, -53, z), 0, 7, 133, 7, 0x846854);
                prism(p, v(x, 75, z), 0, 9, 8, 9, 0xd0b187);
                if i % 2 == 0 {
                    prism(p, v(x, 66, z), 0, 9, 5, 9, 0x576c68)
                }
            }
        }
    }
}
fn pennant(p: &mut Painter, o: V, angle: i32, color: u32, t: u32) {
    prism(p, o, 0, 3, 97, 3, 0xb39574);
    let wave = sin((t as i32 * 3 + angle) & 255) / 30;
    p.poly(
        &[add(o, 2, 95, 0), add(o, 49, 80, wave), add(o, 2, 64, 0)],
        color,
    )
}
fn boathouse(p: &mut Painter) {
    let o = v(368, 80, -115);
    prism(p, o, 0, 174, 198, 213, 0xd8ae80);
    prism(p, add(o, 0, 197, 0), 0, 181, 12, 219, TEAL);
    // Painted fascia, seamed metal roof and a little enamel envelope over the door.
    prism(p, add(o, 0, 187, -215), 0, 176, 10, 3, CREAM);
    prism(p, add(o, 0, 170, -217), 0, 176, 9, 3, CORAL);
    for y in [106, 126, 146, 166, 186, 206, 226] {
        p.poly(
            &[
                v(193, y, -331),
                v(542, y, -331),
                v(542, y + 2, -331),
                v(193, y + 2, -331),
            ],
            0xb49774,
        );
    }
    prism(p, v(368, 220, -335), 0, 34, 20, 3, TEAL);
    p.poly(
        &[v(340, 237, -340), v(396, 237, -340), v(368, 225, -340)],
        CREAM,
    );
    for i in 0..6 {
        prism(p, add(o, -148 + i * 59, 209, 0), 0, 1, 2, 213, 0x477978)
    }
    for x in [225, 332, 451] {
        prism(p, v(x, 128, -331), 0, 27, 72, 3, 0xf2d09c);
        prism(p, v(x, 134, -335), 0, 21, 60, 2, 0x315e68);
        prism(p, v(x, 159, -338), 0, 21, 4, 2, 0xd5ac80);
        prism(p, v(x, 136, -339), 0, 2, 58, 2, 0xd5ac80)
    }
    prism(p, v(191, 82, -169), 0, 3, 131, 41, 0x47777a);
    for z in [-268, -58, 60] {
        prism(p, v(190, 120, z), 0, 4, 58, 21, 0xe8cb9d);
        prism(p, v(185, 128, z), 0, 2, 42, 15, 0x527986)
    }
    let crate_center = v((CRATE.l + CRATE.r) / 2, 80, (CRATE.a + CRATE.b) / 2);
    prism(p, crate_center, 0, 68, 83, 73, 0xc48d59);
    for dx in [-47, 47] {
        prism(p, add(crate_center, dx, 0, 0), 0, 5, 85, 74, 0xebc193)
    }
}
fn lighthouse(p: &mut Painter, t: u32) {
    let o = BEACON;
    for (lo, hi, r, u, c) in [
        (0, 22, 126, 126, 0x788487),
        (22, 45, 123, 111, 0xe9d8b7),
        (45, 139, 111, 104, 0xf7e6c5),
        (139, 209, 104, 97, CORAL),
        (209, 307, 97, 88, 0xf6e6c8),
        (307, 375, 88, 82, CORAL),
        (375, 427, 82, 76, 0xf9e7c6),
        (427, 443, 100, 100, 0x354e59),
        (443, 456, 95, 87, 0x6d8278),
        (456, 516, 60, 60, 0xedbf72),
        (516, 527, 84, 82, 0x334e57),
        (527, 580, 85, 9, 0xb75345),
        (580, 599, 7, 1, 0x374e57),
    ] {
        cylinder(p, o, lo, hi, r, u, c)
    }
    for i in 0..8 {
        let a = i * 32 + 16;
        prism(
            p,
            add(o, 60 * sin(a) / 256, 454, 60 * cos(a) / 256),
            0,
            4,
            63,
            4,
            0x345360,
        )
    }
    let a = (t as i32 * 3) & 255;
    prism(
        p,
        add(o, 61 * sin(a) / 256, 470, 61 * cos(a) / 256),
        a,
        11,
        25,
        3,
        0xffeda7,
    );
    prism(p, add(o, 0, 24, -106), 0, 21, 69, 5, 0x31636a);
    for (y, z) in [(234, -92), (383, -75)] {
        prism(p, add(o, 0, y, z), 0, 14, 28, 3, INK);
        prism(p, add(o, 0, y + 2, z - 3), 0, 8, 22, 2, 0xeebe7b)
    }
    let b = BOX;
    prism(p, b, 0, 6, 45, 6, 0x745d4e);
    prism(p, add(b, 0, 41, 0), 0, 25, 40, 18, CORAL);
    prism(p, add(b, 0, 62, -20), 0, 15, 5, 2, INK);
    prism(p, add(b, 0, 78, 0), 0, 26, 6, 19, 0xf5a56b);
}
fn spruce(p: &mut Painter, x: i32, z: i32, h: i32) {
    let o = v(x, TERRAIN, z);
    prism(p, o, 0, 6, h / 2, 6, 0x8d7460);
    cylinder(p, o, h / 5, h * 3 / 4, h / 4, 1, 0x427d72);
    cylinder(p, o, h / 2, h, h / 5, 0, 0x608e76)
}
fn boat(p: &mut Painter, x: i32, z: i32, t: u32) {
    let o = v(x, -40 + sin(t as i32 * 2 + x) / 40, z);
    cylinder(p, o, 0, 22, 14, 31, 0xa45b48);
    prism(p, add(o, 0, 20, 0), 0, 2, 89, 2, 0x765e51);
    p.poly(
        &[add(o, -2, 104, 0), add(o, -4, 29, 0), add(o, -56, 29, 0)],
        CREAM,
    );
    p.poly(
        &[add(o, 5, 89, 0), add(o, 42, 30, 0), add(o, 5, 30, 0)],
        0xd99167,
    )
}
fn bird(p: &mut Painter, g: &Game) {
    let o = g.pos;
    let a = g.facing;
    // Tapered tail and layered wing feathers keep the courier readable from behind.
    let feather =
        |p: &mut Painter, points: &[V; 3], col| p.poly(&points.map(|l| turn(o, a, l)), col);
    for i in -1..=1 {
        feather(
            p,
            &[
                v(i * 7 - 6, 31, -12),
                v(i * 7 + 8, 31, -12),
                v(i * 9, 35, -43 + i.abs() * 5),
            ],
            0xf1e3c7,
        );
        feather(
            p,
            &[
                v(i * 7 - 4, 32, -28),
                v(i * 7 + 4, 32, -28),
                v(i * 9, 35, -43 + i.abs() * 5),
            ],
            0x4d686d,
        );
    }
    let flap = if g.grounded {
        0
    } else {
        8 + sin(g.tick as i32 * 11) / 25
    };
    let model = |p: &mut Painter, x, y, z, hx, hh, hz, c| {
        prism(p, turn(o, a, v(x, y, z)), a, hx, hh, hz, c)
    };
    let floor = surface(o.x, o.z, o.y + 10);
    if floor > -1000 {
        cylinder(p, v(o.x, floor + 2, o.z), 0, 1, 23, 23, 0x77816a)
    }
    let gait = if g.grounded && (g.velocity.x.abs() + g.velocity.z.abs() > 2) {
        sin(g.tick as i32 * 14) / 34
    } else {
        0
    };
    for side in [-1, 1] {
        model(p, side * 12, 3, side * gait + 6, 7, 6, 11, 0xeb9e4c);
        model(p, side * 12, 9, side * gait, 3, 10, 4, 0xb77a41);
    }
    model(p, 0, 15, 0, 20, 25, 17, 0xe9d7af);
    model(p, 0, 27, 1, 23, 32, 18, 0x487f80);
    for side in [-1, 1] {
        model(
            p,
            side * (22 + flap / 2),
            30 + flap,
            -4,
            5 + flap / 2,
            21,
            14,
            0xacc9bb,
        )
    }
    for side in [-1, 1] {
        let tip = side * (30 + flap);
        feather(
            p,
            &[
                v(side * 26, 48, -7),
                v(tip, 23 + flap, -25),
                v(side * 19, 29, -22),
            ],
            0x778f89,
        );
        feather(
            p,
            &[
                v(side * 26, 46, -11),
                v(tip, 24 + flap, -27),
                v(side * 22, 30, -23),
            ],
            0xe8e0c7,
        );
    }
    model(p, 0, 54, 1, 20, 6, 18, CORAL);
    model(p, -11, 48, -22, 7, 8, 13, 0xc35a44);
    model(p, 0, 58, 5, 19, 29, 19, 0xffedcf);
    model(p, 0, 62, 29, 13, 9, 10, 0xf3a64d);
    for side in [-1, 1] {
        model(p, side * 18, 74, 16, 2, 5, 3, INK)
    }
    model(p, 0, 84, 4, 21, 7, 21, INK);
    model(p, 0, 90, 3, 17, 8, 18, TEAL);
    model(p, 0, 84, 22, 18, 4, 10, INK);
    model(p, 15, 26, -17, 10, 20, 6, 0xa96940);
    model(p, 15, 41, -20, 11, 6, 5, 0xe4aa68);
}
fn stamp(p: &mut Painter, o: V, i: i32, t: u32) {
    let a = sin((t as i32 * 2 + i * 51) & 255) / 9;
    let o = add(o, 0, sin((t as i32 * 3 + i * 57) & 255) / 34, 0);
    // The hollow diamond identifies a stamp even when the gold face blends with the pier.
    let halo = add(o, 0, 0, 7);
    for i in 0..4 {
        let corner = |idx: i32, r: i32| {
            turn(
                halo,
                a,
                v(sin(idx * 64) * r / 256, cos(idx * 64) * r / 256, 0),
            )
        };
        p.poly(
            &[
                corner(i, 40),
                corner(i + 1, 40),
                corner(i + 1, 36),
                corner(i, 36),
            ],
            CREAM,
        );
    }
    prism(p, add(o, 0, -24, 0), a, 22, 48, 5, GOLD);
    for side in [-1, 1] {
        let b = turn(o, a, v(0, -16, side * 6));
        prism(p, b, a, 15, 32, 2, 0xbf634c);
        prism(p, turn(b, a, v(0, 10, side * 2)), a, 10, 9, 1, CREAM)
    }
}
fn stamp_icon(f: &mut Frame, x: i32, y: i32, full: bool) {
    f.rect(x, y, 12, 15, if full { GOLD } else { 0x638082 });
    f.rect(x + 2, y + 2, 8, 11, if full { CORAL } else { 0x405c63 });
    for k in 0..4 {
        f.rect(x, y + 1 + k * 4, 1, 1, INK);
        f.rect(x + 11, y + 1 + k * 4, 1, 1, INK)
    }
    if full {
        f.rect(x + 4, y + 6, 4, 3, CREAM)
    }
}
fn panel(f: &mut Frame, x: i32, y: i32, w: i32, h: i32) {
    f.rect(x + 3, y + 4, w, h, 0x485e60);
    f.rect(x, y, w, h, CREAM);
    f.rect(x + 2, y + 2, w - 4, h - 4, INK)
}
fn card(f: &mut Frame) {
    panel(f, 13, 20, 215, 143);
    f.rect(16, 23, 209, 5, 0x466e70);
    for k in 0..12 {
        f.rect(18 + k * 18, 23, 9, 5, 0xdb8263)
    }
    f.text(24, 38, "GULLPOST", 4, CREAM);
    f.text(24, 71, "EXPRESS", 4, GOLD);
    f.rect(194, 71, 25, 27, CORAL);
    f.text(196, 77, "64", 2, CREAM);
    f.text(22, 113, "LIGHTHOUSE ISLAND", 2, CREAM);
    f.text(25, 143, "THREE POSTMARKS / ONE DELIVERY", 1, 0xa4c1b5);
    panel(f, 13, 224, 241, 47);
    f.rect(22, 232, 68, 13, CORAL);
    f.text(27, 235, "SPACE/ENTER", 1, CREAM);
    f.text(98, 235, "PLAY THE ISLAND", 1, CREAM);
    f.text(24, 254, "F2  WATCH THE COURIER RUN THE ROUTE", 1, GOLD);
}
fn bearing(f: &mut Frame, g: &Game) {
    let target = SEALS
        .iter()
        .enumerate()
        .find(|(i, _)| g.mask & (1 << i) == 0)
        .map_or(BOX, |(_, s)| *s);
    let dx = target.x - g.pos.x;
    let dz = target.z - g.pos.z;
    let side = (dx * cos(g.yaw) - dz * sin(g.yaw)) / 256;
    let ahead = (dx * sin(g.yaw) + dz * cos(g.yaw)) / 256;
    let scale = side.abs().max(ahead.abs()).max(1);
    let x = 8 * side / scale;
    let y = -8 * ahead / scale;
    let (cx, cy) = (438, 18);
    f.circ(cx, cy, 11, INK);
    f.rect(cx - 1, cy - 10, 3, 1, CREAM);
    f.rect(cx - 1, cy + 9, 3, 1, 0x61817e);
    if dx.abs() + dz.abs() < 25 {
        f.circ(cx, cy, 2, GOLD);
        return;
    }
    for i in 0..=8 {
        f.rect(cx + x * i / 8, cy + y * i / 8, 2, 2, GOLD)
    }
    let tip = (cx + x, cy + y);
    for sign in [-1, 1] {
        for i in 0..=4 {
            let tx = tip.0 + (-x + sign * y) * i / 16;
            let ty = tip.1 + (-y - sign * x) * i / 16;
            f.rect(tx, ty, 2, 2, GOLD)
        }
    }
}
fn hud(f: &mut Frame, g: &Game) {
    bearing(f, g);
    panel(f, 8, 8, 132, 21);
    f.text(15, 15, "LIGHTHOUSE ISLAND", 1, CREAM);
    for i in 0..3 {
        stamp_icon(f, 453 + i * 17, 11, g.mask & (1 << i) != 0)
    }
    let text = if g.mask == 7 {
        "DELIVER TO THE RED POSTBOX"
    } else if g.mask & 1 == 0 {
        "STAMP 1 / CROSS THE PIER"
    } else if g.mask & 2 == 0 {
        "STAMP 2 / BOATHOUSE ROOF"
    } else {
        "STAMP 3 / BEACON TERRACE"
    };
    f.rect(10, 34, text.len() as i32 * 6 + 8, 13, INK);
    f.text(14, 37, text, 1, GOLD);
    if g.elapsed < 190 && !g.demo {
        f.rect(0, 270, W, 18, INK);
        f.text(
            12,
            276,
            "WASD MOVE   SPACE JUMP / AIR FLAP   Q/E CAMERA   P PAUSE",
            1,
            CREAM,
        );
    }
    if g.demo {
        f.rect(9, 257, 72, 15, INK);
        f.text(15, 261, "GUIDED RUN", 1, GOLD)
    }
    if g.notice > 0 && g.last == Sfx::Flag && g.checkpoint {
        f.rect(175, 244, 161, 16, INK);
        f.text(182, 249, "SHORE CHECKPOINT REACHED", 1, GOLD)
    }
}
pub fn draw(f: &mut Frame, g: &Game, z: &mut [i32]) {
    let cover = g.mode == Mode::Cover;
    let cam = if cover {
        Camera::postcard()
    } else {
        Camera::follow(g)
    };
    sky(f, cam, g.tick);
    z.fill(0);
    let mut p = Painter {
        f,
        depth: z,
        camera: cam,
    };
    turf(&mut p);
    piers(&mut p);
    boathouse(&mut p);
    lighthouse(&mut p, g.tick);
    for &(x, z, h) in &[
        (-484, 159, 197),
        (-559, 264, 128),
        (-451, 341, 168),
        (546, 381, 184),
        (500, 521, 116),
        (-372, 908, 146),
        (355, 804, 149),
        (-479, -135, 128),
    ] {
        spruce(&mut p, x, z, h)
    }
    for &(x, z) in &[(-1120, -18), (1150, 765), (-2090, 1840)] {
        boat(&mut p, x, z, g.tick)
    }
    pennant(&mut p, v(-121, 67, -1159), 0, CORAL, g.tick);
    pennant(&mut p, POST, 63, TEAL, g.tick);
    pennant(&mut p, add(BOX, 50, 0, 10), 3, CORAL, g.tick);
    for (i, &s) in SEALS.iter().enumerate() {
        if g.mask & (1 << i) == 0 {
            stamp(&mut p, s, i as i32, g.tick)
        }
    }
    bird(&mut p, g);
    match g.mode {
        Mode::Cover => card(f),
        Mode::Run => hud(f, g),
        Mode::Pause => {
            hud(f, g);
            panel(f, 122, 92, 268, 95);
            f.text(145, 108, "ON SHORE LEAVE", 2, CREAM);
            f.text(145, 139, "P / SPACE   RESUME", 1, GOLD);
            f.text(145, 159, "ESC         RETURN TO PCP OS", 1, CREAM)
        }
        Mode::Delivered => {
            panel(f, 111, 72, 290, 146);
            f.text(127, 84, "LIGHTHOUSE ISLAND / POST OFFICE", 1, GOLD);
            f.text(127, 106, "DELIVERED", 4, CREAM);
            for i in 0..3 {
                stamp_icon(f, 128 + i * 19, 151, true)
            }
            f.text(191, 155, "ALL THREE POSTMARKS", 1, GOLD);
            f.text(128, 178, "THE KEEPER HAS YOUR LETTER.", 1, CREAM);
            f.text(128, 197, "SPACE / ENTER   BACK TO TITLE", 1, 0x9bbdb0)
        }
    }
}
