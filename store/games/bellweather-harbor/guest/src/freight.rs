//! The deal ticket always executes the quote calculated against the current book.
use crate::{device, Cue, Outcome};
use bellweather::economy::Economy;

pub enum Receipt {
    Open,
    Trade {
        good: usize,
        qty: u16,
        coins: u64,
        buy: bool,
    },
    NoTrade(bool),
    Accepted,
    Delivered(u64),
    AlreadyReserved,
    MissingGoods,
    Expanded,
    InsufficientFunds,
    Full,
}
pub struct Exchange {
    pub selected: usize,
    pub receipt: Receipt,
    pub receipt_until: u64,
    pub history: bool,
    pub order: Option<crate::order::Ticket>,
}
impl Exchange {
    pub fn new() -> Self {
        Self {
            selected: 0,
            receipt: Receipt::Open,
            receipt_until: 0,
            history: false,
            order: None,
        }
    }
    pub(super) fn handle(&mut self, key: usize, game: &mut Economy, now: u64) -> Outcome {
        if key == 38 || (key == 1 && self.order.is_some()) {
            self.order = if self.order.is_some() {
                None
            } else {
                Some(crate::order::Ticket::new(game, self.selected))
            };
            return Outcome::None;
        }
        if let Some(order) = self.order.as_mut() {
            return order.handle(key, game);
        }
        self.receipt_until = now + 3500;
        match key {
            35 => self.history = !self.history,
            2..=6 => self.selected = key - 2,
            72 | 75 => self.selected = (self.selected + 4) % 5,
            80 | 77 => self.selected = (self.selected + 1) % 5,
            48 | 45 | 31 | 30 => {
                let buy = matches!(key, 48 | 45);
                let fill = game.trade(self.selected, buy, matches!(key, 45 | 30));
                if fill.qty > 0 {
                    self.receipt = Receipt::Trade {
                        good: self.selected,
                        qty: fill.qty,
                        coins: fill.coins,
                        buy,
                    };
                    device::log(if buy {
                        "Bellweather: market purchase\n"
                    } else {
                        "Bellweather: market sale\n"
                    });
                    return Outcome::Changed(Cue::Purchase);
                }
                self.receipt = Receipt::NoTrade(buy);
            }
            19 => {
                if game.reserve_order() {
                    self.receipt = Receipt::Accepted;
                    device::log("Bellweather: bill of lading signed\n");
                    return Outcome::Changed(Cue::CastOff);
                }
                self.receipt = if game.freight.reserved {
                    Receipt::AlreadyReserved
                } else {
                    Receipt::MissingGoods
                };
            }
            22 => {
                if game.upgrade_hold() {
                    self.receipt = Receipt::Expanded;
                    device::log("Bellweather: warehouse expanded\n");
                    return Outcome::Changed(Cue::Manager);
                }
                self.receipt = if game.freight.upgrade_cost().is_some() {
                    Receipt::InsufficientFunds
                } else {
                    Receipt::Full
                };
            }
            _ => {}
        }
        Outcome::None
    }
}
