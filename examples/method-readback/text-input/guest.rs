//! Run the checked-in model on a bounded ASCII excerpt read through the guest file API.
#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::fmt::{self, Write};

#[allow(dead_code)]
#[path = "../../resident-workers/fixture/model.rs"]
mod model;

const POSITIONS: usize = 96;
const VOCAB: &[u8] = include_bytes!(env!("PCPOS_TOKENIZER"));
static mut CONTEXT: model::Context<POSITIONS> = model::Context::new();

global_asm!(
    ".global _start", "_start:", "mov rdi, rsp", "and rsp, -16", "call guest_main", "ud2"
);

fn syscall(n: usize, a: usize, b: usize, c: usize) -> isize {
    let result;
    unsafe {
        asm!("syscall", inlateout("rax") n as isize => result, in("rdi") a, in("rsi") b,
             in("rdx") c, lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    result
}

fn exit(code: usize) -> ! {
    unsafe { asm!("syscall", in("rax") 60usize, in("rdi") code, options(noreturn)); }
}

struct Console;
impl Write for Console {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let mut bytes = text.as_bytes();
        while !bytes.is_empty() {
            let n = syscall(1, 1, bytes.as_ptr() as usize, bytes.len());
            if n <= 0 || n as usize > bytes.len() { return Err(fmt::Error); }
            bytes = &bytes[n as usize..];
        }
        Ok(())
    }
}

// PCP OS supplies NUL-terminated argv entries on the initial process stack.
unsafe fn decimal(ptr: *const u8) -> Option<usize> {
    let mut value = 0usize;
    for i in 0..8 {
        let ch = *ptr.add(i);
        if ch == 0 { return (i != 0).then_some(value); }
        if !ch.is_ascii_digit() { return None; }
        value = value * 10 + usize::from(ch - b'0');
    }
    None
}

fn read_excerpt(offset: usize, dest: &mut [u8]) -> Result<(), usize> {
    let fd = syscall(2, b"core-spec.md\0".as_ptr() as usize, 0, 0);
    if fd < 0 { return Err(if fd == -2 { 2 } else { 3 }); }
    let mut block = [0u8; 512];
    let mut pos = 0;
    let end = offset + dest.len();
    let result = loop {
        let n = syscall(0, fd as usize, block.as_mut_ptr() as usize, block.len());
        if n < 0 || n as usize > block.len() { break Err(3); }
        if n == 0 { break Err(4); }
        let next = pos + n as usize;
        let from = pos.max(offset);
        let to = next.min(end);
        if from < to {
            dest[from-offset..to-offset].copy_from_slice(&block[from-pos..to-pos]);
        }
        if next >= end { break Ok(()); }
        pos = next;
    };
    if syscall(3, fd as usize, 0, 0) != 0 { return Err(3); }
    result
}

// The build pins this 512-piece tokenizer. Each record has a little-endian
// float score, a 32-bit byte count and the literal piece bytes.
struct Vocab([u16; 512]);
impl Vocab {
    fn new() -> Self {
        let mut offsets = [0; 512];
        let mut start = 4;
        for at in &mut offsets {
            *at = u16::try_from(start).unwrap();
            let size = u32::from_le_bytes(VOCAB[start+4..start+8].try_into().unwrap()) as usize;
            start += 8 + size;
        }
        assert_eq!(start, VOCAB.len());
        Self(offsets)
    }

    fn piece(&self, id: usize) -> &[u8] {
        let i = self.0[id] as usize;
        let end = self.0.get(id + 1).map_or(VOCAB.len(), |&n| n as usize);
        &VOCAB[i+8..end]
    }

    fn score(&self, id: usize) -> f32 {
        let i = self.0[id] as usize;
        f32::from_le_bytes(VOCAB[i..i+4].try_into().unwrap())
    }

    fn find(&self, left: &[u8], right: &[u8]) -> Option<usize> {
        (0..self.0.len()).find(|&id| self.piece(id).strip_prefix(left) == Some(right))
    }

    fn encode(&self, text: &[u8], tokens: &mut [usize; POSITIONS]) -> Option<usize> {
        if text.is_empty() || text.len() + 4 > tokens.len() ||
            !text.iter().all(|&b| (b' '..=b'~').contains(&b)) { return None; }
        tokens[0] = 1;
        tokens[1] = self.find(b" ", b"")?;
        for (out, &ch) in tokens[2..].iter_mut().zip(text) {
            *out = self.find(&[ch], b"").unwrap_or(usize::from(ch) + 3);
        }
        let mut count = text.len() + 2;
        loop {
            let mut best = None;
            let mut best_score = -1e10;
            for i in 0..count - 1 {
                if let Some(id) = self.find(self.piece(tokens[i]), self.piece(tokens[i + 1])) {
                    let score = self.score(id);
                    if score > best_score {
                        best = Some((i, id));
                        best_score = score;
                    }
                }
            }
            let Some((i, id)) = best else { return Some(count) };
            tokens[i] = id;
            tokens.copy_within(i+2..count, i+1);
            count -= 1;
        }
    }
}

#[no_mangle]
extern "C" fn guest_main(stack: *const usize) -> ! {
    let (offset, size) = unsafe {
        if *stack != 3 { exit(4); }
        let arg = |index| decimal(*stack.add(index) as *const u8).unwrap_or_else(|| exit(4));
        (arg(2), arg(3))
    };
    let mut source = [0u8; POSITIONS - 4];
    if size == 0 || size > source.len() { exit(4); }
    read_excerpt(offset, &mut source[..size]).unwrap_or_else(|code| exit(code));
    let mut tokens = [0usize; POSITIONS];
    let count = Vocab::new().encode(&source[..size], &mut tokens).unwrap_or_else(|| exit(4));
    let text = core::str::from_utf8(&source[..size]).unwrap();
    writeln!(Console, "EXCERPT {text}").unwrap();
    write!(Console, "INPUT").unwrap();
    for id in &tokens[..count] { write!(Console, " {id}").unwrap(); }
    writeln!(Console).unwrap();
    let model = model::Model::load();
    let context = unsafe { &mut *core::ptr::addr_of_mut!(CONTEXT) };
    let [a, b, c] = model.complete(context, &tokens[..count]).unwrap();
    writeln!(Console, "OUTPUT {a} {b} {c}").unwrap();
    exit(0)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { exit(5) }
