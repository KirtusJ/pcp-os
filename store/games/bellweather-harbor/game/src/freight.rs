//! Freight holds, posted orders, and the current exchange book.

use crate::market::{Market, Quote};

pub const GOODS: [&str; 3] = ["Salt", "Crab", "Timber"];
pub const TIDE_MS: u32 = 30_000;
pub const TIDE_COUNT: usize = 12;

// Wrap the serial number after a whole number of seasons.
const TIDE_WRAP: u64 = u64::MAX - (u64::MAX % TIDE_COUNT as u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Order {
    pub good: usize,
    pub qty: u16,
    pub reward: u64,
}

const ORDERS: [(usize, u16, u64); TIDE_COUNT] = [
    (0, 3, 42),
    (1, 2, 56),
    (0, 7, 91),
    (2, 2, 120),
    (1, 5, 175),
    (0, 9, 108),
    (2, 4, 248),
    (1, 6, 162),
    (0, 10, 110),
    (2, 5, 340),
    (1, 8, 256),
    (0, 5, 65),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Freight {
    pub stock: [u16; 3],
    pub market: Market,
    pub hold_level: u8,
    pub tide: u64,
    pub elapsed_ms: u16,
    /// The current tide's consignment has already left inventory.
    pub reserved: bool,
}

impl Default for Freight {
    fn default() -> Self {
        Self::new()
    }
}

impl Freight {
    pub const fn new() -> Self {
        Self {
            stock: [0; 3],
            market: Market::new(),
            hold_level: 0,
            tide: 0,
            elapsed_ms: 0,
            reserved: false,
        }
    }

    /// Storage limit for each commodity.
    pub fn capacity(&self) -> u16 {
        24 << self.hold_level.min(2)
    }

    pub fn upgrade_cost(&self) -> Option<u64> {
        [1_800, 14_000].get(usize::from(self.hold_level)).copied()
    }

    pub fn price(&self, good: usize) -> u64 {
        self.bid_lot(good, 1).coins
    }

    pub fn ask(&self, good: usize) -> u64 {
        self.ask_lot(good, 1).coins
    }

    pub fn bid_lot(&self, good: usize, qty: u16) -> Quote {
        self.market.bid_lot(good, qty, self.tide)
    }

    pub fn ask_lot(&self, good: usize, qty: u16) -> Quote {
        self.market.ask_lot(good, qty, self.tide, u64::MAX)
    }

    /// The next tide's bid assuming there are no intervening trades.
    pub fn projected_price(&self, good: usize) -> u64 {
        let mut future = self.market.clone();
        future.advance(self.tide, 1);
        future.bid_lot(good, 1, (self.tide % 12 + 1) % 12).coins
    }

    pub fn relative_price(&self, good: usize, offset: i32) -> u64 {
        if offset == 0 {
            return self.price(good);
        }
        if offset == 1 {
            return self.projected_price(good);
        }
        if offset > 0 || offset.unsigned_abs() > u32::from(self.market.history_len) {
            return 0;
        }
        self.market
            .history
            .get(good)
            .map_or(0, |h| u64::from(h[offset.unsigned_abs() as usize - 1]))
    }

    pub fn offer(&self) -> Order {
        let (good, qty, reward) = ORDERS[(self.tide % TIDE_COUNT as u64) as usize];
        Order { good, qty, reward }
    }

    pub(crate) fn unload(&mut self, berth: usize, completed: u64) -> u16 {
        let cap = self.capacity();
        let Some(stock) = self.stock.get_mut(berth) else {
            return 0;
        };
        let landed = completed.min(u64::from(cap - *stock)) as u16;
        *stock += landed;
        landed
    }

    pub(crate) fn reserve_order(&mut self) -> bool {
        let order = self.offer();
        if self.reserved || self.stock[order.good] < order.qty {
            return false;
        }
        self.stock[order.good] -= order.qty;
        self.reserved = true;
        true
    }

    /// Advance the clock in constant time. Only a player's existing order can
    /// settle; the following tides wait for their own explicit reservation.
    pub(crate) fn advance(&mut self, ms: u64) -> u64 {
        let period = u64::from(TIDE_MS);
        let fraction = ms % period + u64::from(self.elapsed_ms);
        let crossed = ms / period + fraction / period;
        self.elapsed_ms = (fraction % period) as u16;
        if crossed == 0 {
            return 0;
        }
        let proceeds = if self.reserved {
            self.offer().reward
        } else {
            0
        };
        self.reserved = false;
        self.market.advance(self.tide, crossed);
        let remaining = TIDE_WRAP - self.tide;
        self.tide = if crossed >= remaining {
            crossed - remaining
        } else {
            self.tide + crossed
        };
        proceeds
    }

    pub(crate) fn valid(&self) -> bool {
        self.hold_level <= 2
            && self.market.valid()
            && self.tide < TIDE_WRAP
            && u32::from(self.elapsed_ms) < TIDE_MS
            && self.stock.iter().all(|&qty| qty <= self.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_quotes_and_market_seasons_wrap_including_serial_overflow() {
        let mut board = Freight::new();
        assert_eq!(board.price(0), 6);
        assert_eq!(board.relative_price(0, -1), 0);
        assert_eq!(board.relative_price(0, 12), 0);
        assert_eq!(board.relative_price(10, i32::MIN), 0);
        board.advance(u64::from(TIDE_MS) * TIDE_COUNT as u64);
        assert_eq!(board.tide, TIDE_COUNT as u64);
        assert_eq!(board.offer(), Freight::new().offer());
        board.tide = TIDE_WRAP - 1;
        board.elapsed_ms = (TIDE_MS - 1) as u16;
        assert!(board.price(0) > 0);
        let mut bulk = board.clone();
        board.advance(1);
        assert_eq!(board.tide, 0);
        assert!(board.price(0) > 0);
        board.advance(u64::MAX - 1);
        bulk.advance(u64::MAX);
        assert_eq!(board, bulk);
        assert!(board.valid());
    }
}
