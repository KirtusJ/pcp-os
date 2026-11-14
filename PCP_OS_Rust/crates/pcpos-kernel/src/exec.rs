//! Loading a program that was not built into the kernel: a statically linked ELF file for x86_64, as a C
//! compiler and linker produce it. The file is data the loader handed over; nothing in it is trusted. Each
//! field is checked before it is used, a segment that asks to be both writable and executable is refused,
//! and nothing is mapped outside the part of the address space that belongs to programs.
//!
//! This file has no `unsafe`.
use crate::arch::{self, Space};
use crate::mem;
use alloc::vec::Vec;

/// Where a position-independent program is placed.
pub const LOAD_BASE: usize = arch::USER_BASE + 0x40_0000;
/// The top of a loaded program's stack, and its size in pages. The page below the stack is never mapped.
pub const STACK_TOP: usize = arch::USER_BASE + 0x7F_FFFF_F000;
pub const STACK_PAGES: usize = 256;

/// What the loader found and did.
#[derive(Clone, Copy, Debug)]
pub struct Loaded {
    pub entry: usize,
    /// The stack pointer the program starts with: it points at its argument count.
    pub stack: usize,
    /// The first address after the program's image, where its heap may begin.
    pub image_end: usize,
    /// Pages mapped for the image and for the stack.
    pub pages: usize,
}

fn u16_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?) as u64)
}
fn u32_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as u64)
}
fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// True if the bytes begin as an ELF file does. Used to tell a program from any other file.
pub fn looks_like_a_program(image: &[u8]) -> bool {
    image.len() >= 64 && image[..4] == [0x7f, b'E', b'L', b'F']
}

