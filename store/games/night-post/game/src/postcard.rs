//! Ten small screen prints. All coordinates here refer to the game's native
//! framebuffer. The back uses the same postmark as the flight scorecard.
use crate::{
    mail::{Card, Side},
    paint::*,
    screen::{bottom, postmark},
    triangle, Game,
};

const PAPER: u32 = 0xf4e5c5;
const PRINT: u32 = 0x312f3f;
const FADED: u32 = 0x806d68;
const RED: u32 = 0xb75f53;
const BLUE: u32 = 0x587485;

fn oval(c: &mut Canvas, x: i32, y: i32, a: i32, b: i32, col: u32) {
    let mut extent = 0;
    for dy in (0..=b).rev() {
        while extent < a && (extent + 1) * (extent + 1) * b * b + dy * dy * a * a <= a * a * b * b {
            extent += 1;
        }
        c.rect(x - extent, y - dy, 2 * extent + 1, 1, col);
        if dy != 0 {
            c.rect(x - extent, y + dy, 2 * extent + 1, 1, col);
        }
    }
}

fn strokes(c: &mut Canvas, points: &[(i32, i32)], col: u32, thick: i32) {
    for p in points.windows(2) {
        for i in 0..thick {
            c.line(p[0].0, p[0].1 + i, p[1].0, p[1].1 + i, col);
        }
    }
}
fn paper(c: &mut Canvas) {
    c.rect(0, 0, 512, 288, 0x283941);
    for y in (0..265).step_by(6) {
        c.rect(0, y, 512, 1, 0x2b3c45);
    }
    c.rect(31, 31, 457, 219, 0x182b32);
    c.rect(26, 26, 458, 220, PAPER);
    for i in 0..200 {
        let v = noise(i + 240);
        let x = 28 + (v % 452) as i32;
        let y = 28 + ((v >> 12) % 213) as i32;
        c.dot(x, y, 0xe4d2b3);
    }
}

fn vignette(c: &mut Canvas, card: usize) {
    let (sky, low, bench) = match card {
        0 | 1 => (0x426574, 0xb7b5a2, 0x835949),
        2 | 7 => (0x294d53, 0x799488, 0x47625e),
        3 | 4 | 6 => (0x425e71, 0xc4a39a, 0x555063),
        _ => (0x35475e, 0x9990a0, 0x644d62),
    };
    for y in 38..196 {
        c.rect(38, y, 434, 1, blend(sky, low, y - 38, 158));
    }
    for i in 0..45 {
        let k = noise(i + card as u32 * 306);
        c.dot(
            41 + (k % 425) as i32,
            42 + ((k >> 10) % 75) as i32,
            0xd9cba9,
        );
    }
    for i in 0..10 {
        let x = 42 + (i * 59 + (card as i32) * 7) % 400;
        let y = 121 + ((i * 7) % 22);
        c.rect(x, y, 9 + i % 6 * 4, 1, blend(sky, low, 2, 3));
    }
    c.rect(38, 163, 434, 33, bench);
    for i in 0..29 {
        let v = noise(i + 108);
        let x = 40 + (v % 385) as i32;
        c.rect(
            x,
            166 + ((v >> 11) % 27) as i32,
            4 + ((v >> 20) % 37) as i32,
            1,
            blend(bench, low, 1, 4),
        );
    }
    match card {
        0 => teapot(c),
        1 => soup(c),
        2 => moth(c),
        3 => spoon(c),
        4 => hands(c),
        5 => record(c),
        6 => cat(c),
        7 => cutting(c),
        8 => chart(c),
        _ => quilt(c),
    }
    // Fine flecks of ink and paper tie the flat printed shapes together.
    for i in 0..385 {
        let v = noise(i + 351);
        let x = 39 + (v % 431) as i32;
        let y = 39 + ((v >> 10) % 155) as i32;
        let at = (y * 512 + x) as usize;
        let current = c.0[at];
        c.dot(x, y, blend(current, PAPER, 1, 6));
    }
}

