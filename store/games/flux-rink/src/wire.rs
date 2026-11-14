//! Bounded resynchronizing wire format for a private point-to-point serial stream.
use crate::Match;
pub const SIZE: usize = 16;
pub type Packet = [u8; SIZE];
pub fn newer(a: u8, b: u8) -> bool {
    let d = a.wrapping_sub(b);
    d != 0 && d < 128
}
fn sum(b: &[u8]) -> u16 {
    let (mut a, mut c) = (0u16, 0u16);
    for &v in b {
        a = (a + v as u16) % 255;
        c = (c + a) % 255;
    }
    c << 8 | a
}
fn header(kind: u8, seq: u16) -> Packet {
    let mut p = [0; SIZE];
    p[..4].copy_from_slice(&[0xf1, 0x7e, 0x10 | kind, seq as u8]);

    p
}
fn seal(mut p: Packet) -> Packet {
    let c = sum(&p[..14]);
    p[14..].copy_from_slice(&c.to_le_bytes());
    p
}
pub fn input(seq: u16, buttons: u8) -> Packet {
    let mut p = header(1, seq);
    p[4] = buttons & 15;
    seal(p)
}
// Six signed positions quantized in four-unit steps fit in 60 bits. A complete
// snapshot plus framing fits a single 16-byte legacy UART FIFO.
pub fn snapshot(seq: u16, m: &Match) -> Packet {
    let mut p = header(2, seq);
    let mut packed = 0u64;
    for (i, b) in [m.player[0], m.player[1], m.puck].iter().enumerate() {
        for (j, v) in [b.x, b.z].iter().enumerate() {
            let q = ((*v).clamp(-2048, 2044).div_euclid(4) + 512) as u64;
            packed |= q << (i * 20 + j * 10);
        }
    }
    p[4..12].copy_from_slice(&packed.to_le_bytes());
    p[12] = m.score[0] | (m.score[1] << 4);
    p[13] = m.serve as u8;
    seal(p)
}
pub fn kind(p: &Packet) -> u8 {
    p[2] & 15
}
pub fn sequence(p: &Packet) -> u8 {
    p[3]
}
pub fn apply(p: &Packet, m: &mut Match) -> bool {
    let score = [p[12] & 15, p[12] >> 4];
    if kind(p) != 2 || score.iter().any(|&n| n > 5) || p[13] > 37 {
        return false;
    }
    let bits = u64::from_le_bytes(p[4..12].try_into().unwrap());
    if bits >> 60 != 0 {
        return false;
    }
    let mut copy = *m;
    for i in 0..3 {
        let coord = |shift: usize| (((bits >> shift) & 1023) as i32 - 512) * 4;
        let (x, z) = (coord(i * 20), coord(i * 20 + 10));
        if x.abs() > 1600 || z.abs() > 2048 {
            return false;
        }
        let b = if i == 2 {
            &mut copy.puck
        } else {
            &mut copy.player[i]
        };
        b.x = x;
        b.z = z;
    }
    copy.score = score;
    copy.serve = p[13] as u16;
    *m = copy;
    true
}
#[derive(Default)]
pub struct Decoder {
    data: Packet,
    at: usize,
}
impl Decoder {
    pub fn byte(&mut self, b: u8) -> Option<Packet> {
        if self.at == 0 && b != 0xf1 {
            return None;
        }
        if self.at == 1 && b != 0x7e {
            self.at = usize::from(b == 0xf1);
            return None;
        }
        self.data[self.at] = b;
        self.at += 1;
        if self.at < SIZE {
            return None;
        }
        let p = self.data;
        self.at = 0;
        if matches!(p[2], 0x11 | 0x12) && sum(&p[..14]) == u16::from_le_bytes([p[14], p[15]]) {
            return Some(p);
        }
        if let Some(pos) = p[1..].windows(2).position(|v| v == [0xf1, 0x7e]) {
            let skip = pos + 1;
            self.data.copy_within(skip.., 0);
            self.at = SIZE - skip;
        } else if p[SIZE - 1] == 0xf1 {
            self.data[0] = 0xf1;
            self.at = 1;
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corruption_fragment_and_wrap() {
        let a = input(65535, 9);
        let mut d = Decoder::default();
        let mut bad = a;
        bad[7] ^= 128;
        for &b in &bad {
            assert!(d.byte(b).is_none());
        }
        for &b in &a[..12] {
            assert!(d.byte(b).is_none());
        }
        let mut found = None;
        for &b in &a[12..] {
            found = d.byte(b);
        }
        assert_eq!(found, Some(a));
        assert!(newer(0, 255));
        assert!(!newer(0, 0));
        assert!(!newer(255, 0));
    }
    #[test]
    fn valid_world_and_invalid_pose() {
        let m = Match::default();
        let mut p = snapshot(8, &m);
        let mut dst = m;
        dst.score = [2, 1];
        assert!(apply(&p, &mut dst));
        assert_eq!(dst, m);
        p[11] |= 0xf0;
        assert!(!apply(&p, &mut dst));
        assert_eq!(dst, m);
    }
}
