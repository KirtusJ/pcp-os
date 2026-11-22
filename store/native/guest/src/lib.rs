#![no_std]
// The calls into PCP OS, the entry and the panic handler are built for the bare target, and, with
// the feature `hard-float-target`, for `x86_64-unknown-linux-gnu` used as a freestanding target
// whose `core` and `alloc` use the processor's floating point (see examples/hard-float). On any
// other target only the parts that need no operating system are built, for tests on the host.
#[cfg(any(target_os = "none", feature = "hard-float-target"))]
mod abi;
pub mod ink;
#[cfg(any(target_os = "none", feature = "hard-float-target"))]
pub use abi::*;
#[cfg(any(target_os = "none", feature = "hard-float-target"))]
mod device;
#[cfg(any(target_os = "none", feature = "hard-float-target"))]
pub use device::*;
#[macro_export]
macro_rules! entry {
    ($function:ident) => {
        core::arch::global_asm!(
            ".global _start",
            ".section .text.start",
            "_start:",
            "and rsp, -16",
            concat!("call ", stringify!($function)),
            "ud2"
        );
    };
}
pub fn digits(mut n: u32, buf: &mut [u8; 10]) -> &str {
    let mut i = 10;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    core::str::from_utf8(&buf[i..]).unwrap()
}
pub fn rgb(a: u32, b: u32, t: i32, d: i32) -> u32 {
    let channel = |s: u32| {
        let x = ((a >> s) & 255) as i32;
        let y = ((b >> s) & 255) as i32;
        (x + (y - x) * t / d) as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}
