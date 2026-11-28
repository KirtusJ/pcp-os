use core::ops::Range;

// Fixed guest memory. A full buffer refuses an edit before removing its selection.
pub const CAP: usize = 32 * 1024;
pub struct Editor {
    pub bytes: [u8; CAP],
    pub len: usize,
    pub caret: usize,
    pub anchor: usize,
    preferred_col: Option<usize>,
}

impl Editor {
    pub const fn new() -> Self {
        Self {
            bytes: [0; CAP],
            len: 0,
            caret: 0,
            anchor: 0,
            preferred_col: None,
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
        self.preferred_col = None;
        self.caret = position.min(self.len);
        if !selecting {
            self.anchor = self.caret;
        }
    }
    pub fn all(&mut self) {
        self.select(0, self.len);
    }
    pub fn select(&mut self, anchor: usize, caret: usize) {
        self.preferred_col = None;
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
    // One position per character boundary, including the insertion point at EOF.
    // Defer soft wrapping until another printable cell actually needs the row.
    pub fn cells(&self, cols: usize) -> impl Iterator<Item = Cell> + '_ {
        let mut row = 0;
        let mut col = 0;
        self.text()
            .char_indices()
            .map(|(i, ch)| (i, Some(ch)))
            .chain(core::iter::once((self.len, None)))
            .map(move |(byte, ch)| {
                if col == cols && ch.is_some_and(|c| c != '\n') {
                    row += 1;
                    col = 0;
                }
                let cell = Cell { byte, ch, row, col };
                match ch {
                    Some('\n') => {
                        row += 1;
                        col = 0;
                    }
                    Some(c) => col = (col + cell_width(c, col)).min(cols),
                    None => {}
                }
                cell
            })
    }
    pub fn caret_cell(&self, cols: usize) -> Cell {
        self.cells(cols)
            .find(|cell| cell.byte == self.caret)
            .unwrap()
    }
    fn position_on_row(&self, row: usize, col: usize, cols: usize) -> usize {
        let mut pos = self.len;
        for cell in self
            .cells(cols)
            .skip_while(|cell| cell.row < row)
            .take_while(|cell| cell.row == row)
        {
            pos = cell.byte;
            if cell.col >= col {
                break;
            }
        }
        pos
    }
    pub fn move_row(&mut self, down: bool, cols: usize, selecting: bool) {
        let at = self.caret_cell(cols);
        let target = self.preferred_col.unwrap_or(at.col);
        let row = if down {
            at.row + 1
        } else {
            at.row.saturating_sub(1)
        };
        self.move_to(self.position_on_row(row, target, cols), selecting);
        self.preferred_col = Some(target);
    }
    pub fn line_edge(&mut self, end: bool, cols: usize, selecting: bool) {
        let row = self.caret_cell(cols).row;
        self.move_to(
            self.position_on_row(row, if end { cols } else { 0 }, cols),
            selecting,
        );
    }
}

pub struct Cell {
    pub byte: usize,
    pub ch: Option<char>,
    pub row: usize,
    pub col: usize,
}

pub fn cell_width(ch: char, column: usize) -> usize {
    if ch == '\t' {
        4 - column % 4
    } else {
        1
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
        e.preferred_col = None;
        true
    }
}
