//! Session-only written response, using the tokenizer and decoder loop from guest trial #205.
use super::{model, Console};
use core::fmt::Write;

const POSITIONS: usize = 160;
const VOCAB: &[u8] = include_bytes!(env!("PCPOS_TOKENIZER"));
static mut CONTEXT: model::Context<POSITIONS> = model::Context::new();

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

pub fn respond(node: usize, model: &model::Model) {
    let level = match node {
        1 => 0,
        2..=4 => 1,
        5..=12 => 2,
        13..=24 => 3,
        _ => unreachable!(),
    };
    let text = include_str!("prompts.txt").lines().nth(level).unwrap();
    let mut tokens = [0usize; POSITIONS];
    let count = Vocab::new().encode(text.as_bytes(), &mut tokens).unwrap();
    writeln!(Console, "EXCERPT {text}").unwrap();
    write!(Console, "INPUT").unwrap();
    for id in &tokens[..count] { write!(Console, " {id}").unwrap(); }
    writeln!(Console).unwrap();
    let context = unsafe { &mut *core::ptr::addr_of_mut!(CONTEXT) };
    let [a, b, c] = model.complete(context, &tokens[..count], None).unwrap();
    write!(Console, "OUTPUT").unwrap();
    let mut next = a;
    let mut stop = "CAP";
    for i in 0..48 {
        write!(Console, " {next}").unwrap();
        if next == 2 { stop = "EOS"; break; }
        if i < 47 {
            next = match i {
                0 => b,
                1 => c,
                _ => model.continue_token(context, next, count + i),
            };
        }
    }
    writeln!(Console).unwrap();
    writeln!(Console, "STOP {stop}").unwrap();
}
