#![no_std]
#![no_main]
//! A test program for the kernel's `proposals` feature. It can make nothing by itself: it asks the
//! kernel to make one file, twice over, and says on the console what the kernel answered each time.
//! What it prints is read by `examples/proposals/check.py`; the person's answer is given there by keys.
use pcp_game_guest::{call, exit, log, open, sleep_us};

// The folder Taylor builds into, a project inside it, and a file of that project.
const NAME: &[u8] = b"taylor/first-project/note.txt";
const CONTENT: &[u8] = b"Hello.\nThis file was made by PCP OS after you allowed it.\n";

fn record(buffer: &mut [u8; 256], attempt: u64, number: u64, name: &[u8], content: &[u8]) -> usize {
    buffer[0..4].copy_from_slice(&1u32.to_le_bytes());
    buffer[4..8].copy_from_slice(&1u32.to_le_bytes());
    buffer[8..16].copy_from_slice(&attempt.to_le_bytes());
    buffer[16..24].copy_from_slice(&number.to_le_bytes());
    buffer[24..28].copy_from_slice(&(name.len() as u32).to_le_bytes());
    buffer[28..32].copy_from_slice(&(content.len() as u32).to_le_bytes());
    buffer[32..32 + name.len()].copy_from_slice(name);
    buffer[32 + name.len()..32 + name.len() + content.len()].copy_from_slice(content);
    32 + name.len() + content.len()
}

fn say(what: &[u8], value: isize) {
    let mut line = [0u8; 96];
    let mut used = 0;
    for b in what {
        line[used] = *b;
        used += 1;
    }
    let negative = value < 0;
    let mut magnitude = value.unsigned_abs();
    let mut digits = [0u8; 20];
    let mut count = 0;
    loop {
        digits[count] = b'0' + (magnitude % 10) as u8;
        magnitude /= 10;
        count += 1;
        if magnitude == 0 {
            break;
        }
    }
    if negative {
        line[used] = b'-';
        used += 1;
    }
    for i in (0..count).rev() {
        line[used] = digits[i];
        used += 1;
    }
    line[used] = b'\n';
    log(&line[..used + 1]);
}

fn ask(fd: isize, attempt: u64, number: u64, name: &[u8], content: &[u8]) {
    let mut buffer = [0u8; 256];
    let length = record(&mut buffer, attempt, number, name, content);
    say(b"probe: write answered ", call(1, fd as usize, buffer.as_ptr() as usize, length, 0, 0, 0));
    let mut receipt = [0u8; 100];
    let got = call(0, fd as usize, receipt.as_mut_ptr() as usize, receipt.len(), 0, 0, 0);
    say(b"probe: receipt bytes ", got);
    if got == 100 {
        say(b"probe: outcome ", u32::from_le_bytes([receipt[4], receipt[5], receipt[6], receipt[7]]) as isize);
    }
}

pcp_game_guest::entry!(probe_main);
#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    // By itself it may not make a file: O_WRONLY | O_CREAT.
    say(b"probe: making a file directly answered ", open(b"made-directly.txt\0", 0x41));
    let fd = open(b"/dev/proposal\0", 2);
    say(b"probe: opening the proposal device answered ", fd);
    if fd < 0 {
        exit(3);
    }
    // Three seconds in which the person may press F12 with nothing waiting.
    log(b"probe: idle\n");
    let mut waited = 0;
    while waited < 300 {
        sleep_us(10_000);
        waited += 1;
    }
    log(b"probe: asking\n");
    ask(fd, 7, 1, NAME, CONTENT); // the person is asked
    let mut small = [0u8; 10];
    say(b"probe: a read too small answered ", call(0, fd as usize, small.as_mut_ptr() as usize, small.len(), 0, 0, 0));
    ask(fd, 7, 1, NAME, CONTENT); // the same proposal again: answered from the receipt, nobody asked
    ask(fd, 7, 1, NAME, b"other words"); // the same numbers for other bytes: refused, the receipt unchanged
    ask(fd, 7, 2, NAME, CONTENT); // a new proposal for a name now taken (if the first was allowed)
    ask(fd, 7, 3, b"Bad Name.txt", CONTENT); // refused unseen: a capital and a space
    ask(fd, 7, 4, b"tab.txt", b"a\tb"); // refused unseen: a character the kernel's screen cannot show exactly
    ask(fd, 7, 5, b"dev/proposal", CONTENT); // refused unseen: where the kernel's devices are
    ask(fd, 7, 6, b"taylor/../escape.txt", CONTENT); // refused unseen: a part beginning with a dot
    log(b"probe: done\n");
    exit(0);
}
