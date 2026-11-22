//! Whether a framebuffer the loader describes may be mapped, and exactly which memory that is.
//!
//! The decision is a function of the description, the loader's memory map and the ranges the kernel
//! itself uses, and of nothing else, so that it can be tested on a host. It answers with the pages to
//! map and the bytes that may be written, or with the reason for refusing.
//!
//! The rules (directors' check-in, 21 November 2026; boundaries set by B3 from measured boots):
//! - 32 bits a pixel, a pitch that holds the width and is a multiple of four, a size that does not wrap, all of it at or above one
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
    // Every row is written as 32-bit words, so every row must begin on one.
    if pitch % 4 != 0 {
        return Err("its pitch is not a multiple of four");
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

/// Whether a program that asks is to draw to memory of its own, which the kernel then shows whole.
/// Every display with one page: there is nowhere else to draw but the picture on the screen, and a
/// program drawing there is seen clearing and repainting. Whether the display's memory could be
/// mapped whole into the program does not enter into it. It once did: only a one-page display that
/// could NOT be mapped whole (800 by 600, ending partway through a page) was drawn off the screen,
/// and one of 1920 by 1080, exactly 2,025 pages, was handed to the program as it stood (the owner's
/// HP EliteBook 840 G7). `mappable_whole` is taken so that the tests can say it makes no difference.
pub fn draws_off_screen(pages: u32, _mappable_whole: bool) -> bool {
    pages == 1
}

/// The pixels of one row that must be written to the display for it to show `new` where it shows
/// `kept`: the first pixel that differs through the last that differs, as (first, one past the last).
/// `None` when the rows are the same and nothing need be written. Both are a row's bytes, four to a
/// pixel. Rows of different lengths, or of a length that is not whole pixels, are not compared: the
/// whole of `new` is to be written, which is always right.
pub fn changed_span(kept: &[u8], new: &[u8]) -> Option<(usize, usize)> {
    if kept.len() != new.len() || new.len() % 4 != 0 {
        return if new.is_empty() { None } else { Some((0, new.len() / 4)) };
    }
    let first = kept.iter().zip(new.iter()).position(|(a, b)| a != b)?;
    let last = kept.iter().zip(new.iter()).rposition(|(a, b)| a != b)?;
    Some((first / 4, last / 4 + 1))
}

/// How many 4 KiB pages hold a picture of `width` by `height` pixels, four bytes each, its rows one
/// straight after another. `None` if that number cannot be had.
pub fn kept_pages(width: u32, height: u32) -> Option<usize> {
    (width as usize).checked_mul(height as usize)?.checked_mul(4).map(|bytes| (bytes + PAGE as usize - 1) / PAGE as usize)
}

/// The parts of `len` bytes beginning at byte `offset` of a picture kept in pages, a page at a time:
/// (which page, where in it, how many bytes, where in the `len` bytes). A row is seldom a whole number
/// of pages, so it falls across two or three.
pub fn kept_parts(offset: usize, len: usize) -> impl Iterator<Item = (usize, usize, usize, usize)> {
    let page = PAGE as usize;
    let mut done = 0;
    core::iter::from_fn(move || {
        if done >= len {
            return None;
        }
        let at = offset + done;
        let n = (page - at % page).min(len - done);
        let part = (at / page, at % page, n, done);
        done += n;
        Some(part)
    })
}

/// Whether a program may be given `pages` whole pages of a device beginning at `phys`, when `bytes` is
/// the [start, end) the kernel may write there. Every byte of every page must be inside it: a picture
/// that ends partway through a page does not make the rest of that page the program's.
pub fn grant(bytes: (usize, usize), phys: usize, pages: usize) -> bool {
    let end = match pages.checked_mul(PAGE as usize).and_then(|n| phys.checked_add(n)) {
        Some(end) => end,
        None => return false,
    };
    pages != 0 && phys % PAGE as usize == 0 && phys >= bytes.0 && end <= bytes.1
}
