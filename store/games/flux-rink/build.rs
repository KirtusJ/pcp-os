use std::{env, f64::consts::TAU, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (name, segments, profile) in [
        (
            "mallet",
            28,
            &[
                (0., 0.),
                (128., 0.),
                (147., 12.),
                (147., 32.),
                (129., 49.),
                (68., 53.),
                (52., 72.),
                (49., 147.),
                (36., 166.),
                (0., 169.),
            ][..],
        ),
        (
            "disc",
            24,
            &[
                (0., 0.),
                (61., 0.),
                (73., 9.),
                (73., 25.),
                (60., 33.),
                (0., 34.),
            ][..],
        ),
    ] {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for (i, &(radius, height)) in profile.iter().enumerate() {
            let end = (i + 1).min(profile.len() - 1);
            let start = i.saturating_sub(1);
            let dr: f64 = profile[end].0 - profile[start].0;
            let dh: f64 = profile[end].1 - profile[start].1;
            let len = dr.hypot(dh);
            for j in 0..segments {
                let a = j as f64 * TAU / segments as f64;
                let (s, c) = a.sin_cos();
                for p in [radius * c, height, radius * s] {
                    vertices.extend_from_slice(&(p.round() as i16).to_le_bytes());
                }
                for n in [dh * c / len, -dr / len, dh * s / len] {
                    vertices.push((n * 126.).round() as i8 as u8);
                }
                if i + 1 < profile.len() {
                    let q = i * segments + j;
                    let b = i * segments + (j + 1) % segments;
                    for ix in [q, q + segments, b + segments, q, b + segments, b] {
                        indices.extend_from_slice(&(ix as u16).to_le_bytes());
                    }
                }
            }
        }
        fs::write(out.join(format!("{name}.v")), vertices).unwrap();
        fs::write(out.join(format!("{name}.i")), indices).unwrap();
    }
}
