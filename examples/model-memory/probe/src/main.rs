#![no_std]
#![no_main]
//! Test programs for `examples/model-memory/check.py`, built three ways from this one file.
//!  front             the foreground program: asks for the model itself (it must be refused), tells
//!                    its companion to go on, and waits until the companion has ended.
//!  front that takes memory (GRAB=1): before telling the companion to go on it takes all the memory
//!                    the kernel will give it and writes to every page of it, so that if any page
//!                    of the model were memory the kernel hands out, the model would be changed.
//!  back (BACK=1)     the companion: asks what is held, reads the manifest, maps the model and reads
//!                    every byte of it, and tries each thing it must not be able to do.
//!  back that writes (BACK=1 WRITE=1): maps the model and stores one byte into it.
//! The model here is not a model: its byte at offset i is `pattern(i)`, so every byte can be checked.
use pcp_game_guest::{call, exit, log, open};

const BACK: bool = option_env!("BACK").is_some();
const WRITE: bool = option_env!("WRITE").is_some();
const GRAB: bool = option_env!("GRAB").is_some();

fn pattern(i: u64) -> u8 {
    (i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 56) as u8 ^ (i >> 12) as u8
}

/// One whole line in one write: another program writes to the same console.
fn say(words: &[u8], value: i64) {
    let mut line = [0u8; 120];
    let mut at = 0;
    let who: &[u8] = if BACK { b"back: " } else { b"front: " };
    for b in who.iter().chain(words) {
        line[at] = *b;
        at += 1;
    }
    let mut digits = [0u8; 20];
    let mut count = 0;
    let mut left = value.unsigned_abs();
    loop {
        digits[count] = b'0' + (left % 10) as u8;
        left /= 10;
        count += 1;
        if left == 0 {
            break;
        }
    }
    if value < 0 {
        line[at] = b'-';
        at += 1;
    }
    for i in (0..count).rev() {
        line[at] = digits[i];
        at += 1;
    }
    line[at] = b'\n';
    log(&line[..at + 1]);
}

pcp_game_guest::entry!(probe_main);
#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    let fd = open(b"/dev/model\0", 0);
    say(b"opening the model answered ", if fd >= 0 { 0 } else { fd as i64 });
    if !BACK {
        // Wait for the companion: a read of the channel answers 0 once it has ended.
        if GRAB {
            const CHUNK: usize = 64 << 20;
            let mut pieces = 0i64;
            loop {
                let at = call(9, 0, CHUNK, 3, 0x22, usize::MAX, 0);
                if at < 0 {
                    break;
                }
                let mut offset = 0;
                while offset < CHUNK {
                    // Safety: the kernel returned this mapping, readable and writable, of CHUNK bytes.
                    unsafe { ((at as usize + offset) as *mut u64).write_volatile(0xDEAD_DEAD_DEAD_DEAD) };
                    offset += 4096;
                }
                pieces += 1;
            }
            say(b"mebibytes taken and written before the companion goes on ", pieces * 64);
        }
        let talk = open(b"/dev/talk\0", 2);
        if talk >= 0 {
            say(b"telling the companion to go on answered ", call(1, talk as usize, b"g".as_ptr() as usize, 1, 0, 0, 0) as i64);
            let mut byte = [0u8; 1];
            let ended = call(0, talk as usize, byte.as_mut_ptr() as usize, 1, 0, 0, 0);
            say(b"the companion ended and the channel answered ", ended as i64);
        }
        exit(0);
    }
    // Go on only when the front says so.
    let talk = open(b"/dev/talk\0", 2);
    let mut byte = [0u8; 1];
    say(b"waiting for the front answered ", call(0, talk as usize, byte.as_mut_ptr() as usize, 1, 0, 0, 0) as i64);
    if fd < 0 {
        exit(0);
    }
    let fd = fd as usize;
    let mut held = [0u8; 48];
    say(b"asking what is held answered ", call(16, fd, 0x6D01, held.as_mut_ptr() as usize, 0, 0, 0) as i64);
    say(b"asking something else of it answered ", call(16, fd, 0x6D02, held.as_mut_ptr() as usize, 0, 0, 0) as i64);
    let manifest_bytes = u64::from_le_bytes([held[0], held[1], held[2], held[3], held[4], held[5], held[6], held[7]]);
    let bytes = u64::from_le_bytes([held[8], held[9], held[10], held[11], held[12], held[13], held[14], held[15]]);
    say(b"manifest bytes ", manifest_bytes as i64);
    say(b"model bytes ", bytes as i64);
    say(b"sum of the pinned digest's bytes ", held[16..48].iter().map(|b| *b as i64).sum());
    // The manifest, a few bytes at a time, and a sum of it that weighs each byte by where it is.
    let (mut read, mut sum) = (0u64, 0u64);
    loop {
        let mut piece = [0u8; 100];
        let got = call(0, fd, piece.as_mut_ptr() as usize, piece.len(), 0, 0, 0);
        if got <= 0 {
            say(b"the read after the manifest's end answered ", got as i64);
            break;
        }
        for b in &piece[..got as usize] {
            read += 1;
            sum = sum.wrapping_add(read * (*b as u64 + 1)) % 1_000_000_007;
        }
    }
    say(b"manifest bytes read ", read as i64);
    say(b"weighted sum of the manifest ", sum as i64);
    say(b"a write to the model answered ", call(1, fd, held.as_ptr() as usize, 8, 0, 0, 0) as i64);
    // What must be refused before the one mapping that is allowed.
    let map = |length: u64, protection: usize| call(9, 0, length as usize, protection, 2, fd, 0);
    say(b"mapping it writable answered ", map(bytes, 3) as i64);
    say(b"mapping it executable answered ", map(bytes, 5) as i64);
    say(b"mapping one byte less answered ", map(bytes - 1, 1) as i64);
    say(b"mapping a page more answered ", map(bytes + 4096, 1) as i64);
    let at = map(bytes, 1);
    say(b"mapping it readable answered ", if at > 0 { 0 } else { at as i64 });
    if at <= 0 {
        exit(0);
    }
    say(b"mapping it a second time answered ", map(bytes, 1) as i64);
    let base = at as usize as *mut u8;
    if WRITE {
        log(b"back: storing a byte into the model\n");
        // Safety: none is claimed. The kernel must stop this program here.
        unsafe { base.write_volatile(1) };
        log(b"back: THE STORE WENT THROUGH\n");
        exit(0);
    }
    let mut wrong = 0i64;
    let mut i = 0u64;
    while i < bytes {
        // Safety: the kernel mapped `bytes` bytes readable from `base`.
        if unsafe { base.add(i as usize).read_volatile() } != pattern(i) {
            wrong += 1;
        }
        i += 1;
    }
    say(b"model bytes read ", i as i64);
    say(b"of those, bytes that were not the model's ", wrong);
    // Making it writable or executable afterwards, and handing its address to the kernel as a buffer.
    say(b"making its first page writable answered ", call(10, at as usize, 4096, 3, 0, 0, 0) as i64);
    say(b"making its first page executable answered ", call(10, at as usize, 4096, 5, 0, 0, 0) as i64);
    // Safety: as above; whatever the calls before answered, the first byte must still be readable and the same.
    say(b"its first byte is still the model's ", (unsafe { base.read_volatile() } == pattern(0)) as i64);
    // Handing its address to the kernel as a buffer: the kernel does not read a program's model pages for it.
    say(b"writing the console from the model's memory answered ", call(1, 1, at as usize, 4, 0, 0, 0) as i64);
    exit(0);
}
