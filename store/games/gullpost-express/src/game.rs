#![cfg_attr(not(test), no_std)]
pub const W: i32 = 512;
pub const H: i32 = 288;
pub const LEFT: u8 = 1;
pub const RIGHT: u8 = 2;
pub const JUMP: u8 = 4;
pub const PAUSE: u8 = 8;
pub const RESET: u8 = 16;
pub const U: i32 = 16;
pub const FLOOR: i32 = 225;
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Cover,
    Run,
    Pause,
    Delivery,
    Home,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sfx {
    Quiet,
    Hop,
    Stamp,
    Bounce,
    Flag,
    Splash,
    Deliver,
    Wing,
}
#[derive(Clone, Copy)]
pub struct Span {
    pub x: i32,
    pub end: i32,
    pub y: i32,
}
const fn s(x: i32, end: i32, y: i32) -> Span {
    Span { x, end, y }
}
pub struct Route {
    pub name: &'static str,
    pub stamp: &'static str,
    pub length: i32,
    pub checkpoint: i32,
    pub spans: &'static [Span],
    pub decks: &'static [Span],
    pub stamps: &'static [(i32, i32)],
    pub crabs: &'static [(i32, i32, i32)],
    pub springs: &'static [i32],
}
pub const ROUTES: [Route; 3] = [
    Route {
        name: "SALTWHARF",
        stamp: "SALT",
        length: 1410,
        checkpoint: 742,
        spans: &[
            s(-80, 282, 225),
            s(337, 590, 225),
            s(648, 865, 225),
            s(921, 1110, 215),
            s(1175, 1540, 225),
        ],
        decks: &[
            s(129, 181, 194),
            s(194, 248, 171),
            s(422, 483, 189),
            s(556, 594, 173),
            s(786, 845, 184),
            s(1020, 1084, 176),
            s(1266, 1325, 188),
        ],
        stamps: &[
            (148, 179),
            (214, 156),
            (303, 185),
            (446, 175),
            (616, 182),
            (724, 199),
            (813, 166),
            (893, 179),
            (1052, 159),
            (1143, 171),
            (1289, 172),
            (1354, 197),
        ],
        crabs: &[(368, 415, 225), (953, 1006, 215), (1190, 1250, 225)],
        springs: &[520],
    },
    Route {
        name: "REEDLIGHT FEN",
        stamp: "REED",
        length: 1580,
        checkpoint: 817,
        spans: &[
            s(-80, 210, 225),
            s(277, 458, 221),
            s(522, 694, 225),
            s(759, 955, 225),
            s(1023, 1197, 217),
            s(1260, 1420, 225),
            s(1482, 1670, 225),
        ],
        decks: &[
            s(98, 150, 188),
            s(335, 399, 180),
            s(601, 663, 185),
            s(860, 912, 181),
            s(1075, 1126, 175),
            s(1310, 1364, 181),
        ],
        stamps: &[
            (125, 172),
            (245, 180),
            (366, 164),
            (492, 179),
            (634, 167),
            (726, 179),
            (825, 197),
            (884, 164),
            (987, 176),
            (1102, 158),
            (1227, 173),
            (1343, 164),
            (1454, 182),
        ],
        crabs: &[
            (295, 326, 221),
            (534, 589, 225),
            (1030, 1064, 217),
            (1373, 1405, 225),
        ],
        springs: &[431, 932],
    },
    Route {
        name: "MOONCAP LIGHT",
        stamp: "MOON",
        length: 1670,
        checkpoint: 874,
        spans: &[
            s(-80, 232, 225),
            s(295, 504, 225),
            s(577, 754, 220),
            s(819, 1038, 225),
            s(1105, 1294, 218),
            s(1367, 1510, 225),
            s(1573, 1770, 225),
        ],
        decks: &[
            s(133, 190, 185),
            s(350, 413, 187),
            s(464, 512, 166),
            s(659, 718, 177),
            s(922, 987, 181),
            s(1137, 1201, 182),
            s(1420, 1473, 179),
        ],
        stamps: &[
            (159, 169),
            (264, 180),
            (379, 168),
            (486, 149),
            (544, 175),
            (686, 160),
            (785, 173),
            (857, 201),
            (949, 162),
            (1071, 178),
            (1164, 166),
            (1330, 173),
            (1447, 162),
            (1543, 179),
        ],
        crabs: &[
            (310, 345, 225),
            (590, 644, 220),
            (834, 875, 225),
            (1218, 1270, 218),
            (1379, 1410, 225),
        ],
        springs: &[439, 1008],
    },
];
pub struct Game {
    pub mode: Mode,
    pub stage: usize,
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub face: i32,
    pub tick: u32,
    pub scroll: i32,
    pub grounded: bool,
    pub wing: bool,
    pub checkpoint: bool,
    pub mask: u32,
    pub dead: u16,
    pub bounces: u8,
    pub attempts: u16,
    pub album: [u8; 3],
    pub notice: u8,
    pub flash: u8,
    pub jump_age: u8,
    pub collected_fx: (i32, i32, u8),
    spring_flight: bool,
    coyote: u8,
    buffer: u8,
    old: u8,
}
impl Game {
    pub fn new() -> Self {
        Self {
            mode: Mode::Cover,
            stage: 0,
            x: 28 * U,
            y: (FLOOR - 8) * U,
            vx: 0,
            vy: 0,
            face: 1,
            tick: 0,
            scroll: 0,
            grounded: true,
            wing: true,
            checkpoint: false,
            mask: 0,
            dead: 0,
            bounces: 0,
            attempts: 0,
            album: [0; 3],
            notice: 0,
            flash: 0,
            jump_age: 0,
            collected_fx: (0, 0, 0),
            spring_flight: false,
            coyote: 4,
            buffer: 0,
            old: 0,
        }
    }
    pub fn route(&self) -> &'static Route {
        &ROUTES[self.stage]
    }
    pub fn foot(&self) -> i32 {
        self.y / U
    }
    pub fn pos(&self) -> i32 {
        self.x / U
    }
    fn place(&mut self) {
        let x = if self.checkpoint {
            self.route().checkpoint
        } else {
            30
        };
        self.x = x * U;
        self.y = FLOOR * U;
        self.vx = 0;
        self.vy = 0;
        self.face = 1;
        self.spring_flight = false;
        self.grounded = true;
        self.coyote = 4;
        self.buffer = 0;
        self.jump_age = 0;
        self.wing = true;
        self.flash = 28;
        self.scroll = (x - 130).clamp(0, self.route().length - W + 96);
    }
    fn start(&mut self, stage: usize) {
        self.stage = stage;
        self.mask = 0;
        self.dead = 0;
        self.checkpoint = false;
        self.notice = 0;
        self.bounces = 0;
        self.collected_fx = (0, 0, 0);
        self.mode = Mode::Run;
        self.place();
    }
    pub fn crab_x(&self, i: usize) -> i32 {
        let (l, r, _) = self.route().crabs[i];
        let len = r - l;
        let a = ((self.tick / 2 + i as u32 * 17) % (2 * len) as u32) as i32;
        l + len - (a - len).abs()
    }
    pub fn step(&mut self, keys: u8) -> Sfx {
        let tap = keys & !self.old;
        self.old = keys;
        if self.mode != Mode::Pause {
            self.tick = self.tick.wrapping_add(1);
        }
        match self.mode {
            Mode::Cover => {
                if tap & JUMP != 0 {
                    self.album = [0; 3];
                    self.attempts = 0;
                    self.start(0);
                    return Sfx::Flag;
                }
                return Sfx::Quiet;
            }
            Mode::Pause => {
                if tap & (PAUSE | JUMP) != 0 {
                    self.mode = Mode::Run
                }
                return Sfx::Quiet;
            }
            Mode::Delivery => {
                if tap & JUMP != 0 {
                    if self.stage < 2 {
                        self.start(self.stage + 1)
                    } else {
                        self.mode = Mode::Home
                    }
                    return Sfx::Flag;
                }
                return Sfx::Quiet;
            }
            Mode::Home => {
                if tap & JUMP != 0 {
                    self.mode = Mode::Cover
                }
                return Sfx::Quiet;
            }
            Mode::Run => {}
        }
        if tap & PAUSE != 0 {
            self.mode = Mode::Pause;
            return Sfx::Quiet;
        }
        if tap & RESET != 0 {
            self.attempts = self.attempts.saturating_add(1);
            self.place();
            return Sfx::Splash;
        }
        self.notice = self.notice.saturating_sub(1);
        self.flash = self.flash.saturating_sub(1);
        self.collected_fx.2 = self.collected_fx.2.saturating_sub(1);
        let r = self.route();
        let mut sound = Sfx::Quiet;
        let dir = (keys & RIGHT != 0) as i32 - (keys & LEFT != 0) as i32;
        if dir == 0 {
            self.vx -= self.vx.signum() * self.vx.abs().min(5)
        } else {
            self.face = dir;
            self.vx = (self.vx + dir * 6).clamp(-47, 47)
        }
        if tap & JUMP != 0 {
            self.buffer = 5
        } else {
            self.buffer = self.buffer.saturating_sub(1)
        }
        if self.buffer > 0 && (self.grounded || self.coyote > 0) {
            self.vy = -102;
            self.coyote = 0;
            self.buffer = 0;
            self.grounded = false;
            self.jump_age = 0;
            sound = Sfx::Hop;
        } else if tap & JUMP != 0 && !self.grounded && self.wing && self.jump_age > 3 {
            self.vy = -67;
            self.spring_flight = false;
            self.wing = false;
            self.buffer = 0;
            sound = Sfx::Wing
        }
        if !self.spring_flight && keys & JUMP == 0 && self.vy < -54 {
            self.vy = -54
        }
        let py = self.y;
        self.x = (self.x + self.vx).clamp(7 * U, (r.length + 35) * U);
        let x = self.pos();
        self.vy = (self.vy + 5).min(124);
        self.y += self.vy;
        self.grounded = false;
        if self.vy >= 0 {
            let mut top = i32::MAX;
            for t in r.spans.iter().chain(r.decks) {
                if x + 5 > t.x && x - 5 < t.end && py <= t.y * U && self.y >= t.y * U {
                    top = top.min(t.y)
                }
            }
            if top != i32::MAX {
                self.y = top * U;
                self.spring_flight = false;
                self.vy = 0;
                self.grounded = true;
                self.wing = true;
                self.coyote = 5;
            }
        }
        if !self.grounded {
            self.coyote = self.coyote.saturating_sub(1);
            self.jump_age = self.jump_age.saturating_add(1);
        }
        for &sx in r.springs {
            if self.grounded && (sx - x).abs() < 9 {
                self.vy = -143;
                self.spring_flight = true;
                self.grounded = false;
                self.coyote = 0;
                self.wing = true;
                sound = Sfx::Bounce;
            }
        }
        for (i, &(sx, sy)) in r.stamps.iter().enumerate() {
            if self.mask & (1 << i) == 0
                && (x - sx).abs() < 14
                && (self.foot() - 10 - sy).abs() < 16
            {
                self.mask |= 1 << i;
                self.collected_fx = (sx, sy, 17);
                sound = Sfx::Stamp
            }
        }
        for (i, &(_, _, ey)) in r.crabs.iter().enumerate() {
            if self.dead & (1 << i) != 0 {
                continue;
            }
            let ex = self.crab_x(i);
            if (x - ex).abs() < 13 && self.foot() > ey - 13 && self.foot() - 16 < ey {
                if self.vy >= 0 && py / U <= ey - 8 {
                    self.dead |= 1 << i;
                    self.vy = -80;
                    self.wing = true;
                    self.grounded = false;
                    self.bounces = self.bounces.saturating_add(1);
                    sound = Sfx::Bounce;
                } else if self.flash == 0 {
                    self.attempts = self.attempts.saturating_add(1);
                    self.place();
                    return Sfx::Splash;
                }
            }
        }
        if self.foot() > H + 7 {
            self.attempts = self.attempts.saturating_add(1);
            self.place();
            return Sfx::Splash;
        }
        if !self.checkpoint && x >= r.checkpoint && self.grounded {
            self.checkpoint = true;
            self.notice = 65;
            sound = Sfx::Flag
        }
        if x >= r.length - 12 && self.grounded {
            self.album[self.stage] = self.mask.count_ones() as u8;
            self.mode = Mode::Delivery;
            return Sfx::Deliver;
        }
        let aim = (x - 145).clamp(0, r.length - W + 96);
        self.scroll += (aim - self.scroll).clamp(-5, 5);
        sound
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn start_pause_and_retry() {
        let mut g = Game::new();
        g.step(JUMP);
        assert_eq!(g.mode, Mode::Run);
        g.step(0);
        g.step(PAUSE);
        let p = g.x;
        g.step(RIGHT);
        assert_eq!(g.x, p);
        g.step(PAUSE);
        assert_eq!(g.mode, Mode::Run);
        g.checkpoint = true;
        g.step(RESET);
        assert_eq!(g.pos(), g.route().checkpoint);
    }
    #[test]
    fn play_all_routes_from_start() {
        let mut g = Game::new();
        g.step(JUMP);
        g.step(0);
        for stage in 0..3 {
            let mut prev = false;
            for _ in 0..5000 {
                let x = g.pos();
                let r = g.route();
                let gap = r.spans.iter().any(|s| x >= s.end - 23 && x < s.end - 1);
                let crab =
                    r.crabs.iter().enumerate().any(|(i, _)| {
                        g.dead & (1 << i) == 0 && (9..46).contains(&(g.crab_x(i) - x))
                    });
                let j = if g.grounded {
                    gap || crab
                } else {
                    prev && g.vy < 0
                        || (!prev
                            && g.wing
                            && g.vy > 20
                            && !r.spans.iter().any(|p| p.x <= x && x < p.end - 12))
                };
                g.step(RIGHT | if j { JUMP } else { 0 });
                prev = j;
                if g.mode == Mode::Delivery {
                    break;
                }
            }
            assert_eq!(
                g.mode,
                Mode::Delivery,
                "stage {} x={} falls={}",
                stage,
                g.pos(),
                g.attempts
            );
            g.step(0);
            g.step(JUMP);
            g.step(0);
        }
        assert_eq!(g.mode, Mode::Home);
    }
}

#[cfg(test)]
#[path = "checks.rs"]
mod checks;
