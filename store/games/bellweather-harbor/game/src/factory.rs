//! Harbor works turn landed or purchased cargo into goods for the exchange.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FactoryPlan {
    pub name: &'static str,
    pub product: &'static str,
    /// Product index on the exchange.
    pub good: usize,
    pub build_cost: u64,
    pub fee: u64,
    /// Crates of salt, crab and timber required for a single batch.
    pub inputs: [u16; 3],
    pub duration_ms: u32,
}

pub const PLANS: [FactoryPlan; 2] = [
    FactoryPlan {
        name: "Cannery",
        product: "Provisions",
        good: 3,
        build_cost: 420,
        fee: 8,
        inputs: [1, 2, 0],
        duration_ms: 8_000,
    },
    FactoryPlan {
        name: "Chandler's Shop",
        product: "Fittings",
        good: 4,
        build_cost: 1_800,
        fee: 14,
        inputs: [1, 0, 2],
        duration_ms: 14_000,
    },
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Factory {
    pub built: bool,
    pub repeat: bool,
    pub running: bool,
    pub elapsed_ms: u32,
    /// Finished crates available for sale. A running batch reserves one more slot.
    pub output: u16,
}

impl Factory {
    pub const fn new() -> Self {
        Self {
            built: false,
            repeat: false,
            running: false,
            elapsed_ms: 0,
            output: 0,
        }
    }

    pub fn can_start(&self, plan: &FactoryPlan, cash: u64, stock: &[u16; 3], cap: u16) -> bool {
        self.built
            && !self.running
            && self.output < cap
            && cash >= plan.fee
            && stock
                .iter()
                .zip(plan.inputs)
                .all(|(&held, needed)| held >= needed)
    }

    /// Prepay exactly one batch, including the fee and all three input checks.
    pub fn start(
        &mut self,
        plan: &FactoryPlan,
        cash: &mut u64,
        stock: &mut [u16; 3],
        cap: u16,
    ) -> bool {
        if !self.can_start(plan, *cash, stock, cap) {
            return false;
        }
        *cash -= plan.fee;
        for (held, needed) in stock.iter_mut().zip(plan.inputs) {
            *held -= needed;
        }
        self.running = true;
        self.elapsed_ms = 0;
        true
    }

    pub fn remaining_ms(&self, plan: &FactoryPlan) -> Option<u32> {
        self.running.then(|| plan.duration_ms - self.elapsed_ms)
    }

    /// Finish at most the already-paid batch. The economy advances competing
    /// businesses and selects the next eligible repeat batch at each boundary.
    pub fn advance(&mut self, plan: &FactoryPlan, ms: u64) -> bool {
        let Some(remaining) = self.remaining_ms(plan) else {
            return false;
        };
        if ms < u64::from(remaining) {
            self.elapsed_ms += ms as u32;
            return false;
        }
        self.output += 1;
        self.elapsed_ms = 0;
        self.running = false;
        true
    }

    pub fn valid(&self, plan: &FactoryPlan, cap: u16) -> bool {
        if !self.built {
            return *self == Self::new();
        }
        if self.running {
            self.output < cap && self.elapsed_ms < plan.duration_ms
        } else {
            self.output <= cap && self.elapsed_ms == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_prepayment_is_atomic_and_cannot_be_repeated() {
        let plan = &PLANS[0];
        let mut works = Factory::new();
        let mut cash = 8;
        let mut cargo = [1, 1, 9];
        assert!(!works.start(plan, &mut cash, &mut cargo, 24));
        works.built = true;
        assert!(!works.start(plan, &mut cash, &mut cargo, 24));
        assert_eq!((cash, cargo), (8, [1, 1, 9]));
        cargo[1] = 2;
        cash = 7;
        assert!(!works.start(plan, &mut cash, &mut cargo, 24));
        assert_eq!((cash, cargo), (7, [1, 2, 9]));
        cash = 8;
        assert!(works.start(plan, &mut cash, &mut cargo, 24));
        assert_eq!((cash, cargo), (0, [0, 0, 9]));
        assert!(!works.start(plan, &mut cash, &mut cargo, 24));
    }

    #[test]
    fn batch_only_releases_one_saleable_crate_at_the_boundary() {
        let plan = &PLANS[1];
        let mut works = Factory {
            built: true,
            repeat: true,
            output: 23,
            ..Factory::new()
        };
        let mut cash = u64::MAX;
        let mut stock = [2, 0, 4];
        assert!(works.start(plan, &mut cash, &mut stock, 24));
        let mut single = works;
        assert!(!works.advance(plan, 13_999));
        assert_eq!(works.output, 23);
        assert_eq!(works.remaining_ms(plan), Some(1));
        assert!(works.advance(plan, 1));
        assert!(single.advance(plan, u64::MAX));
        assert_eq!(works, single);
        assert_eq!(works.output, 24);
        assert!(!works.advance(plan, u64::MAX));
        assert!(!works.start(plan, &mut cash, &mut stock, 24));
        assert_eq!(cash, u64::MAX - plan.fee);
        assert_eq!(stock, [1, 0, 2]);
        works.output -= 1;
        assert!(works.start(plan, &mut cash, &mut stock, 24));
    }

    #[test]
    fn save_validation_requires_a_paid_workspace_and_an_output_slot() {
        let plan = &PLANS[0];
        let mut works = Factory {
            repeat: true,
            ..Factory::new()
        };
        assert!(!works.valid(plan, 24));
        works.built = true;
        works.elapsed_ms = 1;
        assert!(!works.valid(plan, 24));
        works.running = true;
        assert!(works.valid(plan, 24));
        works.output = 24;
        assert!(!works.valid(plan, 24));
        works.output = 0;
        works.elapsed_ms = plan.duration_ms;
        assert!(!works.valid(plan, 24));
    }
}
