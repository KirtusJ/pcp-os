//! What the kernel itself draws on a display: text in one typeface, filled rectangles, and the product's
//! emblem. The colours are those of the Core profile in the PCP design tokens (navy ground, the reference
//! bird's teal), which is the profile of a workspace that claims no domain.
//!
//! This file has no `unsafe`: it asks `dev` to put pixels on the display, and `dev` asks `arch`, which
//! checks every address.
use crate::dev;
use crate::font::{CELL_HEIGHT, CELL_WIDTH, FIRST, GLYPHS};
use alloc::vec::Vec;

/// The Core plate, the text on it, and the accents.
pub const GROUND: u32 = 0x10283F;
pub const TEXT: u32 = 0xF2F7FC;
pub const SOFT: u32 = 0x9FB4C8;
pub const TEAL: u32 = 0x41BAD7;
pub const AMBER: u32 = 0xE3A24B;
pub const PANEL: u32 = 0x18364F;
pub const GOOD: u32 = 0x8FE0C2;
pub const BAD: u32 = 0xF08A72;

pub const CELL_W: u32 = CELL_WIDTH as u32;
pub const CELL_H: u32 = CELL_HEIGHT as u32;

fn blend(under: u32, over: u32, coverage: u32) -> u32 {
    let mix = |shift: u32| {
        let (u, o) = (under >> shift & 0xff, over >> shift & 0xff);
        (u * (255 - coverage) + o * coverage + 127) / 255
    };
    mix(16) << 16 | mix(8) << 8 | mix(0)
}

/// Fill the whole display.
pub fn clear(colour: u32) {
    if let Some(d) = dev::display() {
        dev::display_fill(0, 0, d.width, d.height, colour);
    }
}

pub fn fill(x: u32, y: u32, w: u32, h: u32, colour: u32) {
    dev::display_fill(x, y, w, h, colour);
}

/// Draw text with its top-left corner at (x, y), in pixels. A character the typeface lacks is drawn as `?`.
/// Returns the x just after the text.
pub fn text(x: u32, y: u32, s: &str, colour: u32, ground: u32) -> u32 {
    let n = s.chars().count();
    if n == 0 {
        return x;
    }
    let mut row: Vec<u32> = alloc::vec![ground; n * CELL_WIDTH];
    for line in 0..CELL_HEIGHT {
        for (i, ch) in s.chars().enumerate() {
            let code = if (ch as u32) >= FIRST as u32 && (ch as u32) < 127 { ch as usize - FIRST as usize } else { b'?' as usize - FIRST as usize };
            let glyph = &GLYPHS[code];
            for col in 0..CELL_WIDTH {
                row[i * CELL_WIDTH + col] = blend(ground, colour, glyph[line * CELL_WIDTH + col] as u32);
            }
        }
        dev::display_row(x, y + line as u32, &row);
    }
    x + (n * CELL_WIDTH) as u32
}

/// Text at a column and row of the character grid.
pub fn at(col: u32, row: u32, s: &str, colour: u32, ground: u32) -> u32 {
    text(col * CELL_W, row * CELL_H, s, colour, ground) / CELL_W
}

/// (columns, rows) of the character grid on the display as it is.
pub fn grid() -> (u32, u32) {
    dev::display().map(|d| (d.width / CELL_W, d.height / CELL_H)).unwrap_or((0, 0))
}

/// The emblem's shapes, in tenths of the bird's own units, in drawing order: an ellipse is (centre x, centre y,
/// radius x, radius y); a triangle is its three corners. The same shapes and colours as the product's covers.
enum Shape {
    Ellipse(i64, i64, i64, i64),
    Triangle([(i64, i64); 3]),
}
const BIRD: [(Shape, u32); 6] = [
    (Shape::Ellipse(547, 611, 410, 487), 0x187C78),
    (Shape::Ellipse(670, 298, 277, 277), 0xE2F2ED),
    (Shape::Triangle([(885, 195), (1172, 339), (885, 421)]), 0xE3A24B),
    (Shape::Ellipse(731, 236, 44, 44), 0x18324B),
    (Shape::Ellipse(490, 713, 272, 302), 0x41BAD7),
    (Shape::Triangle([(239, 944), (34, 1302), (444, 1056)]), 0x187C78),
];

fn inside(shape: &Shape, x: i64, y: i64) -> bool {
    match shape {
        Shape::Ellipse(cx, cy, rx, ry) => {
            let (dx, dy) = ((x - cx * 16) * ry, (y - cy * 16) * rx);
            dx * dx + dy * dy <= (rx * ry * 16) * (rx * ry * 16)
        }
        Shape::Triangle(p) => {
            let mut hit = false;
            for i in 0..3 {
                let ((x1, y1), (x2, y2)) = ((p[i].0 * 16, p[i].1 * 16), (p[(i + 1) % 3].0 * 16, p[(i + 1) % 3].1 * 16));
                if (y1 > y) != (y2 > y) {
                    let (lhs, rhs) = ((x - x1) * (y2 - y1), (x2 - x1) * (y - y1));
                    if (y2 > y1 && lhs < rhs) || (y2 < y1 && lhs > rhs) {
                        hit = !hit;
                    }
                }
            }
            hit
        }
    }
}

/// Draw the emblem `height` pixels tall with its top-left corner at (x, y), on `ground`. Each pixel is the
/// mean of nine samples, so the edges are smooth.
pub fn emblem(x: u32, y: u32, height: u32, ground: u32) {
    // The bird is 121 units wide and 132 tall; a unit is 160 of the fixed-point steps used here.
    let width = height * 121 / 132;
    let mut row: Vec<u32> = alloc::vec![ground; width as usize];
    for py in 0..height {
        for px in 0..width {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for sy in 0..3i64 {
                for sx in 0..3i64 {
                    let bx = ((px as i64 * 6 + sx * 2 + 1) * 132 * 160) / (height as i64 * 6);
                    let by = ((py as i64 * 6 + sy * 2 + 1) * 132 * 160) / (height as i64 * 6);
                    let mut c = ground;
                    for (shape, colour) in BIRD.iter() {
                        if inside(shape, bx, by) {
                            c = *colour;
                        }
                    }
                    r += c >> 16 & 0xff;
                    g += c >> 8 & 0xff;
                    b += c & 0xff;
                }
            }
            row[px as usize] = (r / 9) << 16 | (g / 9) << 8 | b / 9;
        }
        dev::display_row(x, y + py, &row);
    }
}
