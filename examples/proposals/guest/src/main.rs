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

fn ask_only(fd: isize, attempt: u64, number: u64, name: &[u8], content: &[u8]) -> isize {
    let mut buffer = [0u8; 256];
    let length = record(&mut buffer, attempt, number, name, content);
    call(1, fd as usize, buffer.as_ptr() as usize, length, 0, 0, 0)
}

fn ask(fd: isize, attempt: u64, number: u64, name: &[u8], content: &[u8]) -> u32 {
    let mut buffer = [0u8; 256];
    let length = record(&mut buffer, attempt, number, name, content);
    say(b"probe: write answered ", call(1, fd as usize, buffer.as_ptr() as usize, length, 0, 0, 0));
    let mut receipt = [0u8; 100];
    let got = call(0, fd as usize, receipt.as_mut_ptr() as usize, receipt.len(), 0, 0, 0);
    say(b"probe: receipt bytes ", got);
    let outcome = u32::from_le_bytes([receipt[4], receipt[5], receipt[6], receipt[7]]);
    if got == 100 {
        say(b"probe: outcome ", outcome as isize);
    }
    outcome
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
    let first = ask(fd, 7, 1, NAME, CONTENT); // the person is asked
    // A read too small, a read of nothing and a read into memory that is not ours each take nothing:
    // the whole receipt is still there straight afterwards, with no proposal made in between.
    let mut small = [0u8; 10];
    say(b"probe: a read too small answered ", call(0, fd as usize, small.as_mut_ptr() as usize, small.len(), 0, 0, 0));
    say(b"probe: a read of nothing answered ", call(0, fd as usize, small.as_mut_ptr() as usize, 0, 0, 0, 0));
    say(b"probe: a read into no memory answered ", call(0, fd as usize, 8, 100, 0, 0, 0));
    let mut whole = [0u8; 100];
    say(b"probe: the read straight after answered ", call(0, fd as usize, whole.as_mut_ptr() as usize, whole.len(), 0, 0, 0));
    say(b"probe: its outcome ", u32::from_le_bytes([whole[4], whole[5], whole[6], whole[7]]) as isize);
    say(b"probe: its attempt ", u64::from_le_bytes([whole[16], whole[17], whole[18], whole[19], whole[20], whole[21], whole[22], whole[23]]) as isize);
    say(b"probe: its proposal ", u64::from_le_bytes([whole[24], whole[25], whole[26], whole[27], whole[28], whole[29], whole[30], whole[31]]) as isize);
    say(b"probe: its lasting ", u32::from_le_bytes([whole[96], whole[97], whole[98], whole[99]]) as isize);
    say(b"probe: its run ", u64::from_le_bytes([whole[8], whole[9], whole[10], whole[11], whole[12], whole[13], whole[14], whole[15]]) as isize);
    for (what, from) in [(&b"probe: its request digest "[..], 32usize), (&b"probe: its content digest "[..], 64)] {
        log(what);
        let mut hex = [0u8; 65];
        for (i, byte) in whole[from..from + 32].iter().enumerate() {
            hex[i * 2] = b"0123456789abcdef"[(byte >> 4) as usize];
            hex[i * 2 + 1] = b"0123456789abcdef"[(byte & 15) as usize];
        }
        hex[64] = b'\n';
        log(&hex);
    }
    ask(fd, 7, 1, NAME, CONTENT); // the same proposal again: answered from the receipt, nobody asked
    ask(fd, 7, 1, NAME, b"other words"); // the same numbers for other content: refused, the receipt unchanged
    ask(fd, 7, 1, b"taylor/first-project/other.txt", CONTENT); // the same numbers for another name: refused likewise
    ask(fd, 7, 2, NAME, CONTENT); // a new proposal for a name now taken (if the first was allowed)
    ask(fd, 7, 3, b"Bad Name.txt", CONTENT); // refused unseen: a capital and a space
    ask(fd, 7, 4, b"tab.txt", b"a\tb"); // refused unseen: a character the kernel's screen cannot show exactly
    ask(fd, 7, 5, b"dev/proposal", CONTENT); // refused unseen: where the kernel's devices are
    ask(fd, 7, 6, b"taylor/../escape.txt", CONTENT); // refused unseen: a part beginning with a dot
    if first == 1 {
        // A thousand lines that each end in a space. Each takes two rows of the kernel's screen (the
        // line, and a note that it ends in a space), so the file is short enough to be taken and too
        // long to be shown in sixty-four pages: the kernel must say so and ask nothing.
        static mut LONG: [u8; 3100] = [0; 3100];
        // Safety: this program has one thread and uses the buffer only here.
        let long = unsafe { &mut *core::ptr::addr_of_mut!(LONG) };
        let name = b"taylor/first-project/long.txt";
        long[0..4].copy_from_slice(&1u32.to_le_bytes());
        long[4..8].copy_from_slice(&1u32.to_le_bytes());
        long[8..16].copy_from_slice(&7u64.to_le_bytes());
        long[16..24].copy_from_slice(&7u64.to_le_bytes());
        long[24..28].copy_from_slice(&(name.len() as u32).to_le_bytes());
        long[28..32].copy_from_slice(&3000u32.to_le_bytes());
        long[32..32 + name.len()].copy_from_slice(name);
        for line in 0..1000 {
            let at = 32 + name.len() + line * 3;
            long[at..at + 3].copy_from_slice(b"a \n");
        }
        let length = 32 + name.len() + 3000;
        log(b"probe: the long file\n");
        say(b"probe: long write answered ", call(1, fd as usize, long.as_ptr() as usize, length, 0, 0, 0));
        let mut receipt = [0u8; 100];
        say(b"probe: long receipt bytes ", call(0, fd as usize, receipt.as_mut_ptr() as usize, 100, 0, 0, 0));
        say(b"probe: long outcome ", u32::from_le_bytes([receipt[4], receipt[5], receipt[6], receipt[7]]) as isize);
    }
    // More proposals than the kernel keeps receipts for: each is for the taken name, so nobody is asked,
    // and after the sixty-fourth of the run the kernel takes no more.
    // Only where the file was made: otherwise the name is free and each would be a question.
    let mut number = 100;
    let mut last = 0;
    while first == 1 && number < 170 {
        let mut buffer = [0u8; 256];
        let length = record(&mut buffer, 9, number, NAME, CONTENT);
        last = call(1, fd as usize, buffer.as_ptr() as usize, length, 0, 0, 0);
        if last == -28 {
            break;
        }
        number += 1;
    }
    say(b"probe: the kernel took no more proposals at number ", number as isize);
    say(b"probe: answering ", last);
    if first == 1 {
        // With the kernel taking no more, the first proposal of all is still answered from its receipt.
        log(b"probe: after the flood\n");
        ask(fd, 7, 1, NAME, CONTENT);
    }
    // What the program can see afterwards: a picture of its build folder, read a few bytes at a time,
    // and the files in it, read and never written; nothing anywhere else.
    say(b"probe: a file outside the build folder was answered ", ask_only(fd, 7, 8, b"elsewhere/note.txt", CONTENT));
    let list = open(b"/dev/build-folder\0", 0);
    say(b"probe: opening the list answered ", list);
    say(b"probe: opening the list to write answered ", open(b"/dev/build-folder\0", 1));
    say(b"probe: opening the list to read and make answered ", open(b"/dev/build-folder\0", 0x40));
    say(b"probe: opening the list to read and empty answered ", open(b"/dev/build-folder\0", 0x200));
    let mut picture = [0u8; 512];
    let mut have = 0usize;
    loop {
        let got = call(0, list as usize, picture[have..].as_mut_ptr() as usize, 7.min(picture.len() - have), 0, 0, 0);
        if got <= 0 {
            say(b"probe: the list ended with ", got);
            break;
        }
        have += got as usize;
    }
    say(b"probe: the list is bytes ", have as isize);
    // The exact bytes, for a reader that was not written with this kernel.
    log(b"probe: list hex ");
    for byte in &picture[..have] {
        log(&[b"0123456789abcdef"[(byte >> 4) as usize], b"0123456789abcdef"[(byte & 15) as usize]]);
    }
    log(b"\n");
    if have >= 24 {
        let word = |at: usize| u32::from_le_bytes([picture[at], picture[at + 1], picture[at + 2], picture[at + 3]]) as usize;
        say(b"probe: list version ", word(0) as isize);
        say(b"probe: list count ", word(4) as isize);
        say(b"probe: list complete ", word(8) as isize);
        say(b"probe: list run ", word(16) as isize);
        let root = word(12);
        log(b"probe: list folder ");
        log(&picture[24..24 + root]);
        log(b"\n");
        let mut at = 24 + root;
        for _ in 0..word(4) {
            let length = u16::from_le_bytes([picture[at], picture[at + 1]]) as usize;
            log(b"probe: listed ");
            log(&picture[at + 8..at + 8 + length]);
            say(b" of bytes ", word(at + 4) as isize);
            at += 8 + length;
        }
        say(b"probe: list bytes left over ", (have - at) as isize);
    }
    say(b"probe: writing to the list answered ", call(1, list as usize, picture.as_ptr() as usize, 4, 0, 0, 0));
    let note = open(b"taylor/first-project/note.txt\0", 0);
    say(b"probe: opening the note to read answered ", if note >= 0 { 0 } else { note });
    if note >= 0 {
        let mut back = [0u8; 128];
        let got = call(0, note as usize, back.as_mut_ptr() as usize, back.len(), 0, 0, 0);
        say(b"probe: the note read back bytes ", got);
        say(b"probe: the note read back the same ", (got as usize == CONTENT.len() && &back[..CONTENT.len()] == CONTENT) as isize);
        say(b"probe: writing to the note answered ", call(1, note as usize, back.as_ptr() as usize, 4, 0, 0, 0));
    }
    say(b"probe: opening the note to write answered ", open(b"taylor/first-project/note.txt\0", 1));
    say(b"probe: opening the note to empty it answered ", open(b"taylor/first-project/note.txt\0", 0x201));
    say(b"probe: opening the note to read and make answered ", open(b"taylor/first-project/note.txt\0", 0x40));
    say(b"probe: opening the note to read and empty answered ", open(b"taylor/first-project/note.txt\0", 0x200));
    say(b"probe: opening the note to make it anew answered ", open(b"taylor/first-project/note.txt\0", 0xc0));
    say(b"probe: opening the list to make it anew answered ", open(b"/dev/build-folder\0", 0xc0));
    // After every refusal the note is read again, to its end, and is what it was.
    let after = open(b"taylor/first-project/note.txt\0", 0);
    say(b"probe: opening the note to read after all that answered ", if after >= 0 { 0 } else { after });
    if after >= 0 {
        let mut back = [0u8; 128];
        let mut have = 0usize;
        loop {
            let got = call(0, after as usize, back[have..].as_mut_ptr() as usize, 5.min(back.len() - have), 0, 0, 0);
            if got <= 0 {
                say(b"probe: the note read after all that ended with ", got);
                break;
            }
            have += got as usize;
        }
        say(b"probe: the note after all that is the same ", (have == CONTENT.len() && &back[..have] == CONTENT) as isize);
        say(b"probe: closing it answered ", call(3, after as usize, 0, 0, 0, 0, 0));
    }
    say(b"probe: removing the note answered ", call(87, b"taylor/first-project/note.txt\0".as_ptr() as usize, 0, 0, 0, 0, 0));
    say(b"probe: renaming the note answered ", call(82, b"taylor/first-project/note.txt\0".as_ptr() as usize, b"taylor/x.txt\0".as_ptr() as usize, 0, 0, 0, 0));
    say(b"probe: a name outside the folder answered ", open(b"elsewhere/note.txt\0", 0));
    say(b"probe: a name beside the folder answered ", open(b"taylorx/note.txt\0", 0));
    // What a program that may propose is not given: sound, and the loader's other files (its own
    // program is one of them, so the name is certain to exist).
    say(b"probe: opening the sound device answered ", open(b"/dev/audio\0", 1));
    say(b"probe: opening its own program among the loader's files answered ", open(b"proposal-probe\0", 0));
    log(b"probe: done\n");
    // Giving up: the second kind of record. After it nothing new is asked of the person; what was
    // answered before is still answered from its receipt; and giving up again changes nothing.
    let mut gone = [0u8; 32];
    gone[0..4].copy_from_slice(&1u32.to_le_bytes());
    gone[4..8].copy_from_slice(&2u32.to_le_bytes());
    gone[8..16].copy_from_slice(&9u64.to_le_bytes());
    gone[16..24].copy_from_slice(&1u64.to_le_bytes());
    let mut ill = gone;
    ill[24] = 1; // a give-up record that names a length is not one
    let outcome_now = |through: isize| {
        let mut receipt = [0u8; 100];
        if call(0, through as usize, receipt.as_mut_ptr() as usize, 100, 0, 0, 0) == 100 { u32::from_le_bytes([receipt[4], receipt[5], receipt[6], receipt[7]]) as isize } else { -1 }
    };
    say(b"probe: given up: an ill-formed record answered ", call(1, fd as usize, ill.as_ptr() as usize, 32, 0, 0, 0));
    say(b"probe: given up: the record answered ", call(1, fd as usize, gone.as_ptr() as usize, 32, 0, 0, 0));
    say(b"probe: given up: its receipt says ", outcome_now(fd));
    say(b"probe: given up: a new proposal then answered ", ask_only(fd, 9, 2, b"taylor/first-project/later.txt", CONTENT));
    say(b"probe: given up: its receipt says ", outcome_now(fd));
    say(b"probe: given up: the first proposal again answered ", ask_only(fd, 7, 1, NAME, CONTENT));
    say(b"probe: given up: its receipt says ", outcome_now(fd));
    say(b"probe: given up: a second record answered ", call(1, fd as usize, gone.as_ptr() as usize, 32, 0, 0, 0));
    let again = open(b"/dev/proposal\0", 2);
    say(b"probe: given up: opening the device anew answered ", if again >= 0 { 0 } else { again });
    if again >= 0 {
        say(b"probe: given up: a new proposal through it answered ", ask_only(again, 9, 3, b"taylor/first-project/other.txt", CONTENT));
        say(b"probe: given up: its receipt says ", outcome_now(again));
    }
    say(b"probe: given up: opening the later file answered ", open(b"taylor/first-project/later.txt\0", 0));
    exit(0);
}
