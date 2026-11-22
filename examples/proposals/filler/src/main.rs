#![no_std]
#![no_main]
//! A test program for `examples/proposals/check.py`: an ordinary program with the store, which writes one
//! file of a set size so that the next proposal finds the store with exactly so much room, then reads
//! it back. The size and the file's name are fixed when it is built (FILL=<bytes>, FILE=<name>), so
//! each is a different program with its own digest.
use pcp_game_guest::{call, exit, log, open};

const fn number(text: &str) -> usize {
    let bytes = text.as_bytes();
    let (mut at, mut value) = (0, 0usize);
    while at < bytes.len() {
        assert!(bytes[at] >= b'0' && bytes[at] <= b'9');
        value = value * 10 + (bytes[at] - b'0') as usize;
        at += 1;
    }
    value
}

const BYTES: usize = number(match option_env!("FILL") { Some(text) => text, None => "0" });
const FILE: &str = match option_env!("FILE") { Some(name) => name, None => "filler.bin" };

fn say(words: &[u8], mut value: usize) {
    let mut digits = [0u8; 20];
    let mut count = 0;
    loop {
        digits[count] = b'0' + (value % 10) as u8;
        value /= 10;
        count += 1;
        if value == 0 {
            break;
        }
    }
    log(words);
    for i in (0..count).rev() {
        log(&digits[i..i + 1]);
    }
    log(b"\n");
}

pcp_game_guest::entry!(filler_main);
#[no_mangle]
pub extern "C" fn filler_main() -> ! {
    static CHUNK: [u8; 65536] = [b'.'; 65536];
    let mut path = [0u8; 64];
    let folder = b"elsewhere/";
    path[..folder.len()].copy_from_slice(folder);
    path[folder.len()..folder.len() + FILE.len()].copy_from_slice(FILE.as_bytes());
    let path = &path[..folder.len() + FILE.len() + 1];
    let fd = open(path, 0x41);
    if fd < 0 {
        log(b"filler: the file could not be made\n");
        exit(3);
    }
    let mut written = 0usize;
    while written < BYTES {
        let want = (BYTES - written).min(CHUNK.len());
        let got = call(1, fd as usize, CHUNK.as_ptr() as usize, want, 0, 0, 0);
        if got <= 0 {
            break;
        }
        written += got as usize;
    }
    say(b"filler: wrote ", written);
    say(b"filler: of ", BYTES);
    // What was written is read back whole from a second descriptor: every byte, and no more.
    call(3, fd as usize, 0, 0, 0, 0, 0);
    let back = open(path, 0);
    let (mut read, mut same) = (0usize, back >= 0);
    static mut BACK: [u8; 65536] = [0; 65536];
    // Safety: one thread; used only here.
    let buffer = unsafe { &mut *core::ptr::addr_of_mut!(BACK) };
    while back >= 0 {
        let got = call(0, back as usize, buffer.as_mut_ptr() as usize, buffer.len(), 0, 0, 0);
        if got <= 0 {
            break;
        }
        same &= buffer[..got as usize].iter().all(|b| *b == b'.');
        read += got as usize;
    }
    say(b"filler: read back ", read);
    say(b"filler: read back the same ", (same && read == written) as usize);
    exit(if written == BYTES { 0 } else { 4 });
}
