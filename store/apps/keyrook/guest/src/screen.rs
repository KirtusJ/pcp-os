use crate::profile::{Profile, WIDTHS};
use keyrook_core::{Challenge, Episode};
use pcp_game_guest::ink::Frame;

pub const W: usize = 800;
pub const H: usize = 480;
const NAVY: u32 = 0x16272c;
const DEEP: u32 = 0x101e22;
const RULE: u32 = 0x36505a;
const PAPER: u32 = 0xf5edda;
const CREAM: u32 = 0xfff9e8;
const MINT: u32 = 0x91dac4;
const MUTED: u32 = 0xa1b7b3;
const RED: u32 = 0xf79663;

pub fn decimal(mut n: u64, buf: &mut [u8; 20]) -> &str {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 { break; }
    }
    core::str::from_utf8(&buf[i..]).unwrap()
}

fn num(f: &mut Frame, x: i32, y: i32, n: u64, scale: i32, c: u32) {
    f.text(x, y, decimal(n, &mut [0; 20]), scale, c);
}

fn hex(f: &mut Frame, x: i32, y: i32, n: u16, count: usize, scale: i32, c: u32) {
    let mut b = [0u8; 4];
    for i in 0..count {
        b[count-1-i] = b"0123456789ABCDEF"[((n >> (4*i)) & 15) as usize];
    }
    f.text(x, y, core::str::from_utf8(&b[..count]).unwrap(), scale, c);
}

fn rook(f: &mut Frame, x: i32, y: i32) {
    f.rect(x, y, 48, 48, NAVY);
    for dx in [9, 20, 31] { f.rect(x+dx, y+8, 7, 11, MINT); }
    f.rect(x+9, y+16, 29, 5, MINT);
    f.rect(x+15, y+21, 17, 13, MINT);
    f.rect(x+12, y+32, 23, 4, MINT);
    f.rect(x+8, y+37, 31, 4, MINT);
}

fn bits(f: &mut Frame, x: i32, y: i32, v: u16, ink: u32) {
    for r in 0..4 {
        for c in 0..4 {
            let active = (v >> (15 - (4*r + c))) & 1 != 0;
            f.rect(x+c*5, y+r*5, 4, 4, if active { ink } else { RULE });
        }
    }
}

pub fn policy_name(policy: u8) -> &'static str {
    if policy == 0 { "EXHAUSTIVE" } else { "MEET IN MIDDLE" }
}

pub fn draw(px: &mut [u32], p: &Profile, pairs: &Challenge, last: &Episode,
            mode: u8, saved: &str) {
    let f = &mut Frame { px, pitch: W };
    f.px.fill(DEEP);
    f.rect(0, 0, 800, 77, PAPER);
    rook(f, 24, 14);
    f.text(87, 12, "KEYROOK", 5, NAVY);
    f.text(89, 55, "RECURSIVE CIPHER LAB", 1, NAVY);
    f.text(535, 18, "BITS PER KEY", 1, NAVY);
    num(f, 535, 35, WIDTHS[p.width] as u64, 3, NAVY);
    f.rect(630, 18, 1, 40, 0xb1b4a7);
    f.text(650, 18, "WORKER DEPTH", 1, NAVY);
    num(f, 650, 35, p.depth as u64, 3, NAVY);

    f.rect(21, 93, 415, 218, NAVY);
    f.text(38, 108, "KNOWN PAIRS", 2, PAPER);
    f.text(39, 140, "PLAINTEXT", 1, MUTED);
    f.text(247, 140, "CIPHERTEXT", 1, MUTED);
    for (i, &(plain, cipher)) in pairs.pairs.iter().enumerate() {
        let y = 162 + 44*i as i32;
        f.rect(38, y-7, 380, 1, RULE);
        bits(f, 39, y+3, plain, MINT);
        hex(f, 70, y+4, plain, 4, 3, PAPER);
        f.rect(181, y+12, 36, 2, RULE);
        f.rect(215, y+9, 3, 8, MINT);
        bits(f, 247, y+3, cipher, RED);
        hex(f, 278, y+4, cipher, 4, 3, PAPER);
    }
    f.text(39, 295, "FIXTURE SEED", 1, MUTED);
    num(f, 125, 295, p.seed as u64, 1, PAPER);

    f.rect(449, 93, 330, 218, CREAM);
    f.rect(449, 93, 5, 218, if last.verified { MINT } else { RED });
    f.text(468, 108, "REFEREE RESULT", 1, NAVY);
    let verdict = if last.verified { "VERIFIED" } else if last.candidates > 1 { "AMBIGUOUS" } else { "UNVERIFIED" };
    f.text(468, 127, verdict, 3, NAVY);
    f.text(468, 161, "RECOVERED KEYS", 1, NAVY);
    if let Some((a, b)) = last.key {
        hex(f, 581, 158, u16::from(a), 2, 2, NAVY);
        f.text(610, 158, "/", 2, NAVY);
        hex(f, 629, 158, u16::from(b), 2, 2, NAVY);
    } else {
        f.text(581, 160, "NONE", 1, NAVY);
    }
    f.rect(468, 185, 291, 1, 0xc7c9b6);
    f.text(468, 200, "STAGE EVALUATIONS", 1, NAVY);
    num(f, 468, 218, last.work, 3, NAVY);
    f.text(652, 200, "WORKER CALLS", 1, NAVY);
    num(f, 652, 218, u64::from(last.branches), 3, NAVY);
    f.text(468, 269, "METHOD USED", 1, NAVY);
    f.text(468, 285, policy_name(last.strategy), 2, NAVY);

    let memory = &p.cells[p.index()];
    f.text(26, 332, "TRAINING MEMORY", 2, MINT);
    f.text(26, 361, "LEARNED SELECTION", 1, MUTED);
    let learned = if memory.runs == [0,0] { "UNTRAINED" } else { policy_name(memory.choose()) };
    f.text(26, 378, learned, 2, PAPER);
    f.text(278, 335, "EXHAUSTIVE TRIALS", 1, MUTED);
    num(f, 278, 352, u64::from(memory.runs[0]), 2, PAPER);
    f.text(278, 379, "MEET IN MIDDLE TRIALS", 1, MUTED);
    num(f, 278, 395, u64::from(memory.runs[1]), 2, PAPER);
    f.text(515, 335, "RUN MODE", 1, MUTED);
    f.text(515, 351, if mode == 2 { "LEARNED" } else { policy_name(mode) }, 2, PAPER);
    f.text(515, 384, saved, 1, MINT);

    f.rect(0, 424, 800, 56, NAVY);
    f.text(25, 437, "ENTER  NEW FIXTURE", 1, PAPER);
    f.text(220, 437, "T  TRAIN BOTH METHODS", 1, MINT);
    f.text(479, 437, "M  CHANGE RUN MODE", 1, PAPER);
    f.text(25, 458, "B  KEY WIDTH", 1, PAPER);
    f.text(220, 458, "D  WORKER DEPTH", 1, PAPER);
    f.text(479, 458, "ESC  EXIT", 1, PAPER);
}
