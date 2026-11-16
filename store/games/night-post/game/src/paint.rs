use crate::{Pixels, H, W};
pub type Color = u32;

pub const INK: Color = 0x171931;
pub const PEARL: Color = 0xffeac8;
pub const AMBER: Color = 0xffb46e;
pub const CORAL: Color = 0xf48173;
pub const JADE: Color = 0x92ded3;
pub const MIST: Color = 0xb6b4cf;

pub struct Canvas<'a>(pub &'a mut Pixels);
impl Canvas<'_> {
    pub fn dot(&mut self, x: i32, y: i32, c: Color) {
        if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
            self.0[y as usize * W + x as usize] = c;
        }
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let a = x.clamp(0, W as i32) as usize;
        let b = (x + w).clamp(0, W as i32) as usize;
        for row in y.clamp(0, H as i32)..(y + h).clamp(0, H as i32) {
            let base = row as usize * W;
            self.0[base + a..base + b].fill(c);
        }
    }
    pub fn line(&mut self, mut x: i32, mut y: i32, ex: i32, ey: i32, c: Color) {
        let (dx, dy) = ((ex - x).abs(), -(ey - y).abs());
        let (sx, sy) = ((ex - x).signum(), (ey - y).signum());
        let mut error = dx + dy;
        loop {
            self.dot(x, y, c);
            if x == ex && y == ey {
                break;
            }
            let e = 2 * error;
            if e >= dy {
                error += dy;
                x += sx;
            }
            if e <= dx {
                error += dx;
                y += sy;
            }
        }
    }
    pub fn poly(&mut self, points: &[(i32, i32)], c: Color) {
        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let max_y = points
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap_or(0)
            .min(H as i32 - 1);
        for y in min_y..=max_y {
            let mut cross = [0; 32];
            let mut count = 0;
            let mut prior = points[points.len() - 1];
            for &next in points {
                if (prior.1 > y) != (next.1 > y) {
                    let x = prior.0 + (next.0 - prior.0) * (y - prior.1) / (next.1 - prior.1);
                    let mut k = count;
                    while k > 0 && cross[k - 1] > x {
                        cross[k] = cross[k - 1];
                        k -= 1;
                    }
                    cross[k] = x;
                    count += 1;
                }
                prior = next;
            }
            for pair in cross[..count].chunks_exact(2) {
                self.rect(pair[0], y, pair[1] - pair[0] + 1, 1, c);
            }
        }
    }
    pub fn disc(&mut self, x: i32, y: i32, r: i32, c: Color) {
        for dy in -r..=r {
            let mut dx = r;
            while dx * dx + dy * dy > r * r {
                dx -= 1;
            }
            self.rect(x - dx, y + dy, dx * 2 + 1, 1, c);
        }
    }
    pub fn text(&mut self, mut x: i32, y: i32, s: &str, scale: i32, c: Color) {
        for b in s.bytes() {
            for (r, bits) in glyph(b).iter().enumerate() {
                for col in 0..5 {
                    if bits & (16 >> col) != 0 {
                        self.rect(x + col * scale, y + r as i32 * scale, scale, scale, c);
                    }
                }
            }
            x += 6 * scale;
        }
    }
}

// A hand-set five-column alphabet, shared by the instrument panel and the title.
fn glyph(b: u8) -> [u8; 7] {
    match b {
        b'a' => [0, 0, 14, 1, 15, 17, 15],
        b'b' => [16, 16, 22, 25, 17, 17, 30],
        b'c' => [0, 0, 14, 16, 16, 16, 14],
        b'd' => [1, 1, 13, 19, 17, 17, 15],
        b'e' => [0, 0, 14, 17, 31, 16, 14],
        b'f' => [6, 8, 8, 30, 8, 8, 8],
        b'g' => [0, 0, 15, 17, 15, 1, 14],
        b'h' => [16, 16, 22, 25, 17, 17, 17],
        b'i' => [4, 0, 12, 4, 4, 4, 14],
        b'j' => [2, 0, 6, 2, 2, 18, 12],
        b'k' => [16, 16, 18, 20, 24, 20, 18],
        b'l' => [12, 4, 4, 4, 4, 4, 14],
        b'm' => [0, 0, 26, 21, 21, 21, 21],
        b'n' => [0, 0, 22, 25, 17, 17, 17],
        b'o' => [0, 0, 14, 17, 17, 17, 14],
        b'p' => [0, 0, 30, 17, 30, 16, 16],
        b'q' => [0, 0, 15, 17, 15, 1, 1],
        b'r' => [0, 0, 22, 25, 16, 16, 16],
        b's' => [0, 0, 15, 16, 14, 1, 30],
        b't' => [8, 8, 30, 8, 8, 9, 6],
        b'u' => [0, 0, 17, 17, 17, 19, 13],
        b'v' => [0, 0, 17, 17, 17, 10, 4],
        b'w' => [0, 0, 17, 17, 21, 21, 10],
        b'x' => [0, 0, 17, 10, 4, 10, 17],
        b'y' => [0, 0, 17, 17, 15, 1, 14],
        b'z' => [0, 0, 31, 2, 4, 8, 31],
        b'\'' => [4, 4, 8, 0, 0, 0, 0],
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [15, 16, 16, 16, 16, 16, 15],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [15, 16, 16, 19, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [31, 4, 4, 4, 4, 4, 31],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 25, 21, 19, 19, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 27, 17],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'.' => [0, 0, 0, 0, 0, 4, 4],
        b',' => [0, 0, 0, 0, 0, 4, 8],
        b'-' => [0, 0, 0, 14, 0, 0, 0],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b':' => [0, 4, 4, 0, 4, 4, 0],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'>' => [16, 8, 4, 2, 4, 8, 16],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        _ => [0; 7],
    }
}

pub fn blend(a: Color, b: Color, n: i32, d: i32) -> Color {
    let mut result = 0;
    for shift in [0, 8, 16] {
        let av = ((a >> shift) & 255) as i32;
        let bv = ((b >> shift) & 255) as i32;
        result |= ((av + (bv - av) * n / d) as u32) << shift;
    }
    result
}
pub fn noise(mut n: u32) -> u32 {
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}
