use flux_rink::{
    wire::{self, Decoder, Packet, SIZE},
    Match,
};
use pcp_game_guest::{call, log, open};
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    Local,
    Host,
    Peer,
}
pub struct Link {
    fd: isize,
    pub role: Role,
    decoder: Decoder,
    tx: Packet,
    at: usize,
    seq: u16,
    last: Option<u8>,
    pub remote: u8,
    pub age: u32,
    pub heard: bool,
    pub target: Match,
    pub prior: Match,
    pub since: u32,
}
impl Link {
    pub fn new() -> Self {
        Self {
            fd: -1,
            role: Role::Local,
            decoder: Decoder::default(),
            tx: [0; SIZE],
            at: SIZE,
            seq: 0,
            last: None,
            remote: 0,
            age: 0,
            heard: false,
            target: Match::default(),
            prior: Match::default(),
            since: 0,
        }
    }
    pub fn start(&mut self, r: Role) -> bool {
        self.stop();
        let f = open(b"/dev/game-link\0", 2 | 0x800);
        if f < 0 {
            return false;
        }
        self.fd = f;
        self.role = r;
        true
    }
    pub fn stop(&mut self) {
        if self.fd >= 0 {
            call(3, self.fd as usize, 0, 0, 0, 0, 0);
        }
        *self = Self::new();
    }
    pub fn poll(&mut self) {
        if self.fd < 0 {
            return;
        }
        let mut b = [0u8; 64];
        for _ in 0..3 {
            let n = call(
                0,
                self.fd as usize,
                b.as_mut_ptr() as usize,
                b.len(),
                0,
                0,
                0,
            );
            if n <= 0 {
                break;
            }
            for &v in &b[..n as usize] {
                if let Some(p) = self.decoder.byte(v) {
                    let seq = wire::sequence(&p);
                    if self.last.is_some_and(|old| !wire::newer(seq, old)) {
                        continue;
                    }
                    if self.role == Role::Host && wire::kind(&p) == 1 && p[4] <= 15 {
                        if self.remote != p[4] && p[4] != 0 {
                            log(b"RINK remote input\n");
                        }
                        self.remote = p[4];
                    } else if self.role == Role::Peer && wire::kind(&p) == 2 {
                        let mut next = self.target;
                        if !wire::apply(&p, &mut next) {
                            continue;
                        }
                        self.prior = if self.heard { self.target } else { next };
                        if self.target.player[1] != next.player[1] && !self.heard_motion() {
                            log(b"RINK peer motion\n");
                        }
                        self.target = next;
                        self.since = 0;
                    } else {
                        continue;
                    }
                    if !self.heard {
                        log(b"RINK peer connected\n");
                    }
                    self.heard = true;
                    self.age = 0;
                    self.last = Some(seq);
                }
            }
        }
        self.flush();
    }
    fn heard_motion(&self) -> bool {
        self.target.player[1].x != 0 || self.target.player[1].z != 1040
    }
    fn flush(&mut self) {
        if self.at < SIZE {
            let part = &self.tx[self.at..];
            let n = call(
                1,
                self.fd as usize,
                part.as_ptr() as usize,
                part.len(),
                0,
                0,
                0,
            );
            if n > 0 {
                self.at += n as usize;
            }
        }
    }
    pub fn tick(&mut self, m: &Match, buttons: u8, tick: u32) {
        if self.fd < 0 {
            return;
        }
        self.age = self.age.saturating_add(1);
        self.since = self.since.saturating_add(1);
        if self.age > 24 {
            self.remote = 0;
        }
        if self.age > 120 {
            self.last = None;
            self.heard = false;
        }
        if tick % 3 == 0 && self.at == SIZE {
            self.seq = self.seq.wrapping_add(1);
            self.tx = if self.role == Role::Host {
                wire::snapshot(self.seq, m)
            } else {
                wire::input(self.seq, buttons)
            };
            self.at = 0;
        }
        self.flush();
    }
    pub fn view(&self) -> Match {
        let mut m = self.target;
        let a = self.since.min(3) as i32;
        if self.prior.score != m.score {
            return m;
        }
        for (src, dst) in self
            .prior
            .player
            .iter()
            .chain(core::iter::once(&self.prior.puck))
            .zip(m.player.iter_mut().chain(core::iter::once(&mut m.puck)))
        {
            dst.x = src.x + (dst.x - src.x) * a / 3;
            dst.z = src.z + (dst.z - src.z) * a / 3;
        }
        m
    }
}
