//! Host-side pixel tests use the same drawing routines as the native ELF.
#![allow(dead_code)]
#[allow(unused_attributes)]
#[path = "../src/island.rs"]
mod game;
use game::{v, H, V, W};
#[path = "../src/ink.rs"]
mod ink;
mod raster {
    include!("../src/raster.rs");
    pub fn above(eye: V) -> Camera {
        Camera {
            eye,
            ..fixture(64, 0)
        }
    }
    pub fn fixture(tilt: i32, yaw: i32) -> Camera {
        Camera {
            eye: super::v(0, 0, 0),
            yaw,
            tilt,
            center: (256, 144),
            focal: 269,
        }
    }
}
mod scene {
    include!("../src/scene.rs");
    pub fn backdrop(f: &mut Frame, c: Camera, t: u32) {
        sky(f, c, t)
    }
    pub fn terrain(p: &mut Painter) {
        turf(p)
    }
    pub fn underside(p: &mut Painter) {
        prism(p, v(0, 80, 250), 0, 96, 64, 96, 0xff4400)
    }
}
use raster::{Camera, Painter};
const N: usize = (W * H) as usize;
fn with_frame(cam: Camera, draw: impl FnOnce(&mut Painter)) -> Vec<u32> {
    let mut pixels = vec![0; N];
    let mut depth = vec![0; N];
    draw(&mut Painter {
        camera: cam,
        depth: &mut depth,
        f: &mut ink::Frame {
            px: &mut pixels,
            pitch: W as usize,
        },
    });
    pixels
}
fn quad(y: i32) -> [V; 4] {
    [
        v(-500, y, 171),
        v(500, y, 171),
        v(500, y, 1900),
        v(-500, y, 1900),
    ]
}
#[test]
fn nearer_surface_wins_at_every_interior_sample() {
    let mut total = 0;
    let mut wrong = 0;
    for tilt in [0, 7, 11, 17] {
        for offset in [0, 3, 9] {
            let cam = raster::fixture(tilt, 0);
            let front = quad(-130 + offset);
            let far = quad(-131 + offset);
            let image = with_frame(cam, |p| {
                p.poly(&far, 1);
                p.poly(&front, 2)
            });
            // Ray/plane intersection at pixel centres; exclude a four-unit margin at the footprint.
            let (s, c) = (game::sin(tilt) as f64 / 256., game::cos(tilt) as f64 / 256.);
            for j in 0..H {
                for i in 0..W {
                    let ry = (144. - (j as f64 + 0.5)) / 269.;
                    let world_y = ry * c - s;
                    if world_y >= 0. {
                        continue;
                    }
                    let k = (-130 + offset) as f64 / world_y;
                    let z = k * (ry * s + c);
                    let x = k * (i as f64 + 0.5 - 256.) / 269. * (s * s + c * c);
                    if x.abs() < 496. && (175.0..1896.0).contains(&z) {
                        total += 1;
                        if image[(j * W + i) as usize] != 2 {
                            wrong += 1
                        }
                    }
                }
            }
        }
    }
    println!("parallel surfaces: {wrong} incorrect / {total} interior samples");
    assert_eq!(wrong, 0)
}
#[test]
fn crossing_near_plane_is_watertight_from_both_directions() {
    let mut diff = 0;
    for shift in -10..=10 {
        let cam = raster::fixture(0, 0);
        let p = [
            v(-70 + shift, -71, 34),
            v(64 + shift, -43, 89),
            v(64 + shift, 73, 89),
            v(-70 + shift, 45, 34),
        ];
        let a = with_frame(cam, |r| r.poly(&p, 1));
        let b = with_frame(cam, |r| {
            r.poly(&[p[3], p[2], p[1]], 1);
            r.poly(&[p[3], p[1], p[0]], 1)
        });
        diff += a.iter().zip(b).filter(|(a, b)| **a != *b).count();
    }
    println!("near-plane split and reversal: {diff} differing coverage samples over 21 offsets");
    assert_eq!(diff, 0)
}
#[test]
fn solid_prism_has_an_underside() {
    let img = with_frame(raster::fixture(0, 0), scene::underside);
    // Eye below the box. A ray through its bottom must hit the shaded lower face before the back wall.
    let pixel = |x, y| img[(y * W + x) as usize];
    let bottom = pixel(256, 66);
    println!("bottom view: visible material {bottom:06x}");
    assert_eq!(
        bottom, 0x902600,
        "upward ray must see the shadowed bottom material"
    )
}
#[test]
fn backdrop_repeats_after_a_full_yaw() {
    let render = |yaw| {
        let mut px = vec![0; N];
        scene::backdrop(
            &mut ink::Frame {
                px: &mut px,
                pitch: W as usize,
            },
            raster::fixture(11, yaw),
            123,
        );
        px
    };
    let before = render(-1);
    let after = render(255);
    let d = before.iter().zip(after).filter(|(a, b)| **a != *b).count();
    println!("equivalent sky headings: {d} differing pixels");
    assert_eq!(d, 0)
}
#[test]
fn hidden_geometry_cannot_overwrite_a_nearer_face() {
    let cam = raster::fixture(0, 0);
    let face = |d| [v(-36, -28, d), v(36, -28, d), v(36, 28, d), v(-36, 28, d)];
    let render = |order: [i32; 2]| {
        with_frame(cam, |p| {
            for d in order {
                p.poly(&face(d), d as u32)
            }
        })
    };
    let a = render([300, 150]);
    assert_eq!(a[(144 * W + 256) as usize], 150);
    assert!(a == render([150, 300]));
    let behind = with_frame(cam, |p| p.poly(&face(20), 1));
    assert!(behind.iter().all(|&x| x == 0))
}

