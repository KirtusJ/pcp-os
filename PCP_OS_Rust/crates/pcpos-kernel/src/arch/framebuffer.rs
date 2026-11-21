//! Whether a framebuffer the loader describes may be mapped, and exactly which memory that is.
//!
//! The decision is a function of the description, the loader's memory map and the ranges the kernel
//! itself uses, and of nothing else, so that it can be tested on a host. It answers with the pages to
//! map and the bytes that may be written, or with the reason for refusing.
//!
//! The rules (directors' check-in, 21 November 2026; boundaries set by B3 from measured boots):
//! - 32 bits a pixel, a pitch that holds the width, a size that does not wrap, all of it at or above one
//!   mebibyte and below four gibibytes, which is as far as the kernel's tables reach.
//! - No page of it may touch a range the kernel uses: its image, the loader's files, the frame memory.
//! - No page of it may be memory the loader's map calls usable.
//! - Below the top of the machine's memory, every page of it must lie in memory the map says is not
//!   usable: a framebuffer in RAM is acceptable only because the firmware has set that RAM aside. A page
//!   there that no descriptor covers is refused.
//! - Above the top of the memory that lies below four gibibytes it is device memory, which a memory map
//!   does not list, and is accepted.
//! - With no map at all, it must lie above the memory the loader reported, or at or above one gibibyte if
//!   the loader reported none.
//!
//! A reserved descriptor does not by itself say "this is the framebuffer": loaders translate several
//! kinds of firmware memory to "reserved". The description comes from the loader; these rules only make
//! sure that believing it cannot hand the display anything the kernel or a program owns.

const PAGE: u64 = 4096;
const FOUR_GIB: u64 = 4 << 30;
const ONE_MIB: u64 = 1 << 20;
const ONE_GIB: u64 = 1 << 30;

/// What may be mapped and written: `pages` is [start, end) in whole pages, `bytes` the picture itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub pages: (usize, usize),
    pub bytes: (usize, usize),
}

/// One entry of a Multiboot memory map: [base, end) and whether the loader calls it usable.
fn entries(map: &[u8]) -> Result<impl Iterator<Item = (u64, u64, bool)> + '_, &'static str> {
    // Walk once to validate, so that the iterator below cannot run off the end.
    let mut rest = map;
    while !rest.is_empty() {
        if rest.len() < 24 {
            return Err("the loader's memory map ends inside an entry");
        }
        let size = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        if size < 20 || size + 4 > rest.len() {
            return Err("the loader's memory map has an entry of impossible size");
        }
        rest = &rest[size + 4..];
    }
    let mut rest = map;
    Ok(core::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let size = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let word = |at: usize| u64::from_le_bytes([rest[at], rest[at + 1], rest[at + 2], rest[at + 3], rest[at + 4], rest[at + 5], rest[at + 6], rest[at + 7]]);
        let (base, length) = (word(4), word(12));
        let kind = u32::from_le_bytes([rest[20], rest[21], rest[22], rest[23]]);
        rest = &rest[size + 4..];
        Some((base, base.saturating_add(length), kind == 1))
    }))
}

/// Decide. `memory_top` is the end of the memory the loader reported without a map, if it reported any;
/// `protected` are the [start, end) ranges the kernel uses.
pub fn decide(address: u64, pitch: u32, width: u32, height: u32, bits: u8, map: Option<&[u8]>, memory_top: Option<u64>,
              protected: &[(usize, usize)]) -> Result<Window, &'static str> {
    if bits != 32 {
        return Err("it is not 32 bits a pixel");
    }
    if width < 320 || height < 200 {
        return Err("it is smaller than 320 by 200");
    }
    if (pitch as u64) < width as u64 * 4 {
        return Err("its pitch is shorter than its width");
    }
    let length = pitch as u64 * height as u64;
    let end = match address.checked_add(length) {
        Some(end) => end,
        None => return Err("its extent wraps"),
    };
    if address % 4 != 0 {
        return Err("its address is not a multiple of four");
    }
    if address < ONE_MIB {
        return Err("it lies in the first mebibyte");
    }
    if end > FOUR_GIB {
        return Err("it reaches beyond four gibibytes");
    }
    let (first, last) = (address / PAGE * PAGE, end.div_ceil(PAGE) * PAGE);
    if protected.iter().any(|&(s, e)| (s as u64) < last && first < e as u64) {
        return Err("it overlaps memory the kernel uses");
    }
    match map {
        Some(map) => {
            let mut top = 0u64;
            for (base, stop, usable) in entries(map)? {
                if usable {
                    if base < last && first < stop {
                        return Err("the loader's map calls part of it usable memory");
                    }
                    // The top of memory that matters is the top below four gibibytes: a machine with more
                    // has usable memory above the gap in which its devices, the display among them, sit.
                    if base < FOUR_GIB {
                        top = top.max(stop.min(FOUR_GIB));
                    }
                }
            }
            if first < top {
                // In RAM: every page must be inside memory the map sets aside. Follow the descriptors
                // from the first page to the last; each step ends where some not-usable entry ends.
                let mut at = first;
                while at < last {
                    let mut reach = at;
                    for (base, stop, usable) in entries(map)? {
                        if !usable && base <= at && stop > at {
                            reach = reach.max(stop);
                        }
                    }
                    if reach == at {
                        return Err("it lies below the top of memory and the loader's map does not set all of it aside");
                    }
                    at = reach;
                }
            }
        }
        None => {
            let floor = memory_top.unwrap_or(ONE_GIB);
            if first < floor {
                return Err("there is no memory map and it lies inside the memory the loader reported");
            }
        }
    }
    Ok(Window { pages: (first as usize, last as usize), bytes: (address as usize, end as usize) })
}
