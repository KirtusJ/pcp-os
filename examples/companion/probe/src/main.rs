#![no_std]
#![no_main]
//! Test programs for `examples/companion/check.py`, built three ways from this one file.
//!  front            the foreground program: sends counted bytes, reads them back changed, fills the
//!                   channel until it takes no more, drains it, and ends.
//!  back (BACK=1)    the companion: reads what arrives a few bytes at a time and writes each byte back
//!                   plus one, until the other end is gone.
//!  back that leaves (BACK=1 LEAVE=1): answers the first exchange and ends, for the front to notice.
//! Each keeps a pattern of its own in a 256-bit register (where the processor has them) for the whole
//! run and counts every time it reads back anything else.
use core::arch::asm;
use pcp_game_guest::{call, exit, log, open};

const BACK: bool = option_env!("BACK").is_some();
const LEAVE: bool = option_env!("LEAVE").is_some();

/// One whole line in one write, so that two programs writing in turn never share a line.
fn say(who: &[u8], words: &[u8], value: isize) {
    let mut out = [0u8; 120];
    let mut used = 0;
    for b in who.iter().chain(words.iter()) {
        out[used] = *b;
        used += 1;
    }
    if value < 0 {
        out[used] = b'-';
        used += 1;
    }
    let mut digits = [0u8; 20];
    let (mut n, mut count) = (value.unsigned_abs(), 0);
    loop {
        digits[count] = b'0' + (n % 10) as u8;
        n /= 10;
        count += 1;
        if n == 0 {
            break;
        }
    }
    for i in (0..count).rev() {
        out[used] = digits[i];
        used += 1;
    }
    out[used] = b'\n';
    log(&out[..used + 1]);
}

fn wide() -> bool {
    let features = core::arch::x86_64::__cpuid(1).ecx;
    features & (1 << 27) != 0 && features & (1 << 28) != 0 && {
        let (low, _high): (u32, u32);
        // Safety: XGETBV is allowed because bit 27 says so.
        unsafe { asm!("xgetbv", in("ecx") 0u32, out("eax") low, out("edx") _high, options(nomem, nostack)) };
        low & 0b110 == 0b110
    }
}

fn fill(pattern: &[u8; 32]) {
    // Safety: only called when `wide()`; the register written is declared.
    unsafe { asm!("vmovups ymm15, [{}]", in(reg) pattern.as_ptr(), out("ymm15") _, options(nostack)) };
}

fn kept(pattern: &[u8; 32]) -> bool {
    let mut bytes = [0u8; 32];
    // Safety: only called when `wide()`.
    unsafe { asm!("vmovups [{}], ymm15", in(reg) bytes.as_mut_ptr(), options(nostack)) };
    bytes == *pattern
}

fn write_all(fd: isize, mut bytes: &[u8]) -> isize {
    while !bytes.is_empty() {
        let n = call(1, fd as usize, bytes.as_ptr() as usize, bytes.len(), 0, 0, 0);
        if n <= 0 {
            return n;
        }
        bytes = &bytes[n as usize..];
    }
    1
}

/// One byte more than the channel holds in one direction.
const HOLD: usize = 65_537;

/// The other end has gone: say what a read and a write of the channel answer now.
fn gone(line: &dyn Fn(&[u8], isize), fd: isize, round: usize, back: &mut [u8], out: &[u8]) {
    line(b"the other end was gone in round ", round as isize);
    let read = call(0, fd as usize, back.as_mut_ptr() as usize, back.len(), 0, 0, 0);
    line(b"a read of it then answered ", read);
    let wrote = call(1, fd as usize, out.as_ptr() as usize, 10, 0, 0, 0);
    line(b"a write to it then answered ", wrote);
}

