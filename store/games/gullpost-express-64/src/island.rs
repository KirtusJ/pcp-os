#![no_std]
// World coordinates are sixty-four units to the metre. One physics tick is 25 ms.
pub const W: i32 = 512;
pub const H: i32 = 288;
pub const U: i32 = 64;
pub const TICK: i64 = 25;
pub const LEFT: u16 = 1;
pub const RIGHT: u16 = 2;
pub const FORWARD: u16 = 4;
pub const BACK: u16 = 8;
pub const JUMP: u16 = 16;
pub const PAUSE: u16 = 32;
pub const RESET: u16 = 64;
pub const ORBIT_L: u16 = 128;
pub const ORBIT_R: u16 = 256;
pub const TOUR: u16 = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Cover,
    Run,
    Pause,
    Delivered,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Quiet,
    Hop,
    Wing,
    Stamp,
    Flag,
    Splash,
    Deliver,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct V {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
pub const fn v(x: i32, y: i32, z: i32) -> V {
    V { x, y, z }
}
pub const fn m(x: i32, y: i32, z: i32) -> V {
    v(x * U, y * U, z * U)
}
pub const fn sin(a: i32) -> i32 {
    const LUT: [i32; 17] = [
        0, 25, 50, 74, 98, 121, 142, 162, 181, 198, 213, 226, 237, 245, 251, 255, 256,
    ];
    let i = a & 255;
    let h = i & 127;
    let k = if h > 64 { 128 - h } else { h };
    let j = (k / 4) as usize;
    let value = if j == 16 {
        256
    } else {
        LUT[j] + (LUT[j + 1] - LUT[j]) * (k % 4) / 4
    };
    if i >= 128 {
        -value
    } else {
        value
    }
}
pub const fn cos(a: i32) -> i32 {
    sin(a + 64)
}
pub const SHORE: [(i32, i32); 10] = [
    (-7, -8),
    (-2, -10),
    (6, -9),
    (11, -5),
    (12, 4),
    (8, 12),
    (3, 15),
    (-6, 14),
    (-11, 8),
    (-12, -2),
];
pub const TERRAIN: i32 = 80;
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub l: i32,
    pub r: i32,
    pub a: i32,
    pub b: i32,
    pub top: i32,
}
pub const fn bounds(l: i32, r: i32, a: i32, b: i32, top: i32) -> Block {
    Block { l, r, a, b, top }
}
pub const PIERS: [Block; 2] = [
    bounds(-145, 145, -1770, -1088, 64),
    bounds(-145, 145, -920, -475, 64),
];
pub const CRATE: Block = bounds(55, 192, -280, -133, 163);
pub const HOUSE: Block = bounds(192, 544, -331, 100, 290);
pub const TERRACE: Block = bounds(-373, 345, 370, 839, 178);
pub const BEACON: V = v(-70, 178, 650);
pub const BOX: V = v(-281, 178, 499);
pub const SEALS: [V; 3] = [v(0, 122, -770), v(361, 350, -107), v(218, 238, 749)];
pub const SPAWN: V = v(0, 64, -1536);
pub const POST: V = v(0, TERRAIN, -390);

