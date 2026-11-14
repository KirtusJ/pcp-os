//! Standing Order Thirteen. A signed committee holds one eighth of new fares
//! in escrow. Only the player's explicit motions advance a hearing.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    Dormant,
    TeaService,
    Quorum,
    HighTea,
    Adjourned,
}

/// One whole exchange viewed from the player's side of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Barter {
    pub tide: u64,
    pub give: usize,
    pub give_qty: u16,
    pub take: usize,
    pub take_qty: u16,
}

impl Barter {
    pub(crate) fn at(tide: u64) -> Self {
        let terms = [
            (0, 3, 2, 1),
            (1, 2, 4, 1),
            (2, 1, 3, 1),
            (4, 1, 1, 2),
            (3, 1, 2, 2),
            (2, 1, 0, 3),
        ];
        let (give, give_qty, take, take_qty) = terms[(tide % 6) as usize];
        Self {
            tide,
            give,
            give_qty,
            take,
            take_qty,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Committee {
    pub stage: Stage,
    /// Total motions heard. Motions 0–2 need salt; 3–5 need provisions.
    pub motions: u8,
    /// Whole coins already earned by the port and held by the quartermaster.
    pub tin: u64,
    /// Remainder of gross fare coins counted toward the next eighth.
    pub carry: u8,
    /// The player has accepted this tide's High Tea barter.
    pub bartered: bool,
    /// Escrow already earned before this run. Claiming it can lawfully bring
    /// today's till above today's gross without generating new charter credit.
    pub(crate) brought_forward: u64,
}

impl Committee {
    pub const fn new() -> Self {
        Self {
            stage: Stage::Dormant,
            motions: 0,
            tin: 0,
            carry: 0,
            bartered: false,
            brought_forward: 0,
        }
    }

    pub fn active(&self) -> bool {
        matches!(
            self.stage,
            Stage::TeaService | Stage::Quorum | Stage::HighTea
        )
    }
    pub fn souvenir(&self) -> bool {
        self.stage == Stage::Adjourned
    }
    pub fn motion_good(&self) -> Option<usize> {
        match self.stage {
            Stage::TeaService => Some(0),
            Stage::Quorum => Some(3),
            _ => None,
        }
    }

    /// Advance the fractional counter per fare stream, before saturation of
    /// the combined income. This keeps huge and partitioned advances identical.
    pub(crate) fn apportion(&mut self, gross: u128) -> u128 {
        if !self.active() {
            return gross;
        }
        let counted = gross + u128::from(self.carry);
        let held = counted / 8;
        self.carry = (counted % 8) as u8;
        self.tin = self
            .tin
            .saturating_add(held.min(u128::from(u64::MAX)) as u64);
        gross - held
    }

    pub(crate) fn valid(&self) -> bool {
        if self.carry >= 8 || (self.bartered && self.stage != Stage::HighTea) {
            return false;
        }
        match self.stage {
            Stage::Dormant => *self == Self::new(),
            Stage::TeaService => self.motions < 3,
            Stage::Quorum => (3..6).contains(&self.motions),
            Stage::HighTea => self.motions == 6,
            Stage::Adjourned => self.motions == 6 && self.tin == 0 && self.carry == 0,
        }
    }
}

impl Default for Committee {
    fn default() -> Self {
        Self::new()
    }
}

/// The sea's additional High Tea bell response. This host-owned cue is live
/// performance only. Clear it when parking the window or restoring a ledger.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct SeaEcho {
    strike_ms: Option<u64>,
    pending: bool,
}
impl SeaEcho {
    pub const fn new() -> Self {
        Self {
            strike_ms: None,
            pending: false,
        }
    }

    pub fn ring(&mut self, now_ms: u64, stage: Stage) -> bool {
        if stage != Stage::HighTea {
            self.clear();
            return false;
        }
        if self
            .strike_ms
            .and_then(|t| now_ms.checked_sub(t))
            .is_some_and(|age| age < 5_000)
        {
            return false;
        }
        self.strike_ms = Some(now_ms);
        self.pending = true;
        true
    }

    pub fn poll(&mut self, now_ms: u64, stage: Stage) -> bool {
        if stage != Stage::HighTea {
            self.clear();
            return false;
        }
        if !self.pending {
            return false;
        }
        let Some(age) = self.strike_ms.and_then(|t| now_ms.checked_sub(t)) else {
            self.clear();
            return false;
        };
        if age < 2_300 {
            return false;
        }
        self.pending = false;
        age < 3_300
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sea_only_answers_live_high_tea_rings_once() {
        let mut echo = SeaEcho::new();
        assert!(!echo.ring(0, Stage::Quorum));
        assert!(echo.ring(100, Stage::HighTea));
        assert!(!echo.ring(1_000, Stage::HighTea));
        assert!(!echo.poll(2_399, Stage::HighTea));
        assert!(echo.poll(2_400, Stage::HighTea));
        assert!(!echo.poll(2_401, Stage::HighTea));
        assert!(echo.ring(5_100, Stage::HighTea));
        assert!(!echo.poll(8_400, Stage::HighTea));
        assert!(echo.ring(10_100, Stage::HighTea));
        assert!(!echo.poll(12_400, Stage::Adjourned));
        assert!(!echo.poll(12_401, Stage::HighTea));
        assert!(echo.ring(u64::MAX, Stage::HighTea));
        assert!(!echo.poll(0, Stage::HighTea));
    }
}
