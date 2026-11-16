//! In-session navigation history for the fixed-origin reader.
use crate::{layout::View, location::Location, CAPACITY};

const ENTRIES: usize = 8;

#[derive(Clone, Copy)]
pub struct Visit {
    pub location: Location,
    pub view: View,
    pub body: [u8; CAPACITY],
    pub length: usize,
}

impl Visit {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            view: View::default(),
            body: [0; CAPACITY],
            length: 0,
        }
    }
}

pub struct History {
    visits: [Visit; ENTRIES],
    oldest: usize,
    // Both cursor and last are offsets from oldest.
    cursor: usize,
    last: usize,
}

impl History {
    pub fn new(first: Visit) -> Self {
        Self {
            visits: [first; ENTRIES],
            oldest: 0,
            cursor: 0,
            last: 0,
        }
    }

    pub fn current(&mut self) -> &mut Visit {
        &mut self.visits[(self.oldest + self.cursor) % ENTRIES]
    }

    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_forward(&self) -> bool {
        self.cursor < self.last
    }

    pub fn back(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn forward(&mut self) {
        self.cursor = (self.cursor + 1).min(self.last);
    }

    pub fn push(&mut self, visit: Visit) {
        if self.cursor == ENTRIES - 1 {
            self.oldest = (self.oldest + 1) % ENTRIES;
        } else {
            self.cursor += 1;
        }
        self.last = self.cursor;
        *self.current() = visit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visit(number: u8) -> Visit {
        let mut v = Visit::new(Location::start());
        v.body[0] = number;
        v.length = 1;
        v
    }

    #[test]
    fn empty_directions_wraparound_and_branching_use_logical_order() {
        let mut h = History::new(visit(0));
        h.back();
        h.forward();
        assert_eq!(h.current().body[0], 0);
        for n in 1..=20 {
            h.push(visit(n));
        }
        assert!(!h.can_forward());
        for n in (13..=20).rev() {
            assert_eq!(h.current().body[0], n);
            h.back();
        }
        assert!(!h.can_back());
        assert_eq!(h.current().body[0], 13);
        h.forward();
        h.current().view = View { row: 3, focus: Some(1) };
        h.forward();
        h.back();
        assert_eq!(h.current().view, View { row: 3, focus: Some(1) });
        h.push(visit(42));
        assert!(!h.can_forward());
        h.forward();
        assert_eq!(h.current().body[0], 42);
        h.back();
        assert_eq!(h.current().body[0], 14);
        h.back();
        assert_eq!(h.current().body[0], 13);
    }
}