fn on_island(x: i32, z: i32) -> bool {
    for i in 0..SHORE.len() {
        let (a, b) = SHORE[i];
        let (c, d) = SHORE[(i + 1) % SHORE.len()];
        if (c - a) * (z - b * U) - (d - b) * (x - a * U) < 0 {
            return false;
        }
    }
    true
}
impl Block {
    pub fn contains(self, x: i32, z: i32, margin: i32) -> bool {
        x >= self.l - margin && x <= self.r + margin && z >= self.a - margin && z <= self.b + margin
    }
}
pub fn surface(x: i32, z: i32, max_y: i32) -> i32 {
    let mut y = if on_island(x, z) && TERRAIN <= max_y {
        TERRAIN
    } else {
        -10000
    };
    for b in [PIERS[0], PIERS[1], CRATE, HOUSE, TERRACE] {
        if b.top <= max_y && b.contains(x, z, 8) {
            y = y.max(b.top)
        }
    }
    y
}
fn obstructed(x: i32, z: i32, y: i32) -> bool {
    for b in [CRATE, HOUSE, TERRACE] {
        if y + 17 < b.top && b.contains(x, z, 13) {
            return true;
        }
    }
    (x - BEACON.x) * (x - BEACON.x) + (z - BEACON.z) * (z - BEACON.z) < 126 * 126
        && y >= TERRAIN - 20
}
fn approach(a: i32, b: i32, n: i32) -> i32 {
    a + (b - a).clamp(-n, n)
}
pub struct Game {
    pub mode: Mode,
    pub pos: V,
    pub velocity: V,
    pub facing: i32,
    pub yaw: i32,
    pub camera: V,
    pub tick: u32,
    pub grounded: bool,
    pub used_wing: bool,
    pub mask: u8,
    pub checkpoint: bool,
    pub falls: u16,
    pub elapsed: u32,
    pub demo: bool,
    pub leg: usize,
    pub notice: u8,
    pub last: Sfx,
    old: u16,
    old_physical: u16,
    coyote: u8,
    buffer: u8,
}
impl Game {
    pub fn new() -> Self {
        Self {
            mode: Mode::Cover,
            pos: SPAWN,
            velocity: V::default(),
            facing: 0,
            yaw: 0,
            camera: SPAWN,
            tick: 0,
            grounded: true,
            used_wing: false,
            mask: 0,
            checkpoint: false,
            falls: 0,
            elapsed: 0,
            demo: false,
            leg: 0,
            notice: 0,
            last: Sfx::Quiet,
            old: 0,
            old_physical: 0,
            coyote: 4,
            buffer: 0,
        }
    }
    fn respawn(&mut self) {
        self.pos = if self.checkpoint { POST } else { SPAWN };
        self.velocity = V::default();
        self.camera = self.pos;
        self.grounded = true;
        self.coyote = 4;
        self.buffer = 0;
        self.used_wing = false;
    }
    pub fn step(&mut self, mut keys: u16) -> Sfx {
        let ui_tap = keys & !self.old_physical;
        self.old_physical = keys;
        if self.mode != Mode::Pause {
            self.tick = self.tick.wrapping_add(1)
        }
        match self.mode {
            Mode::Cover => {
                self.old = keys;
                if ui_tap & (JUMP | TOUR) != 0 {
                    let demo = ui_tap & TOUR != 0;
                    *self = Self::new();
                    self.mode = Mode::Run;
                    self.old = keys;
                    self.old_physical = keys;
                    self.demo = demo;
                    return Sfx::Flag;
                }
                return Sfx::Quiet;
            }
            Mode::Pause => {
                self.old = keys;
                if ui_tap & (PAUSE | JUMP) != 0 {
                    self.mode = Mode::Run
                }
                return Sfx::Quiet;
            }
            Mode::Delivered => {
                self.old = keys;
                if ui_tap & JUMP != 0 {
                    *self = Self::new();
                    self.old = keys;
                    self.old_physical = keys;
                }
                return Sfx::Quiet;
            }
            Mode::Run => {}
        }
        if ui_tap & PAUSE != 0 {
            self.old = keys;
            self.mode = Mode::Pause;
            return Sfx::Quiet;
        }
        if self.demo {
            keys = self.pilot()
        }
        let tap = keys & !self.old;
        self.old = keys;
        self.elapsed += 1;
        self.notice = self.notice.saturating_sub(1);
        if tap & RESET != 0 {
            self.falls = self.falls.saturating_add(1);
            self.respawn();
            return Sfx::Splash;
        }
        self.yaw =
            (self.yaw + 3 * ((keys & ORBIT_R != 0) as i32 - (keys & ORBIT_L != 0) as i32)) & 255;
        let lr = (keys & RIGHT != 0) as i32 - (keys & LEFT != 0) as i32;
        let fb = (keys & FORWARD != 0) as i32 - (keys & BACK != 0) as i32;
        let speed = if lr != 0 && fb != 0 { 8 } else { 12 };
        let a = if self.grounded { 4 } else { 2 };
        self.velocity.x = approach(
            self.velocity.x,
            (lr * cos(self.yaw) + fb * sin(self.yaw)) * speed / 256,
            a,
        );
        self.velocity.z = approach(
            self.velocity.z,
            (fb * cos(self.yaw) - lr * sin(self.yaw)) * speed / 256,
            a,
        );
        if lr != 0 || fb != 0 {
            let d = match (lr, fb) {
                (0, 1) => 0,
                (1, 1) => 32,
                (1, 0) => 64,
                (1, -1) => 96,
                (0, -1) => 128,
                (-1, -1) => 160,
                (-1, 0) => 192,
                _ => 224,
            };
            let target = (self.yaw + d) & 255;
            self.facing =
                (self.facing + (((target - self.facing + 128) & 255) - 128).clamp(-14, 14)) & 255
        }
        let mut fx = Sfx::Quiet;
        if tap & JUMP != 0 {
            self.buffer = 4
        } else {
            self.buffer = self.buffer.saturating_sub(1)
        }
        if self.buffer > 0 && (self.grounded || self.coyote > 0) {
            self.velocity.y = 30;
            self.grounded = false;
            self.coyote = 0;
            self.buffer = 0;
            fx = Sfx::Hop
        } else if tap & JUMP != 0 && !self.used_wing && !self.grounded {
            self.velocity.y = 24;
            self.used_wing = true;
            self.buffer = 0;
            fx = Sfx::Wing
        }
        if keys & JUMP == 0 && self.velocity.y > 15 {
            self.velocity.y = 15
        }
        let old_y = self.pos.y;
        self.velocity.y = (self.velocity.y - 2).max(-40);
        // Rising through the ledge height clears its lip on the same tick.
        let headroom = old_y.max(old_y + self.velocity.y);
        let nx = self.pos.x + self.velocity.x;
        if !obstructed(nx, self.pos.z, headroom) {
            self.pos.x = nx
        } else {
            self.velocity.x = 0
        }
        let nz = self.pos.z + self.velocity.z;
        if !obstructed(self.pos.x, nz, headroom) {
            self.pos.z = nz
        } else {
            self.velocity.z = 0
        }
        self.pos.y += self.velocity.y;
        let floor = surface(self.pos.x, self.pos.z, old_y + 18);
        self.grounded = self.velocity.y <= 0 && self.pos.y <= floor;
        if self.grounded {
            self.pos.y = floor;
            self.velocity.y = 0;
            self.used_wing = false;
            self.coyote = 4
        } else {
            self.coyote = self.coyote.saturating_sub(1)
        }
        if self.pos.y < -105 {
            self.falls = self.falls.saturating_add(1);
            self.respawn();
            fx = Sfx::Splash
        }
        if !self.checkpoint
            && self.grounded
            && self.pos.z >= -460
            && on_island(self.pos.x, self.pos.z)
        {
            self.checkpoint = true;
            fx = Sfx::Flag
        }
        for (i, p) in SEALS.iter().enumerate() {
            if self.mask & (1 << i) == 0
                && (self.pos.x - p.x).abs() < 45
                && (self.pos.z - p.z).abs() < 45
                && (self.pos.y + 54 - p.y).abs() < 72
            {
                self.mask |= 1 << i;
                fx = Sfx::Stamp
            }
        }
        if self.mask == 7
            && self.grounded
            && (self.pos.x - BOX.x).abs() < 53
            && (self.pos.z - BOX.z).abs() < 55
        {
            self.mode = Mode::Delivered;
            fx = Sfx::Deliver
        }
        for (a, b) in [
            (&mut self.camera.x, self.pos.x),
            (&mut self.camera.y, self.pos.y),
            (&mut self.camera.z, self.pos.z),
        ] {
            let d = b - *a;
            *a += if d.abs() < 5 { d } else { d / 5 }
        }
        if fx != Sfx::Quiet {
            self.last = fx;
            self.notice = 50
        }
        fx
    }
    // Each waypoint supplies ordinary controller buttons. The tour and the tests use
    // exactly the same movement, collisions, collectibles and letterbox as a player.
    fn pilot(&mut self) -> u16 {
        const ROUTE: [(i32, i32, i32); 14] = [
            (0, -1160, 0),
            (0, -770, 0),
            (0, -240, 0),
            (115, -222, 150),
            (360, -216, 280),
            (361, -107, 280),
            (380, 242, 0),
            (211, 315, 0),
            (211, 455, 165),
            (218, 749, 165),
            (110, 810, 165),
            (-284, 810, 165),
            (-281, 499, 165),
            (-281, 499, 165),
        ];
        let (mut x, mut z, mut height) = ROUTE[self.leg];
        if (self.pos.x - x).abs() < 19
            && (self.pos.z - z).abs() < 19
            && self.pos.y >= height
            && (match self.leg {
                1 => self.mask & 1 != 0,
                5 => self.mask & 2 != 0,
                9 => self.mask & 4 != 0,
                _ => true,
            })
            && self.leg + 1 < ROUTE.len()
        {
            self.leg += 1;
            (x, z, height) = ROUTE[self.leg]
        }
        let mut k = 0;
        if x - self.pos.x > 10 {
            k |= RIGHT
        }
        if self.pos.x - x > 10 {
            k |= LEFT
        }
        if z - self.pos.z > 10 {
            k |= FORWARD
        }
        if self.pos.z - z > 10 {
            k |= BACK
        }
        let jump_needed = (self.leg == 1 && self.pos.z < -930) || (height > self.pos.y + 17);
        if self.grounded && jump_needed
            || !self.grounded && self.velocity.y > 0 && self.old & JUMP != 0
        {
            k |= JUMP
        }
        if !self.grounded
            && !self.used_wing
            && self.velocity.y <= 1
            && self.old & JUMP == 0
            && jump_needed
        {
            k |= JUMP
        }
        k
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tour_delivers() {
        let mut g = Game::new();
        g.step(TOUR);
        for _ in 0..3000 {
            g.step(0);
            if g.mode == Mode::Delivered {
                break;
            }
        }
        assert_eq!(
            g.mode,
            Mode::Delivered,
            "leg={} pos={:?} falls={}",
            g.leg,
            g.pos,
            g.falls
        );
        assert_eq!(g.mask, 7);
        assert_eq!(g.falls, 0)
    }
    #[test]
    fn retry_and_pause() {
        let mut g = Game::new();
        g.step(JUMP);
        g.step(0);
        g.step(PAUSE);
        let p = g.pos;
        g.step(FORWARD);
        assert_eq!(g.pos.z, p.z);
        g.step(PAUSE);
        g.step(RESET);
        assert_eq!(g.pos.z, SPAWN.z);
        assert_eq!(g.falls, 1)
    }
    #[test]
    fn collision_and_keys() {
        let mut g = Game::new();
        g.step(JUMP);
        g.step(0);
        for _ in 0..30 {
            g.step(ORBIT_R);
        }
        for _ in 0..10 {
            g.step(FORWARD);
        }
        assert!(g.pos.x > 50);
        assert!(obstructed(BEACON.x, BEACON.z, 178));
        assert!(obstructed(350, -250, 80));
        assert_eq!(surface(350, -250, 300), 290);
        assert_eq!(surface(0, -1000, 300), -10000);
    }
}

#[cfg(test)]
mod physics_checks;
