//! What the kernel itself draws on a display: text in one typeface, filled rectangles, and the product's
//! emblem. The colours are those of the Core profile in the PCP design tokens (navy ground, the reference
//! bird's teal), which is the profile of a workspace that claims no domain.
//!
//! This file has no `unsafe`: it asks `dev` to put pixels on the display, and `dev` asks `arch`, which
//! checks every address.
use crate::dev;
use crate::font::{CELL_HEIGHT, CELL_WIDTH, FIRST, GLYPHS};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

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
static SCALE: AtomicU32 = AtomicU32::new(1);
const DOUBLE: &[u8; 95 * 20 * 40] = include_bytes!("font-2x.bin");

pub fn set_scale(value: u32) {
    SCALE.store(value, Ordering::Relaxed);
}

pub fn scale() -> u32 {
    SCALE.load(Ordering::Relaxed)
}

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
    let z = scale();
    dev::display_fill(x * z, y * z, w * z, h * z, colour);
}

/// Draw text with its top-left corner at (x, y), in pixels. A character the typeface lacks is drawn as `?`.
/// Returns the x just after the text.
pub fn text(x: u32, y: u32, s: &str, colour: u32, ground: u32) -> u32 {
    let z = scale() as usize;
    let (width, height) = (CELL_WIDTH * z, CELL_HEIGHT * z);
    let available = dev::display().map(|d| (d.width / z as u32).saturating_sub(x).div_ceil(CELL_W) as usize).unwrap_or(0);
    let n = s.chars().take(available).count();
    if n == 0 {
        return x;
    }
    let mut row: Vec<u32> = alloc::vec![ground; n * width];
    for line in 0..height {
        for (i, ch) in s.chars().take(n).enumerate() {
            let code = if (ch as u32) >= FIRST as u32 && (ch as u32) < 127 { ch as usize - FIRST as usize } else { b'?' as usize - FIRST as usize };
            let glyph: &[u8] = if z == 1 { &GLYPHS[code] } else { &DOUBLE[code * width * height..(code + 1) * width * height] };
            for col in 0..width {
                row[i * width + col] = blend(ground, colour, glyph[line * width + col] as u32);
            }
        }
        dev::display_row(x * z as u32, y * z as u32 + line as u32, &row);
    }
    x + (n * CELL_WIDTH) as u32
}

/// Text at a column and row of the character grid.
pub fn at(col: u32, row: u32, s: &str, colour: u32, ground: u32) -> u32 {
    text(col * CELL_W, row * CELL_H, s, colour, ground) / CELL_W
}

/// (columns, rows) of the character grid on the display as it is.
pub fn grid() -> (u32, u32) {
    let z = scale();
    dev::display().map(|d| (d.width / (CELL_W * z), d.height / (CELL_H * z))).unwrap_or((0, 0))
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
    let z = scale();
    emblem_over(x * z, y * z, height * z, &|_, _| ground);
}

