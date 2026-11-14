#![no_std]
//! Fixed-point, indexed software rendering for a caller-owned RGB target.
pub mod assets;
pub mod lab;
pub mod raster;

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod checks;
