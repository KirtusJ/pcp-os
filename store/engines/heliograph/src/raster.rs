//! Screen-space edge functions on a 1/16 pixel grid. Positive depth points into the view.
use crate::assets::sin;
pub const NEAR: i32 = 192;
const SUB: i32 = 16;
pub struct Mesh<'a> {
    /// Little-endian i16 XYZ followed by signed i8 view-independent normal XYZ.
    pub vertices: &'a [u8],
    /// Three little-endian u16 indices for each outward-wound triangle.
    pub indices: &'a [u8],
    /// Conservative origin-centred model-space bounding radius in vertex units.
    pub radius: i32,
}
impl Mesh<'_> {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 6
    }
    pub fn validate(&self) -> bool {
        let n = self.vertices.len() / 9;
        self.radius > 0
            && self.radius <= 32767
            && self.vertices.len() % 9 == 0
            && self.indices.len() % 6 == 0
            && n > 0
            && n <= 65536
            && self
                .indices
                .chunks_exact(2)
                .all(|i| (u16::from_le_bytes([i[0], i[1]]) as usize) < n)
            && self.vertices.chunks_exact(9).all(|v| {
                let p = |i| i16::from_le_bytes([v[i], v[i + 1]]) as i64;
                p(0) * p(0) + p(2) * p(2) + p(4) * p(4) <= self.radius as i64 * self.radius as i64
            })
    }
}
pub struct Material<'a>(&'a [u8; 128 * 128 * 4]);
impl<'a> Material<'a> {
    pub const fn new(data: &'a [u8; 128 * 128 * 4]) -> Self {
        Self(data)
    }
    #[inline]
    fn color(&self, u: i32, v: i32) -> u32 {
        let i = (v.clamp(0, 127) as usize * 128 + u.clamp(0, 127) as usize) * 4;
        u32::from_le_bytes(self.0[i..i + 4].try_into().unwrap())
    }
}
#[derive(Clone, Copy)]
pub struct Instance {
    pub pos: [i32; 3],
    pub yaw: i32,
    pub tilt: i32,
    pub scale: i32,
}
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Counts {
    pub source: u32,
    pub submitted: u32,
    pub culled: u32,
    pub vertices: u32,
    pub rasterized: u32,
    pub sampled: u32,
    pub visible_writes: u32,
    pub covered: u32,
    pub writes: u32,
}
#[derive(Clone, Copy, Default)]
struct CameraVertex {
    x: i32,
    y: i32,
    z: i32,
    u: i32,
    v: i32,
}
#[derive(Clone, Copy, Default)]
struct Screen {
    x: i32,
    y: i32,
    d: i32,
    u: i32,
    v: i32,
}
#[derive(Clone, Copy, Default)]
pub struct Cached {
    camera: CameraVertex,
    screen: Screen,
}
impl Cached {
    pub const ZERO: Self = Self {
        camera: CameraVertex {
            x: 0,
            y: 0,
            z: 0,
            u: 0,
            v: 0,
        },
        screen: Screen {
            x: 0,
            y: 0,
            d: 0,
            u: 0,
            v: 0,
        },
    };
}
pub struct Target<'a> {
    pub color: &'a mut [u32],
    pub depth: &'a mut [i32],
    width: i32,
    height: i32,
    focal: i32,
}
impl<'a> Target<'a> {
    pub fn new(
        color: &'a mut [u32],
        depth: &'a mut [i32],
        width: i32,
        height: i32,
        focal: i32,
    ) -> Self {
        assert!(
            (1..=4096).contains(&width)
                && (1..=4096).contains(&height)
                && (1..=2048).contains(&focal)
        );
        let len = (width * height) as usize;
        assert!(color.len() >= len && depth.len() >= len);
        Self {
            color,
            depth,
            width,
            height,
            focal,
        }
    }
    /// The coarsest level meeting a projected reference-vertex displacement budget (Q8 pixels).
    /// The near side of the bound is used for distance; the factor two allows for oblique viewing.
    pub fn select_lod(
        &self,
        errors: &[i32],
        instance: Instance,
        radius: i32,
        budget_q8: i32,
    ) -> usize {
        assert!(!errors.is_empty() && budget_q8 >= 0);
        let near = (instance.pos[2] - (radius * instance.scale + 1023) / 1024).max(NEAR);
        errors
            .iter()
            .position(|&e| {
                i64::from(e) * i64::from(instance.scale) * i64::from(self.focal) * 512
                    <= i64::from(budget_q8) * i64::from(near) * 1024
            })
            .unwrap_or(errors.len() - 1)
    }
    fn project(&self, p: CameraVertex) -> Screen {
        let term = |v: i32| (i64::from(v) * i64::from(self.focal * SUB) / i64::from(p.z)) as i32;
        Screen {
            x: self.width * 8 + term(p.x),
            y: self.height * 8 - term(p.y),
            d: (1 << 26) / p.z,
            u: p.u,
            v: p.v,
        }
    }
    /// Call validate once on asset admission. Position units match the mesh, scale is Q10.
    pub fn draw(
        &mut self,
        mesh: &Mesh,
        instance: Instance,
        mat: &Material,
        cache: &mut [Cached],
        counts: &mut Counts,
    ) {
        assert!(
            (1..=2048).contains(&instance.scale)
                && instance.pos.iter().all(|n| (-65535..=65535).contains(n))
        );
        let n = mesh.vertices.len() / 9;
        assert!(cache.len() >= n);
        let [tx, ty, tz] = instance.pos;
        let radius = (mesh.radius * instance.scale + 1023) / 1024 + 4;
        counts.source += mesh.triangles() as u32;
        counts.submitted += mesh.triangles() as u32;
        if tz + radius < NEAR
            || (tx.abs() - radius) as i64 * self.focal as i64
                > (tz + radius) as i64 * (self.width + 1) as i64 / 2
            || (ty.abs() - radius) as i64 * self.focal as i64
                > (tz + radius) as i64 * (self.height + 1) as i64 / 2
        {
            counts.culled += mesh.triangles() as u32;
            return;
        }
        counts.vertices += n as u32;
        let (sy, cy, sp, cp) = (
            sin(instance.yaw),
            sin(instance.yaw + 256),
            sin(instance.tilt),
            sin(instance.tilt + 256),
        );
        let rotate = |x: i32, y: i32, z: i32| {
            let rx = (cy * x + sy * z) >> 14;
            let rz = (-sy * x + cy * z) >> 14;
            [rx, (cp * y - sp * rz) >> 14, (sp * y + cp * rz) >> 14]
        };
        for (v, out) in mesh.vertices.chunks_exact(9).zip(cache.iter_mut()) {
            let s = |i| i16::from_le_bytes([v[i], v[i + 1]]) as i32;
            let [x, y, z] = rotate(s(0), s(2), s(4));
            let [nx, ny, _] = rotate(v[6] as i8 as i32, v[7] as i8 as i32, v[8] as i8 as i32);
            let p = CameraVertex {
                x: tx + (x * instance.scale >> 10),
                y: ty + (y * instance.scale >> 10),
                z: tz + (z * instance.scale >> 10),
                u: (nx + 127) * 128,
                v: (ny + 127) * 128,
            };
            out.camera = p;
            if p.z >= NEAR {
                out.screen = self.project(p);
            }
        }
        for ix in mesh.indices.chunks_exact(6) {
            let at = |i| cache[u16::from_le_bytes([ix[i], ix[i + 1]]) as usize];
            let (a, b, c) = (at(0), at(2), at(4));
            if a.camera.z.min(b.camera.z).min(c.camera.z) >= NEAR {
                self.triangle(a.screen, b.screen, c.screen, mat, counts);
            } else if a.camera.z.max(b.camera.z).max(c.camera.z) >= NEAR {
                let (p, len) = clip_near([a.camera, b.camera, c.camera]);
                let first = self.project(p[0]);
                for j in 1..len - 1 {
                    self.triangle(
                        first,
                        self.project(p[j]),
                        self.project(p[j + 1]),
                        mat,
                        counts,
                    );
                }
            }
        }
    }
    fn triangle(&mut self, a: Screen, b: Screen, c: Screen, mat: &Material, count: &mut Counts) {
        let edge = |p: Screen, q: Screen, x: i32, y: i32| {
            i64::from(q.x - p.x) * i64::from(y - p.y) - i64::from(q.y - p.y) * i64::from(x - p.x)
        };
        let area = edge(a, b, c.x, c.y);
        if area <= 0 {
            return;
        }
        let lo = |a: i32, b: i32, c: i32| ((a.min(b).min(c) + 7) >> 4).max(0);
        let hi = |a: i32, b: i32, c: i32, limit: i32| ((a.max(b).max(c) - 8) >> 4).min(limit - 1);
        let x0 = lo(a.x, b.x, c.x);
        let x1 = hi(a.x, b.x, c.x, self.width);
        let y0 = lo(a.y, b.y, c.y);
        let y1 = hi(a.y, b.y, c.y, self.height);
        if x1 < x0 || y1 < y0 {
            return;
        }
        count.rasterized += 1;
        let top_left = |p: Screen, q: Screen| i64::from(!(q.y < p.y || (q.y == p.y && q.x > p.x)));
        let bias = [top_left(b, c), top_left(c, a), top_left(a, b)];
        let (x, y) = (x0 * 16 + 8, y0 * 16 + 8);
        let mut row = [edge(b, c, x, y), edge(c, a, x, y), edge(a, b, x, y)];
        let dx = [
            (b.y - c.y) as i64 * 16,
            (c.y - a.y) as i64 * 16,
            (a.y - b.y) as i64 * 16,
        ];
        let dy = [
            (c.x - b.x) as i64 * 16,
            (a.x - c.x) as i64 * 16,
            (b.x - a.x) as i64 * 16,
        ];
        // Single reciprocal per triangle. Fall back to exact division for large clipped primitives.
        let inverse = (1i64 << 32) / area;
        let interpolate = |av: i32, bv: i32, cv: i32, wb: i64, wc: i64| {
            if area > (1i64 << 43) {
                return av
                    + ((wb as i128 * (bv - av) as i128 + wc as i128 * (cv - av) as i128)
                        / area as i128) as i32;
            }
            let delta = wb * (bv - av) as i64 + wc * (cv - av) as i64;
            av + if area < 1 << 20 {
                ((delta * inverse) >> 32) as i32
            } else {
                (delta / area) as i32
            }
        };
        let before_hits = count.covered;
        let before_writes = count.writes;
        for yy in y0..=y1 {
            let mut w = row;
            let start = (yy * self.width) as usize;
            for xx in x0..=x1 {
                if w[0] >= bias[0] && w[1] >= bias[1] && w[2] >= bias[2] {
                    count.covered += 1;
                    let k = start + xx as usize;
                    let depth = interpolate(a.d, b.d, c.d, w[1], w[2]);
                    if depth > self.depth[k] {
                        self.depth[k] = depth;
                        self.color[k] = mat.color(
                            interpolate(a.u, b.u, c.u, w[1], w[2]) >> 8,
                            interpolate(a.v, b.v, c.v, w[1], w[2]) >> 8,
                        );
                        count.writes += 1;
                    }
                }
                for j in 0..3 {
                    w[j] += dx[j];
                }
            }
            for j in 0..3 {
                row[j] += dy[j];
            }
        }
        count.sampled += u32::from(count.covered > before_hits);
        count.visible_writes += u32::from(count.writes > before_writes);
    }
}
fn clip_near(input: [CameraVertex; 3]) -> ([CameraVertex; 4], usize) {
    let mut out = [CameraVertex::default(); 4];
    let mut n = 0;
    let mut a = input[2];
    for b in input {
        if (a.z < NEAR && b.z > NEAR) || (a.z > NEAR && b.z < NEAR) {
            // Always interpolate from the same endpoint to keep shared clipped edges identical.
            let (p, q) = if a.z < b.z { (a, b) } else { (b, a) };
            let lerp = |v: i32, w: i32| {
                v + ((w - v) as i64 * (NEAR - p.z) as i64 / (q.z - p.z) as i64) as i32
            };
            out[n] = CameraVertex {
                x: lerp(p.x, q.x),
                y: lerp(p.y, q.y),
                z: NEAR,
                u: lerp(p.u, q.u),
                v: lerp(p.v, q.v),
            };
            n += 1;
        }
        if b.z >= NEAR {
            out[n] = b;
            n += 1;
        }
        a = b;
    }
    (out, n)
}
