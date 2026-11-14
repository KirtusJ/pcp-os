use crate::{
    assets::{LODS, MATERIALS},
    lab,
    raster::{Cached, Counts, Instance, Mesh, Target},
};
use std::{vec, vec::Vec};
fn point(x: i16, y: i16, z: i16, buf: &mut Vec<u8>) {
    for v in [x, y, z] {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf.extend_from_slice(&[0, 0, 130]);
}
struct Owned {
    v: Vec<u8>,
    i: Vec<u8>,
}
impl Owned {
    fn new(p: &[[i16; 3]], indices: &[u16]) -> Self {
        let mut v = Vec::new();
        for &[x, y, z] in p {
            point(x, y, z, &mut v);
        }
        Self {
            v,
            i: indices.iter().flat_map(|i| i.to_le_bytes()).collect(),
        }
    }
    fn mesh(&self) -> Mesh<'_> {
        Mesh {
            vertices: &self.v,
            indices: &self.i,
            radius: 1400,
        }
    }
}
fn instance() -> Instance {
    Instance {
        pos: [0, 0, 0],
        yaw: 0,
        tilt: 0,
        scale: 1024,
    }
}
#[test]
fn checked_assets() {
    assert!(LODS.iter().all(Mesh::validate));
    assert_eq!(LODS[2].triangles(), 49152);
    let bad = Owned::new(&[[0, 0, 100]], &[0, 1, 0]);
    assert!(!bad.mesh().validate());
}
#[test]
fn shared_edge_owns_every_pixel_once() {
    let m = Owned::new(
        &[
            [-128, 128, 512],
            [128, 128, 512],
            [128, -128, 512],
            [-128, -128, 512],
        ],
        &[0, 1, 2, 0, 2, 3],
    );
    let (mut color, mut z) = ([0; 64 * 64], [0; 64 * 64]);
    let mut counts = Counts::default();
    Target::new(&mut color, &mut z, 64, 64, 64).draw(
        &m.mesh(),
        instance(),
        &MATERIALS[0],
        &mut [Cached::ZERO; 4],
        &mut counts,
    );
    assert_eq!((counts.covered, counts.writes), (1024, 1024));
    assert_eq!(counts.sampled, 2);
}
#[test]
fn nearer_object_wins_in_both_orders() {
    let m = Owned::new(
        &[[-128, 128, 512], [128, 128, 512], [0, -128, 512]],
        &[0, 1, 2],
    );
    let run = |orders: [usize; 2]| {
        let (mut c, mut z) = ([0; 4096], [0; 4096]);
        let mut counts = Counts::default();
        for i in orders {
            let mut at = instance();
            at.pos[2] = 120 * i as i32;
            Target::new(&mut c, &mut z, 64, 64, 64).draw(
                &m.mesh(),
                at,
                &MATERIALS[i],
                &mut [Cached::ZERO; 3],
                &mut counts,
            );
        }
        c
    };
    assert_eq!(run([0, 1]), run([1, 0]));
    assert_ne!(run([0, 1])[32 * 64 + 32], 0);
}
#[test]
fn moving_through_near_plane() {
    for (zs, visible) in [
        ([191, 191, 191], false),
        ([192, 192, 192], true),
        ([160, 210, 220], true),
        ([240, 160, 160], true),
        ([192, 160, 160], false),
    ] {
        let m = Owned::new(
            &[[-60, 55, zs[0]], [60, 55, zs[1]], [0, -55, zs[2]]],
            &[0, 1, 2],
        );
        let (mut c, mut z) = ([0; 4096], [0; 4096]);
        let mut count = Counts::default();
        Target::new(&mut c, &mut z, 64, 64, 64).draw(
            &m.mesh(),
            instance(),
            &MATERIALS[0],
            &mut [Cached::ZERO; 3],
            &mut count,
        );
        assert_eq!(count.writes > 0, visible, "{zs:?}");
    }
}
#[test]
fn offscreen_instances_skip_vertex_work() {
    let m = Owned::new(&[[-1, 0, 800], [4, 6, 800], [5, 0, 800]], &[0, 1, 2]);
    let (mut c, mut z) = ([0; 4096], [0; 4096]);
    let mut count = Counts::default();
    let mut at = instance();
    at.pos[0] = 6000;
    Target::new(&mut c, &mut z, 64, 64, 64).draw(
        &m.mesh(),
        at,
        &MATERIALS[0],
        &mut [Cached::ZERO; 3],
        &mut count,
    );
    assert_eq!((count.culled, count.vertices, count.writes), (1, 0, 0));
}
#[test]
fn compare_adaptive_and_reference() {
    let mut cache = vec![Cached::ZERO; lab::CACHE_LEN];
    let mut abs_sum = 0i64;
    let mut flips = 0i64;
    let mut union = 0i64;
    for angle in [426, 468, 510, 552, 594, 636] {
        let (mut a, mut b) = (vec![0; 640 * 360], vec![0; 640 * 360]);
        let mut z = vec![0; 640 * 360];
        let reference = lab::render(
            &mut Target::new(&mut a, &mut z, 640, 360, 444),
            &mut cache,
            3,
            angle,
            0,
        );
        z.fill(0);
        let adaptive = lab::render(
            &mut Target::new(&mut b, &mut z, 640, 360, 444),
            &mut cache,
            5,
            angle,
            0,
        );
        assert_eq!(reference.submitted, adaptive.source);
        assert!(adaptive.submitted < reference.submitted / 6);
        for (p, q) in a.into_iter().zip(b) {
            if p | q != 0 {
                union += 1;
                flips += i64::from(p == 0 || q == 0);
                for shift in [0, 8, 16] {
                    abs_sum += (((p >> shift) & 255) as i64 - ((q >> shift) & 255) as i64).abs();
                }
            }
        }
    }
    std::println!("six gallery poses covered_union={union} outline_disagreements={flips} rgb_abs_sum={abs_sum}");
    assert!(flips * 100 < union * 10);
    assert!(abs_sum < union * 3 * 35);
}
