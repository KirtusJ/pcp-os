//! Console actions for the two production lines. The shared economy validates every cost.
use crate::{device, Cue, Outcome};
use bellweather::{economy::Economy, factory::PLANS};

pub enum Report {
    Idle,
    Opened,
    Started,
    AutoOn,
    AutoOff,
    Sold(u16, u64),
    Unavailable,
    Busy,
    Closed,
    NoSale,
    AlreadyOpen,
}
pub struct WorksDesk {
    pub selected: usize,
    pub ledger: bool,
    pub cargo_log: bool,
    pub report: Report,
    pub until: u64,
}
impl WorksDesk {
    pub fn new() -> Self {
        Self {
            selected: 0,
            ledger: false,
            cargo_log: false,
            report: Report::Idle,
            until: 0,
        }
    }
    pub(super) fn handle(&mut self, key: usize, game: &mut Economy, now: u64) -> Outcome {
        self.until = now + 3400;
        if key == 38 {
            self.ledger = !self.ledger;
            return Outcome::None;
        }
        if self.ledger {
            if key == 46 {
                self.cargo_log = !self.cargo_log;
            }
            return Outcome::None;
        }
        match key {
            2..=3 => {
                self.selected = key - 2;
                self.report = Report::Idle;
            }
            72 | 75 | 77 | 80 => {
                self.selected = 1 - self.selected;
                self.report = Report::Idle;
            }
            24 => {
                if game.open_factory(self.selected) {
                    self.report = Report::Opened;
                    device::log("Bellweather: works opened\n");
                    return Outcome::Changed(Cue::Manager);
                }
                self.report = if game.factories[self.selected].built {
                    Report::AlreadyOpen
                } else {
                    Report::Unavailable
                };
            }
            28 | 57 => {
                if game.start_batch(self.selected) {
                    self.report = Report::Started;
                    device::log("Bellweather: paid batch started\n");
                    return Outcome::Changed(Cue::CastOff);
                }
                let f = &game.factories[self.selected];
                self.report = if !f.built {
                    Report::Closed
                } else if f.running {
                    Report::Busy
                } else {
                    Report::Unavailable
                };
            }
            20 => {
                let on = !game.factories[self.selected].repeat;
                if game.set_repeat(self.selected, on) {
                    self.report = if on { Report::AutoOn } else { Report::AutoOff };
                    device::log("Bellweather: works repeat updated\n");
                    return Outcome::Changed(Cue::Purchase);
                }
                self.report = Report::Closed;
            }
            31 | 30 => {
                let fill = game.trade(PLANS[self.selected].good, false, key == 30);
                if fill.qty > 0 {
                    self.report = Report::Sold(fill.qty, fill.coins);
                    device::log("Bellweather: finished goods sold\n");
                    return Outcome::Changed(Cue::Purchase);
                }
                self.report = Report::NoSale;
            }
            _ => {}
        }
        Outcome::None
    }
}
