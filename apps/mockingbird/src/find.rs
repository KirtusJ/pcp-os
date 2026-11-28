use crate::editor::Editor;

pub struct Find {
    bytes: [u8; 256],
    len: usize,
    pub open: bool,
    pub select_query: bool,
    origin_anchor: usize,
    origin_caret: usize,
    pub message: &'static str,
}
impl Find {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 256],
            len: 0,
            open: false,
            select_query: false,
            origin_anchor: 0,
            origin_caret: 0,
            message: "",
        }
    }
    pub fn query(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }
    pub fn begin(&mut self, e: &Editor) {
        self.origin_anchor = e.anchor;
        self.origin_caret = e.caret;
        let selected = &e.bytes[e.selection()];
        if !selected.is_empty() && selected.len() <= self.bytes.len() && !selected.contains(&b'\n')
        {
            self.bytes[..selected.len()].copy_from_slice(selected);
            self.len = selected.len();
        }
        self.open = true;
        self.select_query = true;
        self.message = "";
    }
    pub fn insert(&mut self, e: &mut Editor, bytes: &[u8]) {
        let start = if self.select_query { 0 } else { self.len };
        if bytes.len() > self.bytes.len() - start
            || core::str::from_utf8(bytes).is_err()
            || bytes.iter().any(|b| matches!(*b, b'\r' | b'\n' | 0))
        {
            self.message = "Use one line, up to 256 bytes";
            return;
        }
        self.bytes[start..start + bytes.len()].copy_from_slice(bytes);
        self.len = start + bytes.len();
        self.select_query = false;
        self.locate(e, self.origin_anchor.min(self.origin_caret), false);
    }
    pub fn backspace(&mut self, e: &mut Editor) {
        self.len = if self.select_query {
            0
        } else {
            self.query()
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i)
        };
        self.select_query = false;
        self.locate(e, self.origin_anchor.min(self.origin_caret), false);
    }
    pub fn repeat(&mut self, e: &mut Editor, previous: bool) {
        // Between searches the document can shrink or change UTF-8 boundaries.
        self.origin_anchor = e.anchor;
        self.origin_caret = e.caret;
        self.next(e, previous);
    }
    pub fn next(&mut self, e: &mut Editor, previous: bool) {
        self.select_query = false;
        let range = e.selection();
        self.locate(e, if previous { range.start } else { range.end }, previous);
    }
    fn locate(&mut self, e: &mut Editor, from: usize, previous: bool) {
        self.message = "";
        if self.len == 0 {
            e.select(self.origin_anchor, self.origin_caret);
            return;
        }
        let query = self.query();
        let text = e.text();
        let found = if previous {
            text[..from].rfind(query).or_else(|| text.rfind(query))
        } else {
            text[from..]
                .find(query)
                .map(|i| from + i)
                .or_else(|| text.find(query))
        };
        if let Some(i) = found {
            e.select(i, i + self.len);
        } else {
            e.select(self.origin_anchor, self.origin_caret);
            self.message = "No match";
        }
    }
}
