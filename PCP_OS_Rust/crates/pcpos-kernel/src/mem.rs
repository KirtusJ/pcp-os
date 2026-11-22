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

/// One range of memory above four gibibytes. Only a program's own pages are taken from it: page tables,
/// and frames a device is told the address of, stay below one gibibyte.
struct High {
    start: usize,
    used: Vec<u64>,
    frames: usize,
    free: usize,
    hint: usize,
}

static HIGH: arch::Single<Vec<High>> = arch::Single::new(Vec::new());
/// Frames of high memory ever handed out, and given back.
static HIGH_COUNTS: arch::Single<(u64, u64)> = arch::Single::new((0, 0));

/// Prepare the pool over the memory `arch::protect` set aside. Returns the number of frames.
pub fn init() -> usize {
    let (start, end) = arch::pool();
    let frames = (end - start) / 4096;
    arch::without_interrupts(|| POOL.with(|p| *p = Some(Pool { start, used: alloc::vec![0; (frames + 63) / 64], frames, free: frames, hint: 0, taken: 0, given: 0 })));
    let (ranges, count) = arch::high_pools();
    let high: Vec<High> = ranges[..count].iter().map(|&(start, end)| {
        let frames = (end - start) / 4096;
        High { start, used: alloc::vec![0; (frames + 63) / 64], frames, free: frames, hint: 0 }
    }).collect();
    arch::without_interrupts(|| HIGH.with(|h| *h = high));
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

/// One frame for a program's own memory, zeroed: from the memory above four gibibytes where there is
/// any free, otherwise as `take`. Nothing that must lie low is ever taken through here.
pub fn take_data() -> Option<usize> {
    let high = arch::without_interrupts(|| {
        HIGH.with(|ranges| {
            for r in ranges.iter_mut().filter(|r| r.free > 0) {
                // Whole words that are full are passed over, so a large range is searched quickly.
                let words = r.used.len();
                for k in 0..words {
                    let w = (r.hint / 64 + k) % words;
                    if r.used[w] == u64::MAX {
                        continue;
                    }
                    let bit = (!r.used[w]).trailing_zeros() as usize;
                    let i = w * 64 + bit;
                    if i >= r.frames {
                        continue;
                    }
                    r.used[w] |= 1 << bit;
                    r.free -= 1;
                    r.hint = i;
                    return Some(r.start + i * 4096);
                }
            }
            None
        })
    });
    match high {
        Some(frame) => {
            arch::without_interrupts(|| HIGH_COUNTS.with(|c| c.0 += 1));
            if arch::frame_zero(frame) { Some(frame) } else { None }
        }
        None => take(),
    }
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
    let high = arch::without_interrupts(|| {
        HIGH.with(|ranges| {
            let r = ranges.iter_mut().find(|r| frame >= r.start && (frame - r.start) / 4096 < r.frames)?;
            let i = (frame - r.start) / 4096;
            if frame % 4096 != 0 || r.used[i / 64] >> (i % 64) & 1 == 0 {
                return Some(false);
            }
            r.used[i / 64] &= !(1 << (i % 64));
            r.free += 1;
            Some(true)
        })
    });
    if let Some(given) = high {
        if given {
            arch::without_interrupts(|| HIGH_COUNTS.with(|c| c.1 += 1));
        }
        return given;
    }
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

/// Only with the `high-memory-demo` feature, at start-up: take what a device would be given (a run of
/// frames, and one single frame) and what a program would be given, say where each lies, write to the
/// program's frame and read it back, and give everything back. With memory above four gibibytes present
/// the first two must lie below one gibibyte and the third above four.
#[cfg(feature = "high-memory-demo")]
pub fn high_memory_demo() {
    let (run, single, data) = (take_run(4), take(), take_data());
    let low = |f: Option<usize>| f.is_some_and(|f| f < 1 << 30);
    let written = data.is_some_and(|f| arch::frame_write(f, 0, &[0xA5; 4096]));
    let returned = [run, single, data].iter().all(|f| f.is_some_and(give)) && (1..4).all(|n| run.is_some_and(|f| give(f + n * 4096)));
    // Taken again, the program's frame comes back zeroed whatever was written to it.
    let again = take_data();
    let zeroed = again.is_some_and(|f| arch::frame_is_zero(f)) && again.is_some_and(give);
    crate::log(format_args!("high memory demo: run below one gibibyte {}, single frame below one gibibyte {}, program frame above four gibibytes {}, written {}, all given back {}, zeroed when taken again {}\n",
        low(run) as u8, low(single) as u8, data.is_some_and(|f| f >= 4 << 30) as u8, written as u8, returned as u8, zeroed as u8));
}

/// The same four figures for the memory above four gibibytes, all its ranges together.
pub fn census_high() -> (usize, usize, u64, u64) {
    arch::without_interrupts(|| {
        let (frames, free) = HIGH.with(|ranges| ranges.iter().fold((0, 0), |(f, e), r| (f + r.frames, e + r.free)));
        let (taken, given) = HIGH_COUNTS.with(|c| *c);
        (frames, free, taken, given)
    })
}

/// (frames in the pool, frames free now, frames ever handed out, frames ever given back).
pub fn census() -> (usize, usize, u64, u64) {
    arch::without_interrupts(|| POOL.with(|p| p.as_ref().map(|p| (p.frames, p.free, p.taken, p.given)).unwrap_or((0, 0, 0, 0))))
}
