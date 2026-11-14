//! The town's exchange owns a finite stock of each good. Each step along the
//! inventory curve has a dock bid and a higher ask for the reverse trade.

use crate::factory::PLANS;

pub const NAMES: [&str; 5] = ["Salt", "Crab", "Timber", "Provisions", "Fittings"];
pub const MARKET_CAP: u16 = 96;
pub const OPEN: u16 = 40;
const SEASON: usize = 12;
const WINDOW: usize = 8;

// Reference bids for a normal stock of 40 crates. The first three rows are
// the harbor's original seasonal trading rates.
const REFERENCE: [[u16; SEASON]; 5] = [
    [6, 8, 10, 7, 5, 8, 12, 10, 7, 5, 7, 9],
    [18, 22, 25, 33, 29, 21, 16, 20, 28, 34, 27, 20],
    [42, 34, 36, 49, 68, 62, 47, 36, 41, 56, 72, 54],
    [85, 89, 97, 108, 118, 104, 93, 84, 88, 103, 109, 96],
    [145, 131, 126, 151, 174, 193, 182, 158, 144, 161, 182, 164],
];

// Fishing, ferry and export schedules peak on different tides. Nearby
// suppliers replenish depleted stores while local customers buy excess stock.
const TARGET: [u16; SEASON] = [40, 38, 35, 33, 36, 41, 45, 48, 46, 43, 40, 38];
const GOOD_PHASE: [usize; 5] = [0, 3, 7, 9, 5];
// Profit an outside workshop requires after paying dock asks and its batch fee.
pub const RIVAL_MARGIN: [u64; 2] = [5, 9];

fn scheduled_rival(new_phase: usize) -> Option<usize> {
    match new_phase % 3 {
        1 => Some(1),
        2 => Some(0),
        _ => None,
    }
}

