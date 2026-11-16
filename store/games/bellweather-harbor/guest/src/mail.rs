//! Mailbag timing. The marker makes one return trip in 2.4 seconds.
use bellweather::economy::Economy;

pub struct MailRun {
    pub started: u64,
    pub outcome: Catch,
}
pub enum Catch {
    Sailing,
    Missed,
    Stamped,
    AlreadyHeld,
    Closed,
}

impl MailRun {
    pub fn open(game: &Economy, berth: usize, now: u64) -> Self {
        let outcome = if game.shops[berth].owned == 0 {
            Catch::Closed
        } else if game.postmarks & (1 << berth) != 0 {
            Catch::AlreadyHeld
        } else {
            Catch::Sailing
        };
        Self {
            started: now,
            outcome,
        }
    }
    pub fn position(&self, now: u64) -> u64 {
        let phase = now.saturating_sub(self.started) % 2400;
        phase.min(2400 - phase)
    }
    /// Captures the instant of keydown. A missed bag can be retried immediately.
    pub fn attempt(&mut self, game: &mut Economy, berth: usize, now: u64) -> bool {
        if matches!(self.outcome, Catch::Missed) {
            self.started = now;
            self.outcome = Catch::Sailing;
        } else if matches!(self.outcome, Catch::Sailing) {
            if (480..=720).contains(&self.position(now)) && game.mark_berth(berth) {
                self.outcome = Catch::Stamped;
                return true;
            }
            self.outcome = Catch::Missed;
        }
        false
    }
}