fn teapot(c: &mut Canvas) {
    // A peg rail and the courier's scarf hang behind the pot.
    c.rect(59, 65, 142, 7, 0x765f56);
    for x in [74, 118, 161] {
        c.rect(x, 62, 4, 13, PRINT);
        c.dot(x + 1, 62, PEARL);
    }
    c.poly(
        &[
            (156, 70),
            (165, 69),
            (173, 98),
            (165, 123),
            (156, 119),
            (162, 96),
        ],
        RED,
    );
    c.poly(
        &[
            (160, 73),
            (166, 71),
            (178, 104),
            (172, 116),
            (165, 112),
            (170, 101),
        ],
        0xe39272,
    );
    oval(c, 280, 173, 72, 7, 0x6c5048);
    oval(c, 324, 133, 26, 25, PRINT);
    oval(c, 324, 133, 20, 18, 0xc39a82);
    oval(c, 324, 133, 15, 14, 0x8d9b94);
    c.poly(
        &[
            (242, 142),
            (222, 131),
            (206, 106),
            (198, 106),
            (205, 127),
            (216, 146),
            (242, 156),
        ],
        0x354e65,
    );
    c.poly(
        &[
            (236, 138),
            (219, 128),
            (207, 109),
            (204, 109),
            (212, 128),
            (225, 142),
        ],
        0x8bb8b4,
    );
    oval(c, 272, 142, 45, 34, PRINT);
    oval(c, 271, 140, 43, 31, 0x447b89);
    oval(c, 260, 135, 23, 21, 0x5c93a0);
    for i in 0..5 {
        c.poly(
            &[
                (243 + i * 11, 145),
                (248 + i * 11, 139),
                (253 + i * 11, 145),
                (248 + i * 11, 151),
            ],
            0xbee1c9,
        );
    }
    oval(c, 270, 113, 29, 7, PRINT);
    oval(c, 270, 110, 26, 5, 0x81afa9);
    oval(c, 271, 103, 7, 6, 0xd4bc8f);
    c.rect(248, 167, 45, 5, 0x354e65);
    // One brass key, caught on the lip of the pot.
    c.line(287, 112, 307, 90, 0xe3c889);
    oval(c, 310, 88, 5, 4, AMBER);
    oval(c, 310, 88, 2, 1, 0x7e857f);
    c.rect(288, 106, 5, 2, AMBER);
    c.rect(291, 103, 5, 2, AMBER);
}
fn soup(c: &mut Canvas) {
    c.poly(
        &[(83, 161), (89, 110), (102, 93), (108, 110), (114, 161)],
        0x345662,
    );
    c.rect(89, 106, 16, 5, 0xefc47d);
    c.rect(96, 100, 2, 7, PRINT);
    c.poly(
        &[(80, 160), (111, 149), (136, 158), (145, 164), (80, 164)],
        0x465965,
    );
    for (x, y, scale) in [(265, 143, 3), (338, 156, 1)] {
        oval(
            c,
            x + 13 * scale,
            y + 2 * scale,
            6 * scale,
            5 * scale,
            PRINT,
        );
        oval(
            c,
            x + 13 * scale,
            y + 2 * scale,
            4 * scale,
            3 * scale,
            0xa7a69c,
        );
        c.poly(
            &[
                (x - 11 * scale, y),
                (x + 12 * scale, y),
                (x + 9 * scale, y + 9 * scale),
                (x - 7 * scale, y + 9 * scale),
            ],
            if scale == 1 { 0xe4c795 } else { 0x4c7381 },
        );
        oval(c, x, y, 12 * scale, 3 * scale, PEARL);
        oval(c, x, y, 10 * scale, 2 * scale, 0xbb8257);
        c.rect(
            x - 7 * scale,
            y + 7 * scale,
            16 * scale,
            scale,
            if scale == 1 { 0xbf9478 } else { 0x8fc0bb },
        );
        strokes(
            c,
            &[
                (x - 3, y - 11),
                (x - 7, y - 19),
                (x - 2, y - 27),
                (x - 5, y - 34),
            ],
            0xe0cfba,
            1,
        );
    }
    c.poly(&[(379, 120), (400, 117), (412, 161), (387, 162)], PAPER);
    for i in 0..4 {
        c.line(386 + i, 130 + i * 6, 398 + i, 129 + i * 6, 0x957d70);
    }
}
fn moth(c: &mut Canvas) {
    c.poly(&[(98, 97), (245, 84), (256, 160), (111, 177)], PRINT);
    c.poly(&[(102, 95), (244, 84), (250, 155), (112, 173)], 0xe2cbaa);
    for k in 0..4 {
        let x = 125 + k * 19;
        for j in 0..4 {
            c.line(x + 3 * j, 113, x + 2 + 3 * j, 130, 0x877368);
        }
        c.line(x - 2, 128, x + 12, 115, 0x877368);
    }
    let x = 278;
    let y = 129;
    for sign in [-1, 1] {
        let q = |v: i32| x + sign * v;
        c.poly(
            &[
                (q(2), y),
                (q(20), y - 31),
                (q(61), y - 48),
                (q(53), y - 19),
                (q(34), y + 2),
                (q(11), y + 12),
            ],
            0x674d53,
        );
        c.poly(
            &[
                (q(6), y - 1),
                (q(23), y - 29),
                (q(57), y - 43),
                (q(44), y - 18),
                (q(23), y + 1),
            ],
            0xecbb80,
        );
        c.poly(
            &[
                (q(8), y + 1),
                (q(40), y - 5),
                (q(31), y + 17),
                (q(14), y + 24),
            ],
            0xc88069,
        );
        oval(c, q(32), y - 19, 9, 7, 0x795766);
        oval(c, q(32), y - 19, 5, 4, 0xd9c496);
        c.line(q(2), y - 8, q(11), y - 30, PRINT);
        for i in 0..7 {
            c.line(
                q(5 + i),
                y - 14 - i * 2,
                q(13 + i),
                y - 17 - i * 3,
                0x524e4b,
            );
        }
    }
    oval(c, x, y + 1, 4, 14, PRINT);
    oval(c, x, y - 8, 5, 5, 0xc9a27d);
}
fn spoon(c: &mut Canvas) {
    // A gull on the rail, with an entirely disproportionate piece of cutlery.
    c.rect(78, 149, 277, 8, 0x675f63);
    for x in [101, 224, 337] {
        c.rect(x, 155, 8, 34, 0x544f58);
    }
    for x in [113, 225, 315] {
        c.poly(
            &[
                (x, 100),
                (x + 4, 97),
                (x + 9, 101),
                (x + 15, 97),
                (x + 9, 103),
            ],
            0x78838a,
        );
    }
    c.poly(
        &[
            (178, 140),
            (201, 128),
            (217, 109),
            (245, 104),
            (257, 91),
            (268, 85),
            (279, 88),
            (283, 98),
            (277, 111),
            (267, 120),
            (258, 136),
            (244, 144),
            (215, 147),
        ],
        0xf1e6cc,
    );
    c.poly(
        &[
            (181, 140),
            (211, 127),
            (238, 108),
            (252, 110),
            (238, 126),
            (214, 137),
        ],
        0x8b99a2,
    );
    c.poly(&[(182, 138), (201, 128), (213, 127), (197, 138)], PRINT);
    c.dot(275, 93, INK);
    c.poly(&[(280, 97), (299, 99), (280, 103)], AMBER);
    for x in [233, 245] {
        c.line(x, 143, x - 1, 150, CORAL);
        c.line(x - 3, 150, x + 4, 150, CORAL);
    }
    strokes(c, &[(292, 100), (303, 117), (317, 140)], 0x465769, 4);
    strokes(c, &[(292, 99), (305, 119), (318, 140)], 0xd3dcd2, 2);
    oval(c, 322, 145, 9, 12, 0x687d8b);
    oval(c, 321, 142, 7, 10, 0xb7c6c1);
    c.line(319, 134, 316, 140, PEARL);
}
fn hands(c: &mut Canvas) {
    c.poly(&[(117, 76), (361, 66), (373, 170), (126, 180)], PRINT);
    c.poly(&[(120, 73), (358, 65), (369, 167), (128, 176)], PAPER);
    for k in 0..6 {
        c.line(141, 91 + 12 * k, 347, 83 + 12 * k, 0xc9bda7);
    }
    let big = [
        (201, 157),
        (194, 143),
        (182, 129),
        (176, 113),
        (181, 112),
        (194, 128),
        (194, 93),
        (199, 91),
        (203, 119),
        (204, 82),
        (210, 80),
        (214, 117),
        (215, 78),
        (221, 77),
        (225, 118),
        (230, 87),
        (236, 87),
        (235, 128),
        (242, 107),
        (247, 106),
        (249, 111),
        (244, 140),
        (235, 155),
        (235, 163),
    ];
    strokes(c, &big, 0x74858a, 2);
    let small = [
        (211, 153),
        (204, 141),
        (195, 132),
        (194, 125),
        (198, 125),
        (205, 132),
        (205, 108),
        (209, 105),
        (212, 125),
        (214, 99),
        (218, 99),
        (219, 124),
        (221, 99),
        (225, 99),
        (225, 125),
        (229, 106),
        (232, 107),
        (230, 131),
        (236, 119),
        (239, 120),
        (237, 140),
        (229, 151),
    ];
    strokes(c, &small, 0xa56e5c, 2);
    for i in 0..4 {
        c.line(270, 106 + i * 9, 333 - i * 4, 103 + i * 9, 0xb2a18e);
    }
    c.poly(
        &[(302, 152), (341, 125), (344, 128), (305, 155), (298, 158)],
        0x936e59,
    );
}
fn record(c: &mut Canvas) {
    c.poly(&[(151, 145), (284, 125), (321, 153), (186, 176)], PRINT);
    c.poly(&[(151, 139), (282, 122), (318, 147), (185, 168)], 0xad7960);
    oval(c, 232, 142, 51, 14, PRINT);
    oval(c, 232, 141, 42, 10, 0x4d5360);
    oval(c, 232, 140, 37, 8, PRINT);
    oval(c, 232, 140, 15, 5, 0xc08c72);
    c.dot(232, 140, PEARL);
    strokes(
        c,
        &[(281, 137), (258, 127), (254, 117), (260, 109), (268, 101)],
        0xc19b78,
        4,
    );
    c.poly(
        &[
            (265, 110),
            (263, 87),
            (283, 69),
            (313, 55),
            (305, 72),
            (310, 84),
            (297, 93),
            (293, 103),
        ],
        0xe0ad76,
    );
    c.poly(
        &[
            (276, 102),
            (286, 79),
            (313, 55),
            (318, 71),
            (310, 87),
            (293, 103),
        ],
        0xad7563,
    );
    c.poly(&[(286, 94), (292, 76), (309, 62), (303, 80)], 0x805a59);
    c.line(286, 136, 275, 145, PEARL);
    c.rect(273, 143, 4, 3, PRINT);
    // The famous sack of onions, in its best hat.
    c.poly(
        &[
            (343, 169),
            (336, 152),
            (340, 137),
            (349, 131),
            (351, 126),
            (361, 126),
            (365, 132),
            (373, 146),
            (374, 162),
            (366, 171),
        ],
        0xcba16d,
    );
    for x in [344, 353, 362] {
        c.line(x, 138, x + 3, 164, 0xb38e64);
    }
    c.rect(343, 124, 29, 3, PRINT);
    c.poly(&[(348, 124), (350, 115), (363, 115), (366, 124)], PRINT);
}
fn cat(c: &mut Canvas) {
    c.rect(88, 65, 153, 77, 0x334450);
    c.rect(93, 70, 143, 64, 0x84949c);
    c.rect(97, 74, 135, 56, 0x677d88);
    c.rect(161, 69, 5, 66, PRINT);
    c.rect(93, 134, 145, 5, 0xc4987f);
    c.rect(99, 116, 58, 1, 0xc6b5a4);
    c.rect(172, 119, 54, 1, 0xc6b5a4);
    c.poly(
        &[
            (203, 155),
            (223, 139),
            (294, 136),
            (329, 149),
            (328, 171),
            (216, 175),
        ],
        0x714e48,
    );
    c.poly(
        &[
            (207, 150),
            (236, 137),
            (303, 138),
            (327, 147),
            (296, 157),
            (229, 157),
        ],
        0xc69368,
    );
    c.poly(&[(232, 140), (242, 138), (250, 156), (241, 158)], 0x70544c);
    c.rect(240, 155, 11, 5, 0xe6c785);
    oval(c, 275, 137, 32, 15, 0x58494a);
    oval(c, 272, 134, 30, 13, 0x977261);
    for i in 0..3 {
        strokes(
            c,
            &[(269 + i * 8, 122), (267 + i * 8, 130), (270 + i * 8, 135)],
            0x604f50,
            2,
        );
    }
    oval(c, 247, 133, 14, 11, 0xb58a70);
    c.poly(&[(234, 132), (233, 117), (244, 126)], 0xa27b67);
    c.poly(&[(246, 126), (254, 116), (256, 132)], 0xab826c);
    c.poly(&[(236, 126), (235, 121), (241, 126)], 0xdb9c85);
    c.line(238, 134, 243, 135, PRINT);
    c.line(247, 134, 251, 133, PRINT);
    c.dot(244, 138, PRINT);
    strokes(
        c,
        &[(297, 134), (302, 141), (294, 148), (276, 149), (263, 146)],
        0x78594e,
        4,
    );
    c.rect(354, 121, 39, 44, PRINT);
    c.rect(358, 127, 31, 23, 0x8e6157);
    c.poly(
        &[(361, 149), (364, 136), (369, 142), (375, 130), (382, 147)],
        AMBER,
    );
    c.rect(368, 75, 9, 46, 0x4d4b52);
    c.rect(351, 119, 44, 4, 0x665d5a);
}
fn cutting(c: &mut Canvas) {
    // Porthole, swinging mug, and a spruce cutting in a repurposed tin.
    oval(c, 162, 109, 35, 35, 0x4b686c);
    oval(c, 162, 109, 29, 29, 0xb6c7b6);
    oval(c, 162, 109, 25, 25, 0x748c95);
    c.poly(
        &[(140, 120), (157, 105), (166, 112), (176, 96), (185, 123)],
        0xc4c9bb,
    );
    for x in [218, 324] {
        c.line(x, 43, x + 5, 72, 0xb89b7e);
    }
    c.rect(216, 72, 16, 10, 0xd7baa0);
    c.rect(322, 72, 15, 11, 0x819cab);
    c.rect(251, 137, 46, 35, 0x476b70);
    oval(c, 274, 138, 23, 5, 0xb1c4a8);
    oval(c, 274, 138, 20, 3, 0x594c49);
    c.rect(251, 151, 46, 12, 0xc8a87c);
    c.text(259, 154, "COCOA", 1, 0x625449);
    strokes(
        c,
        &[(274, 138), (272, 115), (282, 87), (283, 65)],
        0x615a46,
        2,
    );
    for (x, y, dx) in [
        (274, 121, -20),
        (275, 112, 20),
        (280, 98, -16),
        (281, 90, 16),
        (283, 77, -10),
    ] {
        c.line(x, y, x + dx, y - 13, 0x354e49);
        for j in 1..8 {
            let px = x + j * dx / 8;
            let py = y - 13 * j / 8;
            c.line(px, py, px + dx.signum() * 7, py - 7, 0x9abb99);
            c.line(px, py + 1, px + dx.signum() * 8, py, 0x587b67);
        }
    }
    c.line(242, 170, 310, 148, 0xd6bd8c);
    c.line(242, 171, 310, 149, 0x897459);
}
fn chart(c: &mut Canvas) {
    c.poly(&[(124, 83), (307, 69), (341, 154), (152, 174)], 0x384859);
    c.poly(&[(127, 80), (307, 66), (337, 151), (152, 170)], 0xc1beb1);
    c.poly(&[(137, 87), (302, 75), (326, 145), (159, 161)], 0x455773);
    let points = [
        (160, 118),
        (183, 100),
        (216, 114),
        (237, 91),
        (281, 97),
        (298, 119),
        (270, 136),
    ];
    strokes(c, &points, 0x8392a2, 1);
    for &(x, y) in &points {
        c.line(x - 3, y, x + 3, y, PEARL);
        c.line(x, y - 3, x, y + 3, PEARL);
    }
    for i in 0..18 {
        let k = noise(i + 720);
        c.dot(
            156 + (k % 141) as i32,
            94 + ((k >> 11) % 43) as i32,
            0xa3aaa9,
        );
    }
    c.rect(242, 117, 9, 10, CORAL);
    c.line(246, 117, 246, 126, 0x865863);
    c.line(242, 121, 250, 121, 0x865863);
    oval(c, 358, 147, 20, 19, 0xc5a378);
    oval(c, 358, 147, 16, 15, 0x46586a);
    c.poly(&[(350, 157), (357, 143), (366, 135), (361, 150)], 0xd9c4a1);
}
fn quilt(c: &mut Canvas) {
    const SWATCH: [u32; 6] = [0x8f9897, 0xe1ba90, 0x9b6b78, 0x638280, 0xd09877, 0x79728a];
    for row in 0..4 {
        for col in 0..7 {
            let x = 150 + col * 27 - row * 3;
            let y = 82 + row * 21 + triangle(col + 2, 8) * 2;
            let a = SWATCH[((row * 3 + col) % 6) as usize];
            let b = SWATCH[((row + col * 3 + 2) % 6) as usize];
            c.poly(
                &[(x, y), (x + 27, y + 1), (x + 24, y + 23), (x - 3, y + 22)],
                a,
            );
            c.poly(&[(x, y), (x + 27, y + 1), (x - 3, y + 22)], b);
            for k in 0..5 {
                c.line(x + 2 + k * 5, y + 2, x + 4 + k * 5, y + 2, 0xc7ac94);
            }
        }
    }
    c.poly(&[(230, 110), (258, 111), (255, 133), (227, 132)], 0xb45951);
    c.line(240, 118, 250, 119, 0x703f43);
    c.line(240, 120, 250, 121, 0xd18067);
    strokes(
        c,
        &[
            (261, 115),
            (269, 101),
            (275, 104),
            (271, 112),
            (264, 115),
            (259, 125),
        ],
        0xf0d4a3,
        1,
    );
    c.line(260, 123, 275, 96, PEARL);
}

