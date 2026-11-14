use crate::{device, Cue, Outcome};
use bellweather::{committee::Stage, economy::Economy};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Business,
    Minutes,
    Adjourn,
    Barter(bellweather::committee::Barter),
}

#[derive(Clone, Copy)]
pub enum Receipt {
    None,
    Signed,
    Filed(usize),
    Claimed(u64),
    Short,
    Kept,
}

pub struct Meeting {
    pub page: Page,
    pub receipt: Receipt,
    pub expected: u8,
    pub minute: usize,
}

impl Meeting {
    pub fn open(g: &Economy) -> Self {
        Self {
            page: Page::Business,
            receipt: Receipt::None,
            expected: g.committee.motions,
            minute: 0,
        }
    }

    pub fn visible(g: &Economy) -> bool {
        g.committee.stage != Stage::Dormant || g.committee_invited()
    }

    pub(super) fn handle(&mut self, key: usize, game: &mut Economy) -> Outcome {
        if let Page::Barter(offer) = self.page {
            match key {
                1 | 14 | 48 => self.page = Page::Business,
                19 => {
                    if let Some(current) = game.sea_offer() {
                        self.page = Page::Barter(current);
                    }
                }
                28 if game.barter(offer.tide) => {
                    device::log("Bellweather: sea barter exchanged\n");
                    return Outcome::Changed(Cue::Purchase);
                }
                _ => {}
            }
            return Outcome::None;
        }
        if self.page == Page::Minutes {
            let last = 3 + usize::from(game.committee.motions);
            match key {
                1 | 14 | 50 => self.page = Page::Business,
                75 => self.minute = self.minute.saturating_sub(1),
                77 => self.minute = (self.minute + 1).min(last),
                _ => {}
            }
            return Outcome::None;
        }
        if self.page == Page::Adjourn {
            match key {
                1 | 14 | 37 => {
                    self.page = Page::Business;
                    self.receipt = Receipt::Kept;
                }
                28 if game.adjourn_committee() => {
                    self.page = Page::Business;
                    self.receipt = Receipt::None;
                    device::log("Bellweather: committee adjourned\n");
                    return Outcome::Changed(Cue::Charter);
                }
                _ => {}
            }
            return Outcome::None;
        }
        match key {
            50 => {
                self.page = Page::Minutes;
                self.minute = 0;
            }
            31 if game.sign_order_thirteen() => {
                self.receipt = Receipt::Signed;
                device::log("Bellweather: standing order thirteen signed\n");
                return Outcome::Changed(Cue::Manager);
            }
            28 if game.committee.motion_good().is_some() => {
                if game.submit_motion(self.expected) {
                    self.receipt = Receipt::Filed(usize::from(self.expected));
                    self.expected = game.committee.motions;
                    device::log("Bellweather: committee motion carried\n");
                    return Outcome::Changed(Cue::Purchase);
                }
                self.receipt = Receipt::Short;
            }
            46 if game.committee.active() => {
                let amount = game.claim_tea_tin();
                self.receipt = Receipt::Claimed(amount);
                if amount > 0 {
                    device::log("Bellweather: tea tin collected\n");
                    return Outcome::Changed(Cue::Purchase);
                }
            }
            30 if game.committee.stage == Stage::HighTea => self.page = Page::Adjourn,
            48 => {
                if let Some(offer) = game.sea_offer() {
                    self.page = Page::Barter(offer);
                }
            }
            _ => {}
        }
        Outcome::None
    }
}
