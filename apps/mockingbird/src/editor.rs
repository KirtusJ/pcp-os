use core::ops::Range;

// Fixed guest memory. A full buffer refuses an edit before removing its selection.
pub const CAP: usize = 32 * 1024;
pub struct Editor {
    pub bytes: [u8; CAP],
    pub len: usize,
    pub caret: usize,
    anchor: usize,
}

impl Editor {
    pub const fn new() -> Self {
        Self {
            bytes: [0; CAP],
            len: 0,
            caret: 0,
            anchor: 0,
        }
    }
    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }
    fn previous(&self) -> usize {
        self.text()[..self.caret]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn next(&self) -> usize {
        self.caret
            + self.text()[self.caret..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)
    }
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
    pub fn move_to(&mut self, position: usize, selecting: bool) {
        self.caret = position.min(self.len);
        if !selecting {
            self.anchor = self.caret;
        }
    }
    pub fn all(&mut self) {
        self.anchor = 0;
        self.caret = self.len;
    }
    pub fn replace(&mut self, text: &[u8]) -> bool {
        let range = self.selection();
        let kept = self.len - range.len();
        if core::str::from_utf8(text).is_err() || text.len() > CAP - kept {
            return false;
        }
        let end = range.start + text.len();
        self.bytes.copy_within(range.end..self.len, end);
        self.bytes[range.start..end].copy_from_slice(text);
        self.len = kept + text.len();
        self.move_to(end, false);
        true
    }
    pub fn erase(&mut self, back: bool) {
        if self.selection().is_empty() {
            self.move_to(if back { self.previous() } else { self.next() }, true);
        }
        self.replace(&[]);
    }
    pub fn horizontal(&mut self, left: bool, selecting: bool) {
        let range = self.selection();
        let to = if !selecting && !range.is_empty() {
            if left {
                range.start
            } else {
                range.end
            }
        } else if left {
            self.previous()
        } else {
            self.next()
        };
        self.move_to(to, selecting);
    }
    pub fn row_col(&self, pos: usize, cols: usize) -> (usize, usize) {
        let mut at = (0, 0);
        for b in self.text()[..pos].chars() {
            advance(&mut at, b, cols);
        }
        at
    }
    pub fn vertical(&mut self, row: usize, col: usize, cols: usize, selecting: bool) {
        let mut at = (0, 0);
        let mut pos = 0;
        for (i, b) in self.text().char_indices() {
            if at.0 > row {
                break;
            }
            pos = i;
            if at.0 == row && (at.1 >= col || b == '\n') {
                break;
            }
            let mut next = at;
            advance(&mut next, b, cols);
            if at.0 == row && next.0 > row {
                break;
            }
            at = next;
            pos = i + b.len_utf8();
        }
        self.move_to(pos, selecting);
    }
}

pub fn advance(at: &mut (usize, usize), byte: char, cols: usize) {
    let width = if byte == '\t' { 4 - at.1 % 4 } else { 1 };
    if byte == '\n' || at.1 + width >= cols {
        at.0 += 1;
        at.1 = 0;
    } else {
        at.1 += width;
    }
}

pub fn printable(k: usize, shift: bool, caps: bool) -> Option<u8> {
    let (plain, upper): (&[u8], &[u8]) = match k {
        2..=13 => (b"1234567890-=", b"!@#$%^&*()_+"),
        16..=27 => (b"qwertyuiop[]", b"QWERTYUIOP{}"),
        30..=41 => (b"asdfghjkl;'\x60", b"ASDFGHJKL:\"\x7e"),
        43 => return Some(if shift { b'|' } else { b'\\' }),
        44..=53 => (b"zxcvbnm,./", b"ZXCVBNM<>?"),
        57 => return Some(b' '),
        _ => return None,
    };
    let base = match k {
        2..=13 => 2,
        16..=27 => 16,
        30..=41 => 30,
        _ => 44,
    };
    let i = k - base;
    Some(if shift ^ (caps && plain[i].is_ascii_alphabetic()) {
        upper[i]
    } else {
        plain[i]
    })
}
