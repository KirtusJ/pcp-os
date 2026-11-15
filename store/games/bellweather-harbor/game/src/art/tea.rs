//! The Emergency Tea Committee works with ordinary folding furniture until a
//! quorum puts the furniture, the crane, and eventually the tide to work.
use super::*;

const IRON: Color = 0x354650;
const WICKER: Color = 0xc89a64;
const PEARL: Color = 0xf3dfb5;
const COZY: Color = 0xb35d63;
const PLUM: Color = 0x754c60;
const ENAMEL: Color = 0x91b4a6;
const GLAZE: Color = 0xeacda9;
const AMBER: Color = 0xc48648;

fn chair(c: &mut Canvas, x: i32, y: i32, paper: Color) {
    c.line((x - 3, y - 11), (x + 1, y), IRON);
    c.line((x + 5, y - 5), (x - 2, y), IRON);
    c.line((x + 1, y - 5), (x + 5, y), 0x7c6b57);
    c.shape(
        &[
            (x - 4, y - 12),
            (x + 1, y - 12),
            (x + 2, y - 8),
            (x - 3, y - 8),
        ],
        IRON,
    );
    c.rect(x - 3, y - 11, 4, 2, paper);
    c.rect(x - 2, y - 6, 8, 2, IRON);
    c.rect(x - 1, y - 6, 6, 1, paper);
    c.pixel(x + 1, y - 4, WICKER);
}

fn tea_table(c: &mut Canvas, x: i32, y: i32, t: u32) {
    c.line((x + 2, y + 1), (x + 19, y + 8), IRON);
    c.line((x + 18, y + 1), (x + 2, y + 8), IRON);
    c.line((x + 24, y + 1), (x + 20, y + 7), IRON);
    c.rect(x, y - 2, 27, 4, IRON);
    c.rect(x + 1, y - 2, 25, 3, PEARL);
    // A checkered tablecloth; the short overhang shows the reverse of the weave.
    for i in (1..24).step_by(4) {
        c.rect(x + i, y - 2, 2, 1, COZY);
        c.rect(x + i + 2, y, 2, 1, COZY);
    }
    c.rect(x + 1, y + 1, 24, 1, 0xbba284);
    c.rect(x + 4, y - 11, 5, 8, IRON);
    c.rect(x + 5, y - 10, 3, 5, COZY);
    c.rect(x + 5, y - 5, 3, 2, ENAMEL);
    c.rect(x + 4, y - 12, 5, 2, PEARL);
    c.pixel(x + 5, y - 8, PEARL);
    c.pixel(x + 7, y - 7, PEARL);
    // Teacups and a small pile of biscuits.
    for dx in [12, 18] {
        c.rect(x + dx, y - 5, 3, 3, GLAZE);
        c.rect(x + dx, y - 5, 3, 1, ENAMEL);
        c.pixel(x + dx + 3, y - 4, PEARL);
    }
    c.rect(x + 22, y - 4, 4, 2, AMBER);
    let lift = triangle(t / 4, 2);
    c.pixel(x + 6 + lift, y - 14 - lift, 0xe1d6bd);
}

