#![no_std]
pub mod wire;
pub const WIDTH: i32 = 1400;
pub const LENGTH: i32 = 1900;
pub const GOAL: i32 = 440;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Body {
    pub x: i32,
    pub z: i32,
    pub vx: i32,
    pub vz: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub player: [Body; 2],
    pub puck: Body,
    pub score: [u8; 2],
    pub serve: u16,
}
impl Default for Match {
    fn default() -> Self {
        let mut m = Self {
            player: [Body::default(); 2],
            puck: Body::default(),
            score: [0; 2],
            serve: 0,
        };
        m.faceoff(0);
        m
    }
}
fn paddle(b: &mut Body, input: u8, home: i32) {
    let x = (input & 2 != 0) as i32 - (input & 1 != 0) as i32;
    let z = (input & 4 != 0) as i32 - (input & 8 != 0) as i32;
    let a = if x * z == 0 { 7 } else { 5 };
    b.vx = (b.vx * 4 / 5 + a * x).clamp(-32, 32);
    b.vz = (b.vz * 4 / 5 + a * z).clamp(-32, 32);
    b.x = (b.x + b.vx).clamp(-WIDTH + 145, WIDTH - 145);
    let zz = (home * (b.z + b.vz)).clamp(120, LENGTH - 150);
    b.z = home * zz;
}
impl Match {
    fn faceoff(&mut self, server: usize) {
        self.player = [
            Body {
                z: -1040,
                ..Body::default()
            },
            Body {
                z: 1040,
                ..Body::default()
            },
        ];
        self.puck = Body {
            x: 0,
            z: if server == 0 { -480 } else { 480 },
            vx: 0,
            vz: 0,
        };
        self.serve = 37;
    }
    pub fn step(&mut self, inputs: [u8; 2]) {
        if self.score.contains(&5) {
            return;
        }
        paddle(&mut self.player[0], inputs[0], -1);
        paddle(&mut self.player[1], inputs[1], 1);
        if self.serve > 0 {
            self.serve -= 1;
            return;
        }
        let p = &mut self.puck;
        p.x += p.vx;
        p.z += p.vz;
        for (i, b) in self.player.iter().enumerate() {
            let (dx, dz) = (p.x - b.x, p.z - b.z);
            let sq = dx * dx + dz * dz;
            if sq < 215 * 215 {
                let norm = sqrt(sq).max(1);
                let (nx, nz) = if dx == 0 && dz == 0 {
                    (0, if i == 0 { 256 } else { -256 })
                } else {
                    (dx * 256 / norm, dz * 256 / norm)
                };
                p.x = b.x + nx * 216 / 256;
                p.z = b.z + nz * 216 / 256;
                let toward = ((b.vx - p.vx) * nx + (b.vz - p.vz) * nz) / 256;
                if toward > 0 || p.vx.abs() + p.vz.abs() < 12 {
                    let thrust = (toward * 2 + 11).max(15);
                    p.vx = (p.vx + thrust * nx / 256).clamp(-84, 84);
                    p.vz = (p.vz + thrust * nz / 256).clamp(-84, 84);
                }
            }
        }
        if p.x.abs() > WIDTH - 70 {
            p.x = p.x.clamp(-WIDTH + 70, WIDTH - 70);
            p.vx = -p.vx * 15 / 16;
        }
        if p.z.abs() > LENGTH - 65 {
            if p.x.abs() < GOAL - 65 {
                let winner = if p.z > 0 { 0 } else { 1 };
                self.score[winner] += 1;
                if self.score[winner] < 5 {
                    self.faceoff(1 - winner);
                }
            } else {
                p.z = p.z.clamp(-LENGTH + 65, LENGTH - 65);
                p.vz = -p.vz * 15 / 16;
            }
        }
    }
}
fn sqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut a = n;
    let mut b = (a + 1) / 2;
    while b < a {
        a = b;
        b = (a + n / a) / 2;
    }
    a
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub fps: bool,
    pub cap: u8,
    pub detail: bool,
    pub wide: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            fps: true,
            cap: 1,
            detail: true,
            wide: false,
        }
    }
}
impl Settings {
    pub fn encode(self) -> [u8; 5] {
        [
            1,
            self.fps as u8,
            self.cap,
            self.detail as u8,
            self.wide as u8,
        ]
    }
    pub fn decode(b: [u8; 5]) -> Option<Self> {
        if b[0] != 1 || b[1] > 1 || b[2] > 2 || b[3] > 1 || b[4] > 1 {
            None
        } else {
            Some(Self {
                fps: b[1] != 0,
                cap: b[2],
                detail: b[3] != 0,
                wide: b[4] != 0,
            })
        }
    }
    pub fn interval(self) -> u64 {
        [33333, 16667, 0][self.cap as usize]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_players_and_goals() {
        let mut m = Match::default();
        m.step([2, 1]);
        assert!(m.player[0].x > 0 && m.player[1].x < 0);
        for _ in 0..5 {
            m.serve = 0;
            m.puck = Body {
                x: 0,
                z: 1890,
                vz: 32,
                vx: 0,
            };
            m.step([0, 0]);
        }
        assert_eq!(m.score, [5, 0]);
        let end = m;
        m.step([2, 4]);
        assert_eq!(m, end);
    }
    #[test]
    fn foul_line_and_closed_end() {
        let mut m = Match::default();
        for _ in 0..90 {
            m.step([4, 8]);
        }
        assert!(m.player[0].z <= -120 && m.player[1].z >= 120);
        m.puck = Body {
            x: 700,
            z: 1860,
            vx: 0,
            vz: 36,
        };
        m.step([0, 0]);
        assert_eq!(m.score, [0, 0]);
        assert!(m.puck.vz < 0);
    }
    #[test]
    fn settings_format() {
        assert_eq!(
            Settings::decode(Settings::default().encode()),
            Some(Settings::default())
        );
        assert!(Settings::decode([1, 1, 3, 0, 0]).is_none());
    }
}
