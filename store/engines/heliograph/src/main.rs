#![no_std]
#![no_main]
use heliograph::{
    assets::LODS,
    lab::{self, CACHE_LEN, H, W},
    raster::{Cached, Counts, Target},
};
mod ink;
include!("abi.inc.rs");
const SIZE: usize = (W * H) as usize;
static mut PIXELS: [u32; SIZE] = [0; SIZE];
static mut PLATE: [u32; SIZE] = [0; SIZE];
static mut DEPTH: [i32; SIZE] = [0; SIZE];
static mut CACHE: [Cached; CACHE_LEN] = [Cached::ZERO; CACHE_LEN];
fn us() -> u64 {
    let mut t = [0i64; 2];
    call(228, 1, t.as_mut_ptr() as usize, 0, 0, 0, 0);
    (t[0] * 1_000_000 + t[1] / 1000) as u64
}
fn number(n: u64) -> ([u8; 20], usize) {
    let mut n = n;
    let mut b = [0; 20];
    let mut i = b.len();
    loop {
        i -= 1;
        b[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            return (b, i);
        }
    }
}
fn metric(k: &[u8], n: u64) {
    log(k);
    let (b, i) = number(n);
    log(&b[i..]);
}
fn label(f: &mut ink::Frame, x: i32, y: i32, n: u32, color: u32) {
    let (b, i) = number(n as u64);
    f.text(x, y, core::str::from_utf8(&b[i..]).unwrap(), 1, color);
}
fn overlay(pixels: &mut [u32], tier: usize, c: &Counts, material: usize) {
    let f = &mut ink::Frame {
        px: pixels,
        pitch: W as usize,
    };
    let (light, muted, accent) = (0xe8e5d9, 0xabc2c3, 0xe9a050);
    f.rect(21, 18, 4, 13, accent);
    f.text(32, 21, "HELIOGRAPH", 2, light);
    if tier == 0 || tier == 4 {
        f.text(23, 61, "MATERIAL STUDY", 1, muted);
        f.text(23, 77, ["GOLD", "JADE", "TITANIUM"][material], 3, light);
        f.text(23, 105, "TORUS KNOT", 1, muted);
    }
    f.rect(0, 325, W, 35, 0x10181c);
    f.rect(21, 325, 34, 2, accent);
    f.text(22, 334, "TRIANGLES", 1, muted);
    label(f, 84, 334, c.submitted, light);
    f.text(161, 334, "HIT A PIXEL", 1, muted);
    label(f, 237, 334, c.sampled, light);
    f.text(22, 347, "1-6  SCENES", 1, muted);
    f.text(113, 347, "M  MATERIAL", 1, muted);
    f.text(207, 347, "SPACE  HOLD", 1, muted);
    f.text(303, 347, "ARROWS  ROTATE", 1, muted);
    f.text(425, 347, "B  BENCHMARK", 1, muted);
    f.text(546, 347, "ESC  EXIT", 1, muted);
    f.text(425, 334, "R  RESOLUTION TEST", 1, muted);
}
include!("mode_probe.rs");
const SAMPLES: usize = 40;
#[no_mangle]
pub extern "C" fn lab_entry() -> ! {
    let fb = open(b"/dev/fb0\0", 2);
    let kb = open(b"/dev/kbd\0", 0x800);
    let mut d = [W as u32, H as u32, 0, 0, 0, 0];
    if kb < 0
        || fb < 0
        || ioctl(fb, 0x5002, d.as_mut_ptr() as usize) != 0
        || d[0] != W as u32
        || d[1] != H as u32
        || d[3] == 0
        || d[2] < W as u32 * 4
    {
        log(b"HELIOGRAPH display error\n");
        exit(2);
    }
    let pitch = d[2] as usize / 4;
    let page = pitch * H as usize;
    let addr = call(9, 0, page * d[3] as usize * 4, 3, 1, fb as usize, 0);
    if addr < 0 {
        log(b"HELIOGRAPH mapping error\n");
        exit(2);
    }
    if !LODS.iter().all(|m| m.validate()) {
        log(b"HELIOGRAPH invalid mesh\n");
        exit(3);
    }
    let (p, back, z, cache) = unsafe {
        (
            &mut *(&raw mut PIXELS),
            &mut *(&raw mut PLATE),
            &mut *(&raw mut DEPTH),
            &mut *(&raw mut CACHE),
        )
    };
    lab::backdrop(back);
    let (mut tier, mut angle, mut material) = (0usize, 426i32, 0usize);
    let mut samples = [(Counts::default(), 0u64, 0u64); SAMPLES + 2];
    let mut held = [false; 128];
    let mut paused = false;
    let mut bench = false;
    let mut frame = 0;
    let mut phase = 0;
    log(b"HELIOGRAPH ready\n");
    loop {
        loop {
            let mut e = [0u8; 2];
            if call(0, kb as usize, e.as_mut_ptr() as usize, 2, 0, 0, 0) != 2 {
                break;
            }
            let key = (e[0] & 127) as usize;
            let down = e[1] & 1 == 0;
            let press = down && !held[key];
            held[key] = down;
            if press {
                match key {
                    1 => {
                        log(b"HELIOGRAPH exit\n");
                        exit(0);
                    }
                    19 => {
                        resolution_probe(fb, cache);
                        d[4] = 0;
                    }
                    48 => {
                        bench = true;
                        frame = 0;
                        phase = 0;
                        log(b"HELIOGRAPH benchmark start\n");
                    }
                    2..=7 => {
                        tier = key - 2;
                        bench = false;
                        metric(b"HELIOGRAPH mode=", tier as u64);
                        log(b"\n");
                    }
                    57 => {
                        paused = !paused;
                        log(b"HELIOGRAPH hold toggle\n");
                    }
                    50 => {
                        material = (material + 1) % 3;
                        log(b"HELIOGRAPH material\n");
                    }
                    _ => {}
                }
            }
        }
        if bench {
            tier = [0, 4, 1, 2, 5, 3][phase];
            angle = 426 + (frame as i32) * 7;
            material = 0;
        }
        let start = us();
        p.copy_from_slice(back);
        z.fill(0);
        let counts = lab::render(
            &mut Target::new(p, z, W, H, 444),
            cache,
            tier,
            angle,
            material,
        );
        overlay(p, tier, &counts, material);
        let rendered = us();
        let dest = if d[3] > 1 { 1 - d[4].min(1) } else { 0 };
        let out = unsafe {
            slice::from_raw_parts_mut((addr as *mut u32).add(page * dest as usize), page)
        };
        for (a, b) in p.chunks_exact(W as usize).zip(out.chunks_exact_mut(pitch)) {
            b[..W as usize].copy_from_slice(a);
        }
        if d[3] > 1 {
            if ioctl(fb, 0x5003, dest as usize) != 0 {
                log(b"HELIOGRAPH flip error\n");
                exit(2);
            }
            d[4] = dest;
        }
        let end = us();
        if bench {
            // Leave two warm-up frames in the raw log; common poses are measured for every density.
            log(b"F\n");
            samples[frame] = (counts, rendered - start, end - rendered);
            if frame + 1 == samples.len() {
                for (frame, (counts, r, p)) in samples.iter().copied().enumerate() {
                    metric(b"HFRAME tier=", tier as u64);
                    metric(b" frame=", frame as u64);
                    metric(b" source=", counts.source as u64);
                    metric(b" tris=", counts.submitted as u64);
                    metric(b" culled=", counts.culled as u64);
                    metric(b" transformed=", counts.vertices as u64);
                    metric(b" raster=", counts.rasterized as u64);
                    metric(b" sampled=", counts.sampled as u64);
                    metric(b" zpass_tris=", counts.visible_writes as u64);
                    metric(b" zpass_px=", counts.writes as u64);
                    metric(b" render_us=", r);
                    metric(b" present_us=", p);
                    metric(b" total_us=", r + p);
                    log(b"\n");
                }
            }
            frame += 1;
            if frame == SAMPLES + 2 {
                frame = 0;
                phase += 1;
                if phase == 6 {
                    log(b"HELIOGRAPH benchmark done\n");
                    bench = false;
                    tier = 0;
                    angle = 426;
                }
            }
        } else {
            if held[75] {
                angle -= 4;
            } else if held[77] {
                angle += 4;
            } else if !paused {
                angle += 2;
            }
            angle &= 1023;
            let remaining = 25_000u64.saturating_sub(end - start);
            if remaining > 0 {
                let t = [0i64, (remaining * 1000) as i64];
                call(35, t.as_ptr() as usize, 0, 0, 0, 0, 0);
            }
        }
    }
}
