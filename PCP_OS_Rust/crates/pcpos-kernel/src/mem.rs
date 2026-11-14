//! The frame pool: which 4 KiB frames of the machine's memory are in use. A frame is handed out zeroed, to
//! one holder, until that holder gives it back. This file has no `unsafe`: it decides which frame, and
//! `arch` does the zeroing.
use crate::arch;
use alloc::vec::Vec;

struct Pool {
    start: usize,
    /// One bit for each frame: set while it is handed out.
    used: Vec<u64>,
    frames: usize,
    free: usize,
    /// Where the last search ended: the next one starts there.
    hint: usize,
    /// How many frames were ever handed out, and given back.
    taken: u64,
    given: u64,
}

static POOL: arch::Single<Option<Pool>> = arch::Single::new(None);

/// Prepare the pool over the memory `arch::protect` set aside. Returns the number of frames.
pub fn init() -> usize {
    let (start, end) = arch::pool();
    let frames = (end - start) / 4096;
    arch::without_interrupts(|| POOL.with(|p| *p = Some(Pool { start, used: alloc::vec![0; (frames + 63) / 64], frames, free: frames, hint: 0, taken: 0, given: 0 })));
    frames
}

/// One frame, zeroed, or `None` if none is left.
pub fn take() -> Option<usize> {
    arch::without_interrupts(|| {
        POOL.with(|p| {
            let p = p.as_mut()?;
            if p.free == 0 {
                return None;
            }
            for k in 0..p.frames {
                let i = (p.hint + k) % p.frames;
                if p.used[i / 64] >> (i % 64) & 1 == 0 {
                    p.used[i / 64] |= 1 << (i % 64);
                    p.free -= 1;
                    p.taken += 1;
                    p.hint = (i + 1) % p.frames;
                    let frame = p.start + i * 4096;
                    return if arch::frame_zero(frame) { Some(frame) } else { None };
                }
            }
            None
        })
    })
}

/// `count` frames that lie one after another in memory, zeroed: for a device that is told one address and a length.
pub fn take_run(count: usize) -> Option<usize> {
    arch::without_interrupts(|| {
        POOL.with(|p| {
            let p = p.as_mut()?;
            if count == 0 || count > p.free {
                return None;
            }
            let mut run = 0;
            for i in 0..p.frames {
                run = if p.used[i / 64] >> (i % 64) & 1 == 0 { run + 1 } else { 0 };
                if run == count {
                    let first = i + 1 - count;
                    for j in first..=i {
                        p.used[j / 64] |= 1 << (j % 64);
                        arch::frame_zero(p.start + j * 4096);
                    }
                    p.free -= count;
                    p.taken += count as u64;
                    return Some(p.start + first * 4096);
                }
            }
            None
        })
    })
}

/// Give a frame back. False, and nothing changes, if it is not a frame of the pool or is not handed out:
/// a frame cannot be given back twice.
pub fn give(frame: usize) -> bool {
    arch::without_interrupts(|| {
        POOL.with(|p| match p.as_mut() {
            Some(p) if frame >= p.start && frame % 4096 == 0 && (frame - p.start) / 4096 < p.frames => {
                let i = (frame - p.start) / 4096;
                if p.used[i / 64] >> (i % 64) & 1 == 0 {
                    return false;
                }
                p.used[i / 64] &= !(1 << (i % 64));
                p.free += 1;
                p.given += 1;
                true
            }
            _ => false,
        })
    })
}

/// (frames in the pool, frames free now, frames ever handed out, frames ever given back).
pub fn census() -> (usize, usize, u64, u64) {
    arch::without_interrupts(|| POOL.with(|p| p.as_ref().map(|p| (p.frames, p.free, p.taken, p.given)).unwrap_or((0, 0, 0, 0))))
}
