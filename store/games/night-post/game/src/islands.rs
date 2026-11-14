//! Pixel scenery for the open-water and northern routes.
use crate::{paint::*, scenery::post, triangle, Game, Route, W};

fn ellipse(c: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, col: Color) {
    let mut edge = 0;
    for dy in (0..=ry).rev() {
        while edge < rx
            && (edge + 1) * (edge + 1) * ry * ry + dy * dy * rx * rx <= rx * rx * ry * ry
        {
            edge += 1;
        }
        c.rect(x - edge, y - dy, 2 * edge + 1, 1, col);
        if dy != 0 {
            c.rect(x - edge, y + dy, 2 * edge + 1, 1, col);
        }
    }
}
pub fn star(c: &mut Canvas, x: i32, y: i32, r: i32, col: Color) {
    c.line(x - r, y, x + r, y, col);
    c.line(x, y - r, x, y + r, col);
    c.dot(x + 1, y + 1, col);
    c.dot(x - 1, y - 1, col);
}
fn swell(x: i32) -> i32 {
    const WAVE: [i32; 32] = [
        0, 3, 6, 9, 11, 13, 15, 16, 16, 16, 15, 13, 11, 9, 6, 3, 0, -3, -6, -9, -11, -13, -15, -16,
        -16, -16, -15, -13, -11, -9, -6, -3,
    ];
    let i = x.div_euclid(8).rem_euclid(32) as usize;
    WAVE[i] + (WAVE[(i + 1) % 32] - WAVE[i]) * x.rem_euclid(8) / 8
}
fn northern_lights(c: &mut Canvas, d: i32, t: i32) {
    for x in 0..W as i32 {
        let phase = x + d / 10 + t / 8;
        let edge = 70 + swell(phase * 2) * 2 + swell(phase / 2 + 118);
        let high = 27 + triangle(phase + 70, 75) / 2;
        for j in 0..high {
            let intensity = if j < 4 {
                16 + j * 2
            } else {
                22 * (high - j) / high
            };
            let tone = blend(0x193455, 0x6ce2b9, intensity, 26);
            c.dot(x, edge - j, tone);
        }
        c.dot(x, edge, 0x78cca9);
        if (x + t / 4).rem_euclid(7) < 2 {
            c.dot(x, edge + 1, 0x4b9990);
        }
        let second = 87 + swell(phase + 103) + swell(phase / 2 + 61);
        c.dot(x, second, 0x486982);
        if x % 4 != 0 {
            c.dot(x, second - 1, 0x395674);
        }
    }
}
pub fn horizon(c: &mut Canvas, route: Route, d: i32, t: i32) {
    if route == Route::Coast {
        crate::scenery::sky(c, d, t);
        return;
    }
    let north = route == Route::Aurora;
    let (zenith, horizon, water, deep) = if north {
        (0x14223f, 0x70879c, 0x536f87, 0x24354e)
    } else {
        (0x182944, 0xe5a38b, 0x566f83, 0x1e354e)
    };
    for y in 0..228 {
        let n = if north { y * y / 228 } else { y * y / 200 };
        c.rect(0, y, 512, 1, blend(zenith, horizon, n.min(228), 228));
    }
    for i in 0..103 {
        let k = noise(i + 430);
        let x = ((k % 528) as i32 - d / 35).rem_euclid(528) - 8;
        let y = (k / 1024 % 146) as i32;
        c.dot(x, y, if i % 6 == 0 { PEARL } else { 0x919cae });
        if (t / 16 + i as i32) % 24 == 0 {
            star(c, x, y, 2, PEARL);
        }
    }
    let mx = if north { 427 - d / 105 } else { 351 - d / 100 };
    let my = if north { 92 } else { 114 };
    if north {
        northern_lights(c, d, t);
        c.disc(mx, my, 17, 0xa6b1bc);
        c.disc(mx, my, 15, 0xe3d7c5);
        c.disc(mx - 6, my - 5, 13, 0x29405c);
    } else {
        c.disc(mx, my, 36, 0xbc9298);
        c.disc(mx, my, 34, 0xf9caa3);
        c.disc(mx + 1, my - 2, 31, 0xffdaaf);
        for (ox, oy, w) in [(-17, -18, 7), (15, 10, 6), (-8, 18, 5), (7, -9, 3)] {
            c.rect(mx + ox, my + oy, w, 1, 0xeebd9e);
        }
    }
    let mut old_y = [228; 2];
    for x in 0..512 {
        for layer in 0..2 {
            let offset = x + d / (7 - 2 * layer as i32);
            let peak = if north {
                triangle(offset + 107 * layer as i32, 148) * 3 / 5 + triangle(offset, 56) / 2
            } else {
                triangle(offset, 177) / 5 + triangle(offset + 40, 78) / 7
            };
            let top = 219 - peak + layer as i32 * 10;
            c.rect(
                x,
                top,
                1,
                228 - top,
                if layer == 0 {
                    if north {
                        0x657a92
                    } else {
                        0x947b8c
                    }
                } else if north {
                    0x344c64
                } else {
                    0x4e596d
                },
            );
            if north && layer == 0 && peak > 29 {
                let snow = (peak - 29) / 3 + (top - old_y[layer]).abs().min(4);
                c.rect(x, top, 1, snow, 0xc7cfcc);
            }
            old_y[layer] = top;
        }
    }
    for y in 228..288 {
        c.rect(0, y, 512, 1, blend(water, deep, y - 228, 60));
    }
    for i in 0..166 {
        let k = noise(i + 927);
        let y = 229 + (k % 58) as i32;
        let x = (((k >> 11) % 570) as i32 - d / (7 - (y - 229) / 13) - t / 14).rem_euclid(570) - 29;
        let reflection = (x - mx).abs() < 11 + (y - 228);
        let col = if reflection {
            if north {
                0x91a8b3
            } else {
                0xd49a88
            }
        } else if north {
            0x577489
        } else {
            0x688395
        };
        c.rect(x, y, 2 + ((k >> 20) % 20) as i32, 1, col);
    }
    if north {
        for i in 0..7 {
            let x = (i * 103 - d / 3).rem_euclid(630) - 55;
            let y = 230 + i % 3 * 4;
            c.poly(
                &[
                    (x, y),
                    (x + 6, y - 3),
                    (x + 13, y - 3),
                    (x + 20, y - 7),
                    (x + 26, y - 1),
                    (x + 31, y),
                ],
                0xb2c8cc,
            );
            c.rect(x + 5, y + 1, 21, 1, 0x6a879a);
        }
    } else {
        // A distant pier with red channel lights, and a whale that surfaces offshore.
        let pier = (265 - d / 4).rem_euclid(720) - 40;
        c.rect(pier, 216, 71, 3, 0x48576a);
        for ox in [4, 22, 40, 62] {
            c.rect(pier + ox, 218, 2, 11, 0x48576a);
        }
        c.rect(pier + 52, 203, 2, 13, INK);
        c.rect(pier + 51, 202, 4, 2, CORAL);
        let wx = (330 - d / 2).rem_euclid(790) - 65;
        let lift = (triangle(t / 2 + 53, 112) - 26).max(0) / 3;
        if lift > 0 {
            ellipse(c, wx, 241, 23, lift, 0x263e53);
            c.poly(
                &[(wx + 10, 237), (wx + 19, 230 - lift / 2), (wx + 17, 238)],
                0x263e53,
            );
            c.rect(wx - 29, 241, 60, 11, 0x39576c);
            c.rect(wx - 27, 241, 56, 1, 0x96aaa8);
            if lift > 6 {
                c.line(wx - 9, 239 - lift, wx - 12, 231 - lift, 0xbac7c4);
                c.dot(wx - 14, 230 - lift, PEARL);
            }
        }
    }
}
fn net(c: &mut Canvas, x: i32, y: i32, r: i32, lit: bool, t: i32) {
    let glow = if lit { JADE } else { AMBER };
    for k in -r..=r {
        let h = y + 3 - k * k / (r * r / 4).max(1);
        c.dot(x + k, h, glow);
        if k % 4 == 0 {
            c.line(x + k, h, x + k + 1, h - 2, 0xcfaa96);
        }
    }
    for sx in [-r, r] {
        c.rect(x + sx, y - 7, 1, 10, INK);
        c.dot(x + sx, y - 8, PEARL);
    }
    if !lit {
        let v = y - 17 - triangle(t / 3, 14) / 3;
        c.line(x - 3, v - 3, x, v, AMBER);
        c.line(x, v, x + 3, v - 3, AMBER);
    } else {
        star(c, x, y - 13, 3, JADE);
    }
}
fn ship(c: &mut Canvas, x: i32, y: i32, id: usize, lit: bool, t: i32) {
    let base = y + 20;
    let hull = [0xa65c53, 0x55777c, 0x815a76][id];
    c.line(x - 55, base + 10, x - 77, base + 10, 0x88a3a7);
    c.rect(x - 64, base + 13, 104, 1, 0x668492);
    c.poly(
        &[
            (x - 54, base - 1),
            (x + 54, base - 1),
            (x + 41, base + 11),
            (x + 24, base + 15),
            (x - 34, base + 15),
            (x - 47, base + 9),
        ],
        INK,
    );
    c.poly(
        &[
            (x - 52, base),
            (x + 50, base),
            (x + 40, base + 7),
            (x - 45, base + 7),
        ],
        hull,
    );
    c.rect(x - 48, base - 2, 98, 2, 0xe8bf91);
    c.rect(x - 16, base - 12, 42, 10, 0xab9f94);
    c.rect(x - 17, base - 13, 45, 2, INK);
    for i in 0..6 {
        c.rect(
            x - 12 + i * 6,
            base - 10,
            3,
            4,
            if lit { JADE } else { AMBER },
        );
    }
    match id {
        0 => {
            c.rect(x - 33, base - 30, 2, 28, INK);
            c.poly(
                &[(x - 35, base - 28), (x - 48, base - 7), (x - 35, base - 9)],
                0xe6c49e,
            );
            c.line(x - 33, base - 30, x - 20, base - 2, 0xc4a7a0);
        }
        1 => {
            for dx in [-38, -29] {
                c.rect(x + dx, base - 22, 6, 19, 0x9c665e);
                c.rect(x + dx, base - 23, 6, 4, INK);
            }
            let wind = triangle(t / 3, 16) / 4;
            c.rect(x - 43 - wind, base - 28, 9, 2, 0xab95a2);
        }
        _ => {
            c.rect(x - 42, base - 14, 20, 11, 0x778589);
            for k in 0..3 {
                c.rect(x - 39 + k * 5, base - 12, 3, 3, PEARL);
            }
        }
    }
    c.line(x + 28, base - 5, x + 32, y - 23, INK);
    c.poly(
        &[(x + 31, y - 24), (x + 46, y - 21), (x + 31, y - 18)],
        CORAL,
    );
    c.line(x - 30, base - 22, x + 31, y - 22, 0x69596a);
    for i in 0..8 {
        let xx = x - 28 + i * 8;
        let yy = y - 18 + triangle(i, 8);
        c.dot(xx, yy, AMBER);
        c.dot(xx, yy + 1, PEARL);
    }
    net(c, x, y, 21, lit, t);
}
fn airship(c: &mut Canvas, x: i32, y: i32, id: usize, lit: bool, t: i32) {
    let dx = x - 30;
    let dy = y - 26;
    let colors = [
        (0xb86c5c, 0xf6b47c),
        (0x697a99, 0xc5bac1),
        (0x59898c, 0xc2d4af),
    ][id];
    c.poly(
        &[
            (dx - 29, dy),
            (dx - 49, dy - 16),
            (dx - 47, dy - 3),
            (dx - 37, dy + 2),
            (dx - 47, dy + 9),
            (dx - 31, dy + 7),
        ],
        0x42475f,
    );
    ellipse(c, dx, dy, 35, 14, INK);
    ellipse(c, dx, dy - 1, 34, 12, colors.0);
    c.line(dx - 28, dy - 6, dx + 25, dy - 6, colors.1);
    c.rect(dx - 16, dy - 11, 26, 2, colors.1);
    for k in [-19, -1, 17] {
        let rise = if k == -1 { 12 } else { 10 };
        c.line(dx + k - 2, dy - rise, dx + k - 5, dy, colors.1);
        c.line(dx + k - 5, dy, dx + k - 2, dy + 9, 0x6e5565);
    }
    for (a, b) in [(-24, -10), (18, 4), (28, 17)] {
        c.line(dx + a, dy + 8, x + b, y + 9, 0x9fa5b1);
    }
    c.poly(
        &[
            (x - 24, y + 5),
            (x + 24, y + 5),
            (x + 18, y + 15),
            (x - 9, y + 15),
        ],
        INK,
    );
    c.rect(x - 16, y + 7, 32, 3, colors.0);
    for k in 0..4 {
        c.dot(x - 7 + k * 6, y + 12, if lit { JADE } else { AMBER });
    }
    c.line(x + 21, y + 5, x + 27, y - 10, 0xc5aca7);
    c.poly(
        &[
            (x + 27, y - 11),
            (x + 35, y - 10 + triangle(t, 18) / 4),
            (x + 26, y - 6),
        ],
        CORAL,
    );
    net(c, x, y, 18, lit, t);
}
pub fn destination(c: &mut Canvas, g: &Game, id: usize, t: i32) {
    let x = g.post_x(id);
    if !(-110..622).contains(&x) {
        return;
    }
    let y = g.post_y(id);
    let lit = g.delivered & (1 << id) != 0;
    target_art(c, g.route, x, y, id, lit, t);
}
pub fn target_art(c: &mut Canvas, route: Route, x: i32, y: i32, id: usize, lit: bool, t: i32) {
    match route {
        Route::Coast => post(c, x, y, id, lit, t),
        Route::Flotilla => ship(c, x, y, id, lit, t),
        Route::Aurora => airship(c, x, y, id, lit, t),
    }
}
