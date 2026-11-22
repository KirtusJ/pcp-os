#![no_std]
#![no_main]
//! A test program for `examples/large-memory/check.py`: asks for ordinary memory 64 MiB at a time and
//! writes to every page of each piece, until the kernel refuses or 160 pieces are held; says how much it
//! got. Each page is given a value of its own at both ends, and once every piece is held all of them
//! are read again: two pages that were one piece of memory, or a page that did not keep what was
//! written, would then show. The kernel takes every page back when the program ends.
use pcp_game_guest::{call, exit, log};
fn say(words: &[u8], mut value: usize) {
    let mut digits = [0u8; 20];
    let mut count = 0;
    loop { digits[count] = b'0' + (value % 10) as u8; value /= 10; count += 1; if value == 0 { break; } }
    log(words);
    for i in (0..count).rev() { log(&digits[i..i + 1]); }
    log(b"\n");
}
pcp_game_guest::entry!(probe_main);
#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    const CHUNK: usize = 64 << 20;
    let mut got = 0usize;
    let mut dirty = 0usize;
    let mut held = [core::ptr::null_mut::<u8>(); 160];
    // What a page holds: which piece and which page of it, so no two pages hold the same, and never zero.
    let mark = |piece: usize, page: usize| 0xA5A5_0000_0000_0000u64 | (piece as u64) << 32 | page as u64;
    loop {
        let at = call(9, 0, CHUNK, 3, 0x22, usize::MAX, 0);
        if at < 0 { say(b"memory: refused with ", (-at) as usize); break; }
        let base = at as usize as *mut u8;
        let mut offset = 0;
        while offset < CHUNK {
            // Memory is handed over holding nothing: every page reads zero before this program writes it.
            // Safety: as below.
            if unsafe { base.add(offset).read_volatile() } != 0 || unsafe { base.add(offset + 4095).read_volatile() } != 0 {
                dirty += 1;
            }
            // Safety: the kernel returned this mapping, readable and writable, of CHUNK bytes; a page's
            // first and last eight bytes lie within it and are aligned to eight.
            unsafe {
                (base.add(offset) as *mut u64).write_volatile(mark(got, offset >> 12));
                (base.add(offset + 4088) as *mut u64).write_volatile(!mark(got, offset >> 12));
            }
            offset += 4096;
        }
        held[got] = base;
        got += 1;
        if got % 8 == 0 { say(b"memory: mebibytes so far ", got * 64); }
        if got == 160 { break; }
    }
    // Every piece is held now. Read every page of every piece again.
    let mut changed = 0usize;
    for (piece, base) in held[..got].iter().enumerate() {
        let mut offset = 0;
        while offset < CHUNK {
            // Safety: as above; the mapping is still held.
            let (first, last) = unsafe { ((base.add(offset) as *const u64).read_volatile(), (base.add(offset + 4088) as *const u64).read_volatile()) };
            if first != mark(piece, offset >> 12) || last != !mark(piece, offset >> 12) {
                changed += 1;
            }
            offset += 4096;
        }
    }
    say(b"memory: pages that did not read zero ", dirty);
    say(b"memory: pages read again after all were written ", got * (CHUNK >> 12));
    say(b"memory: of those, pages that did not hold what was written ", changed);
    say(b"memory: mebibytes mapped and touched ", got * 64);
    exit(0);
}
