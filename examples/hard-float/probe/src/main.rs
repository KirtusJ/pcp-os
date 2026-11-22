#![no_std]
#![no_main]
//! Test programs for `examples/hard-float/check.py`. One source, built several ways:
//!
//! * for the bare target (software floating point) or, with the feature `hard`, for the Linux
//!   target used freestanding (the processor's floating point);
//! * `BACK=1`: the program that runs beside the other and sums a different series until it is ended;
//! * `BACK_SERIES=1`: the front program summing the back program's series, to say what the back
//!   program's sums should be;
//! * `CLOBBER=1`: the red-zone test with a call in its loop, which must be caught (the control).
//!
//! Every build prints the bits of its sums, so two builds, or one program and what ran beside it,
//! can be compared exactly.
extern crate alloc;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use pcp_game_guest::{exit, log};

const BACK: bool = option_env!("BACK").is_some();
const CLOBBER: bool = option_env!("CLOBBER").is_some();
/// Sum the back program's series, but as the front program does: once through and end.
const BACK_SERIES: bool = option_env!("BACK_SERIES").is_some();

/// A heap that only grows, in the program's own memory: enough for the vectors below.
struct Heap {
    room: UnsafeCell<[u8; 1 << 20]>,
    used: AtomicUsize,
}
// Safety: `used` hands every byte out once; nothing is given back.
unsafe impl Sync for Heap {}
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = self.room.get() as usize;
        let mut at = self.used.load(Ordering::Relaxed);
        loop {
            let start = (base + at + layout.align() - 1) & !(layout.align() - 1);
            let end = start - base + layout.size();
            if end > 1 << 20 {
                return core::ptr::null_mut();
            }
            match self.used.compare_exchange(at, end, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return start as *mut u8,
                Err(now) => at = now,
            }
        }
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
#[global_allocator]
static HEAP: Heap = Heap { room: UnsafeCell::new([0; 1 << 20]), used: AtomicUsize::new(0) };

fn hex(mut v: u64, out: &mut [u8; 17]) {
    for i in (0..16).rev() {
        out[i] = b"0123456789abcdef"[(v & 15) as usize];
        v >>= 4;
    }
    out[16] = b'\n';
}

fn say(who: &[u8], what: &[u8], value: u64) {
    let mut line = [0u8; 17];
    hex(value, &mut line);
    log(who);
    log(what);
    log(&line);
}

#[inline(never)]
fn dot32(n: u32) -> f32 {
    let (mut s, mut x) = (0.0f32, 0.5f32);
    for i in 0..n {
        let y = (i as f32) * 0.001 + 1.0;
        s += x * y;
        x = x * 0.999 + 0.0001;
    }
    s
}

#[inline(never)]
fn dot64(n: u32) -> f64 {
    let (mut s, mut x) = (0.0f64, 0.5f64);
    for i in 0..n {
        let y = (i as f64) * 0.001 + 1.0;
        s += x * y;
        x = x * 0.999 + 0.0001;
    }
    s
}

/// The same kind of sum over numbers kept on the heap.
#[inline(never)]
fn heap_sum(n: usize) -> f32 {
    let a: Vec<f32> = (0..n).map(|i| (i as f32) * 0.25 + 1.0).collect();
    let b: Vec<f32> = (0..n).map(|i| 1.0 / ((i as f32) + 2.0)).collect();
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// The red zone: the 128 bytes below the stack pointer, which code for the Linux target may use
/// without moving the pointer. Fill four places in it, spend `rounds` turns of a loop that touches
/// no stack, and count the turns in which a place no longer held what was put there. If the kernel
/// took an interrupt on this program's stack it would write there. With `CLOBBER` the loop makes a
/// call, which writes a return address at the first place: the count must then not be zero.
#[inline(never)]
fn red_zone(rounds: u64) -> u64 {
    let wrong: u64;
    // Safety: writes only within the 128 bytes below the stack pointer, which belong to this
    // function on this target, and makes no call unless CLOBBER asks for the control.
    unsafe {
        asm!(
            "mov qword ptr [rsp - 8], {p}",
            "mov qword ptr [rsp - 64], {p}",
            "mov qword ptr [rsp - 120], {p}",
            "mov qword ptr [rsp - 128], {p}",
            "2:",
            "test {c}, {c}",
            "jz 3f",
            "call 5f",
            "3:",
            "cmp qword ptr [rsp - 8], {p}",
            "jne 4f",
            "cmp qword ptr [rsp - 64], {p}",
            "jne 4f",
            "cmp qword ptr [rsp - 120], {p}",
            "jne 4f",
            "cmp qword ptr [rsp - 128], {p}",
            "je 6f",
            "4:",
            "inc {w}",
            "mov qword ptr [rsp - 8], {p}",
            "6:",
            "dec {n}",
            "jnz 2b",
            "jmp 7f",
            "5:",
            "ret",
            "7:",
            p = in(reg) 0x5A5A_A5A5_C3C3_3C3Cu64,
            c = in(reg) CLOBBER as u64,
            n = inout(reg) rounds => _,
            w = inout(reg) 0u64 => wrong,
        );
    }
    wrong
}

pcp_game_guest::entry!(probe_main);
#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    let who: &[u8] = if BACK { b"back" } else { b"front" };
    // The two builds sum different series, so each has its own right answer.
    let n = core::hint::black_box(if BACK || BACK_SERIES { 3_000_000u32 } else { 5_000_000u32 });
    let (first32, first64) = (dot32(n).to_bits(), dot64(n).to_bits());
    say(who, b": f32 ", first32 as u64);
    say(who, b": f64 ", first64);
    // Both programs use the heap. (A program with nothing writable is linked with an empty
    // segment at address zero, which the kernel refuses.)
    say(who, b": heap ", heap_sum(core::hint::black_box(100_000)).to_bits() as u64);
    if !BACK {
        say(who, b": red zone wrong ", red_zone(core::hint::black_box(400_000_000)));
    }
    // The front program repeats its sums 40 times and ends; the back one until the kernel ends it.
    let (mut wrong, mut round) = (0u64, 0u64);
    while BACK || round < 40 {
        // The count is hidden from the compiler each round, so each round does the sums again.
        let again = core::hint::black_box(n);
        if dot32(again).to_bits() != first32 || dot64(again).to_bits() != first64 {
            wrong += 1;
        }
        round += 1;
        if BACK && round % 8 == 0 {
            say(who, b": rounds so far, and of them wrong ", (round << 32) | wrong.min(0xffff_ffff));
        }
    }
    say(who, b": rounds wrong ", wrong);
    log(who);
    log(b": done\n");
    exit(0)
}