/// Map a program's image and a stack into an empty address space, and write its arguments on the stack.
pub fn load(space: &mut Space, image: &[u8], args: &[&str], seed: [u8; 16]) -> Result<Loaded, &'static str> {
    if !looks_like_a_program(image) {
        return Err("not an ELF file");
    }
    if image[4] != 2 || image[5] != 1 || u16_at(image, 18) != Some(62) {
        return Err("not a 64-bit little-endian program for x86_64");
    }
    let kind = u16_at(image, 16).ok_or("truncated header")?;
    let base = match kind {
        2 => 0,
        3 => LOAD_BASE,
        _ => return Err("neither an executable nor a position-independent executable"),
    };
    let entry = u64_at(image, 24).ok_or("truncated header")? as usize;
    let phoff = u64_at(image, 32).ok_or("truncated header")? as usize;
    let (phentsize, phnum) = (u16_at(image, 54).ok_or("truncated header")? as usize, u16_at(image, 56).ok_or("truncated header")? as usize);
    if phentsize != 56 || phnum == 0 || phnum > 64 || phoff.checked_add(phnum * 56).map(|e| e > image.len()).unwrap_or(true) {
        return Err("program headers are missing or lie outside the file");
    }
    // Pages already mapped for this image, with the frame behind each and what the program may do with it.
    let mut mapped: Vec<(usize, usize, bool, bool)> = Vec::new();
    let mut image_end = 0usize;
    let mut interpreter = false;
    for n in 0..phnum {
        let h = phoff + n * 56;
        let (ptype, flags) = (u32_at(image, h).ok_or("truncated")?, u32_at(image, h + 4).ok_or("truncated")?);
        if ptype == 3 {
            interpreter = true;
        }
        if ptype != 1 {
            continue;
        }
        let offset = u64_at(image, h + 8).ok_or("truncated")? as usize;
        let vaddr = (u64_at(image, h + 16).ok_or("truncated")? as usize).checked_add(base).ok_or("a segment address overflows")?;
        let (filesz, memsz) = (u64_at(image, h + 32).ok_or("truncated")? as usize, u64_at(image, h + 40).ok_or("truncated")? as usize);
        let (write, exec) = (flags & 2 != 0, flags & 1 != 0);
        if write && exec {
            return Err("a segment asks to be both writable and executable");
        }
        if filesz > memsz || memsz > 1 << 30 || offset.checked_add(filesz).map(|e| e > image.len()).unwrap_or(true) {
            return Err("a segment lies outside the file or is larger than a gibibyte");
        }
        let end = vaddr.checked_add(memsz).ok_or("a segment address overflows")?;
        if vaddr < arch::USER_BASE || end > STACK_TOP - (STACK_PAGES + 16) * 4096 {
            return Err("a segment lies outside the memory a program may have");
        }
        image_end = image_end.max(end);
        let mut page = vaddr & !0xfff;
        while page < end {
            let frame = match mapped.iter().find(|m| m.0 == page) {
                Some(&(_, frame, w, x)) => {
                    if w != write || x != exec {
                        return Err("two segments with different permissions share a page");
                    }
                    frame
                }
                None => {
                    let frame = mem::take().ok_or("out of memory")?;
                    if !space.map(page, frame, write, exec) {
                        mem::give(frame);
                        return Err("a page of the program could not be mapped");
                    }
                    mapped.push((page, frame, write, exec));
                    frame
                }
            };
            // The part of the file that belongs on this page.
            let from = page.max(vaddr);
            let to = (page + 4096).min(vaddr + filesz);
            if to > from {
                let src = offset + (from - vaddr);
                if !arch::frame_write(frame, from - page, &image[src..src + (to - from)]) {
                    return Err("a page of the program could not be written");
                }
            }
            page += 4096;
        }
    }
    if interpreter {
        return Err("the program asks for a dynamic loader, and there is none: it must be statically linked");
    }
    if mapped.is_empty() {
        return Err("the program has nothing to load");
    }
    let entry = entry.checked_add(base).ok_or("the entry address overflows")?;
    if !mapped.iter().any(|m| m.0 == entry & !0xfff && m.3) {
        return Err("the entry address is not in executable memory of the program");
    }
    let image_pages = mapped.len();
    // The stack.
    for n in 0..STACK_PAGES {
        let frame = mem::take().ok_or("out of memory")?;
        if !space.map(STACK_TOP - (n + 1) * 4096, frame, true, false) {
            mem::give(frame);
            return Err("the stack could not be mapped");
        }
    }
    // What a C program expects to find on its stack: the argument count, pointers to the arguments, an empty
    // environment, and the auxiliary values its library reads at start. The strings and the sixteen seed
    // bytes lie above the pointers.
    let mut strings: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    for a in args.iter().take(32) {
        offsets.push(strings.len());
        strings.extend_from_slice(&a.as_bytes()[..a.len().min(255)]);
        strings.push(0);
    }
    let seed_at = strings.len();
    strings.extend_from_slice(&seed);
    while strings.len() % 16 != 0 {
        strings.push(0);
    }
    let strings_base = STACK_TOP - strings.len();
    let aux: [(u64, u64); 12] = [(3, (base + phoff) as u64), (4, 56), (5, phnum as u64), (6, 4096), (7, 0), (9, entry as u64),
                                 (11, 0), (12, 0), (13, 0), (14, 0), (23, 0), (25, (strings_base + seed_at) as u64)];
    let mut words: Vec<u64> = Vec::new();
    words.push(offsets.len() as u64);
    for o in &offsets {
        words.push((strings_base + o) as u64);
    }
    words.push(0); // end of the arguments
    words.push(0); // an empty environment
    for (k, v) in aux {
        words.push(k);
        words.push(v);
    }
    words.push(0);
    words.push(0);
    if words.len() % 2 != 0 {
        words.push(0); // so that the stack pointer is a multiple of 16
    }
    let stack = strings_base - words.len() * 8;
    let mut bytes: Vec<u8> = Vec::with_capacity(words.len() * 8 + strings.len());
    for w in &words {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    bytes.extend_from_slice(&strings);
    if !space.copy_out(stack, &bytes) {
        return Err("the arguments could not be written on the stack");
    }
    Ok(Loaded { entry, stack, image_end: (image_end + 4095) & !4095, pages: image_pages + STACK_PAGES })
}