fn town_stock(stock: u16, good: usize, new_phase: usize) -> u16 {
    let target = TARGET[(new_phase + GOOD_PHASE[good]) % SEASON];
    target.clamp(stock.saturating_sub(4), stock + 4)
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Quote {
    pub qty: u16,
    pub coins: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Market {
    pub stock: [u16; 5],
    /// Actual closing bids, newest first. A zero close means the dock was full.
    pub history: [[u16; WINDOW]; 5],
    pub history_len: u8,
}

impl Default for Market {
    fn default() -> Self {
        Self::new()
    }
}

impl Market {
    pub const fn new() -> Self {
        Self {
            stock: [OPEN; 5],
            history: [[0; WINDOW]; 5],
            history_len: 0,
        }
    }

    /// Coins the dock pays for up to qty crates, constrained by available space.
    pub fn bid_lot(&self, good: usize, qty: u16, tide: u64) -> Quote {
        let Some(&stock) = self.stock.get(good) else {
            return Quote::default();
        };
        let reference = REFERENCE[good][(tide % SEASON as u64) as usize];
        let qty = qty.min(MARKET_CAP - stock);
        let coins = (stock..stock + qty)
            .map(|level| u64::from(bid_at(reference, level)))
            .sum();
        Quote { qty, coins }
    }

    /// Coins a player pays for up to qty raw crates, constrained by budget and stock.
    pub fn ask_lot(&self, good: usize, qty: u16, tide: u64, budget: u64) -> Quote {
        if good >= 3 {
            return Quote::default();
        }
        let stock = self.stock[good];
        let reference = REFERENCE[good][(tide % SEASON as u64) as usize];
        let mut quote = Quote::default();
        for level in (stock - stock.min(qty)..stock).rev() {
            // Selling this crate back would reverse the step at the same level.
            let bid = u64::from(bid_at(reference, level));
            let ask = bid + bid.div_ceil(8);
            if ask > budget - quote.coins {
                break;
            }
            quote.qty += 1;
            quote.coins += ask;
        }
        quote
    }

    fn rival_can_clear(&self, works: usize, new_phase: usize) -> bool {
        let plan = &PLANS[works];
        let mut required = plan.fee + RIVAL_MARGIN[works];
        for (raw, needed) in plan.inputs.iter().copied().enumerate() {
            let fill = self.ask_lot(raw, needed, new_phase as u64, u64::MAX);
            if fill.qty != needed {
                return false;
            }
            required += fill.coins;
        }
        let receipt = self.bid_lot(plan.good, 1, new_phase as u64);
        receipt.qty == 1 && receipt.coins >= required
    }

    /// Outside works pay the post-restock book, buy a whole recipe and sell one
    /// finished crate. These traders have independent tills and hold no cargo
    /// between tides. The player's inventory and working capital are untouched.
    fn clear_rival(&mut self, new_phase: usize) -> [bool; 2] {
        let mut cleared = [false; 2];
        if let Some(works) = scheduled_rival(new_phase) {
            if self.rival_can_clear(works, new_phase) {
                let plan = &PLANS[works];
                for (held, needed) in self.stock.iter_mut().zip(plan.inputs) {
                    *held -= needed;
                }
                self.stock[plan.good] += 1;
                cleared[works] = true;
            }
        }
        cleared
    }

    /// Forecast the next closing's outside work, assuming no player trade first.
    pub fn project_rival(&self, current_tide: u64) -> [bool; 2] {
        let mut projected = self.clone();
        projected.close_tide((current_tide % SEASON as u64) as usize)
    }

    fn close_tide(&mut self, phase: usize) -> [bool; 2] {
        let new_phase = (phase + 1) % SEASON;
        for (good, stock) in self.stock.iter_mut().enumerate() {
            let closes = &mut self.history[good];
            closes.copy_within(..WINDOW - 1, 1);
            closes[0] = if *stock == MARKET_CAP {
                0
            } else {
                bid_at(REFERENCE[good][phase], *stock)
            };
            *stock = town_stock(*stock, good, new_phase);
        }
        self.history_len = (self.history_len + 1).min(WINDOW as u8);
        self.clear_rival(new_phase)
    }

    /// Settle all intervening tides without inventing any player trades.
    ///
    /// At most 47 closings are needed for any elapsed time: all legal stocks
    /// at a given starting phase converge within 19 tides, and subsequent
    /// 12-tide seasons repeat the same stocks and all eight recorded closes.
    /// The interval proof in the tests certifies this bound for the authored
    /// prices, recipes, margins and town targets at every starting phase.
    pub fn advance(&mut self, tide: u64, mut crossed: u64) {
        let initial_phase = (tide % SEASON as u64) as usize;
        while crossed >= SEASON as u64 {
            let before = self.stock;
            for i in 0..SEASON {
                self.close_tide((initial_phase + i) % SEASON);
            }
            crossed -= SEASON as u64;
            if self.stock == before {
                // A whole season returned to the same stocks. Prices depend
                // on phase and stock, so future seasons reproduce this history.
                crossed %= SEASON as u64;
                break;
            }
        }
        for i in 0..crossed {
            self.close_tide((initial_phase + i as usize) % SEASON);
        }
    }

    pub fn valid(&self) -> bool {
        let len = usize::from(self.history_len);
        if len > WINDOW || self.stock.iter().any(|&n| n > MARKET_CAP) {
            return false;
        }
        self.history.iter().enumerate().all(|(good, closes)| {
            let maximum = bid_at(*REFERENCE[good].iter().max().unwrap_or(&0), 0);
            closes[..len].iter().all(|&n| n <= maximum) && closes[len..].iter().all(|&n| n == 0)
        })
    }
}

// Rounded inverse inventory curve. At stock 40 the price is exactly the
// reference; at stock zero willing buyers offer triple the reference.
fn bid_at(reference: u16, stock: u16) -> u16 {
    let denominator = 20 + u32::from(stock);
    ((u32::from(reference) * (20 + u32::from(OPEN)) + denominator / 2) / denominator) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_quotes_and_liquidity_limits() {
        let mut dock = Market::new();
        for (good, expected) in [6, 18, 42, 85, 145].into_iter().enumerate() {
            assert_eq!(
                dock.bid_lot(good, 1, 0),
                Quote {
                    qty: 1,
                    coins: expected
                }
            );
        }
        assert_eq!(dock.ask_lot(0, u16::MAX, 0, 13), Quote { qty: 1, coins: 7 });
        assert_eq!(
            dock.ask_lot(0, u16::MAX, 0, 14),
            Quote { qty: 2, coins: 14 }
        );
        assert_eq!(dock.ask_lot(0, u16::MAX, 0, u64::MAX).qty, OPEN);
        assert_eq!(dock.bid_lot(0, u16::MAX, 0).qty, MARKET_CAP - OPEN);
        assert_eq!(dock.ask_lot(3, 1, 0, u64::MAX), Quote::default());
        assert_eq!(dock.bid_lot(usize::MAX, 1, 0), Quote::default());
        dock.stock[1] = MARKET_CAP;
        assert_eq!(dock.bid_lot(1, 1, 0), Quote::default());
        dock.stock[1] = 0;
        assert_eq!(dock.ask_lot(1, 1, 0, u64::MAX), Quote::default());
    }

    #[test]
    fn round_trips_pay_a_spread_and_bulk_matches_unit_trades_in_both_directions() {
        for tide in 0..SEASON as u64 {
            for good in 0..3 {
                for stock in 0..=MARKET_CAP {
                    let mut dock = Market::new();
                    dock.stock[good] = stock;
                    let ask = dock.ask_lot(good, 7, tide, u64::MAX);
                    let mut split = 0;
                    for _ in 0..ask.qty {
                        split += dock.ask_lot(good, 1, tide, u64::MAX).coins;
                        dock.stock[good] -= 1;
                    }
                    assert_eq!(split, ask.coins);
                    let bid = dock.bid_lot(good, ask.qty, tide);
                    assert_eq!(bid.qty, ask.qty);
                    assert!(bid.qty == 0 || bid.coins < ask.coins);
                    split = 0;
                    for _ in 0..bid.qty {
                        split += dock.bid_lot(good, 1, tide).coins;
                        dock.stock[good] += 1;
                    }
                    assert_eq!(split, bid.coins);
                    assert_eq!(dock.stock[good], stock);
                    let sale = dock.bid_lot(good, 7, tide);
                    dock.stock[good] += sale.qty;
                    let buyback = dock.ask_lot(good, sale.qty, tide, u64::MAX);
                    assert_eq!(sale.qty, buyback.qty);
                    assert!(sale.qty == 0 || sale.coins < buyback.coins);
                }
            }
        }
    }

    #[test]
    fn local_demand_absorbs_surplus_and_history_records_actual_closes() {
        let mut dock = Market::new();
        let mut projected = dock.clone();
        projected.advance(2, 1);
        dock.stock = [0, 96, 70, 64, 12];
        // Tide 3 has town customers alone, without an outside workshop.
        let last = core::array::from_fn::<_, 5, _>(|g| dock.bid_lot(g, 1, 2).coins);
        dock.advance(2, 1);
        assert_eq!(dock.stock, [4, 92, 66, 60, 16]);
        assert_eq!(dock.history_len, 1);
        for (g, expected) in last.into_iter().enumerate() {
            assert_eq!(u64::from(dock.history[g][0]), expected);
        }
        assert_eq!(dock.history[1][0], 0);
        assert_ne!(dock.bid_lot(0, 1, 3), projected.bid_lot(0, 1, 3));
        assert!(dock.valid());
    }

    #[test]
    fn bulk_updates_match_the_actual_individual_closes_even_for_huge_deltas() {
        let mut opening = Market::new();
        opening.stock = [0, 5, 95, 12, 78];
        let start = 9;
        for crossed in [0, 1, 7, 12, 24, 55, 101, 999] {
            let mut bulk = opening.clone();
            let mut stepped = opening.clone();
            bulk.advance(start, crossed);
            for i in 0..crossed {
                stepped.advance(start + i, 1);
            }
            assert_eq!(stepped, bulk);
            assert!(bulk.valid());
        }
        let mut bulk = opening.clone();
        bulk.advance(start, u64::MAX);
        let mut split = opening;
        let first = u64::MAX / 2;
        split.advance(start, first);
        split.advance(start + first, u64::MAX - first);
        assert_eq!(split, bulk);
        assert_eq!(bulk.history_len, 8);
    }

    #[test]
    fn workshops_only_process_profitable_recipes_with_room_and_stock() {
        for (i, phase) in [(0, 2), (1, 1)] {
            let plan = &PLANS[i];
            let mut dock = Market::new();
            assert!(dock.rival_can_clear(i, phase));
            let mut expected = dock.stock;
            for (held, qty) in expected.iter_mut().zip(plan.inputs) {
                *held -= qty;
            }
            expected[plan.good] += 1;
            assert!(dock.clear_rival(phase)[i]);
            assert_eq!(dock.stock, expected);
            dock.stock = [OPEN; 5];
            for scarce in [0, i + 1, plan.good] {
                let mut denied = dock.clone();
                denied.stock[scarce] = if scarce == plan.good { MARKET_CAP } else { 0 };
                let before = denied.stock;
                assert_eq!(denied.clear_rival(phase), [false; 2]);
                assert_eq!(denied.stock, before);
            }
            // The inputs remain physically available, but scarcity makes buying them unprofitable.
            dock.stock[0] = 1;
            dock.stock[i + 1] = 2;
            dock.stock[plan.good] = MARKET_CAP - 1;
            let before = dock.stock;
            assert_eq!(dock.clear_rival(phase), [false; 2]);
            assert_eq!(dock.stock, before);
        }
    }

    #[test]
    fn margin_floor_is_inclusive_and_changes_decisions_by_one_coin() {
        for (i, phase) in [(0, 2), (1, 1)] {
            let plan = &PLANS[i];
            let mut saw = [false; 2];
            let mut dock = Market::new();
            for raw_stock in 2..=MARKET_CAP {
                dock.stock[i + 1] = raw_stock;
                let required = dock.ask_lot(0, 1, phase, u64::MAX).coins
                    + dock.ask_lot(i + 1, 2, phase, u64::MAX).coins
                    + plan.fee
                    + RIVAL_MARGIN[i];
                for product_stock in 0..MARKET_CAP {
                    dock.stock[plan.good] = product_stock;
                    let bid = dock.bid_lot(plan.good, 1, phase).coins;
                    if bid == required {
                        assert!(dock.rival_can_clear(i, phase as usize));
                        saw[1] = true;
                    } else if bid + 1 == required {
                        assert!(!dock.rival_can_clear(i, phase as usize));
                        saw[0] = true;
                    }
                }
            }
            assert_eq!(saw, [true; 2]);
        }
    }

    #[test]
    fn forecast_and_actual_closing_use_the_same_conditional_trade() {
        let mut seed = 51u32;
        let mut both_outcomes = [[false; 2]; 2];
        for n in 0..4_096 {
            let mut dock = Market::new();
            for stock in &mut dock.stock {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *stock = (seed % 97) as u16;
            }
            let phase = n % SEASON;
            let next_phase = (phase + 1) % SEASON;
            let prediction = dock.project_rival(phase as u64);
            let mut expected: [u16; 5] =
                core::array::from_fn(|g| town_stock(dock.stock[g], g, next_phase));
            if let Some(i) = scheduled_rival(next_phase) {
                both_outcomes[i][usize::from(prediction[i])] = true;
                if prediction[i] {
                    for (held, needed) in expected.iter_mut().zip(PLANS[i].inputs) {
                        *held -= needed;
                    }
                    expected[PLANS[i].good] += 1;
                }
            } else {
                assert_eq!(prediction, [false; 2]);
            }
            dock.advance(phase as u64, 1);
            assert_eq!(dock.stock, expected);
            assert!(dock.valid());
        }
        assert_eq!(both_outcomes, [[true; 2]; 2]);
    }

    #[test]
    fn every_legal_inventory_at_each_starting_phase_coalesces_by_tide_nineteen() {
        for start in 0..SEASON {
            // This box initially contains all 97^5 possible dock inventories.
            let mut low = [0; 5];
            let mut high = [MARKET_CAP; 5];
            for k in 1..=19 {
                let phase = (start + k) % SEASON;
                for g in 0..5 {
                    // Town movement is nondecreasing in the stock it starts from.
                    low[g] = town_stock(low[g], g, phase);
                    high[g] = town_stock(high[g], g, phase);
                }
                if let Some(i) = scheduled_rival(phase) {
                    let plan = &PLANS[i];
                    // Having more raw stock only lowers its ask and improves
                    // availability. Lower product stock raises its bid and
                    // leaves more room. The corners thus bound eligibility.
                    let mut worst = Market {
                        stock: low,
                        ..Market::new()
                    };
                    worst.stock[plan.good] = high[plan.good];
                    let must_run = worst.rival_can_clear(i, phase);
                    let mut best = Market {
                        stock: high,
                        ..Market::new()
                    };
                    best.stock[plan.good] = low[plan.good];
                    let can_run = best.rival_can_clear(i, phase);
                    if can_run {
                        for (held, qty) in low.iter_mut().zip(plan.inputs) {
                            *held = held.saturating_sub(qty);
                        }
                        high[plan.good] = (high[plan.good] + 1).min(MARKET_CAP);
                    }
                    if must_run {
                        for (held, qty) in high.iter_mut().zip(plan.inputs) {
                            *held -= qty;
                        }
                        low[plan.good] += 1;
                    }
                }
            }
            assert_eq!(low, high, "two starts could differ at phase {}", start);
        }
    }
}
