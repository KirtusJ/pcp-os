use crate::{H, W};
use flux_rink::{Match, Settings};
use heliograph::{
    assets::{sin, MATERIALS},
    raster::{Cached, Counts, Instance, Mesh, Target},
};
use pcp_game_guest::ink::Frame;
pub(super) const TILT: i32 = 122;
macro_rules! mesh {
    ($s:literal) => {
        Mesh {
            vertices: include_bytes!(concat!(env!("OUT_DIR"), "/", $s, ".v")),
            indices: include_bytes!(concat!(env!("OUT_DIR"), "/", $s, ".i")),
            radius: 240,
        }
    };
}
const MALLET: Mesh = mesh!("mallet");
const DISC: Mesh = mesh!("disc");
pub(super) fn cam(x: i32, z: i32) -> [i32; 3] {
    [
        z,
        (x * sin(TILT) >> 14) + 155,
        7200 + (x * sin(TILT + 256) >> 14),
    ]
}
pub(super) fn focal(s: &Settings) -> i32 {
    if s.wide {
        748
    } else {
        863
    }
}
pub(super) fn project(x: i32, z: i32, f: i32) -> (i32, i32) {
    let p = cam(x, z);
    (W / 2 + p[0] * f / p[2], H / 2 - p[1] * f / p[2])
}
pub use crate::pavilion::bake as background;
pub fn actors(px: &mut [u32], depth: &mut [i32], cache: &mut [Cached], m: &Match, s: &Settings) {
    let f = focal(s);
    let mut frame = Frame {
        px,
        pitch: W as usize,
    };
    for (body, r, c) in [
        (m.player[0], 16, 0xffa465),
        (m.player[1], 16, 0x61efde),
        (m.puck, 9, 0xb9e8ef),
    ] {
        let (x, y) = project(body.x, body.z, f);
        let radius = r * 7200 / cam(body.x, body.z)[2];
        crate::pavilion::under(&mut frame, x, y, radius, c, s.detail);
    }
    if s.detail {
        for (i, b) in m.player.iter().enumerate() {
            let (x, y) = project(b.x, b.z, f);
            let team = if i == 0 { 0xf6ba66 } else { 0x73eadb };
            for dy in -5i32..=5 {
                for dx in -23i32..=23 {
                    let d = dx * dx * 25 + dy * dy * 529;
                    if (10450..=13225).contains(&d) && dx.abs() > 6 {
                        frame.rect(x + dx, y + dy, 1, 1, team);
                    }
                }
            }
        }
    }
    let mut t = Target::new(px, depth, W, H, f);
    let mut count = Counts::default();
    for (i, b) in [m.player[0], m.player[1], m.puck].iter().enumerate() {
        let at = Instance {
            pos: cam(b.x, b.z),
            yaw: 0,
            tilt: -TILT,
            scale: 1024,
        };
        t.draw(
            if i == 2 { &DISC } else { &MALLET },
            at,
            &MATERIALS[i],
            cache,
            &mut count,
        );
    }
}
