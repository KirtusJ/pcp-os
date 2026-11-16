//! Authoring stage. Bake an indexed demonstration asset and studio material captures.
use std::{env, f64::consts::TAU, fs, path::PathBuf};
type V = [f64; 3];
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V, b: V) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn norm(a: V) -> V {
    let l = dot(a, a).sqrt();
    a.map(|x| x / l)
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn path(t: f64) -> V {
    let r = 1160.0 + 410.0 * (3.0 * t).cos();
    [
        r * (2.0 * t).cos(),
        r * (2.0 * t).sin(),
        680.0 * (3.0 * t).sin(),
    ]
}
fn mesh(rows: usize, cols: usize) -> (Vec<u8>, Vec<u8>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    for r in 0..rows {
        let t = TAU * r as f64 / rows as f64;
        let p = path(t);
        let along = norm(sub(path(t + 0.0001), path(t - 0.0001)));
        let a = norm(cross(along, [0.0, 0.0, 1.0]));
        let b = cross(along, a);
        for c in 0..cols {
            let phi = TAU * c as f64 / cols as f64;
            let n: V = std::array::from_fn(|k| a[k] * phi.cos() + b[k] * phi.sin());
            for k in 0..3 {
                v.extend_from_slice(&((p[k] + 325.0 * n[k]).round() as i16).to_le_bytes());
            }
            for x in n {
                v.push((x * 126.0).round() as i8 as u8);
            }
            let cur = r * cols + c;
            let next = r * cols + (c + 1) % cols;
            let down = ((r + 1) % rows) * cols + c;
            let diag = ((r + 1) % rows) * cols + (c + 1) % cols;
            for j in [cur, next, diag, cur, diag, down] {
                i.extend_from_slice(&(j as u16).to_le_bytes());
            }
        }
    }
    (v, i)
}
fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn softbox(r: V, dir: V, width: f64, height: f64, blur: f64) -> f64 {
    let d = norm(dir);
    let side = norm(cross([0.0, 1.0, 0.0], d));
    let up = cross(d, side);
    let f = dot(r, d);
    if f <= 0.0 {
        return 0.0;
    }
    (1.0 - smooth(width - blur, width + blur, (dot(r, side) / f).abs()))
        * (1.0 - smooth(height - blur, height + blur, (dot(r, up) / f).abs()))
}
fn material(kind: usize) -> Vec<u8> {
    let (albedo, metal, rough) = match kind {
        0 => ([0.92, 0.46, 0.12], 0.86, 0.065),
        1 => ([0.07, 0.42, 0.34], 0.7, 0.10),
        _ => ([0.60, 0.68, 0.78], 0.78, 0.085),
    };
    let mut out = Vec::new();
    for y in 0..128 {
        for x in 0..128 {
            let nx = x as f64 / 63.5 - 1.0;
            let ny = y as f64 / 63.5 - 1.0;
            let n = norm([nx, ny, (1.0 - nx * nx - ny * ny).max(0.0).sqrt()]);
            let r = [
                2.0 * n[2] * n[0],
                2.0 * n[2] * n[1],
                2.0 * n[2] * n[2] - 1.0,
            ];
            let key = softbox(r, [-0.58, 0.85, 0.24], 0.54, 0.28, rough) * 3.5;
            let strip = softbox(r, [0.9, 0.33, -0.25], 0.09, 0.84, rough * 0.70) * 2.6;
            let rim = softbox(r, [-0.75, 0.2, -0.8], 0.16, 0.52, rough) * 1.7;
            let sky = smooth(-0.15, 0.25, r[1]);
            let bounce = 1.0 - smooth(-0.82, -0.19, r[1]);
            let diffuse = 0.18 + 0.85 * dot(n, norm([-0.5, 0.85, 0.5])).max(0.0);
            let fres = (1.0 - n[2]).powi(5);
            let mut pixel = 0u32;
            for k in 0..3 {
                let env = [0.04, 0.058, 0.09][k]
                    + sky * [0.41, 0.53, 0.68][k]
                    + bounce * [0.38, 0.26, 0.18][k]
                    + key * [1.0, 0.93, 0.78][k]
                    + strip * [0.65, 0.86, 1.0][k]
                    + rim;
                let f0 = 0.04 * (1.0 - metal) + albedo[k] * metal;
                let light = albedo[k] * (1.0 - metal) * diffuse + env * (f0 + (1.0 - f0) * fres);
                let mapped = (light * (2.51 * light + 0.03)
                    / (light * (2.43 * light + 0.59) + 0.14))
                    .clamp(0.0, 1.0);
                let srgb = if mapped <= 0.0031308 {
                    12.92 * mapped
                } else {
                    1.055 * mapped.powf(1.0 / 2.4) - 0.055
                };
                pixel |= ((255.0 * srgb).round() as u32) << ((2 - k) * 8);
            }
            out.extend_from_slice(&pixel.to_le_bytes());
        }
    }
    out
}

// Compare every highest-detail vertex to the corresponding point on each coarser triangle.
fn sampled_error(low: &[u8], rows: usize, cols: usize, reference: &[u8]) -> i32 {
    let point = |bytes: &[u8], i: usize, k: usize| {
        i16::from_le_bytes(bytes[i * 9 + 2 * k..i * 9 + 2 * k + 2].try_into().unwrap()) as f64
    };
    let mut max_sq = 0.0f64;
    for r in 0..384 {
        for c in 0..64 {
            let u = r as f64 * rows as f64 / 384.0;
            let v = c as f64 * cols as f64 / 64.0;
            let (fr, fc) = (u.fract(), v.fract());
            let at = |dr, dc| ((u as usize + dr) % rows) * cols + (v as usize + dc) % cols;
            let mut err = 0.0;
            for k in 0..3 {
                let a = point(low, at(0, 0), k);
                let d = point(low, at(1, 1), k);
                let p = if fc >= fr {
                    a * (1.0 - fc) + point(low, at(0, 1), k) * (fc - fr) + d * fr
                } else {
                    a * (1.0 - fr) + point(low, at(1, 0), k) * (fr - fc) + d * fc
                };
                err += (p - point(reference, r * 64 + c, k)).powi(2);
            }
            max_sq = max_sq.max(err);
        }
    }
    max_sq.sqrt().ceil() as i32
}
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let root = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let reference = mesh(384, 64).0;
    let mut errors = Vec::new();
    for (name, r, c) in [("low", 48, 12), ("mid", 128, 24), ("high", 384, 64)] {
        let (v, i) = mesh(r, c);
        errors.push(sampled_error(&v, r, c, &reference));
        fs::write(root.join(format!("{name}.v")), v).unwrap();
        fs::write(root.join(format!("{name}.i")), i).unwrap();
    }
    fs::write(root.join("errors.rs"), format!("{errors:?}")).unwrap();
    for k in 0..3 {
        fs::write(root.join(format!("mat{k}")), material(k)).unwrap();
    }
    let mut sine = String::from("[");
    for a in 0..1024 {
        sine += &format!(
            "{},",
            (16384.0 * (a as f64 * TAU / 1024.0).sin()).round() as i32
        );
    }
    sine.push(']');
    fs::write(root.join("sine.rs"), sine).unwrap();
}