fn grandmother(c: &mut Canvas, x: i32, feet: i32, who: u8, t: u32) {
    let (coat, hem, hair) = match who {
        0 => (COZY, PLUM, 0xc4b2a7),
        1 => (0x6f9d90, 0x436d70, 0xd9d1ba),
        _ => (0x8297b0, 0x475977, 0xa58b80),
    };
    let nod = triangle(t / (8 + who as u32), 1);
    let face = feet - 13 + nod;
    c.rect(x - 3, feet - 2, 2, 3, IRON);
    c.rect(x + 1, feet - 2, 2, 3, IRON);
    c.shape(
        &[
            (x - 2, feet - 10),
            (x + 2, feet - 10),
            (x + 4, feet - 7),
            (x + 4, feet - 2),
            (x - 4, feet - 2),
            (x - 3, feet - 8),
        ],
        hem,
    );
    c.shape(
        &[
            (x - 2, feet - 10),
            (x + 2, feet - 10),
            (x + 3, feet - 7),
            (x + 2, feet - 4),
            (x - 3, feet - 4),
            (x - 3, feet - 7),
        ],
        coat,
    );
    c.line((x - 3, feet - 9), (x, feet - 7), PEARL);
    c.rect(x - 2, face - 1, 5, 5, 0x8c6557);
    c.rect(x - 1, face, 4, 4, 0xdfad85);
    c.pixel(x + 3, face + 1, 0xf1c69b);
    c.rect(x - 2, face - 2, 5, 2, hair);
    c.pixel(x + 1, face + 1, IRON);
    match who {
        0 => {
            // Rose shawl, pinned silver bun, pince-nez, and a pointing finger.
            c.disc(x - 3, face - 2, 2, hair);
            c.rect(x, face + 1, 3, 1, PEARL);
            c.pixel(x + 2, face + 1, IRON);
            c.line((x + 2, feet - 7), (x + 6, feet - 9 - nod), coat);
            c.rect(x + 6, feet - 10 - nod, 2, 2, 0xe4b38d);
        }
        1 => {
            // A soft felt cloche and a clipboard full of neatly numbered motions.
            c.rect(x - 2, face - 4, 5, 3, 0x74745d);
            c.rect(x - 4, face - 2, 9, 1, WICKER);
            c.pixel(x + 2, face - 3, COZY);
            c.rect(x, feet - 8, 5, 6, 0x82674e);
            c.rect(x + 1, feet - 7, 3, 4, PEARL);
            c.rect(x + 1, feet - 5, 2, 1, 0x9c9a85);
        }
        _ => {
            // Blue cardigan and a very long umbrella, determinedly kept closed.
            c.rect(x - 3, face - 2, 6, 1, PEARL);
            c.rect(x - 2, face - 4, 4, 2, hair);
            c.line((x + 4, feet - 8), (x + 6, feet + 1), IRON);
            c.line((x + 4, feet - 5), (x + 5, feet - 1), PLUM);
        }
    }
}

fn floating_chairs(c: &mut Canvas, t: u32) {
    for (n, (x, y)) in [(108, 164), (134, 156), (154, 171), (184, 178), (276, 169)]
        .into_iter()
        .enumerate()
    {
        let bob = triangle(t / (12 + n as u32), 1);
        let xx = x + triangle(t / (26 + n as u32), 3);
        let yy = y + bob;
        c.rect(xx - 6, yy + 2, 15, 1, 0x7e978d);
        c.rect(xx - 2, yy + 4, 7, 1, 0xb3a88a);
        c.rect(xx - 5, yy, 12, 2, 0x75665b);
        c.rect(xx - 4, yy, 10, 1, WICKER);
        chair(c, xx, yy, if n % 2 == 0 { PEARL } else { COZY });
    }
}

fn sash(c: &mut Canvas, t: u32) {
    // The crane has been elected chair. Its sash crosses the machinery house.
    c.shape(&[(400, 67), (404, 66), (424, 80), (421, 82)], PLUM);
    c.line((402, 67), (422, 80), 0xdcaa76);
    c.line((405, 68), (423, 80), COZY);
    let flutter = triangle(t / 13, 2);
    c.shape(
        &[
            (419, 79),
            (424, 80),
            (425 + flutter, 85),
            (422, 84),
            (420, 86),
        ],
        COZY,
    );
    c.disc(419, 79, 3, AMBER);
    c.disc(419, 79, 2, PEARL);
    c.pixel(419, 79, COZY);
}

fn steam(c: &mut Canvas, x: i32, y: i32, t: u32) {
    for n in 0..3 {
        let life = (t / 3 + n * 7) % 23;
        let side = triangle(life + n * 3, 4) - 2;
        let xx = x + side + life as i32 / 6;
        let yy = y - life as i32 / 2;
        let col = if life < 10 { 0xf2d9b5 } else { 0xc6c2ac };
        c.rect(xx, yy, 2 + (life / 9) as i32, 1, col);
        if life < 15 {
            c.pixel(xx - 1, yy + 1, col);
        }
    }
}

