use alloc::{string::String, vec::Vec};
use pcp_game_guest::{call, load, open, save};
use recursion_desk_core::Desk;

pub const MAX_DEPTH: u16 = 2;
pub const MAX_LIVE: u16 = 4;
pub const MAX_TOTAL: u32 = 32;
pub const MAX_LABEL: usize = 30;
const MAX_EVENTS: usize = MAX_TOTAL as usize * 2;
const RECORD: usize = 2 + MAX_LABEL;
const SIZE: usize = 12 + MAX_EVENTS * RECORD + 4;
const PATH: &[u8] = b"parrots.queue\0";
const TEMP: &[u8] = b"parrots.tmp\0";

pub struct Journal {
    pub desk: Desk,
    events: Vec<[u8; RECORD]>,
    locked: bool,
}

fn checksum(b: &[u8]) -> u32 {
    b.iter().fold(2166136261, |n, &v| (n ^ u32::from(v)).wrapping_mul(16777619))
}

pub fn label_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'.' | b'/')
}

impl Journal {
    fn empty() -> Self {
        Self { desk: Desk::new(MAX_DEPTH, MAX_LIVE, MAX_TOTAL).unwrap(), events: Vec::new(), locked: false }
    }

    pub fn locked(&self) -> bool { self.locked }

    fn unavailable(reason: &'static str) -> (Self, &'static str) {
        (Self { locked: true, ..Self::empty() }, reason)
    }

    // Only successful core transitions are recorded. Reopening runs them again
    // against the same bounds and derives the whole tree, including closed nodes.
    fn apply(&mut self, record: [u8; RECORD]) -> Result<u32, String> {
        if self.locked { return Err("queue recovery required".into()); }
        let arg = u32::from(record[1]);
        let id = match record[0] {
            1 => {
                let payload = &record[2..];
                let len = payload.iter().position(|&b| b == 0).unwrap_or(payload.len());
                if !payload[..len].iter().copied().all(label_char) || payload[len..].iter().any(|&b| b != 0) {
                    return Err("Invalid saved label".into());
                }
                let label = core::str::from_utf8(&payload[..len]).map_err(|_| "Invalid saved label")?;
                self.desk.open((arg != 0).then_some(arg), label)?
            }
            action @ (2 | 3) if record[2..].iter().all(|&b| b == 0) => {
                if action == 2 { self.desk.finish(arg)?; } else { self.desk.cancel(arg)?; }
                arg
            }
            _ => return Err("Invalid event".into()),
        };
        self.events.push(record);
        Ok(id)
    }

    pub fn add(&mut self, parent: Option<u32>, label: &str) -> Result<u32, String> {
        let mut event = [0; RECORD];
        if label.len() > MAX_LABEL { return Err("Label is too long".into()); }
        event[0] = 1;
        event[1] = u8::try_from(parent.unwrap_or(0)).map_err(|_| "Unknown parent")?;
        event[2..2+label.len()].copy_from_slice(label.as_bytes());
        self.apply(event)
    }

    pub fn close(&mut self, id: u32, cancel: bool) -> Result<u32, String> {
        let mut event = [0; RECORD];
        event[0] = if cancel { 3 } else { 2 };
        event[1] = u8::try_from(id).map_err(|_| "Unknown request")?;
        self.apply(event)
    }

    pub fn read() -> (Self, &'static str) {
        let f = open(PATH, 0);
        if f == -2 { return (Self::empty(), "New queue"); }
        if f < 0 { return Self::unavailable("Store read failed"); }
        call(3, f as usize, 0, 0, 0, 0, 0);
        let mut b = [0; SIZE];
        if load(PATH, &mut b) && &b[..8] == b"PARROTS1" && b[9..12] == [0; 3] &&
            b[SIZE-4..] == checksum(&b[..SIZE-4]).to_le_bytes() {
            let count = b[8] as usize;
            if count <= MAX_EVENTS && b[12+count*RECORD..SIZE-4].iter().all(|&v| v == 0) {
                let mut journal = Self::empty();
                if b[12..12+count*RECORD].chunks_exact(RECORD).all(|r| journal.apply(r.try_into().unwrap()).is_ok()) {
                    return (journal, "Queue restored");
                }
            }
        }
        Self::unavailable("Queue unreadable")
    }

    pub fn write(&self) -> &'static str {
        if self.locked { return "Write blocked"; }
        // At most one open and one close for each of MAX_TOTAL nodes.
        if self.events.len() > MAX_EVENTS { return "Save failed"; }
        let mut b = [0; SIZE];
        b[..8].copy_from_slice(b"PARROTS1");
        b[8] = self.events.len() as u8;
        for (to, event) in b[12..SIZE-4].chunks_exact_mut(RECORD).zip(&self.events) {
            to.copy_from_slice(event);
        }
        let sum = checksum(&b[..SIZE-4]).to_le_bytes();
        b[SIZE-4..].copy_from_slice(&sum);
        if !save(TEMP, PATH, &b) { return "Save failed"; }
        let dir = open(b"/\0", 0);
        if dir < 0 { return "Session save"; }
        let result = call(74, dir as usize, 0, 0, 0, 0, 0);
        call(3, dir as usize, 0, 0, 0, 0, 0);
        match result {
            0 => "Disk save",
            -19 | -38 => "Session save",
            _ => "Sync failed",
        }
    }
}