/// The emblem at a place given in display pixels, over whatever `under` says is beneath each pixel.
pub fn emblem_over(x: u32, y: u32, height: u32, under: &dyn Fn(u32, u32) -> u32) {
    // The bird is 121 units wide and 132 tall; a unit is 160 of the fixed-point steps used here.
    let width = height * 121 / 132;
    let mut row: Vec<u32> = alloc::vec![0; width as usize];
    for py in 0..height {
        for px in 0..width {
            let ground = under(x + px, y + py);
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

// ---- the desktop's own drawing: a soft field, panels with a lit edge, and text over either ---------------------
//
// Everything here is computed from a few numbers; nothing is a stored picture, so it is as sharp at one
// resolution as at another. Places and sizes are in display pixels.

/// The whole part of a square root.
pub fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let (mut low, mut high) = (1u64, 1u64 << 32);
    while low + 1 < high {
        let mid = (low + high) / 2;
        if mid * mid <= n { low = mid } else { high = mid }
    }
    low
}

/// The desktop's field: the Core plate shaded from a little lighter at the top to a little darker at the
/// bottom, a faint teal light around one point, and thin rings about that point (the rings of the product's
/// backgrounds).
#[derive(Clone, Copy)]
pub struct Field {
    pub width: u32,
    pub height: u32,
    /// The centre of the light and of the rings.
    pub cx: u32,
    pub cy: u32,
}

const FIELD_TOP: u32 = 0x132B45;
const FIELD_MID: u32 = 0x0E2238;
const FIELD_END: u32 = 0x0A1A2C;
const FIELD_LIGHT: u32 = 0x1F6F7A;

impl Field {
    /// The unit every length of the field is a multiple of: one at 1080 rows.
    fn unit(&self, n: u32) -> u64 {
        (n as u64 * self.height as u64 / 1080).max(1)
    }

    fn shade(&self, y: u32) -> u32 {
        let turn = self.height * 55 / 100;
        if y < turn {
            blend(FIELD_TOP, FIELD_MID, y * 255 / turn.max(1))
        } else {
            blend(FIELD_MID, FIELD_END, (y - turn) * 255 / (self.height - turn).max(1))
        }
    }

    /// The colour at a point, given its whole distance from the centre.
    fn lit(&self, y: u32, distance: u64) -> u32 {
        let mut c = self.shade(y);
        let reach = self.unit(620);
        if distance < reach {
            c = blend(c, FIELD_LIGHT, ((reach - distance) * 107 / reach) as u32);
        }
        let (first, step) = (self.unit(230), self.unit(140));
        if distance + 1 >= first && (distance + 1 - first) % step < 2 {
            c = blend(c, TEXT, 15);
        }
        c
    }

    /// The colour of the field at one point.
    pub fn colour(&self, x: u32, y: u32) -> u32 {
        let (dx, dy) = (x as i64 - self.cx as i64, y as i64 - self.cy as i64);
        self.lit(y, isqrt((dx * dx + dy * dy) as u64))
    }

    /// Paint the field over the whole display. The distance is carried from one pixel to the next, since it
    /// changes by less than one between neighbours.
    pub fn paint(&self) {
        let mut row: Vec<u32> = alloc::vec![0; self.width as usize];
        for y in 0..self.height {
            let dy = y as i64 - self.cy as i64;
            let mut r = isqrt((self.cx as u64 * self.cx as u64) + (dy * dy) as u64);
            for x in 0..self.width {
                let dx = x as i64 - self.cx as i64;
                let d2 = (dx * dx + dy * dy) as u64;
                while r * r > d2 { r -= 1 }
                while (r + 1) * (r + 1) <= d2 { r += 1 }
                row[x as usize] = self.lit(y, r);
            }
            dev::display_row(0, y, &row);
        }
    }
}

/// A flat bar across the display with a lit top edge and a line beneath: the status bar and the key bar.
/// Returns the colour text on it lies on, for each row.
pub fn bar(y: u32, height: u32, width: u32, top: u32, bottom: u32, line: u32) -> impl Fn(u32, u32) -> u32 {
    let shade = move |_x: u32, py: u32| blend(top, bottom, (py.saturating_sub(y)) * 255 / height.max(1));
    let mut row: Vec<u32> = alloc::vec![0; width as usize];
    for py in y..y + height {
        let c = if py == y { blend(shade(0, py), TEXT, 56) } else if py + 1 == y + height { line } else { shade(0, py) };
        row.iter_mut().for_each(|p| *p = c);
        dev::display_row(0, py, &row);
    }
    shade
}

/// A panel: `tint` laid over what is beneath at `strength` of 255, with an outline and, where `lit`, one
/// brighter line inside its top edge. Returns what text on it lies on.
pub fn panel<'a>(x: u32, y: u32, w: u32, h: u32, under: &'a dyn Fn(u32, u32) -> u32, tint: u32, strength: u32, outline: u32, lit: bool) -> impl Fn(u32, u32) -> u32 + 'a {
    let body = move |px: u32, py: u32| blend(under(px, py), tint, strength);
    let mut row: Vec<u32> = alloc::vec![0; w as usize];
    for py in y..y + h {
        for (i, p) in row.iter_mut().enumerate() {
            let px = x + i as u32;
            let edge = py == y || py + 1 == y + h || i == 0 || i as u32 + 1 == w;
            *p = if edge { blend(under(px, py), outline, 200) } else if lit && py == y + 1 { blend(body(px, py), TEXT, 60) } else { body(px, py) };
        }
        dev::display_row(x, py, &row);
    }
    body
}

/// Text at a place in display pixels, in the ordinary size or twice it, over whatever `under` says is
/// beneath each pixel. Returns the x just after the text.
pub fn label(x: u32, y: u32, s: &str, colour: u32, large: bool, under: &dyn Fn(u32, u32) -> u32) -> u32 {
    let z = if large { 2usize } else { 1 };
    let (width, height) = (CELL_WIDTH * z, CELL_HEIGHT * z);
    let available = dev::display().map(|d| (d.width.saturating_sub(x) as usize) / width).unwrap_or(0);
    let n = s.chars().take(available).count();
    if n == 0 {
        return x;
    }
    let mut row: Vec<u32> = alloc::vec![0; n * width];
    for line in 0..height {
        for (i, ch) in s.chars().take(n).enumerate() {
            let code = if (ch as u32) >= FIRST as u32 && (ch as u32) < 127 { ch as usize - FIRST as usize } else { b'?' as usize - FIRST as usize };
            let glyph: &[u8] = if z == 1 { &GLYPHS[code] } else { &DOUBLE[code * width * height..(code + 1) * width * height] };
            for col in 0..width {
                let (px, py) = (x + (i * width + col) as u32, y + line as u32);
                row[i * width + col] = blend(under(px, py), colour, glyph[line * width + col] as u32);
            }
        }
        dev::display_row(x, y + line as u32, &row);
    }
    x + (n * width) as u32
}

/// How wide `s` is as a label.
pub fn label_width(s: &str, large: bool) -> u32 {
    s.chars().count() as u32 * CELL_W * if large { 2 } else { 1 }
}