fn pot_float(c: &mut Canvas, t: u32) {
    let x = 229 + triangle(t / 22, 3);
    let y = 165 + triangle(t / 16, 1);
    // Three battered fenders and a float just broad enough to take her weight.
    c.rect(x - 24, y + 11, 52, 1, 0x819b94);
    c.rect(x - 18, y + 14, 38, 1, 0x9d9d8d);
    hull(c, x - 27, y + 5, 58, 5, 0xc79868, 0x775d51);
    for dx in [-18, 0, 18] {
        c.disc(x + dx, y + 7, 2, IRON);
        c.pixel(x + dx, y + 6, 0xc8a77d);
    }
    // The enamel handle is hollow, showing the sea through it.
    for (off, col) in [(0, IRON), (1, ENAMEL), (2, GLAZE)] {
        c.line((x - 11, y - 17 + off), (x - 24, y - 19 + off), col);
        c.line((x - 24 - off, y - 18), (x - 26 - off, y - 11), col);
        c.line((x - 26 - off, y - 11), (x - 21 - off, y - 6), col);
        c.line((x - 21, y - 6 + off), (x - 14, y - 6 + off), col);
    }
    c.shape(
        &[
            (x + 10, y - 12),
            (x + 20, y - 15),
            (x + 25, y - 22),
            (x + 32, y - 22),
            (x + 28, y - 16),
            (x + 24, y - 9),
            (x + 14, y - 4),
        ],
        IRON,
    );
    c.shape(
        &[
            (x + 12, y - 11),
            (x + 21, y - 14),
            (x + 26, y - 21),
            (x + 30, y - 21),
            (x + 25, y - 14),
            (x + 22, y - 9),
            (x + 13, y - 6),
        ],
        GLAZE,
    );
    c.line((x + 25, y - 22), (x + 32, y - 22), ENAMEL);
    c.shape(
        &[
            (x - 10, y - 19),
            (x + 10, y - 19),
            (x + 16, y - 14),
            (x + 19, y - 6),
            (x + 16, y + 1),
            (x + 8, y + 5),
            (x - 9, y + 5),
            (x - 17, y + 1),
            (x - 19, y - 6),
            (x - 16, y - 14),
        ],
        IRON,
    );
    c.shape(
        &[
            (x - 10, y - 18),
            (x + 9, y - 18),
            (x + 15, y - 13),
            (x + 17, y - 6),
            (x + 14, y),
            (x + 8, y + 3),
            (x - 8, y + 3),
            (x - 15, y),
            (x - 17, y - 6),
            (x - 15, y - 13),
        ],
        ENAMEL,
    );
    c.shape(
        &[
            (x - 9, y - 17),
            (x + 7, y - 17),
            (x + 12, y - 13),
            (x + 13, y - 7),
            (x + 10, y - 2),
            (x - 8, y),
            (x - 13, y - 4),
            (x - 13, y - 12),
        ],
        0xe5cfab,
    );
    c.line((x - 14, y - 9), (x - 12, y - 14), PEARL);
    c.rect(x - 13, y - 11, 2, 5, PEARL);
    c.rect(x - 13, y - 19, 26, 2, IRON);
    c.shape(
        &[
            (x - 12, y - 20),
            (x - 8, y - 22),
            (x + 6, y - 22),
            (x + 12, y - 20),
        ],
        GLAZE,
    );
    c.rect(x - 2, y - 25, 5, 3, IRON);
    c.rect(x - 1, y - 25, 3, 2, WICKER);
    // A hand-painted blue cornflower makes the kitchen crockery unmistakable.
    for (dx, dy) in [(0, -3), (-3, -1), (-2, 2), (2, 2), (3, -1)] {
        c.rect(x + dx, y - 8 + dy, 3, 2, 0x638c9d);
    }
    c.rect(x, y - 8, 2, 2, AMBER);
    c.line((x, y - 5), (x + 2, y - 2), 0x71977e);
    steam(c, x + 28, y - 25, t);
    // On a tiny tender at the spout, one member makes her formal offer.
    let tender = x + 39;
    c.rect(tender - 4, y + 9, 18, 1, 0xb5ac94);
    hull(c, tender - 8, y + 3, 25, 3, 0xb18c69, 0x6d6356);
    grandmother(c, tender - 1, y + 2, 2, t);
    // Soup-boat etiquette requires a teacup even when the guest is the sea.
    let pour = (t / 2) % 55;
    if (12..48).contains(&pour) {
        let path = [
            (31, -21),
            (34, -21),
            (37, -19),
            (39, -16),
            (41, -11),
            (43, -7),
        ];
        for (n, pts) in path.windows(2).enumerate() {
            if pour < 14 + n as u32 || pour > 41 + n as u32 {
                continue;
            }
            c.line(
                (x + pts[0].0, y + pts[0].1),
                (x + pts[1].0, y + pts[1].1),
                0xcf8f52,
            );
            c.pixel(x + pts[0].0, y + pts[0].1, 0xf2c681);
        }
    }
    c.rect(x + 40, y - 6, 6, 1, PEARL);
    c.rect(x + 41, y - 5, 4, 2, GLAZE);
    c.line((tender + 1, y - 4), (x + 42, y - 3), 0xd5a67e);
    if t / 42 % 2 == 0 {
        chair(c, tender + 11, y - 1, WICKER);
    } else {
        chair(c, tender + 10, y - 3, WICKER);
    }
}

