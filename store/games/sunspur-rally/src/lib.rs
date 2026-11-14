#![no_std]
use heliograph::assets::sin;
pub const FINISH: i32 = 134000;
pub const GATES: [(i32, i32); 5] = [
    (16000, -240),
    (37000, 240),
    (61000, -220),
    (90000, 240),
    (116000, 0),
];
pub fn bend(z: i32) -> i32 {
    sin(z / 65) * 1050 / 16384 + sin(z / 133 + 187) * 940 / 16384
}
#[derive(Default, Clone, Copy)]
pub struct Input {
    pub gas: bool,
    pub brake: bool,
    pub steer: i32,
}
#[derive(Default, Debug)]
pub struct Race {
    pub z: i32,
    pub offset: i32,
    pub speed: i32,
    pub ticks: u32,
    pub gates: u32,
    pub taken: u32,
    pub finished: bool,
    drift: i32,
}
impl Race {
    pub fn step(&mut self, k: Input) {
        if self.finished {
            return;
        }
        self.ticks += 1;
        let cap = if self.offset.abs() > 470 { 46 } else { 117 };
        if k.gas {
            self.speed += 1;
        } else {
            self.speed -= 1;
        }
        if k.brake {
            self.speed -= 4;
        }
        if self.speed > cap {
            self.speed -= 3;
        }
        self.speed = self.speed.clamp(0, 117);
        let before = self.z;
        self.z = (self.z + self.speed).min(FINISH);
        let curve = bend(self.z + 500) - 2 * bend(self.z) + bend(self.z - 500);
        self.drift += curve * self.speed;
        let push = self.drift / 900;
        self.drift %= 900;
        self.offset =
            (self.offset + k.steer.clamp(-1, 1) * (3 + self.speed / 12) - push).clamp(-760, 760);
        for (i, &(z, x)) in GATES.iter().enumerate() {
            if before < z && self.z >= z {
                self.gates += 1;
                if (self.offset - x).abs() <= 150 {
                    self.taken |= 1 << i;
                }
            }
        }
        self.finished = self.z == FINISH;
    }
    pub fn score(&self) -> u32 {
        self.ticks.saturating_sub(self.taken.count_ones() * 60)
    }
    pub fn demo(&self) -> Input {
        let x = GATES
            .iter()
            .find(|(z, _)| *z >= self.z)
            .map(|p| if p.0 - self.z < 4500 { p.1 } else { 0 })
            .unwrap_or(0);
        Input {
            gas: true,
            brake: false,
            steer: if (x - self.offset).abs() > 10 {
                (x - self.offset).signum()
            } else {
                0
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_route() {
        let mut r = Race::default();
        for _ in 0..3000 {
            r.step(r.demo());
        }
        assert!(r.finished);
        assert_eq!((r.gates, r.taken), (5, 31));
        let t = r.ticks;
        r.step(Input::default());
        assert_eq!(r.ticks, t);
    }
    #[test]
    fn dirt_slows_down() {
        let mut r = Race {
            offset: 700,
            speed: 110,
            ..Race::default()
        };
        for _ in 0..40 {
            r.step(Input {
                gas: true,
                ..Input::default()
            });
        }
        assert!(r.speed < 55);
    }
}
