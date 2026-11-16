use crate::{game::*, ink::Frame};

#[derive(Clone, Copy, Default)]
struct EyePoint {
    x: i32,
    y: i32,
    d: i32,
}
#[derive(Clone, Copy, Default)]
struct Pixel {
    x: i32,
    y: i32,
    q: i32,
}
#[derive(Clone, Copy)]
pub struct Camera {
    eye: V,
    yaw: i32,
    tilt: i32,
    center: (i32, i32),
    focal: i32,
}
impl Camera {
    pub fn postcard() -> Self {
        Self {
            eye: v(-1175, 895, -1690),
            yaw: 23,
            tilt: 11,
            center: (340, 137),
            focal: 279,
        }
    }
    pub fn follow(g: &Game) -> Self {
        let aim = v(g.camera.x, g.camera.y + 88, g.camera.z);
        let mut eye = aim;
        for step in 1..=20 {
            let back = (540 * step) / 20;
            let p = v(
                aim.x - sin(g.yaw) * back / 256,
                aim.y + 193 * step / 20,
                aim.z - cos(g.yaw) * back / 256,
            );
            let tower = (p.x - BEACON.x).abs() < 130
                && (p.z - BEACON.z).abs() < 130
                && p.y < BEACON.y + 650;
            if step > 2 && (p.y <= surface(p.x, p.z, 10000) + 15 || tower) {
                break;
            }
            eye = p;
        }
        Self {
            eye,
            yaw: g.yaw,
            tilt: 11,
            center: (W / 2, 132),
            focal: 269,
        }
    }
    // Camera coordinates retain eight fractional bits through yaw, pitch and clipping.
    fn transform(self, p: V) -> EyePoint {
        let x = p.x - self.eye.x;
        let y = p.y - self.eye.y;
        let z = p.z - self.eye.z;
        let f = x * sin(self.yaw) + z * cos(self.yaw);
        EyePoint {
            x: x * cos(self.yaw) - z * sin(self.yaw),
            y: y * cos(self.tilt) + (i64::from(f) * i64::from(sin(self.tilt)) / 256) as i32,
            d: (i64::from(f) * i64::from(cos(self.tilt)) / 256) as i32 - y * sin(self.tilt),
        }
    }
    fn project(self, p: EyePoint) -> Pixel {
        let scale = i64::from(self.focal) * 256;
        Pixel {
            x: self.center.0 * 256 + (i64::from(p.x) * scale / i64::from(p.d)) as i32,
            y: self.center.1 * 256 - (i64::from(p.y) * scale / i64::from(p.d)) as i32,
            q: ((1i64 << 44) / i64::from(p.d)) as i32,
        }
    }
    pub fn horizon(self) -> i32 {
        self.center.1 - self.focal * sin(self.tilt) / cos(self.tilt)
    }
    pub fn yaw(self) -> i32 {
        self.yaw
    }
}
const NEAR: i32 = 58 * 256;
impl EyePoint {
    fn intersection(self, other: Self) -> Self {
        // Order by depth so both faces adjoining an edge produce the same rounded vertex.
        let (a, b) = if self.d < other.d {
            (self, other)
        } else {
            (other, self)
        };
        let distance = i64::from(NEAR - a.d);
        let range = i64::from(b.d - a.d);
        let blend = |start: i32, end: i32| {
            start + ((i64::from(end - start) * distance).div_euclid(range)) as i32
        };
        Self {
            x: blend(a.x, b.x),
            y: blend(a.y, b.y),
            d: NEAR,
        }
    }
}
pub struct Painter<'a, 'b> {
    pub f: &'a mut Frame<'b>,
    pub depth: &'a mut [i32],
    pub camera: Camera,
}
impl Painter<'_, '_> {
    pub fn poly(&mut self, vs: &[V], color: u32) {
        // The scene's convex faces have at most eight vertices; one near plane adds at most one.
        let mut buf = [EyePoint::default(); 10];
        let mut len = 0;
        let mut a = self.camera.transform(vs[vs.len() - 1]);
        for &p in vs {
            let b = self.camera.transform(p);
            if (a.d < NEAR && b.d > NEAR) || (b.d < NEAR && a.d > NEAR) {
                buf[len] = a.intersection(b);
                len += 1
            }
            if b.d >= NEAR {
                buf[len] = b;
                len += 1
            }
            a = b;
        }
        if len < 3 {
            return;
        }
        let first = self.camera.project(buf[0]);
        let mut prev = self.camera.project(buf[1]);
        for p in &buf[2..len] {
            let next = self.camera.project(*p);
            self.triangle(first, prev, next, color);
            prev = next;
        }
    }
    fn triangle(&mut self, mut a: Pixel, mut b: Pixel, mut c: Pixel, col: u32) {
        if a.y > b.y {
            core::mem::swap(&mut a, &mut b)
        }
        if b.y > c.y {
            core::mem::swap(&mut b, &mut c)
        }
        if a.y > b.y {
            core::mem::swap(&mut a, &mut b)
        }
        if a.y == c.y
            || a.y >= H * 256
            || c.y <= 0
            || a.x.min(b.x).min(c.x) >= W * 256
            || a.x.max(b.x).max(c.x) < 0
        {
            return;
        }
        for (e, h) in [(a, b), (b, c)] {
            let top = first_sample(e.y).max(0);
            let end = first_sample(h.y).min(H);
            if top >= end {
                continue;
            }
            for y in top..end {
                let cy = y * 256 + 128;
                let (mut left, mut right) = (edge_at(a, c, cy), edge_at(e, h, cy));
                if left.0 > right.0 {
                    core::mem::swap(&mut left, &mut right)
                }
                let lo = first_sample(left.0).max(0);
                let hi = first_sample(right.0).min(W);
                if lo >= hi {
                    continue;
                }
                let width = i64::from(right.0 - left.0);
                let dq = i64::from(right.1 - left.1);
                let dz = (dq * 256 / width) as i32;
                let mut q = left.1 + (i64::from(lo * 256 + 128 - left.0) * dq / width) as i32;
                let at = y as usize * W as usize;
                let zbuf = &mut self.depth[at + lo as usize..at + hi as usize];
                let dst = &mut self.f.px[at + lo as usize..at + hi as usize];
                for (z, p) in zbuf.iter_mut().zip(dst) {
                    if q > *z {
                        *z = q;
                        *p = col
                    }
                    q = q.wrapping_add(dz);
                }
            }
        }
    }
}
// Pixels are sampled at half-integers. Upper and left edges include a sample on the edge.
fn first_sample(subpixel: i32) -> i32 {
    (subpixel + 127) >> 8
}
fn edge_at(a: Pixel, b: Pixel, y: i32) -> (i32, i32) {
    let dy = i64::from(y - a.y);
    let height = i64::from(b.y - a.y);
    let x = i64::from(a.x) + (i64::from(b.x - a.x) * dy).div_euclid(height);
    let q = i64::from(a.q) + i64::from(b.q - a.q) * dy / height;
    (x as i32, q as i32)
}