fn souvenir(c: &mut Canvas, t: u32) {
    let x = 25;
    let y = 150;
    c.rect(x - 7, y + 3, 17, 1, 0xa08667);
    c.rect(x - 7, y + 2, 16, 1, ENAMEL);
    c.rect(x - 5, y + 1, 12, 1, PEARL);
    c.rect(x - 3, y - 4, 7, 5, IRON);
    c.rect(x - 2, y - 3, 5, 4, GLAZE);
    c.rect(x - 3, y - 5, 7, 2, PEARL);
    c.rect(x - 2, y - 5, 5, 1, 0x795c46);
    c.rect(x + 4, y - 4, 3, 1, PEARL);
    c.rect(x + 6, y - 3, 1, 2, GLAZE);
    c.rect(x + 4, y - 1, 3, 1, ENAMEL);
    c.pixel(x + 1, y - 2, 0x638c9d);
    c.rect(x - 5, y, 4, 2, AMBER);
    c.pixel(x - 4, y, PEARL);
    if t / 65 % 3 == 0 {
        c.pixel(x - 1, y - 7, 0xdfcaaa);
    }
}

/// Paint the committee's public art state after the normal harbor watch.
/// 0 dormant, 1 invitation, 2 tea service, 3 quorum, 4 high tea, 5 adjourned.
pub fn draw_tea_committee(buf: &mut [u32; ART_W * ART_H], tick: u32, stage: u8) {
    if stage == 0 || stage > 5 {
        return;
    }
    let mut c = Canvas(buf);
    if stage == 5 {
        souvenir(&mut c, tick);
        return;
    }
    if stage == 1 {
        chair(&mut c, 25, 154, WICKER);
        c.rect(25, 140, 2, 7, 0x9b7756);
        c.rect(21, 137, 10, 6, 0x887158);
        c.rect(22, 138, 8, 4, PEARL);
        c.rect(24, 139, 5, 1, COZY);
        c.rect(24, 141, 3, 1, 0x8f9180);
        return;
    }
    chair(&mut c, 14, 158, WICKER);
    grandmother(&mut c, 17, 152, 0, tick);
    grandmother(&mut c, 29, 149, 1, tick);
    tea_table(&mut c, 12, 151, tick);
    // At High Tea, the third member has moved from the pier to the tender.
    if stage < 4 {
        grandmother(&mut c, 60, 140, 2, tick);
    }
    if stage >= 3 {
        floating_chairs(&mut c, tick);
        sash(&mut c, tick);
    }
    if stage == 4 {
        pot_float(&mut c, tick);
    }
}
