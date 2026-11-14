//! A local draft. Only an accepted instruction is written to the shared ledger.
use crate::{device, Cue, Outcome};
use bellweather::{
    economy::{Economy, TicketStatus},
    market::MARKET_CAP,
};
pub enum Message {
    Draft,
    Placed,
    Cancelled,
    Busy,
    NoPending,
}
pub struct Ticket {
    pub good: usize,
    pub qty: u16,
    pub floor: u16,
    pub step: u16,
    pub message: Message,
}
impl Ticket {
    pub fn new(game: &Economy, selected: usize) -> Self {
        let pending = &game.ticket;
        if pending.status == TicketStatus::Pending {
            Self {
                good: pending.good as usize,
                qty: pending.requested,
                floor: pending.floor,
                step: 1,
                message: Message::Draft,
            }
        } else {
            Self {
                good: selected,
                qty: 1,
                floor: game.freight.price(selected).clamp(1, u64::from(u16::MAX)) as u16,
                step: 1,
                message: Message::Draft,
            }
        }
    }
    pub(super) fn handle(&mut self, key: usize, g: &mut Economy) -> Outcome {
        if key == 46 {
            if g.cancel_sale() {
                self.message = Message::Cancelled;
                device::log("Bellweather: standing order cancelled\n");
                return Outcome::Changed(Cue::Purchase);
            }
            self.message = Message::NoPending;
            return Outcome::None;
        }
        if g.ticket.status == TicketStatus::Pending {
            self.message = Message::Busy;
            return Outcome::None;
        }
        self.message = Message::Draft;
        match key {
            2..=6 => {
                self.good = key - 2;
                self.floor = g.freight.price(self.good).clamp(1, u64::from(u16::MAX)) as u16;
            }
            75 => self.qty = self.qty.saturating_sub(self.step).max(1),
            77 => self.qty = self.qty.saturating_add(self.step).min(MARKET_CAP),
            72 => self.floor = self.floor.saturating_add(self.step),
            80 => self.floor = self.floor.saturating_sub(self.step).max(1),
            45 => {
                self.step = if self.step == 1 {
                    10
                } else if self.step == 10 {
                    100
                } else {
                    1
                }
            }
            28 => {
                if g.place_sale(self.good, self.floor, self.qty) {
                    self.message = Message::Placed;
                    device::log("Bellweather: standing order placed\n");
                    return Outcome::Changed(Cue::CastOff);
                }
                self.message = Message::Busy;
            }
            _ => {}
        }
        Outcome::None
    }
}
