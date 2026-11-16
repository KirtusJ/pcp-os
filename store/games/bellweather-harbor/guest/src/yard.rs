//! Shipyard controls. Recipes, payment and elapsed time belong to the shared game.
use crate::{device, Cue, Outcome};
use bellweather::{economy::Economy, yard::PLANS};

pub enum Notice {
    Plan,
    Paid,
    Busy,
    Missing,
    Prior,
    Finished,
    Launched(usize),
}
pub struct Slipway {
    pub selected: usize,
    pub notice: Notice,
    pub notice_until: u64,
}
impl Slipway {
    pub fn open(game: &Economy) -> Self {
        Self {
            selected: game.yard.next().unwrap_or(PLANS.len() - 1),
            notice: Notice::Plan,
            notice_until: 0,
        }
    }
    pub(super) fn handle(&mut self, key: usize, game: &mut Economy, now: u64) -> Outcome {
        match key {
            2..=4 => self.selected = key - 2,
            72 | 75 => self.selected = (self.selected + PLANS.len() - 1) % PLANS.len(),
            77 | 80 => self.selected = (self.selected + 1) % PLANS.len(),
            28 | 48 => {
                self.notice_until = now + 3300;
                if game.build(self.selected) {
                    self.notice = Notice::Paid;
                    device::log("Bellweather: keel laid\n");
                    return Outcome::Changed(Cue::Purchase);
                }
                self.notice = if game.yard.completed & (1 << self.selected) != 0 {
                    Notice::Finished
                } else if game.yard.building {
                    Notice::Busy
                } else if game.yard.next() != Some(self.selected) {
                    Notice::Prior
                } else {
                    Notice::Missing
                };
            }
            _ => {}
        }
        Outcome::None
    }
}
