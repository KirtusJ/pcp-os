//! The channel between a program and its companion: two bounded queues of bytes, one for each direction.
//! The kernel carries the bytes and never reads them. No `unsafe`, no device, and nothing here decides
//! who holds an end: that is the process table's.
#[cfg(test)]
extern crate alloc;

use alloc::collections::VecDeque;

/// The most bytes that wait in one direction. A writer that finds no room is told so, or waits.
pub const ROOM: usize = 64 * 1024;

pub struct Channel {
    /// `waiting[e]` holds what was written for end `e` and not yet read by it.
    waiting: [VecDeque<u8>; 2],
    /// Whether the program holding each end is still there.
    here: [bool; 2],
}

impl Channel {
    /// Both queues with their whole room set aside now, so that carrying bytes never asks for memory
    /// later. `None` if the kernel has not that much.
    pub fn new() -> Option<Self> {
        let (mut a, mut b) = (VecDeque::new(), VecDeque::new());
        a.try_reserve_exact(ROOM).ok()?;
        b.try_reserve_exact(ROOM).ok()?;
        Some(Self { waiting: [a, b], here: [true, true] })
    }

    /// Whether end `end` (0 or 1) has anything to read.
    pub fn readable(&self, end: usize) -> bool {
        !self.waiting[end].is_empty()
    }

    /// How many bytes end `end` may write now.
    pub fn room(&self, end: usize) -> usize {
        ROOM - self.waiting[1 - end].len()
    }

    /// Whether the other end's program is still there.
    pub fn peer(&self, end: usize) -> bool {
        self.here[1 - end]
    }

    /// The first `most` bytes waiting for this end, without taking them.
    pub fn peek(&self, end: usize, most: usize) -> impl Iterator<Item = u8> + '_ {
        self.waiting[end].iter().take(most).copied()
    }

    /// Take up to `most` bytes that were written for this end, in the order they were written.
    pub fn read(&mut self, end: usize, most: usize, mut into: impl FnMut(u8)) -> usize {
        let count = most.min(self.waiting[end].len());
        for byte in self.waiting[end].drain(..count) {
            into(byte);
        }
        count
    }

    /// Queue as many of these bytes as there is room for, for the other end. Returns how many.
    pub fn write(&mut self, end: usize, bytes: &[u8]) -> usize {
        let count = bytes.len().min(self.room(end));
        self.waiting[1 - end].extend(&bytes[..count]);
        count
    }

    /// The program at this end is gone. What it wrote can still be read by the other; what was written
    /// for it never will be, and is dropped.
    pub fn leave(&mut self, end: usize) {
        self.here[end] = false;
        self.waiting[end].clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn take(c: &mut Channel, end: usize, most: usize) -> Vec<u8> {
        let mut out = Vec::new();
        c.read(end, most, |b| out.push(b));
        out
    }

    #[test]
    fn bytes_arrive_in_order_each_way_and_never_at_the_wrong_end() {
        let mut c = Channel::new().unwrap();
        assert!(!c.readable(0) && !c.readable(1) && c.room(0) == ROOM && c.room(1) == ROOM);
        assert_eq!(c.write(0, b"to one"), 6);
        assert_eq!(c.write(1, b"to zero"), 7);
        // A reader sees only what was written for it, in order, across short reads.
        assert_eq!(take(&mut c, 1, 2), b"to");
        assert_eq!(take(&mut c, 1, 100), b" one");
        assert_eq!(take(&mut c, 0, 100), b"to zero");
        assert!(take(&mut c, 0, 100).is_empty() && take(&mut c, 1, 100).is_empty());
    }

    #[test]
    fn a_full_queue_takes_no_more_and_loses_nothing() {
        let mut c = Channel::new().unwrap();
        let big: Vec<u8> = (0..ROOM + 10).map(|i| i as u8).collect();
        assert_eq!(c.write(0, &big), ROOM);
        assert_eq!((c.room(0), c.write(0, b"x")), (0, 0));
        // The other direction is not affected.
        assert_eq!(c.write(1, b"y"), 1);
        assert_eq!(take(&mut c, 1, 10), &big[..10]);
        assert_eq!(c.write(0, &big[ROOM..]), 10);
        assert_eq!(take(&mut c, 1, ROOM + 10), &big[10..]);
    }

    #[test]
    fn when_an_end_is_gone_its_words_can_still_be_read_and_words_for_it_are_dropped() {
        let mut c = Channel::new().unwrap();
        c.write(0, b"last words");
        c.write(1, b"never read");
        c.leave(0);
        assert!(!c.peer(1) && c.peer(0));
        assert_eq!(take(&mut c, 1, 100), b"last words");
        assert!(!c.readable(0) && !c.readable(1));
    }
}
