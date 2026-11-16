use keyrook_core::Memory;
use pcp_game_guest::{call, load, open, save};

const PATH: &[u8] = b"keyrook.mem\0";
const TEMP: &[u8] = b"keyrook.tmp\0";
const SIZE: usize = 16 + 12 * 32 + 4;
pub const WIDTHS: [u8; 3] = [4, 6, 8];

#[derive(Default)]
pub struct Profile {
    pub width: usize,
    pub depth: u8,
    pub seed: u32,
    pub cells: [Memory; 12],
}

impl Profile {
    pub fn index(&self) -> usize {
        self.width * 4 + self.depth as usize
    }

    pub fn next_seed(&mut self) {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
    }

    pub fn read() -> Option<Self> {
        let mut b = [0; SIZE];
        if !load(PATH, &mut b) || &b[..8] != b"KEYROOK1" || b[8] > 2 || b[9] > 3 ||
            b[10..12] != [0, 0] || b[SIZE-4..] != checksum(&b[..SIZE-4]).to_le_bytes() {
            return None;
        }
        let mut p = Self {
            width: b[8] as usize, depth: b[9], seed: word(&b[12..16]),
            ..Self::default()
        };
        for (m, bytes) in p.cells.iter_mut().zip(b[16..SIZE-4].chunks_exact(32)) {
            for i in 0..2 {
                let r = &bytes[16*i..16*i+16];
                m.runs[i] = word(&r[..4]);
                m.wins[i] = word(&r[4..8]);
                m.work[i] = u64::from_le_bytes(r[8..16].try_into().unwrap());
                if m.wins[i] > m.runs[i] || (m.runs[i] == 0) != (m.work[i] == 0) {
                    return None;
                }
            }
        }
        Some(p)
    }

    pub fn persist(&self) -> &'static str {
        let mut b = [0; SIZE];
        b[..8].copy_from_slice(b"KEYROOK1");
        b[8] = self.width as u8;
        b[9] = self.depth;
        b[12..16].copy_from_slice(&self.seed.to_le_bytes());
        for (m, bytes) in self.cells.iter().zip(b[16..SIZE-4].chunks_exact_mut(32)) {
            for i in 0..2 {
                let r = &mut bytes[16*i..16*i+16];
                r[..4].copy_from_slice(&m.runs[i].to_le_bytes());
                r[4..8].copy_from_slice(&m.wins[i].to_le_bytes());
                r[8..16].copy_from_slice(&m.work[i].to_le_bytes());
            }
        }
        let sum = checksum(&b[..SIZE-4]).to_le_bytes();
        b[SIZE-4..].copy_from_slice(&sum);
        if !save(TEMP, PATH, &b) { return "SAVE FAILED"; }
        let dir = open(b"/\0", 0);
        if dir < 0 { return "SESSION SAVE"; }
        let sync = call(74, dir as usize, 0, 0, 0, 0, 0);
        call(3, dir as usize, 0, 0, 0, 0, 0);
        match sync {
            0 => "DISK SAVE",
            -19 | -38 => "SESSION SAVE",
            _ => "SYNC FAILED",
        }
    }
}

fn word(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}

fn checksum(b: &[u8]) -> u32 {
    b.iter().fold(2166136261, |n, &v| (n ^ u32::from(v)).wrapping_mul(16777619))
}
