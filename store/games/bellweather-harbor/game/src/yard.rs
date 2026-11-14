//! The shipwright fits out one named vessel at a time. Chartering a new harbor
//! keeps the hull on the slipway and every vessel already launched.

pub struct Plan {
    pub name: &'static str,
    pub kind: &'static str,
    pub cost: u64,
    /// Salt, crab and timber crates, in Freight Exchange order.
    pub cargo: [u16; 3],
    pub duration_ms: u32,
    pub bonus_percent: u32,
}

pub const PLANS: [Plan; 3] = [
    Plan {
        name: "Pilgrim",
        kind: "Pilot cutter",
        cost: 24,
        cargo: [4, 0, 0],
        duration_ms: 12_000,
        bonus_percent: 15,
    },
    Plan {
        name: "Juniper",
        kind: "Packet boat",
        cost: 300,
        cargo: [6, 4, 0],
        duration_ms: 25_000,
        bonus_percent: 20,
    },
    Plan {
        name: "Northstar",
        kind: "Lightship",
        cost: 3_600,
        cargo: [12, 8, 4],
        duration_ms: 45_000,
        bonus_percent: 25,
    },
];

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Yard {
    /// Bit i is set when vessel i is in service.
    pub completed: u8,
    pub building: bool,
    pub elapsed_ms: u32,
}

impl Yard {
    pub const fn new() -> Self {
        Self {
            completed: 0,
            building: false,
            elapsed_ms: 0,
        }
    }

    /// Vessel on the slipway, or the next vessel offered by the shipwright.
    pub fn next(&self) -> Option<usize> {
        let next = self.completed.trailing_ones() as usize;
        (next < PLANS.len()).then_some(next)
    }

    pub fn bonus_percent(&self) -> u32 {
        PLANS
            .iter()
            .enumerate()
            .filter(|(i, _)| self.completed & (1 << i) != 0)
            .map(|(_, plan)| plan.bonus_percent)
            .sum()
    }

    pub(crate) fn valid(&self) -> bool {
        if !matches!(self.completed, 0 | 1 | 3 | 7) {
            return false;
        }
        if self.building {
            self.next()
                .is_some_and(|i| self.elapsed_ms < PLANS[i].duration_ms)
        } else {
            self.elapsed_ms == 0
        }
    }
}