#[test]
fn terrace_never_shows_buried_tan_cap() {
    let mut g = game::Game::new();
    g.step(game::TOUR);
    let mut peak = 0;
    let mut contaminated = 0;
    let mut views = 0;
    let mut px = vec![0; N];
    let mut depth = vec![0; N];
    for _ in 0..600 {
        g.step(0);
        if g.elapsed % 12 == 0 && g.mode == game::Mode::Run {
            let mut frame = ink::Frame {
                px: &mut px,
                pitch: W as usize,
            };
            scene::draw(&mut frame, &g, &mut depth);
            let n = px.iter().filter(|&&x| x == 0xcfb187).count();
            views += 1;
            contaminated += n;
            peak = peak.max(n);
        }
        if g.mode == game::Mode::Delivered {
            break;
        }
    }
    println!("terrace cap: {contaminated} tan pixels in {views} route views; peak {peak}");
    assert_eq!(contaminated, 0)
}
#[test]
fn orbit_wrap_scrolls_the_existing_backdrop() {
    let render = |yaw| {
        let mut px = vec![0; N];
        scene::backdrop(
            &mut ink::Frame {
                px: &mut px,
                pitch: W as usize,
            },
            raster::fixture(11, yaw),
            123,
        );
        px
    };
    let a = render(255);
    let b = render(0);
    let mut wrong = 0;
    for y in 0..H {
        for x in 0..W - 5 {
            if a[(y * W + x + 5) as usize] != b[(y * W + x) as usize] {
                wrong += 1
            }
        }
    }
    println!("north heading crossing: {wrong} shifted-pixel mismatches");
    assert_eq!(wrong, 0)
}
#[test]
fn terrace_grass_remains_covered() {
    // The full image lies inside the terrace footprint with a downward-looking camera
    // 192 units above its centre. Includes the corners and both triangles' shared edge.
    let image = with_frame(
        raster::above(v(-14, game::TERRACE.top + 192, 604)),
        scene::terrain,
    );
    let grass = image.iter().filter(|&&c| c == 0xa6b67b).count();
    println!("downward terrace: {grass} grass pixels / {N} samples");
    assert_eq!(grass, N);
}
#[test]
fn orbit_is_safe_and_repeatable_at_island_obstacles() {
    let mut g = game::Game::new();
    g.mode = game::Mode::Run;
    let mut px = vec![0; N];
    let mut depth = vec![0; N];
    let mut visited = 0;
    for pos in [v(195, 292, 89), v(-200, 178, 650), v(3, 64, -1450)] {
        g.pos = pos;
        g.camera = pos;
        for yaw in (0..256).step_by(4) {
            g.yaw = yaw;
            scene::draw(
                &mut ink::Frame {
                    px: &mut px,
                    pitch: W as usize,
                },
                &g,
                &mut depth,
            );
            assert!(depth.iter().all(|&z| z >= 0));
            assert!(depth.iter().any(|&z| z > 0));
            if yaw % 32 == 0 {
                let expected = px.clone();
                px.fill(0xdeadbeef);
                depth.fill(-99);
                scene::draw(
                    &mut ink::Frame {
                        px: &mut px,
                        pitch: W as usize,
                    },
                    &g,
                    &mut depth,
                );
                assert!(px == expected);
            }
            visited += 1;
        }
    }
    println!(
        "orbit: {visited} views, {} repeated with poisoned buffers",
        visited / 8
    );
}

// An independent half-space oracle. At Z=538, focal length 269 maps two world
// units to one pixel exactly, including half-integer vertices and edge ties.
#[test]
fn shared_diagonal_matches_half_space_oracle() {
    let corners = [
        v(-129, 79, 538),
        v(221, 49, 538),
        v(175, -143, 538),
        v(-141, -103, 538),
    ];
    let cam = raster::fixture(0, 0);
    let draw = |reverse| {
        with_frame(cam, |r| {
            let triangles = [
                ([corners[0], corners[1], corners[2]], 1),
                ([corners[0], corners[2], corners[3]], 2),
            ];
            for i in if reverse { [1, 0] } else { [0, 1] } {
                r.poly(&triangles[i].0, triangles[i].1);
            }
        })
    };
    let a = draw(false);
    let b = draw(true);
    let xy = corners.map(|p| (512 + p.x, 288 - p.y));
    let inside = |verts: [usize; 3], x: i32, y: i32| {
        (0..3).all(|i| {
            let (ax, ay) = xy[verts[i]];
            let (bx, by) = xy[verts[(i + 1) % 3]];
            let dx = bx - ax;
            let dy = by - ay;
            let e = dx * (y - ay) - dy * (x - ax);
            e > 0 || e == 0 && (dy < 0 || dy == 0 && dx > 0)
        })
    };
    let mut diff = 0;
    let mut order_diff = 0;
    for y in 0..H {
        for x in 0..W {
            let (sx, sy) = (2 * x + 1, 2 * y + 1);
            let expected = if inside([0, 1, 2], sx, sy) {
                1
            } else if inside([0, 2, 3], sx, sy) {
                2
            } else {
                0
            };
            let idx = (y * W + x) as usize;
            diff += usize::from(a[idx] != expected);
            order_diff += usize::from(a[idx] != b[idx]);
        }
    }
    println!("half-pixel quadrilateral: {diff} oracle mismatches / {N}; {order_diff} after reversing draw order");
    assert_eq!((diff, order_diff), (0, 0));
}
