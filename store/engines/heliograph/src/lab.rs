//! Scene client. The renderer itself has no camera path, built-in mesh, or OS calls in its draw API.
use crate::{
    assets::{sin, LODS, MATERIALS, SAMPLED_ERRORS},
    raster::{Cached, Counts, Instance, Target},
};
pub const W: i32 = 640;
pub const H: i32 = 360;
pub const CACHE_LEN: usize = 384 * 64;
pub fn backdrop(px: &mut [u32]) {
    for y in 0..H {
        for x in 0..W {
            let a = (154000 - ((x - 290) * (x - 290) + (y - 112) * (y - 112) * 2)).max(0);
            let t = a / 4400;
            let horizon = 244;
            let grain = (((x as u32).wrapping_mul(1973) ^ (y as u32).wrapping_mul(9277))
                .wrapping_mul(26699)
                >> 17)
                & 3;
            let (r, g, b) = if y < horizon {
                (12 + t / 2, 18 + t / 2, 23 + t / 2)
            } else {
                let yy = y - horizon;
                let glow = ((135 - yy).max(0) * t) / 245;
                (
                    24 + glow + yy / 18,
                    31 + glow + yy / 20,
                    34 + glow + yy / 24,
                )
            };
            px[(y * W + x) as usize] =
                ((r as u32 + grain) << 16) | ((g as u32 + grain) << 8) | (b as u32 + grain);
        }
    }
}
fn contact(px: &mut [u32], x: i32, y: i32, rx: i32, ry: i32) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let xx = x + dx;
            let yy = y + dy;
            if xx < 0 || xx >= W || yy < 0 || yy >= H {
                continue;
            }
            let t = 256 - dx * dx * 256 / (rx * rx) - dy * dy * 256 / (ry * ry);
            if t > 0 {
                let k = (yy * W + xx) as usize;
                let c = px[k];
                let f = (256 - t * t / 400) as u32;
                px[k] = ((((c >> 16) & 255) * f >> 8) << 16)
                    | ((((c >> 8) & 255) * f >> 8) << 8)
                    | ((c & 255) * f >> 8);
            }
        }
    }
}
/// tier 0 uses one display piece. The three stress tiers repeat the same 24 poses at different index densities.
pub fn render(
    target: &mut Target,
    cache: &mut [Cached],
    tier: usize,
    angle: i32,
    material: usize,
) -> Counts {
    let mut counts = Counts::default();
    if tier == 0 || tier == 4 {
        contact(target.color, W / 2 + 40, 296, 135, 12);
        target.draw(
            &LODS[if tier == 4 { 2 } else { 1 }],
            Instance {
                pos: [485, 160, 5500],
                yaw: angle,
                tilt: 76,
                scale: 1030,
            },
            &MATERIALS[material],
            cache,
            &mut counts,
        );
    } else {
        for y in 0..4 {
            for x in 0..6 {
                let id = y * 6 + x;
                let yaw = angle + id * 17;
                let z = 4950 + x * 410 + y * 820 + sin(angle + id * 63) / 128;
                let pos = [
                    (x * 2 - 5) * 52 * z / 444,
                    ((3 - 2 * y) * 35 + 5) * z / 444,
                    z,
                ];
                let instance = Instance {
                    pos,
                    yaw,
                    tilt: 60 + id * 5,
                    scale: 365,
                };
                let lod = if tier == 5 {
                    target.select_lod(&SAMPLED_ERRORS, instance, LODS[2].radius, 660)
                } else {
                    tier - 1
                };
                counts.source += (LODS[2].triangles() - LODS[lod].triangles()) as u32;
                target.draw(
                    &LODS[lod],
                    instance,
                    &MATERIALS[(id as usize + material) % 3],
                    cache,
                    &mut counts,
                );
            }
        }
    }
    counts
}
