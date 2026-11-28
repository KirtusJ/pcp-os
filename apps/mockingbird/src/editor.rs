use core::ops::Range;

// Fixed guest memory. A full buffer refuses an edit before removing its selection.
pub const CAP: usize = 32 * 1024;
pub struct Editor {
    pub bytes: [u8; CAP],
    pub len: usize,
    pub caret: usize,
    pub anchor: usize,
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
    pub fn select(&mut self, anchor: usize, caret: usize) {
        self.anchor = anchor;
        self.caret = caret;
    }
    fn deletion(&self, back: bool) -> Range<usize> {
        let selection = self.selection();
        if !selection.is_empty() {
            return selection;
        }
        if back {
            self.previous()..self.caret
        } else {
            self.caret..self.next()
        }
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

pub fn cell_width(ch: char, column: usize) -> usize {
    if ch == '\t' {
        4 - column % 4
    } else {
        1
    }
}

pub fn advance(at: &mut (usize, usize), byte: char, cols: usize) {
    let width = cell_width(byte, at.1);
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

// Store only the used prefix when taking a checkpoint. A circular history keeps
// the cost bounded even for the largest paste. Undo swaps in the previous
// document and selection; that same slot then holds the corresponding redo.
const STEPS: usize = 12;
pub struct History {
    slots: [Editor; STEPS],
    first: usize,
    count: usize,
    applied: usize,
    typing_until: Option<u64>,
}
impl History {
    pub const fn new() -> Self {
        Self {
            slots: [const { Editor::new() }; STEPS],
            first: 0,
            count: 0,
            applied: 0,
            typing_until: None,
        }
    }
    pub fn break_typing(&mut self) {
        self.typing_until = None;
    }
    fn checkpoint(&mut self, e: &Editor) {
        self.count = self.applied;
        if self.count == STEPS {
            self.first = (self.first + 1) % STEPS;
            self.count -= 1;
        }
        let slot = &mut self.slots[(self.first + self.count) % STEPS];
        slot.bytes[..e.len].copy_from_slice(&e.bytes[..e.len]);
        slot.len = e.len;
        slot.caret = e.caret;
        slot.anchor = e.anchor;
        self.count += 1;
        self.applied = self.count;
    }
    pub fn edit(
        &mut self,
        e: &mut Editor,
        range: Range<usize>,
        bytes: &[u8],
        typing: Option<u64>,
    ) -> bool {
        if core::str::from_utf8(bytes).is_err() || bytes.len() > CAP - (e.len - range.len()) {
            return false;
        }
        if e.bytes[range.clone()] == *bytes {
            e.move_to(range.end, false);
            self.break_typing();
            return true;
        }
        let grouped = range.is_empty()
            && typing
                .zip(self.typing_until)
                .is_some_and(|(now, end)| now <= end);
        if !grouped {
            self.checkpoint(e);
        }
        self.typing_until = typing.map(|t| t + 1_200_000);
        let end = range.start + bytes.len();
        e.bytes.copy_within(range.end..e.len, end);
        e.bytes[range.start..end].copy_from_slice(bytes);
        e.len = e.len - range.len() + bytes.len();
        e.move_to(end, false);
        true
    }
    pub fn erase(&mut self, e: &mut Editor, back: bool) {
        self.edit(e, e.deletion(back), &[], None);
    }
    pub fn step(&mut self, e: &mut Editor, redo: bool) -> bool {
        self.break_typing();
        let index = if redo {
            if self.applied == self.count {
                return false;
            }
            let i = self.applied;
            self.applied += 1;
            i
        } else {
            if self.applied == 0 {
                return false;
            }
            self.applied -= 1;
            self.applied
        };
        let slot = &mut self.slots[(self.first + index) % STEPS];
        let n = e.len.max(slot.len);
        e.bytes[..n].swap_with_slice(&mut slot.bytes[..n]);
        core::mem::swap(&mut e.len, &mut slot.len);
        core::mem::swap(&mut e.caret, &mut slot.caret);
        core::mem::swap(&mut e.anchor, &mut slot.anchor);
        true
    }
}
