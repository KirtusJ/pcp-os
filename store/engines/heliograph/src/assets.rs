use crate::raster::{Material, Mesh};
macro_rules! mesh {
    ($lod:literal) => {
        Mesh {
            vertices: include_bytes!(concat!(env!("OUT_DIR"), "/", $lod, ".v")),
            indices: include_bytes!(concat!(env!("OUT_DIR"), "/", $lod, ".i")),
            radius: 1940,
        }
    };
}
pub const LODS: [Mesh<'static>; 3] = [mesh!("low"), mesh!("mid"), mesh!("high")];
macro_rules! mat {
    ($id:literal) => {
        Material::new(include_bytes!(concat!(env!("OUT_DIR"), "/mat", $id)))
    };
}
pub const MATERIALS: [Material<'static>; 3] = [mat!("0"), mat!("1"), mat!("2")];
pub static SINE: [i32; 1024] = include!(concat!(env!("OUT_DIR"), "/sine.rs"));
pub fn sin(x: i32) -> i32 {
    SINE[(x & 1023) as usize]
}

pub const SAMPLED_ERRORS: [i32; 3] = include!(concat!(env!("OUT_DIR"), "/errors.rs"));
