//! Freestanding two-argument job for the opt-in isolated PCP guest fixture.
#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
// Linux initial stack: argc, argv[0], argv[1]. Pass the original stack to Rust.
global_asm!(
    ".global _start",
    "_start:",
    "mov rdi, rsp",
    "call worker_main",
    "ud2"
);

#[no_mangle]
extern "C" fn worker_main(stack: *const usize) -> ! {
    // The trusted kernel's ELF loader constructs the argument pointers and NUL terminator.
    let n = unsafe {
        if *stack != 2 {
            exit(255);
        }
        let s = *stack.add(2) as *const u8;
        let (a, b) = (*s, *s.add(1));
        if !a.is_ascii_digit() || !b.is_ascii_digit() || *s.add(2) != 0 {
            exit(255);
        }
        (a - b'0') * 10 + b - b'0'
    };
    if !(12..=36).contains(&n) {
        exit(255);
    }
    let (mut a, mut b) = (0u16, 1u16);
    for _ in 0..n {
        (a, b) = (b, (a + b) % 251);
    }
    exit(a as u64)
}

fn exit(code: u64) -> ! {
    // Linux ABI exit(2). No user grant is required to terminate this process.
    unsafe {
        asm!("syscall", in("rax") 60u64, in("rdi") code, options(noreturn));
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    exit(255)
}