fn postage(c: &mut Canvas, g: &Game, x: i32, y: i32) {
    let index = g.mailbox.card;
    if let Some((route, id)) = Card::address(index) {
        postmark(
            c,
            x,
            y,
            route,
            id,
            true,
            g.album & (route.bit(id) << 9) != 0,
        );
    } else {
        c.rect(x, y, 27, 23, 0xd5b79a);
        c.rect(x + 2, y + 2, 23, 19, 0x567985);
        c.poly(
            &[
                (x + 4, y + 6),
                (x + 19, y + 9),
                (x + 16, y + 14),
                (x + 6, y + 17),
                (x + 9, y + 12),
            ],
            PEARL,
        );
        c.line(x + 13, y + 14, x + 21, y + 7, AMBER);
    }
    for k in 0..3 {
        strokes(
            c,
            &[
                (x - 36, y + 6 + k * 4),
                (x - 30, y + 4 + k * 4),
                (x - 24, y + 6 + k * 4),
                (x - 18, y + 8 + k * 4),
                (x - 11, y + 6 + k * 4),
                (x - 5, y + 5 + k * 4),
            ],
            FADED,
            1,
        );
    }
}
pub fn reader(c: &mut Canvas, g: &Game) {
    let card = Card::for_index(g.mailbox.card);
    paper(c);
    match g.mailbox.side {
        Side::Picture => {
            vignette(c, g.mailbox.card);
            c.text(40, 208, card.title, 3, PRINT);
            let len = card.place.len() as i32 * 6;
            c.text(470 - len, 225, card.place, 1, FADED);
        }
        Side::Message => {
            // An edge-printed airmail stripe, with the address on the right.
            for i in 0..24 {
                let x = 35 + i * 19;
                c.poly(
                    &[(x, 31), (x + 6, 31), (x + 2, 35), (x - 4, 35)],
                    if i % 2 == 0 { RED } else { BLUE },
                );
            }
            c.text(44, 49, card.title, 2, PRINT);
            c.rect(44, 69, 232, 1, 0xcfbba0);
            for (i, s) in card.lines.iter().enumerate() {
                c.text(44, 83 + 13 * i as i32, s, 1, PRINT);
            }
            c.text(44, 207, card.sender, 1, RED);
            c.rect(299, 47, 1, 178, 0xcebea4);
            postage(c, g, 431, 49);
            c.text(318, 105, "TO", 1, FADED);
            c.text(318, 121, "The night courier", 1, PRINT);
            c.text(318, 137, "Sorting Office", 1, PRINT);
            c.text(318, 153, "Third hook down", 1, PRINT);
            c.rect(318, 169, 137, 1, 0xc8b69a);
            c.text(318, 196, "POSTED FROM", 1, FADED);
            c.text(318, 211, card.place, 1, RED);
        }
    }
    let mut status = *b"RETURN MAIL  00/00";
    let received = g.album & 511;
    let total = 1 + received.count_ones();
    let ordinal = 1 + (received & ((1u32 << g.mailbox.card) - 1)).count_ones();
    status[13] = b'0' + (ordinal / 10) as u8;
    status[14] = b'0' + (ordinal % 10) as u8;
    status[16] = b'0' + (total / 10) as u8;
    status[17] = b'0' + (total % 10) as u8;
    c.text(
        27,
        10,
        core::str::from_utf8(&status).unwrap_or("RETURN MAIL"),
        1,
        PEARL,
    );
    c.text(
        422,
        10,
        if g.mailbox.side == Side::Picture {
            "PICTURE"
        } else {
            "MESSAGE"
        },
        1,
        JADE,
    );
    let left = if total == 1 {
        "SPACE  TURN OVER"
    } else {
        "LEFT / RIGHT  CARDS    SPACE  TURN OVER"
    };
    bottom(c, left, "M  BACK");
}
