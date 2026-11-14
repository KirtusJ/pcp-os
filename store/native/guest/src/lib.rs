#![no_std]
#[cfg(target_os = "none")]
mod abi;
pub mod ink;
#[cfg(target_os = "none")]
pub use abi::*;
#[cfg(target_os = "none")]
mod device;
#[cfg(target_os = "none")]
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