pcp_game_guest::entry!(probe_main);
#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    let who: &[u8] = if BACK { b"back: " } else { b"front: " };
    let line = |words: &[u8], value: isize| say(who, words, value);
    let pattern = if BACK { [0xEEu8; 32] } else { [0x11u8; 32] };
    let wide = wide();
    if wide {
        fill(&pattern);
    }
    let mut lost = 0isize;
    let check = |lost: &mut isize| {
        if wide && !kept(&pattern) {
            *lost += 1;
            fill(&pattern);
        }
    };
    let fd = open(b"/dev/talk\0", 2);
    line(b"opening the channel answered ", if fd >= 0 { 0 } else { fd });
    if fd < 0 {
        exit(0);
    }
    if BACK {
        let mut buffer = [0u8; 7];
        let mut total = 0isize;
        loop {
            let n = call(0, fd as usize, buffer.as_mut_ptr() as usize, buffer.len(), 0, 0, 0);
            check(&mut lost);
            if n <= 0 {
                line(b"read ended with ", n);
                break;
            }
            // One byte of 255 by itself is the front saying it has finished: no byte it sends as data
            // is above 250.
            if n == 1 && buffer[0] == 255 {
                line(b"told to finish after ", total);
                break;
            }
            // One byte of 254 by itself asks it to hold: it says so, then reads nothing until it has
            // written one byte more than the channel holds. The front can then fill its own direction
            // knowing that nothing is being taken from it, however the two are scheduled, and the last
            // byte written here waits for room until the front reads.
            if n == 1 && buffer[0] == 254 {
                let held = [0xA5u8; 4096];
                let mut wrote = write_all(fd, &[254]);
                let mut left = HOLD;
                while wrote > 0 && left > 0 {
                    let now = left.min(held.len());
                    wrote = write_all(fd, &held[..now]);
                    left -= now;
                    check(&mut lost);
                }
                line(b"held until it had written ", (HOLD - left) as isize);
                continue;
            }
            for b in buffer[..n as usize].iter_mut() {
                *b = b.wrapping_add(1);
            }
            total += n;
            if write_all(fd, &buffer[..n as usize]) <= 0 {
                line(b"write ended early at ", total);
                break;
            }
            check(&mut lost);
            if LEAVE && total >= 1000 {
                line(b"leaving after ", total);
                break;
            }
        }
        line(b"bytes answered ", total);
        line(b"wide registers used ", wide as isize);
        line(b"times its register held something else ", lost);
        exit(0);
    }
    // The front: a hundred exchanges of a thousand bytes, each read back in full and compared.
    let mut out = [0u8; 1000];
    let mut back = [0u8; 1000];
    let (mut wrong, mut rounds) = (0isize, 0isize);
    'exchanges: for round in 0..100usize {
        for (i, b) in out.iter_mut().enumerate() {
            *b = ((round * 7 + i) % 251) as u8;
        }
        if write_all(fd, &out) <= 0 {
            // The other end went before this was written. Which of the write and the read finds out
            // first depends on how the two were scheduled; both are then asked afresh.
            gone(&line, fd, round, &mut back, &out);
            break;
        }
        let mut have = 0;
        while have < back.len() {
            let n = call(0, fd as usize, back[have..].as_mut_ptr() as usize, back.len() - have, 0, 0, 0);
            check(&mut lost);
            if n <= 0 {
                gone(&line, fd, round, &mut back, &out);
                break 'exchanges;
            }
            have += n as usize;
        }
        wrong += out.iter().zip(back.iter()).filter(|(o, b)| o.wrapping_add(1) != **b).count() as isize;
        rounds += 1;
    }
    line(b"exchanges completed ", rounds);
    line(b"bytes that came back wrong ", wrong);
    if rounds == 100 {
        // Ask the companion to hold, and wait for it to say it has: from then on it takes nothing.
        let asked = write_all(fd, &[254]);
        let mut said = [0u8; 1];
        let n = call(0, fd as usize, said.as_mut_ptr() as usize, 1, 0, 0, 0);
        line(b"asking it to hold answered ", if asked == 1 && n == 1 { said[0] as isize } else { -1 });
        // Fill the channel without reading: opened again not to wait, it must say when it is full,
        // and everything it took must come back.
        let fast = open(b"/dev/talk\0", 2 | 0x800);
        let mut sent = 0isize;
        let full = loop {
            let n = call(1, fast as usize, out.as_ptr() as usize, out.len(), 0, 0, 0);
            if n <= 0 {
                break n;
            }
            sent += n;
            if sent > 1_000_000 {
                break 0;
            }
        };
        line(b"the full channel answered ", full);
        line(b"bytes it took before that ", sent);
        // What the companion wrote while it held: all the channel holds and one byte that waited for room.
        let (mut waiting, mut other) = (0usize, 0isize);
        while waiting < HOLD {
            let n = call(0, fd as usize, back.as_mut_ptr() as usize, back.len().min(HOLD - waiting), 0, 0, 0);
            if n <= 0 {
                break;
            }
            other += back[..n as usize].iter().filter(|b| **b != 0xA5).count() as isize;
            waiting += n as usize;
            check(&mut lost);
        }
        line(b"bytes the companion wrote while it held ", waiting as isize);
        line(b"of those, bytes that were not what it wrote ", other);
        let mut returned = 0isize;
        while returned < sent {
            let n = call(0, fd as usize, back.as_mut_ptr() as usize, back.len().min((sent - returned) as usize), 0, 0, 0);
            if n <= 0 {
                break;
            }
            returned += n;
            check(&mut lost);
        }
        line(b"bytes that came back after it ", returned);
        // Tell the companion to finish, and wait until its end of the channel is gone.
        let told = write_all(fd, &[255]);
        let n = call(0, fd as usize, back.as_mut_ptr() as usize, back.len(), 0, 0, 0);
        line(b"telling it to finish answered ", told);
        line(b"the read after that answered ", n);
    }
    line(b"wide registers used ", wide as isize);
    line(b"times its register held something else ", lost);
    exit(0);
}
