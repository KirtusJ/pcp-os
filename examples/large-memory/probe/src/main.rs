#![no_std]
#![no_main]
//! A test program for `examples/large-memory/check.py`: asks for ordinary memory 64 MiB at a time and
//! writes to every page of each piece, until the kernel refuses or 160 pieces are held; says how much it
//! got, and ends. The kernel then takes every page back.
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
            // Safety: the kernel returned this mapping, readable and writable, of CHUNK bytes.
            unsafe { base.add(offset).write_volatile((offset >> 12) as u8) };
            offset += 4096;
        }
        got += 1;
        if got % 8 == 0 { say(b"memory: mebibytes so far ", got * 64); }
        if got == 160 { break; }
    }
    say(b"memory: pages that did not read zero ", dirty);
    say(b"memory: mebibytes mapped and touched ", got * 64);
    exit(0);
}
