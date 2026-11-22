#![cfg_attr(target_os = "none", no_std)]

#[cfg(any(target_os = "none", test))]
mod cmdline;
pub mod elf;
pub mod memory;
pub mod payload;

#[cfg(target_os = "none")]
mod firmware;
pub type Failure = &'static str;
pub type Result<T> = core::result::Result<T, Failure>;
pub const NAMES: [&str; 4] = ["KERNEL.ELF", "TAILOR.ELF", "WORKER.ELF", "model.manifest"];
pub const FIXTURE: &[u8] = b"THIS KERNEL IS BUILT WITH model-memory-fixture";
