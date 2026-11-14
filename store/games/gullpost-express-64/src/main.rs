#![no_std]
#![no_main]
mod raster;
mod scene;
use gull64_island as game;
mod ink;

use core::{
    arch::{asm, global_asm},
    slice,
};
use game::*;

global_asm!(
    ".global _start",
    ".section .text.start",
    "_start:",
    "and rsp, -16",
    "call gull_entry",
    "ud2"
);
fn call(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("syscall",inlateout("rax") n=>r,in("rdi") a,in("rsi") b,in("rdx") c,in("r10") d,in("r8") e,in("r9") f,lateout("rcx") _,lateout("r11") _,options(nostack));
    }
    r
}
fn open(path: &[u8], flags: usize) -> isize {
    call(2, path.as_ptr() as usize, flags, 0, 0, 0, 0)
}
fn ioctl(fd: isize, req: usize, arg: usize) -> isize {
    call(16, fd as usize, req, arg, 0, 0, 0)
}
fn log(s: &[u8]) {
    call(1, 1, s.as_ptr() as usize, s.len(), 0, 0, 0);
}
fn exit(n: usize) -> ! {
    call(60, n, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop()
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    log(b"G64: game panic\n");
    exit(3)
}
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}
#[no_mangle]
unsafe extern "C" fn memset(dst: *mut u8, b: i32, n: usize) -> *mut u8 {
    for i in 0..n {
        dst.add(i).write_volatile(b as u8)
    }
    dst
}
#[no_mangle]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        dst.add(i).write_volatile(src.add(i).read())
    }
    dst
}
fn ms() -> i64 {
    let mut time = [0i64; 2];
    call(228, 1, time.as_mut_ptr() as usize, 0, 0, 0, 0);
    time[0] * 1000 + time[1] / 1_000_000
}
fn sleep(n: i64) {
    if n > 0 {
        let time = [0i64, n.min(50) * 1_000_000];
        call(35, time.as_ptr() as usize, 0, 0, 0, 0, 0);
    }
}
fn bind(code: usize) -> u16 {
    match code {
        75 | 30 => LEFT,
        72 | 17 => FORWARD,
        80 | 31 => BACK,
        16 => ORBIT_L,
        18 => ORBIT_R,
        60 => TOUR,
        77 | 32 => RIGHT,
        57 | 28 => JUMP,
        25 => PAUSE,
        19 => RESET,
        _ => 0,
    }
}
struct Speaker {
    fd: isize,
    sfx: Sfx,
    at: usize,
    phase: u32,
}
impl Speaker {
    fn pump(&mut self, event: Sfx) {
        if self.fd < 0 {
            return;
        }
        if event != Sfx::Quiet {
            self.sfx = event;
            self.at = 0;
            self.phase = 0
        }
        let length = match self.sfx {
            Sfx::Quiet => 0,
            Sfx::Deliver => 17500,
            Sfx::Flag => 11000,
            Sfx::Splash => 7200,
            _ => 5600,
        };
        if self.at >= length {
            return;
        }
        let mut info = [0u32; 5];
        if ioctl(self.fd, 0x5011, info.as_mut_ptr() as usize) != 0 || info[..3] != [48000, 2, 16] {
            return;
        }
        let frames = (info[3] as usize / 4).min(1200).min(length - self.at);
        let mut pcm = [0i16; 2400];
        let mut phase = self.phase;
        let mut accepted_phases = [0u32; 1200];
        for i in 0..frames {
            let t = self.at + i;
            let f = match self.sfx {
                Sfx::Hop => 330 + t / 15,
                Sfx::Wing => 630 - t / 20,

                Sfx::Stamp => {
                    if t < 2200 {
                        880
                    } else {
                        1318
                    }
                }
                Sfx::Splash => 240 - t / 37,
                Sfx::Flag => [523, 659, 784][(t / 3700).min(2)],
                Sfx::Deliver => [523, 659, 784, 1047][(t / 4400).min(3)],
                _ => 0,
            };
            phase = (phase + f as u32) % 48000;
            let tri = (phase.min(48000 - phase) as i32 - 12000) / 2;
            let amp = (t.min(150) * 100 / 150)
                .min((length - t) * 100 / (length / 2))
                .min(100) as i32;
            let v = (tri * amp / 100) as i16;
            pcm[i * 2] = v;
            pcm[i * 2 + 1] = v;
            accepted_phases[i] = phase;
        }
        if frames > 0 {
            let sent = call(
                1,
                self.fd as usize,
                pcm.as_ptr() as usize,
                frames * 4,
                0,
                0,
                0,
            );
            if sent >= 4 {
                let n = (sent as usize / 4).min(frames);
                self.at += n;
                self.phase = accepted_phases[n - 1]
            }
        }
    }
}

include!("runtime.rs");
