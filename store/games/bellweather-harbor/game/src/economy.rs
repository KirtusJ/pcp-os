//! Bellweather Harbor's deterministic economy. Time is supplied by its host.
//! Prices and payouts are whole coins. Every port handles the same save bytes.

use crate::committee::{Barter, Committee, Stage};
use crate::factory::{Factory, PLANS as WORKS};
use crate::freight::{Freight, TIDE_MS};
use crate::manifest::{Ending, Manifest};
use crate::market::Quote;
use crate::neighbors::{self, Favor, Neighbors};
use crate::yard::{Yard, PLANS};
use core::{cell::Cell, convert::TryInto};

pub const SHOP_NAMES: [&str; 5] = ["Rowboat", "Crab Pots", "Ferry", "Dry Dock", "Ocean Liner"];
pub const MAX_UNITS: u16 = 200;
pub const MILESTONES: [u16; 6] = [10, 25, 50, 100, 150, 200];
pub const CHARTER_UNIT: u64 = 500_000;
pub const CHARTER_BONUS_PERCENT: u32 = 25;
pub const OFFLINE_LIMIT_S: u64 = 12 * 60 * 60;
pub const LEGACY_SAVE_LEN: usize = 71;
pub const V2_SAVE_LEN: usize = 72;
pub const V3_SAVE_LEN: usize = 90;
pub const V4_SAVE_LEN: usize = 96;
pub const V5_INITIAL_SAVE_LEN: usize = 507;
pub const V5_SAVE_LEN: usize = 526;
pub const V6_SAVE_LEN: usize = 532;
pub const V7_SAVE_LEN: usize = 545;
pub const V8_INITIAL_SAVE_LEN: usize = 564;
pub const SAVE_LEN: usize = 565;
const VALID_POSTMARKS: u8 = (1 << SHOP_NAMES.len()) - 1;

const BASE_COST: [u64; 5] = [12, 140, 2_200, 34_000, 480_000];
const UNIT_YIELD: [u64; 5] = [8, 56, 540, 6_400, 57_600];
const CYCLE_MS: [u32; 5] = [2_000, 6_000, 14_000, 26_000, 45_000];
const MANAGER_COST: [u64; 5] = [96, 750, 9_000, 90_000, 900_000];

// Prefix sums give each bundle exactly the same price as buying its units singly.
const fn price_table() -> [[u64; MAX_UNITS as usize + 1]; 5] {
    let mut table = [[0; MAX_UNITS as usize + 1]; 5];
    let mut i = 0;
    while i < 5 {
        let mut price = BASE_COST[i];
        let mut n = 0;
        while n < MAX_UNITS as usize {
            table[i][n + 1] = table[i][n] + price;
            price += (price * 13).div_ceil(100);
            n += 1;
        }
        i += 1;
    }
    table
}
static PRICES: [[u64; MAX_UNITS as usize + 1]; 5] = price_table();

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Bulk {
    One,
    Ten,
    Max,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Shop {
    pub owned: u16,
    pub manager: bool,
    pub running: bool,
    pub elapsed_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TicketStatus {
    Empty,
    Pending,
    Filled,
    Expired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub good: u8,
    pub floor: u16,
    pub requested: u16,
    pub remaining: u16,
    pub filled: u16,
    pub proceeds: u64,
    pub tides_left: u8,
    pub status: TicketStatus,
}

impl Ticket {
    pub const fn new() -> Self {
        Self {
            good: 0,
            floor: 0,
            requested: 0,
            remaining: 0,
            filled: 0,
            proceeds: 0,
            tides_left: 0,
            status: TicketStatus::Empty,
        }
    }

    fn valid(&self) -> bool {
        if self.status == TicketStatus::Empty {
            return *self == Self::new();
        }
        if self.good >= 5
            || self.floor == 0
            || !(1..=96).contains(&self.requested)
            || self.remaining.checked_add(self.filled) != Some(self.requested)
            || self.tides_left > 12
            || (self.filled == 0 && self.proceeds != 0)
            || self.proceeds < u64::from(self.filled) * u64::from(self.floor)
            || self.proceeds > u64::from(self.filled) * u64::from(u16::MAX)
        {
            return false;
        }
        match self.status {
            TicketStatus::Pending => self.remaining > 0 && self.tides_left > 0,
            TicketStatus::Filled => self.remaining == 0 && self.tides_left == 0,
            TicketStatus::Expired | TicketStatus::Cancelled => {
                self.remaining > 0 && self.tides_left == 0
            }
            TicketStatus::Empty => false,
        }
    }
}

/// Physical movements during this charter. Opening carries stock from an older ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Flow {
    pub opening: u16,
    pub landed: u64,
    pub bought: u64,
    pub made: u64,
    pub sold: u64,
    pub used: u64,
    pub delivered: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Ledger {
    pub sales: u64,
    pub purchases: u64,
    pub fees: u64,
    pub construction: u64,
    pub orders: u64,
    pub flows: [Flow; 5],
    // Trade receipts recover purchase and processing outlays before qualifying for charters.
    unrecovered: u64,
    eligible: u64,
}

impl Ledger {
    const fn new() -> Self {
        let flow = Flow {
            opening: 0,
            landed: 0,
            bought: 0,
            made: 0,
            sold: 0,
            used: 0,
            delivered: 0,
        };
        Self {
            sales: 0,
            purchases: 0,
            fees: 0,
            construction: 0,
            orders: 0,
            flows: [flow; 5],
            unrecovered: 0,
            eligible: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Economy {
    pub cash: u64,
    /// Gross receipts since the most recent charter. Spending never reduces this.
    pub earned: u64,
    pub charters: u32,
    /// Permanent Night Post stamps, one bit per berth in shop order.
    pub postmarks: u8,
    pub freight: Freight,
    pub yard: Yard,
    pub factories: [Factory; 2],
    pub ledger: Ledger,
    pub ticket: Ticket,
    pub manifest: Manifest,
    pub neighbors: Neighbors,
    pub committee: Committee,
    pub shops: [Shop; 5],
    // A clock correction must not turn hours already settled into new earnings.
    settled_through_s: Cell<u64>,
}

impl Default for Economy {
    fn default() -> Self {
        Self::new()
    }
}

impl Economy {
    pub const fn new() -> Self {
        let mut shops = [Shop {
            owned: 0,
            manager: false,
            running: false,
            elapsed_ms: 0,
        }; 5];
        shops[0].owned = 1;
        Self {
            cash: 0,
            earned: 0,
            charters: 0,
            postmarks: 0,
            freight: Freight::new(),
            yard: Yard::new(),
            factories: [Factory::new(); 2],
            ledger: Ledger::new(),
            ticket: Ticket::new(),
            manifest: Manifest::new(),
            neighbors: Neighbors::new(),
            committee: Committee::new(),
            shops,
            settled_through_s: Cell::new(0),
        }
    }

    pub fn inspect_lead(&mut self, chapter: usize, lead: usize) -> bool {
        self.manifest.inspect(chapter, lead)
    }
    pub fn prove_pair(&mut self, chapter: usize, a: usize, b: usize) -> bool {
        self.manifest.prove(chapter, a, b)
    }
    pub fn choose_ending(&mut self, choice: u8) -> bool {
        self.manifest.choose(choice)
    }
    pub fn bell_tip(&self) -> u64 {
        1 + u64::from(self.manifest.ending == Ending::Print)
    }

    /// A tip from the quay, and an immediate departure when the rowboat is idle.
    pub fn ring(&mut self) -> u64 {
        let tip = self.bell_tip();
        self.credit(tip);
        self.dispatch(0);
        self.retry_works();
        tip
    }

    /// One click dispatches the entire business for one cycle.
    pub fn dispatch(&mut self, i: usize) -> bool {
        let Some(shop) = self.shops.get_mut(i) else {
            return false;
        };
        if shop.owned == 0 || shop.running {
            return false;
        }
        shop.running = true;
        true
    }

    pub fn payout(&self, i: usize) -> u64 {
        let Some(shop) = self.shops.get(i) else {
            return 0;
        };
        let milestones = MILESTONES.partition_point(|&level| level <= shop.owned);
        let base = (UNIT_YIELD[i] * u64::from(shop.owned)) << milestones;
        self.apply_fleet_bonus(i, base)
    }

    /// What one unit in this berth pays, including charter and shipyard bonuses.
    pub fn opening_payout(&self, i: usize) -> u64 {
        self.apply_fleet_bonus(i, UNIT_YIELD.get(i).copied().unwrap_or(0))
    }

    fn apply_fleet_bonus(&self, i: usize, base: u64) -> u64 {
        let bonus = 100
            + u128::from(self.charters) * u128::from(CHARTER_BONUS_PERCENT)
            + u128::from(self.yard.bonus_percent());
        let kitchen = u128::from(i == 0 && self.manifest.ending == Ending::Kitchen);
        (u128::from(base) * bonus / 100 + kitchen).min(u128::from(u64::MAX)) as u64
    }

    pub fn duration_ms(&self, i: usize) -> u32 {
        let Some(shop) = self.shops.get(i) else {
            return 0;
        };
        let speedups = [25, 100, 200].partition_point(|&level| level <= shop.owned);
        CYCLE_MS[i] >> speedups
    }

    pub fn next_milestone(&self, i: usize) -> Option<u16> {
        let owned = self.shops.get(i)?.owned;
        MILESTONES.iter().copied().find(|&level| level > owned)
    }

    /// These are full bundle prices even if unaffordable. A Max quote always fits
    /// the current wallet, returning (0, 0) when even one unit is out of reach.
    pub fn quote(&self, i: usize, bulk: Bulk) -> (u16, u64) {
        let Some(shop) = self.shops.get(i) else {
            return (0, 0);
        };
        let n = usize::from(shop.owned);
        if n >= MAX_UNITS as usize {
            return (0, 0);
        }
        let prices = &PRICES[i][n..];
        let count = match bulk {
            Bulk::One => 1,
            Bulk::Ten => 10.min(prices.len() - 1),
            Bulk::Max => prices.partition_point(|&total| total - prices[0] <= self.cash) - 1,
        };
        (count as u16, prices[count] - prices[0])
    }

    pub fn buy(&mut self, i: usize, bulk: Bulk) -> u16 {
        let (count, cost) = self.quote(i, bulk);
        if count == 0 || cost > self.cash {
            return 0;
        }
        let old_period = self.duration_ms(i);
        self.cash -= cost;
        self.shops[i].owned += count;
        // Preserve the fraction of the current trip when a faster hull unlocks.
        let period = self.duration_ms(i);
        let shop = &mut self.shops[i];
        shop.elapsed_ms =
            (u64::from(shop.elapsed_ms) * u64::from(period) / u64::from(old_period)) as u32;
        count
    }

    pub fn manager_cost(&self, i: usize) -> u64 {
        MANAGER_COST.get(i).copied().unwrap_or(0)
    }

    pub fn hire(&mut self, i: usize) -> bool {
        let cost = self.manager_cost(i);
        let Some(shop) = self.shops.get_mut(i) else {
            return false;
        };
        if shop.owned == 0 || shop.manager || self.cash < cost {
            return false;
        }
        self.cash -= cost;
        shop.manager = true;
        shop.running = true;
        true
    }

    pub fn inventory(&self, good: usize) -> u16 {
        match good {
            0..=2 => self.freight.stock[good],
            3..=4 => self.factories[good - 3].output,
            _ => 0,
        }
    }

    /// Space for delivered cargo. A prepaid batch already owns its output slot.
    pub fn cargo_room(&self, good: usize) -> u16 {
        if good >= 5 {
            return 0;
        }
        let reserved = good >= 3 && self.factories[good - 3].running;
        self.freight.capacity() - self.inventory(good) - u16::from(reserved)
    }

    pub fn quote_trade(&self, good: usize, buy: bool, all: bool) -> Quote {
        self.quote_lot(good, buy, if all { u16::MAX } else { 1 })
    }
    pub fn quote_lot(&self, good: usize, buy: bool, requested: u16) -> Quote {
        if buy {
            if good >= 3 {
                return Quote::default();
            }
            let space = self.cargo_room(good);
            let limit = space.min(requested);
            self.freight
                .market
                .ask_lot(good, limit, self.freight.tide, self.cash)
        } else {
            let stock = self.inventory(good);
            self.freight.bid_lot(good, stock.min(requested))
        }
    }

    pub fn trade(&mut self, good: usize, buy: bool, all: bool) -> Quote {
        self.trade_lot(good, buy, if all { u16::MAX } else { 1 })
    }

    pub fn trade_lot(&mut self, good: usize, buy: bool, requested: u16) -> Quote {
        let quote = self.quote_lot(good, buy, requested);
        if quote.qty == 0 {
            return quote;
        }
        if buy {
            self.freight.market.stock[good] -= quote.qty;
            self.freight.stock[good] += quote.qty;
            self.cash -= quote.coins;
            let flow = &mut self.ledger.flows[good];
            flow.bought = flow.bought.saturating_add(u64::from(quote.qty));
            self.ledger.purchases = self.ledger.purchases.saturating_add(quote.coins);
            self.ledger.unrecovered = self.ledger.unrecovered.saturating_add(quote.coins);
        } else {
            self.apply_sale(good, quote);
        }
        self.retry_works();
        quote
    }

    fn apply_sale(&mut self, good: usize, quote: Quote) {
        self.freight.market.stock[good] += quote.qty;
        if good < 3 {
            self.freight.stock[good] -= quote.qty;
        } else {
            self.factories[good - 3].output -= quote.qty;
        }
        let flow = &mut self.ledger.flows[good];
        flow.sold = flow.sold.saturating_add(u64::from(quote.qty));
        self.ledger.sales = self.ledger.sales.saturating_add(quote.coins);
        self.trade_receipt(quote.coins);
    }

    pub fn standing_good(&self) -> Option<usize> {
        (self.ticket.status == TicketStatus::Pending).then_some(usize::from(self.ticket.good))
    }
    pub fn place_sale(&mut self, good: usize, floor: u16, qty: u16) -> bool {
        if self.standing_good().is_some() || good >= 5 || floor == 0 || !(1..=96).contains(&qty) {
            return false;
        }
        self.ticket = Ticket {
            good: good as u8,
            floor,
            requested: qty,
            remaining: qty,
            tides_left: 12,
            status: TicketStatus::Pending,
            ..Ticket::new()
        };
        true
    }
    pub fn cancel_sale(&mut self) -> bool {
        if self.standing_good().is_none() {
            return false;
        }
        self.ticket.status = TicketStatus::Cancelled;
        self.ticket.tides_left = 0;
        true
    }
    /// The dock only matches crates whose individual bid meets the floor.
    /// A standing sale reserves neither a crate nor a slot in the dock's hold.
    pub fn ticket_quote(&self, good: usize, qty: u16, floor: u16) -> Quote {
        if floor == 0 || good >= 5 {
            return Quote::default();
        }
        let available = self.inventory(good).min(qty);
        let mut book = self.freight.market.clone();
        let mut fill = Quote::default();
        for _ in 0..available {
            let one = book.bid_lot(good, 1, self.freight.tide);
            if one.coins < u64::from(floor) {
                break;
            }
            book.stock[good] += 1;
            fill.qty += 1;
            fill.coins += one.coins;
        }
        fill
    }

    fn clear_ticket(&mut self) -> u64 {
        let Some(good) = self.standing_good() else {
            return 0;
        };
        let fill = self.ticket_quote(good, self.ticket.remaining, self.ticket.floor);
        self.ticket.tides_left -= 1;
        if fill.qty > 0 {
            self.apply_sale(good, fill);
            self.ticket.remaining -= fill.qty;
            self.ticket.filled += fill.qty;
            self.ticket.proceeds += fill.coins;
        }
        if self.ticket.remaining == 0 {
            self.ticket.status = TicketStatus::Filled;
            self.ticket.tides_left = 0;
        } else if self.ticket.tides_left == 0 {
            self.ticket.status = TicketStatus::Expired;
        }
        fill.coins
    }

    /// Legacy one-way exchange control.
    pub fn sell(&mut self, good: usize, all: bool) -> u64 {
        self.trade(good, false, all).coins
    }

    fn trade_receipt(&mut self, income: u64) {
        let recovery = self.ledger.unrecovered.min(income);
        self.ledger.unrecovered -= recovery;
        self.cash = self.cash.saturating_add(income);
        self.earned = self.earned.saturating_add(income);
        self.ledger.eligible = self.ledger.eligible.saturating_add(income - recovery);
    }

    pub fn charter_earned(&self) -> u64 {
        self.ledger.eligible
    }

    pub fn open_factory(&mut self, i: usize) -> bool {
        let Some(plan) = WORKS.get(i) else {
            return false;
        };
        if self.factories[i].built || self.cash < plan.build_cost {
            return false;
        }
        self.cash -= plan.build_cost;
        self.ledger.construction = self.ledger.construction.saturating_add(plan.build_cost);
        self.factories[i].built = true;
        true
    }
    pub fn current_favor(&self) -> Favor {
        self.neighbors.offer(self.freight.tide)
    }

    pub fn can_help(&self) -> bool {
        let favor = self.current_favor();
        !self.neighbors.served
            && favor
                .cargo
                .iter()
                .enumerate()
                .all(|(g, &qty)| self.inventory(g) >= qty)
    }

    /// Recheck the displayed visitor, tide, materials and price before any
    /// crate leaves the hold. An expired request never pays its successor.
    pub fn help_neighbor(&mut self, expected: Favor) -> bool {
        if expected != self.current_favor() || !self.can_help() {
            return false;
        }
        for (g, qty) in expected.cargo.into_iter().enumerate() {
            if g < 3 {
                self.freight.stock[g] -= qty;
            } else {
                self.factories[g - 3].output -= qty;
            }
            let flow = &mut self.ledger.flows[g];
            flow.delivered = flow.delivered.saturating_add(u64::from(qty));
        }
        self.neighbors.served = true;
        let n = &mut self.neighbors.helped[expected.neighbor];
        *n = n.saturating_add(1);
        self.ledger.orders = self.ledger.orders.saturating_add(expected.reward);
        self.trade_receipt(expected.reward);
        self.retry_works();
        true
    }

    pub fn lanterns_sailing(&self) -> bool {
        self.neighbors.lanterns_unlocked() && neighbors::watch(self.freight.tide) == 2
    }

    pub fn committee_invited(&self) -> bool {
        self.committee.stage == Stage::Dormant
            && self.yard.completed & 4 != 0
            && self.neighbors.lanterns_unlocked()
    }

    pub fn sign_order_thirteen(&mut self) -> bool {
        if !self.committee_invited() {
            return false;
        }
        self.committee.stage = Stage::TeaService;
        true
    }

    pub fn can_submit_motion(&self) -> bool {
        self.committee
            .motion_good()
            .is_some_and(|g| self.inventory(g) > 0)
    }

    /// Revalidate the numbered item the player saw before taking one crate.
    pub fn submit_motion(&mut self, expected_total: u8) -> bool {
        if self.committee.motions != expected_total || !self.can_submit_motion() {
            return false;
        }
        let Some(g) = self.committee.motion_good() else {
            return false;
        };
        if g < 3 {
            self.freight.stock[g] -= 1;
        } else {
            self.factories[g - 3].output -= 1;
        }
        let flow = &mut self.ledger.flows[g];
        flow.delivered = flow.delivered.saturating_add(1);
        self.committee.motions += 1;
        self.committee.stage = match self.committee.motions {
            0..=2 => Stage::TeaService,
            3..=5 => Stage::Quorum,
            _ => Stage::HighTea,
        };
        self.retry_works();
        true
    }

    pub fn sea_offer(&self) -> Option<Barter> {
        (self.committee.stage == Stage::HighTea).then(|| Barter::at(self.freight.tide))
    }

    pub fn can_barter(&self) -> bool {
        let Some(deal) = self.sea_offer() else {
            return false;
        };
        !self.committee.bartered
            && self.inventory(deal.give) >= deal.give_qty
            && self.cargo_room(deal.take) >= deal.take_qty
    }

    /// One voluntary exchange at the table. The sea brings recovered cargo;
    /// none of the terms are a sale, a factory receipt or a monetary reward.
    pub fn barter(&mut self, expected_tide: u64) -> bool {
        let Some(deal) = self.sea_offer() else {
            return false;
        };
        if deal.tide != expected_tide || !self.can_barter() {
            return false;
        }
        if deal.give < 3 {
            self.freight.stock[deal.give] -= deal.give_qty;
        } else {
            self.factories[deal.give - 3].output -= deal.give_qty;
        }
        if deal.take < 3 {
            self.freight.stock[deal.take] += deal.take_qty;
        } else {
            self.factories[deal.take - 3].output += deal.take_qty;
        }
        let given = &mut self.ledger.flows[deal.give];
        given.delivered = given.delivered.saturating_add(u64::from(deal.give_qty));
        let found = &mut self.ledger.flows[deal.take];
        found.landed = found.landed.saturating_add(u64::from(deal.take_qty));
        self.committee.bartered = true;
        self.retry_works();
        true
    }

    /// Transfer already-earned fare coins. No further gross or charter income
    /// is booked, including for money held across a previous charter.
    pub fn claim_tea_tin(&mut self) -> u64 {
        if !self.committee.active() {
            return 0;
        }
        let amount = core::mem::take(&mut self.committee.tin);
        self.cash = self.cash.saturating_add(amount);
        self.retry_works();
        amount
    }

    pub fn adjourn_committee(&mut self) -> bool {
        if self.committee.stage != Stage::HighTea {
            return false;
        }
        self.claim_tea_tin();
        self.committee.stage = Stage::Adjourned;
        self.committee.bartered = false;
        self.committee.carry = 0;
        self.credit(13);
        self.retry_works();
        true
    }

    pub fn can_start_batch(&self, i: usize) -> bool {
        let Some(plan) = WORKS.get(i) else {
            return false;
        };
        self.factories[i].can_start(
            plan,
            self.cash,
            &self.freight.stock,
            self.freight.capacity(),
        )
    }
    pub fn start_batch(&mut self, i: usize) -> bool {
        let Some(plan) = WORKS.get(i) else {
            return false;
        };
        let cap = self.freight.capacity();
        if !self.factories[i].start(plan, &mut self.cash, &mut self.freight.stock, cap) {
            return false;
        }
        self.ledger.fees = self.ledger.fees.saturating_add(plan.fee);
        self.ledger.unrecovered = self.ledger.unrecovered.saturating_add(plan.fee);
        for (flow, used) in self.ledger.flows.iter_mut().zip(plan.inputs) {
            flow.used = flow.used.saturating_add(u64::from(used));
        }
        true
    }
    pub fn set_repeat(&mut self, i: usize, on: bool) -> bool {
        let Some(works) = self.factories.get_mut(i) else {
            return false;
        };
        if !works.built || works.repeat == on {
            return false;
        }
        works.repeat = on;
        self.retry_works();
        true
    }
    fn retry_works(&mut self) {
        for i in 0..2 {
            if self.factories[i].repeat {
                self.start_batch(i);
            }
        }
    }

    pub fn conversion_margin(&self, i: usize) -> i64 {
        let Some(plan) = WORKS.get(i) else { return 0 };
        let alternative: u64 = plan
            .inputs
            .iter()
            .enumerate()
            .map(|(g, &q)| self.freight.bid_lot(g, q).coins)
            .sum();
        self.freight.bid_lot(plan.good, 1).coins as i64 - plan.fee as i64 - alternative as i64
    }

    /// Commit stock to the posted order. Its receipt arrives on the next tide.
    pub fn reserve_order(&mut self) -> bool {
        let order = self.freight.offer();
        if !self.freight.reserve_order() {
            return false;
        }
        self.ledger.flows[order.good].delivered = self.ledger.flows[order.good]
            .delivered
            .saturating_add(u64::from(order.qty));
        true
    }

    pub fn upgrade_hold(&mut self) -> bool {
        let Some(cost) = self.freight.upgrade_cost() else {
            return false;
        };
        if self.cash < cost {
            return false;
        }
        self.cash -= cost;
        self.freight.hold_level += 1;
        self.retry_works();
        true
    }

    fn credit(&mut self, income: u64) {
        self.cash = self.cash.saturating_add(income);
        self.earned = self.earned.saturating_add(income);
        self.ledger.eligible = self.ledger.eligible.saturating_add(income);
    }

    /// The quoted materials must still be on the quay and payment is all-or-nothing.
    pub fn can_build(&self, vessel: usize) -> bool {
        let Some(plan) = PLANS.get(vessel) else {
            return false;
        };
        !self.yard.building
            && self.yard.next() == Some(vessel)
            && self.cash >= plan.cost
            && self
                .freight
                .stock
                .iter()
                .zip(plan.cargo)
                .all(|(&held, needed)| held >= needed)
    }

    pub fn build(&mut self, vessel: usize) -> bool {
        if !self.can_build(vessel) {
            return false;
        }
        let plan = &PLANS[vessel];
        self.cash -= plan.cost;
        for (good, needed) in plan.cargo.iter().copied().enumerate() {
            self.freight.stock[good] -= needed;
            let flow = &mut self.ledger.flows[good];
            flow.used = flow.used.saturating_add(u64::from(needed));
        }
        self.yard.building = true;
        true
    }

    /// Settle every completed cycle, preserving exact timer remainders even for
    /// a long absence. A manual dispatch pays only once.
    pub fn advance(&mut self, mut ms: u64) -> u64 {
        self.retry_works();
        let mut income = 0u64;
        while ms > 0 {
            let tide_in = u64::from(TIDE_MS) - u64::from(self.freight.elapsed_ms);
            let mut step = if self.standing_good().is_some() {
                ms.min(tide_in)
            } else {
                ms
            };
            for (i, works) in self.factories.iter().enumerate() {
                if let Some(remaining) = works.remaining_ms(&WORKS[i]) {
                    step = step.min(u64::from(remaining));
                } else if works.repeat && works.output < self.freight.capacity() {
                    if let Some(ready) = self.next_supply_for(i) {
                        step = step.min(ready);
                    }
                }
            }
            income = income.saturating_add(self.advance_port(step));
            for (i, works) in self.factories.iter_mut().enumerate() {
                if works.advance(&WORKS[i], step) {
                    let flow = &mut self.ledger.flows[WORKS[i].good];
                    flow.made = flow.made.saturating_add(1);
                }
            }
            // The market and harbor clear first; all finished products are on
            // the quay before the broker trades and the next works shift starts.
            if step == tide_in && self.standing_good().is_some() {
                income = income.saturating_add(self.clear_ticket());
            }
            self.retry_works();
            ms -= step;
        }
        income
    }

    /// Earliest possible restart. If a missing input has no scheduled source,
    /// only a player action can unblock this works. Even a huge advance skips
    /// that blocked interval in one step.
    fn next_supply_for(&self, i: usize) -> Option<u64> {
        let plan = &WORKS[i];
        let mut wait = 0u64;
        for (g, &need) in plan.inputs.iter().enumerate() {
            let missing = need.saturating_sub(self.freight.stock[g]);
            if missing == 0 {
                continue;
            }
            let shop = self.shops[g];
            if !shop.running || (!shop.manager && missing > 1) {
                return None;
            }
            let period = u64::from(self.duration_ms(g));
            wait = wait.max(period * u64::from(missing) - u64::from(shop.elapsed_ms));
        }
        if wait > 0 {
            return Some(wait);
        }
        if self.cash >= plan.fee {
            return None;
        }
        // Fees are at most 14 coins. Even the smallest fare covers the deficit
        // in two arrivals, so considering only the next receipt stays bounded.
        let mut next = self
            .shops
            .iter()
            .enumerate()
            .filter(|(_, s)| s.running)
            .map(|(g, s)| u64::from(self.duration_ms(g) - s.elapsed_ms))
            .min();
        if self.freight.reserved {
            let order = u64::from(TIDE_MS) - u64::from(self.freight.elapsed_ms);
            next = Some(next.map_or(order, |t| t.min(order)));
        }
        next
    }

    fn advance_port(&mut self, ms: u64) -> u64 {
        if self.yard.building {
            if let Some(i) = self.yard.next() {
                let until_launch = u64::from(PLANS[i].duration_ms - self.yard.elapsed_ms);
                if ms >= until_launch {
                    // Ships arriving on the launch millisecond receive the new rate.
                    // A player must explicitly commission the following vessel, so
                    // even an enormous advance has at most one launch boundary.
                    let before_launch = until_launch - 1;
                    let income = self.advance_businesses(before_launch);
                    self.yard.completed |= 1 << i;
                    self.yard.building = false;
                    self.yard.elapsed_ms = 0;
                    return income.saturating_add(self.advance_businesses(ms - before_launch));
                }
                self.yard.elapsed_ms += ms as u32;
            }
        }
        self.advance_businesses(ms)
    }

    fn advance_businesses(&mut self, ms: u64) -> u64 {
        let before_tide = self.freight.tide;
        let order = self.freight.advance(ms);
        if self.freight.tide != before_tide {
            self.neighbors.served = false;
            self.committee.bartered = false;
        }
        self.ledger.orders = self.ledger.orders.saturating_add(order);
        self.trade_receipt(order);
        let mut income = 0u64;
        let mut gross = 0u64;
        for i in 0..self.shops.len() {
            if !self.shops[i].running {
                continue;
            }
            let period = u64::from(self.duration_ms(i));
            let payout = self.payout(i);
            let shop = &mut self.shops[i];
            // Splitting whole cycles first avoids overflow even for u64::MAX ms.
            let fraction = u64::from(shop.elapsed_ms) + ms % period;
            let completed = ms / period + fraction / period;
            let partial = (fraction % period) as u32;
            if shop.manager || completed == 0 {
                shop.elapsed_ms = partial;
            } else {
                shop.running = false;
                shop.elapsed_ms = 0;
            }
            let cycles = if shop.manager {
                completed
            } else {
                completed.min(1)
            };
            let fares = u128::from(cycles) * u128::from(payout);
            let available = self.committee.apportion(fares);
            income = income.saturating_add(available.min(u128::from(u64::MAX)) as u64);
            gross = gross.saturating_add(fares.min(u128::from(u64::MAX)) as u64);
            let landed = self.freight.unload(i, cycles);
            if landed > 0 {
                self.manifest.unlocked = true;
            }
            self.ledger.flows[i].landed = self.ledger.flows[i]
                .landed
                .saturating_add(u64::from(landed));
        }
        self.cash = self.cash.saturating_add(income);
        self.earned = self.earned.saturating_add(gross);
        self.ledger.eligible = self.ledger.eligible.saturating_add(gross);
        income.saturating_add(order)
    }

    pub fn charter_offer(&self) -> u32 {
        let award = (self.charter_earned() / CHARTER_UNIT).isqrt() as u32;
        award.min(u32::MAX - self.charters)
    }

    pub fn charter_target(&self) -> u64 {
        let next = 1 + u64::from(self.charter_offer());
        CHARTER_UNIT.saturating_mul(next.saturating_mul(next))
    }

    /// Record a caught mailbag at an owned berth. The pilot awards one stamp
    /// per berth across all charters, independent of coins and shift timers.
    pub fn mark_berth(&mut self, i: usize) -> bool {
        let Some(shop) = self.shops.get(i) else {
            return false;
        };
        let stamp = 1 << i;
        if shop.owned == 0 || self.postmarks & stamp != 0 {
            return false;
        }
        self.postmarks |= stamp;
        true
    }

    /// Start a fresh harbor with an increased permanent payout bonus.
    pub fn charter(&mut self) -> u32 {
        let award = self.charter_offer();
        if award > 0 {
            *self = Self {
                charters: self.charters + award,
                postmarks: self.postmarks,
                yard: self.yard,
                manifest: self.manifest,
                neighbors: self.neighbors,
                committee: Committee {
                    brought_forward: self.committee.tin,
                    ..self.committee
                },
                // The port calendar and appointment at the quay carry through
                // a new charter; businesses, cargo, market stock and orders reset.
                freight: Freight {
                    tide: self.freight.tide,
                    elapsed_ms: self.freight.elapsed_ms,
                    ..Freight::new()
                },
                settled_through_s: self.settled_through_s.clone(),
                ..Self::new()
            };
        }
        award
    }

    /// BWH8 appends committee minutes, fare escrow and the opening escrow balance to BWH7.
    /// All integers are little-endian, with a trailing CRC-32.
    pub fn encode(&self, now_s: u64) -> [u8; SAVE_LEN] {
        let mut bytes = [0; SAVE_LEN];
        bytes[..4].copy_from_slice(b"BWH8");
        let settled_s = now_s.max(self.settled_through_s.get());
        self.settled_through_s.set(settled_s);
        bytes[4..12].copy_from_slice(&settled_s.to_le_bytes());
        bytes[12..20].copy_from_slice(&self.cash.to_le_bytes());
        bytes[20..28].copy_from_slice(&self.earned.to_le_bytes());
        bytes[28..32].copy_from_slice(&self.charters.to_le_bytes());
        for (shop, record) in self.shops.iter().zip(bytes[32..67].chunks_exact_mut(7)) {
            record[..2].copy_from_slice(&shop.owned.to_le_bytes());
            record[2] = u8::from(shop.manager) | (u8::from(shop.running) << 1);
            record[3..].copy_from_slice(&shop.elapsed_ms.to_le_bytes());
        }
        bytes[67] = self.postmarks;
        for (stock, field) in self
            .freight
            .stock
            .iter()
            .zip(bytes[68..74].chunks_exact_mut(2))
        {
            field.copy_from_slice(&stock.to_le_bytes());
        }
        bytes[74] = self.freight.hold_level;
        bytes[75..83].copy_from_slice(&self.freight.tide.to_le_bytes());
        bytes[83..85].copy_from_slice(&self.freight.elapsed_ms.to_le_bytes());
        bytes[85] = u8::from(self.freight.reserved);
        bytes[86] = self.yard.completed;
        bytes[87] = u8::from(self.yard.building);
        bytes[88..92].copy_from_slice(&self.yard.elapsed_ms.to_le_bytes());
        let mut offset = 92;
        let mut write = |field: &[u8]| {
            bytes[offset..offset + field.len()].copy_from_slice(field);
            offset += field.len();
        };
        for stock in self.freight.market.stock {
            write(&stock.to_le_bytes());
        }
        write(&[self.freight.market.history_len]);
        for history in self.freight.market.history {
            for bid in history {
                write(&bid.to_le_bytes());
            }
        }
        for works in self.factories {
            write(&[u8::from(works.built)
                | (u8::from(works.repeat) << 1)
                | (u8::from(works.running) << 2)]);
            write(&works.elapsed_ms.to_le_bytes());
            write(&works.output.to_le_bytes());
        }
        let l = &self.ledger;
        for n in [
            l.sales,
            l.purchases,
            l.fees,
            l.construction,
            l.orders,
            l.unrecovered,
            l.eligible,
        ] {
            write(&n.to_le_bytes());
        }
        for flow in l.flows {
            write(&flow.opening.to_le_bytes());
            for n in [
                flow.landed,
                flow.bought,
                flow.made,
                flow.sold,
                flow.used,
                flow.delivered,
            ] {
                write(&n.to_le_bytes());
            }
        }
        write(&[self.ticket.status as u8, self.ticket.good]);
        for value in [
            self.ticket.floor,
            self.ticket.requested,
            self.ticket.remaining,
            self.ticket.filled,
        ] {
            write(&value.to_le_bytes());
        }
        write(&self.ticket.proceeds.to_le_bytes());
        write(&[self.ticket.tides_left]);
        write(&[u8::from(self.manifest.unlocked), self.manifest.solved]);
        write(&self.manifest.read);
        write(&[self.manifest.ending as u8]);
        for count in self.neighbors.helped {
            write(&count.to_le_bytes());
        }
        write(&[u8::from(self.neighbors.served)]);
        write(&[
            self.committee.stage as u8,
            self.committee.motions,
            self.committee.carry,
        ]);
        write(&self.committee.tin.to_le_bytes());
        write(&self.committee.brought_forward.to_le_bytes());
        write(&[u8::from(self.committee.bartered)]);
        debug_assert_eq!(offset, SAVE_LEN - 4);
        let checksum = crc32(&bytes[..offset]);
        bytes[offset..].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }

    /// Advance an imported save by at most 12 hours. Persist the result at
    /// now_s immediately so a second ordinary load cannot collect it again.
    pub fn decode(bytes: &[u8], now_s: u64) -> Option<(Self, u64)> {
        let postmarks = match (bytes.get(..4)?, bytes.len()) {
            (b"BWH1", LEGACY_SAVE_LEN) => 0,
            (b"BWH2", V2_SAVE_LEN)
            | (b"BWH3", V3_SAVE_LEN)
            | (b"BWH4", V4_SAVE_LEN)
            | (b"BWH5", V5_INITIAL_SAVE_LEN)
            | (b"BWH5", V5_SAVE_LEN)
            | (b"BWH6", V6_SAVE_LEN)
            | (b"BWH7", V7_SAVE_LEN)
            | (b"BWH8", V8_INITIAL_SAVE_LEN)
            | (b"BWH8", SAVE_LEN)
                if bytes[67] & !VALID_POSTMARKS == 0 =>
            {
                bytes[67]
            }
            _ => return None,
        };
        let (payload, checksum) = bytes.split_at(bytes.len() - 4);
        if crc32(payload).to_le_bytes() != checksum {
            return None;
        }
        let mut freight = Freight::new();
        if bytes.len() >= V3_SAVE_LEN {
            for (stock, field) in freight.stock.iter_mut().zip(bytes[68..74].chunks_exact(2)) {
                *stock = u16::from_le_bytes(field.try_into().ok()?);
            }
            freight.hold_level = bytes[74];
            freight.tide = u64::from_le_bytes(bytes[75..83].try_into().ok()?);
            freight.elapsed_ms = u16::from_le_bytes(bytes[83..85].try_into().ok()?);
            freight.reserved = match bytes[85] {
                0 => false,
                1 => true,
                _ => return None,
            };
            if !freight.valid() {
                return None;
            }
        }
        let mut yard = Yard::new();
        if bytes.len() >= V4_SAVE_LEN {
            yard.completed = bytes[86];
            yard.building = match bytes[87] {
                0 => false,
                1 => true,
                _ => return None,
            };
            yard.elapsed_ms = u32::from_le_bytes(bytes[88..92].try_into().ok()?);
            if !yard.valid() {
                return None;
            }
        }
        let saved_s = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
        let mut game = Self {
            cash: u64::from_le_bytes(bytes[12..20].try_into().ok()?),
            earned: u64::from_le_bytes(bytes[20..28].try_into().ok()?),
            charters: u32::from_le_bytes(bytes[28..32].try_into().ok()?),
            postmarks,
            freight,
            yard,
            settled_through_s: Cell::new(saved_s.max(now_s)),
            ..Self::new()
        };
        for (shop, record) in game.shops.iter_mut().zip(bytes[32..67].chunks_exact(7)) {
            *shop = Shop {
                owned: u16::from_le_bytes(record[..2].try_into().ok()?),
                manager: record[2] & 1 != 0,
                running: record[2] & 2 != 0,
                elapsed_ms: u32::from_le_bytes(record[3..].try_into().ok()?),
            };
            if record[2] > 3
                || shop.owned > MAX_UNITS
                || (shop.running && shop.owned == 0)
                || (!shop.running && (shop.manager || shop.elapsed_ms != 0))
            {
                return None;
            }
        }
        if game.shops[0].owned == 0
            || game
                .shops
                .iter()
                .enumerate()
                .any(|(i, shop)| shop.elapsed_ms >= game.duration_ms(i))
        {
            return None;
        }
        if bytes.len() >= V5_INITIAL_SAVE_LEN {
            let mut fields = &payload[92..];
            for stock in &mut game.freight.market.stock {
                *stock = u16::from_le_bytes(take(&mut fields)?);
            }
            game.freight.market.history_len = take::<1>(&mut fields)?[0];
            for row in &mut game.freight.market.history {
                for price in row {
                    *price = u16::from_le_bytes(take(&mut fields)?);
                }
            }
            if !game.freight.market.valid() {
                return None;
            }
            for (works, plan) in game.factories.iter_mut().zip(&WORKS) {
                let flags = take::<1>(&mut fields)?[0];
                if flags > 7 {
                    return None;
                }
                works.built = flags & 1 != 0;
                works.repeat = flags & 2 != 0;
                works.running = flags & 4 != 0;
                works.elapsed_ms = u32::from_le_bytes(take(&mut fields)?);
                works.output = u16::from_le_bytes(take(&mut fields)?);
                // Recovered crates can be warehoused before the corresponding
                // works is built. Older formats could only hold factory output.
                let mut machinery = *works;
                if bytes.len() == SAVE_LEN && !works.built {
                    if works.output > game.freight.capacity() {
                        return None;
                    }
                    machinery.output = 0;
                }
                if !machinery.valid(plan, game.freight.capacity()) {
                    return None;
                }
            }
            let l = &mut game.ledger;
            for field in [
                &mut l.sales,
                &mut l.purchases,
                &mut l.fees,
                &mut l.construction,
                &mut l.orders,
                &mut l.unrecovered,
                &mut l.eligible,
            ] {
                *field = u64::from_le_bytes(take(&mut fields)?);
            }
            for flow in &mut l.flows {
                flow.opening = u16::from_le_bytes(take(&mut fields)?);
                for field in [
                    &mut flow.landed,
                    &mut flow.bought,
                    &mut flow.made,
                    &mut flow.sold,
                    &mut flow.used,
                    &mut flow.delivered,
                ] {
                    *field = u64::from_le_bytes(take(&mut fields)?);
                }
                if flow.opening > 96 {
                    return None;
                }
            }
            if bytes.len() >= V5_SAVE_LEN {
                let [state, good] = take(&mut fields)?;
                game.ticket.status = match state {
                    0 => TicketStatus::Empty,
                    1 => TicketStatus::Pending,
                    2 => TicketStatus::Filled,
                    3 => TicketStatus::Expired,
                    4 => TicketStatus::Cancelled,
                    _ => return None,
                };
                game.ticket.good = good;
                for v in [
                    &mut game.ticket.floor,
                    &mut game.ticket.requested,
                    &mut game.ticket.remaining,
                    &mut game.ticket.filled,
                ] {
                    *v = u16::from_le_bytes(take(&mut fields)?);
                }
                game.ticket.proceeds = u64::from_le_bytes(take(&mut fields)?);
                game.ticket.tides_left = take::<1>(&mut fields)?[0];
                if !game.ticket.valid() || game.ticket.proceeds > l.sales {
                    return None;
                }
            }
            if bytes.len() >= V6_SAVE_LEN {
                let [unlocked, solved] = take(&mut fields)?;
                game.manifest.unlocked = match unlocked {
                    0 => false,
                    1 => true,
                    _ => return None,
                };
                game.manifest.solved = solved;
                game.manifest.read = take(&mut fields)?;
                game.manifest.ending = match take::<1>(&mut fields)?[0] {
                    0 => Ending::None,
                    1 => Ending::Print,
                    2 => Ending::Kitchen,
                    _ => return None,
                };
                if !game.manifest.valid() {
                    return None;
                }
            }
            if bytes.len() >= V7_SAVE_LEN {
                for count in &mut game.neighbors.helped {
                    *count = u32::from_le_bytes(take(&mut fields)?);
                }
                game.neighbors.served = match take::<1>(&mut fields)?[0] {
                    0 => false,
                    1 => true,
                    _ => return None,
                };
                if game.neighbors.served
                    && game.neighbors.helped[neighbors::visitor(game.freight.tide)] == 0
                {
                    return None;
                }
            }
            if bytes.len() >= V8_INITIAL_SAVE_LEN {
                let [stage, motions, carry] = take(&mut fields)?;
                game.committee.stage = match stage {
                    0 => Stage::Dormant,
                    1 => Stage::TeaService,
                    2 => Stage::Quorum,
                    3 => Stage::HighTea,
                    4 => Stage::Adjourned,
                    _ => return None,
                };
                game.committee.motions = motions;
                game.committee.carry = carry;
                game.committee.tin = u64::from_le_bytes(take(&mut fields)?);
                game.committee.brought_forward = u64::from_le_bytes(take(&mut fields)?);
                if bytes.len() == SAVE_LEN {
                    game.committee.bartered = match take::<1>(&mut fields)?[0] {
                        0 => false,
                        1 => true,
                        _ => return None,
                    };
                }
                if !game.committee.valid()
                    || (game.committee.stage != Stage::Dormant
                        && (game.yard.completed & 4 == 0 || !game.neighbors.lanterns_unlocked()))
                    || (game.committee.brought_forward > 0 && game.charters == 0)
                {
                    return None;
                }
            }
            if !fields.is_empty()
                || l.eligible > game.earned
                || l.unrecovered > l.purchases.saturating_add(l.fees)
            {
                return None;
            }
            for (i, flow) in game.ledger.flows.iter().enumerate() {
                // Saturated counters are lower bounds; all representable ledgers
                // must reconcile exactly to the stock physically on the quay.
                let inflow = [flow.landed, flow.bought, flow.made];
                let outflow = [flow.used, flow.sold, flow.delivered];
                if !inflow.contains(&u64::MAX) && !outflow.contains(&u64::MAX) {
                    let supply =
                        u128::from(flow.opening) + inflow.into_iter().map(u128::from).sum::<u128>();
                    let uses = u128::from(game.inventory(i))
                        + outflow.into_iter().map(u128::from).sum::<u128>();
                    if supply != uses {
                        return None;
                    }
                }
            }
        } else {
            game.ledger.eligible = game.earned;
            for (flow, stock) in game.ledger.flows.iter_mut().zip(game.freight.stock) {
                flow.opening = stock;
            }
        }
        // Once a balance saturates it represents a lower bound. Below that
        // ceiling the current till and the tin cannot exceed total receipts.
        if game.earned < u64::MAX
            && u128::from(game.cash) + u128::from(game.committee.tin)
                > u128::from(game.earned) + u128::from(game.committee.brought_forward)
        {
            return None;
        }
        if bytes.len() < V6_SAVE_LEN {
            // Pre-casebook editions did not always record landings. Material
            // progress provides an invitation even when an old hold is empty.
            game.manifest.unlocked = game
                .ledger
                .flows
                .iter()
                .any(|f| f.landed > 0 || f.opening > 0)
                || game.earned >= 8
                || game.charters > 0
                || game.postmarks != 0
                || game.shops.iter().skip(1).any(|s| s.owned > 0)
                || game.shops[0].owned > 1
                || game.shops[0].manager;
        }
        let offline_s = now_s.saturating_sub(saved_s).min(OFFLINE_LIMIT_S);
        let income = game.advance(offline_s * 1_000);
        Some((game, income))
    }
}

fn take<const N: usize>(fields: &mut &[u8]) -> Option<[u8; N]> {
    let field = fields.get(..N)?.try_into().ok()?;
    *fields = &fields[N..];
    Some(field)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn funded(coins: u64) -> Economy {
        let mut game = Economy::new();
        game.credit(coins);
        game
    }

    #[test]
    fn ring_tips_and_a_single_manual_trip() {
        let mut game = Economy::new();
        assert!(!game.dispatch(1));
        assert_eq!(game.ring(), 1);
        assert!(!game.dispatch(0));
        assert_eq!(game.advance(1_400), 0);
        assert_eq!(game.ring(), 1);
        assert_eq!(game.shops[0].elapsed_ms, 1_400);
        assert_eq!(game.advance(60_000), 8);
        assert_eq!(game.cash, 10);
        assert_eq!(game.earned, game.cash);
        assert_eq!(game.advance(60_000), 0);
        assert!(game.dispatch(0));
    }

    #[test]
    fn managers_preserve_existing_trip_and_earn_while_away() {
        let mut game = funded(200);
        assert!(!game.hire(1));
        game.dispatch(0);
        game.advance(500);
        assert!(game.hire(0));
        assert!(!game.hire(0));
        assert_eq!(game.cash, 104);
        assert_eq!(game.advance(5_500), 24);
        assert_eq!(game.shops[0].elapsed_ms, 0);
        assert!(game.shops[0].running);
        assert_eq!(game.advance(2_499), 8);
        assert_eq!(game.shops[0].elapsed_ms, 499);
    }

    #[test]
    fn advances_compose_exactly_across_many_businesses() {
        let mut game = funded(20_000_000);
        for i in 0..5 {
            game.buy(i, Bulk::Ten);
            if i % 2 == 0 {
                game.hire(i);
            } else {
                game.dispatch(i);
            }
        }
        let mut single = game.clone();
        let pieces = [1, 15, 529, 2_085, 1_000_059, 999_123];
        let chunk_income: u64 = pieces.iter().map(|&delta| game.advance(delta)).sum();
        assert_eq!(single.advance(pieces.iter().sum()), chunk_income);
        assert_eq!(single, game);
    }

    #[test]
    fn quote_agrees_with_individual_prices_and_is_atomic() {
        let mut game = funded(650);
        let mut singles = game.clone();
        let (count, price) = game.quote(0, Bulk::Ten);
        assert_eq!(count, 10);
        assert_eq!(game.buy(0, Bulk::Ten), 10);
        for _ in 0..10 {
            assert_eq!(singles.buy(0, Bulk::One), 1);
        }
        assert_eq!(game.cash, 650 - price);
        assert_eq!(singles, game);
        assert_eq!(game.quote(2, Bulk::Max), (0, 0));
        assert_eq!(game.buy(2, Bulk::One), 0);
        let (count, price) = game.quote(1, Bulk::Max);
        assert_eq!(count, 2);
        assert_eq!(game.buy(1, Bulk::Max), count);
        assert_eq!(game.cash, singles.cash - price);
        let before = game.clone();
        assert_eq!(game.buy(1, Bulk::Ten), 0);
        assert_eq!(game, before);
        assert_eq!(game.quote(5, Bulk::Max), (0, 0));
    }

    #[test]
    fn all_milestones_and_shop_caps_preserve_progress() {
        let mut game = funded(u64::MAX);
        game.shops[0].owned = 24;
        game.dispatch(0);
        game.advance(1_000);
        assert_eq!(game.payout(0), 24 * 8 * 2);
        assert_eq!(game.next_milestone(0), Some(25));
        assert_eq!(game.buy(0, Bulk::One), 1);
        assert_eq!(game.payout(0), 25 * 8 * 4);
        assert_eq!(game.duration_ms(0), 1_000);
        assert_eq!(game.shops[0].elapsed_ms, 500);
        assert_eq!(game.advance(499), 0);
        assert_eq!(game.advance(1), 800);
        game.shops[0].owned = 198;
        assert_eq!(game.quote(0, Bulk::Ten).0, 2);
        assert_eq!(game.buy(0, Bulk::Max), 2);
        assert_eq!(game.duration_ms(0), 250);
        assert_eq!(game.payout(0), 200 * 8 * 64);
        assert_eq!(game.next_milestone(0), None);
        assert_eq!(game.buy(0, Bulk::One), 0);
    }

    #[test]
    fn charter_settlement_is_optional_and_permanent() {
        let mut game = funded(4 * CHARTER_UNIT - 1);
        assert_eq!(game.charter_offer(), 1);
        assert_eq!(game.charter_target(), 4 * CHARTER_UNIT);
        game.ring();
        assert_eq!(game.charter_offer(), 2);
        assert!(game.hire(0));
        assert_eq!(game.charter(), 2);
        assert_eq!(game.charters, 2);
        assert_eq!(game.cash, 0);
        assert_eq!(game.earned, 0);
        assert!(!game.shops[0].manager);
        assert_eq!(game.payout(0), 12);
        assert_eq!(game.opening_payout(1), 84);
        assert_eq!(game.payout(1), 0);
        assert_eq!(game.charter_target(), CHARTER_UNIT);
        assert_eq!(game.charter(), 0);
    }

    #[test]
    fn v1_save_imports_with_empty_album_and_freight() {
        // A stable fixture protects older installations from silent wire changes.
        let mut v1 = [0; 71];
        v1[..8].copy_from_slice(&[0x42, 0x57, 0x48, 0x31, 0x15, 0xcd, 0x5b, 0x07]);
        v1[32] = 1;
        v1[67..].copy_from_slice(&[0x1d, 0x92, 0xb0, 0xad]);
        let (game, paid) = Economy::decode(&v1, 123_456_789).unwrap();
        assert_eq!(paid, 0);
        assert_eq!(game.postmarks, 0);
        assert_eq!(game.encode(123_456_789), Economy::new().encode(123_456_789));
        v1[32] ^= 1;
        assert!(Economy::decode(&v1, 123_456_789).is_none());
    }

    #[test]
    fn stamp_album_survives_charters_and_never_adds_coins() {
        let mut game = funded(5_000_000);
        assert!(!game.mark_berth(1));
        assert!(!game.mark_berth(usize::MAX));
        for berth in 0..SHOP_NAMES.len() {
            game.buy(berth, Bulk::One);
            let (cash, earned) = (game.cash, game.earned);
            assert!(game.mark_berth(berth));
            assert!(!game.mark_berth(berth));
            assert_eq!((game.cash, game.earned), (cash, earned));
        }
        assert!(game.charter() > 0);
        let (restored, _) = Economy::decode(&game.encode(11), 12).unwrap();
        assert_eq!(restored.shops[4].owned, 0);
        assert_eq!(restored.postmarks, 0b11111);
    }

    #[test]
    fn v2_migration_preserves_stamps_charters_and_work_in_progress() {
        // Fixed 72-byte fixture exported by the BWH2 edition.
        let mut v2 = [0; V2_SAVE_LEN];
        v2[..6].copy_from_slice(b"BWH2\xe8\x03");
        v2[12] = 20;
        v2[20..22].copy_from_slice(&[0x2c, 1]);
        v2[28] = 2;
        v2[32..37].copy_from_slice(&[5, 0, 3, 0xe8, 3]);
        v2[67..].copy_from_slice(&[0x1b, 0x85, 0xf1, 0xfe, 0x69]);
        let (game, income) = Economy::decode(&v2, 1_001).unwrap();
        assert_eq!(income, 60);
        assert_eq!((game.cash, game.earned, game.charters), (80, 360, 2));
        assert_eq!((game.postmarks, game.shops[0].owned), (0x1b, 5));
        assert_eq!(game.freight.stock, [1, 0, 0]);
        assert!(game.shops[0].manager);
        assert_eq!(game.shops[0].elapsed_ms, 0);
        assert_eq!(&game.encode(1_001)[..4], b"BWH8");
    }

    #[test]
    fn v3_migration_preserves_order_timer_stock_and_previous_work() {
        let mut v3 = [0; V3_SAVE_LEN];
        v3[..6].copy_from_slice(b"BWH3\xe8\x03");
        v3[12] = 20;
        v3[20..22].copy_from_slice(&[0x2c, 1]);
        v3[28] = 2;
        v3[32..37].copy_from_slice(&[5, 0, 3, 0xe8, 3]);
        v3[67..76].copy_from_slice(&[0x1b, 9, 0, 7, 0, 5, 0, 1, 13]);
        v3[83..].copy_from_slice(&[72, 113, 1, 3, 0x83, 0xfc, 0x0b]);
        let (mut game, income) = Economy::decode(&v3, 1_000).unwrap();
        assert_eq!(income, 0);
        assert_eq!(game.yard, Yard::new());
        assert_eq!(game.freight.stock, [9, 7, 5]);
        assert_eq!(game.freight.capacity(), 48);
        assert_eq!(game.postmarks, 0x1b);
        assert_eq!(game.advance(1_000), 60 + 56);
        assert_eq!(game.freight.tide, 14);
        assert_eq!(game.freight.stock, [10, 7, 5]);
    }

    #[test]
    fn v8_save_round_trip_and_single_bit_damage_detection() {
        let mut game = funded(100_000);
        game.hire(0);
        game.buy(2, Bulk::Ten);
        game.dispatch(2);
        game.advance(333);
        assert!(game.mark_berth(2));
        let save = game.encode(160);
        assert_eq!(&save[..4], b"BWH8");
        assert_eq!(save[67], 0b100);
        let (restored, paid) = Economy::decode(&save, 160).unwrap();
        assert_eq!(paid, 0);
        assert_eq!(restored.encode(160), save);
        assert_eq!(restored.shops, game.shops);
        assert!(Economy::decode(&save[..SAVE_LEN - 1], 160).is_none());
        for bit in 0..SAVE_LEN * 8 {
            let mut damaged = save;
            damaged[bit / 8] ^= 1 << (bit % 8);
            assert!(Economy::decode(&damaged, 160).is_none());
        }
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn invalid_state_is_rejected_even_with_a_valid_checksum() {
        let mut game = Economy::new();
        game.postmarks = 32;
        assert!(Economy::decode(&game.encode(0), 0).is_none());
        game.postmarks = 0;
        game.shops[0].owned = 0;
        assert!(Economy::decode(&game.encode(0), 0).is_none());
        game.shops[0].owned = 1;
        game.shops[3].manager = true;
        assert!(Economy::decode(&game.encode(0), 0).is_none());
        game.shops[3] = Shop::default();
        game.shops[0].elapsed_ms = game.duration_ms(0);
        game.shops[0].running = true;
        assert!(Economy::decode(&game.encode(0), 0).is_none());
    }

    #[test]
    fn offline_limit_partial_trip_and_resave_handle_clock_rollback() {
        let mut game = funded(1_000);
        game.hire(0);
        game.buy(1, Bulk::One);
        game.dispatch(1);
        game.advance(995);
        let later = 100 + OFFLINE_LIMIT_S * 10;
        let (loaded, income) = Economy::decode(&game.encode(100), later).unwrap();
        assert_eq!(income, OFFLINE_LIMIT_S / 2 * 8 + 56);
        assert_eq!(loaded.shops[0].elapsed_ms, 995);
        assert!(!loaded.shops[1].running);
        let save = loaded.encode(later);
        assert_eq!(loaded.encode(later - 5), save);
        let (rewound, income) = Economy::decode(&save, later - 100).unwrap();
        assert_eq!(income, 0);
        assert_eq!(rewound.encode(later - 100), save);
        assert_eq!(Economy::decode(&save, later).unwrap().1, 0);
        assert_eq!(Economy::decode(&save, later + 2).unwrap().1, 8);
    }

    #[test]
    fn maximal_time_and_receipts_saturate_without_overflow() {
        let mut game = funded(u64::MAX);
        game.charters = u32::MAX;
        game.buy(4, Bulk::Max);
        game.hire(4);
        assert_eq!(game.advance(u64::MAX), u64::MAX);
        assert_eq!(game.cash, u64::MAX);
        assert_eq!(game.earned, u64::MAX);
        assert_eq!(game.charter_offer(), 0);
        assert!(game.shops[4].elapsed_ms < game.duration_ms(4));
        assert!(Economy::decode(&game.encode(0), u64::MAX).is_some());
    }

    #[test]
    fn one_crate_per_completed_voyage_and_spot_sales_credit_exactly() {
        let mut game = funded(10_000);
        game.buy(0, Bulk::Ten);
        game.buy(1, Bulk::One);
        game.buy(2, Bulk::One);
        for berth in 0..3 {
            game.dispatch(berth);
        }
        game.advance(2_000);
        assert_eq!(game.freight.stock, [1, 0, 0]);
        game.advance(24_000);
        assert_eq!(game.freight.stock, [1, 1, 1]);
        game.dispatch(0);
        game.advance(2_000);
        assert_eq!(game.freight.stock, [2, 1, 1]);
        let before = (game.cash, game.earned);
        assert_eq!(game.sell(0, false), 6);
        assert_eq!(game.freight.stock, [1, 1, 1]);
        assert_eq!(game.sell(0, true), 6);
        assert_eq!(game.sell(1, true), 18);
        assert_eq!(game.sell(2, false), 42);
        assert_eq!(game.sell(2, false), 0);
        assert_eq!(game.sell(3, true), 0);
        assert_eq!((game.cash, game.earned), (before.0 + 72, before.1 + 72));
    }

    #[test]
    fn incoming_receipt_settles_once_even_across_many_tides_or_reload() {
        let mut game = Economy::new();
        assert!(!game.reserve_order());
        for _ in 0..3 {
            game.dispatch(0);
            game.advance(2_000);
        }
        assert_eq!(game.freight.stock, [3, 0, 0]);
        assert!(game.reserve_order());
        assert!(!game.reserve_order());
        assert_eq!(game.freight.stock, [0, 0, 0]);
        assert_eq!(game.sell(0, true), 0);
        assert_eq!(game.cash, 24);
        let mut single = game.clone();
        assert_eq!(game.advance(23_999), 0);
        assert!(game.freight.reserved);
        assert_eq!(game.advance(1), 42);
        assert!(!game.freight.reserved);
        assert_eq!(game.advance(7_200_000), 0);
        assert_eq!(single.advance(7_224_000), 42);
        assert_eq!(game, single);
        let mut pending = Economy::new();
        pending.freight.stock[0] = 3;
        pending.ledger.flows[0].opening = 3;
        pending.reserve_order();
        let (loaded, income) = Economy::decode(&pending.encode(100), u64::MAX).unwrap();
        assert_eq!(income, 42);
        assert_eq!(loaded.freight.tide, OFFLINE_LIMIT_S / 30);
        assert_eq!(
            Economy::decode(&loaded.encode(u64::MAX), u64::MAX)
                .unwrap()
                .1,
            0
        );
    }

    #[test]
    fn hold_upgrades_raise_each_crate_limit_and_charters_close_the_board() {
        let mut game = funded(800_000);
        game.mark_berth(0);
        for i in 0..3 {
            game.buy(i, Bulk::One);
            game.hire(i);
        }
        game.advance(3_600_000);
        assert_eq!(game.freight.stock, [24; 3]);
        let before = (game.cash, game.earned);
        assert!(game.upgrade_hold());
        assert_eq!((game.cash, game.earned), (before.0 - 1_800, before.1));
        assert_eq!(game.freight.stock, [24; 3]);
        game.advance(3_600_000);
        assert_eq!(game.freight.stock, [48; 3]);
        assert!(game.upgrade_hold());
        assert!(!game.upgrade_hold());
        game.advance(3_600_000);
        assert_eq!(game.freight.stock, [96; 3]);
        assert!(game.reserve_order());
        assert!(game.charter() >= 1);
        assert_eq!(
            game.freight,
            Freight {
                tide: 360,
                ..Freight::new()
            }
        );
        assert_eq!(game.postmarks, 1);
        assert_eq!(game.advance(30_000), 0);
        assert!(!game.upgrade_hold());
    }

    #[test]
    fn malformed_freight_and_shipyard_fields_with_a_valid_crc_are_rejected() {
        let save = Economy::new().encode(0);
        for (offset, value) in [
            (68, 25),
            (74, 3),
            (84, 128),
            (85, 2),
            (86, 2),
            (87, 2),
            (88, 1),
        ] {
            let mut invalid = save;
            invalid[offset] = value;
            let checksum = crc32(&invalid[..SAVE_LEN - 4]);
            invalid[SAVE_LEN - 4..].copy_from_slice(&checksum.to_le_bytes());
            assert!(Economy::decode(&invalid, 0).is_none());
        }
        let mut game = Economy::new();
        game.yard.building = true;
        game.yard.completed = 7;
        assert!(Economy::decode(&game.encode(0), 0).is_none());
        game.yard.completed = 3;
        game.yard.elapsed_ms = 45_000;
        assert!(Economy::decode(&game.encode(0), 0).is_none());
    }

    #[test]
    fn commissioning_is_sequential_and_spends_exactly_once() {
        let mut game = funded(3_924);
        game.freight.stock = [22, 12, 4];
        for (i, plan) in PLANS.iter().enumerate() {
            let before = game.clone();
            assert!(!game.build(i + 1));
            assert_eq!(game, before);
            assert!(game.build(i));
            let expected_stock: [u16; 3] =
                core::array::from_fn(|n| before.freight.stock[n] - plan.cargo[n]);
            assert_eq!(game.freight.stock, expected_stock);
            assert_eq!(game.cash, before.cash - plan.cost);
            assert_eq!(game.earned, before.earned);
            let paid = game.clone();
            assert!(!game.build(i));
            assert_eq!(game, paid);
            game.advance(u64::from(plan.duration_ms));
            assert!(!game.build(i));
        }
        assert_eq!(game.yard.next(), None);
        assert_eq!(game.yard.bonus_percent(), 60);
        assert_eq!(game.cash, 0);
        assert_eq!(game.freight.stock, [0; 3]);
        game.charters = 1;
        assert_eq!(game.opening_payout(1), 56 * 185 / 100);
        assert_eq!(game.ring(), 1);
        game.freight.stock[0] = 8;
        let sale = game.freight.bid_lot(0, 8).coins;
        assert_eq!(game.sell(0, true), sale);
    }

    #[test]
    fn launch_applies_to_arrivals_on_that_millisecond_and_composes() {
        let mut game = funded(1_000);
        game.freight.stock[0] = 4;
        game.hire(0);
        assert!(game.build(0));
        let mut bulk = game.clone();
        assert_eq!(game.advance(11_999), 5 * 8);
        assert_eq!(game.yard.elapsed_ms, 11_999);
        assert_eq!(game.yard.bonus_percent(), 0);
        assert_eq!(game.advance(1), 9);
        assert_eq!(game.yard.completed, 1);
        assert_eq!(game.advance(18_000), 9 * 9);
        assert_eq!(bulk.advance(30_000), 5 * 8 + 10 * 9);
        assert_eq!(bulk, game);
        assert!(game.yard.valid());
    }

    #[test]
    fn paid_work_survives_charter_and_offline_launch_does_not_replay() {
        let mut game = funded(CHARTER_UNIT);
        game.freight.stock[0] = 3;
        assert!(!game.build(0));
        game.freight.stock[0] = 4;
        game.ledger.flows[0].opening = 4;
        assert!(game.build(0));
        game.advance(2_001);
        assert_eq!(game.charter(), 1);
        assert!(game.yard.building);
        assert_eq!(game.yard.elapsed_ms, 2_001);
        game.ring();
        let (launched, income) = Economy::decode(&game.encode(100), 110).unwrap();
        assert_eq!(income, 10);
        assert_eq!(launched.yard.completed, 1);
        assert_eq!(launched.yard.elapsed_ms, 0);
        assert!(!launched.yard.building);
        assert_eq!(launched.payout(0), 11);
        let (again, income) = Economy::decode(&launched.encode(110), 110).unwrap();
        assert_eq!(income, 0);
        assert_eq!(again, launched);
    }
    #[test]
    fn v4_with_a_paid_hull_and_freight_migrates_without_rebooking_receipts() {
        // Exported by the BWH4 browser build, halfway through a rowboat trip.
        let v4: [u8; V4_SAVE_LEN] = [
            66, 87, 72, 52, 160, 134, 1, 0, 0, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 240, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 1, 0, 3, 232, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 248, 42, 0, 0, 1, 184, 11, 0, 0, 62, 43, 194, 111,
        ];
        let (mut port, gain) = Economy::decode(&v4, 100_000).unwrap();
        assert_eq!(gain, 0);
        assert_eq!(port.charter_earned(), 240);
        assert_eq!(port.ledger.flows[0].opening, 1);
        assert_eq!(port.ledger.sales, 0);
        assert_eq!(port.yard.elapsed_ms, 3_000);
        assert_eq!(port.freight.market.history_len, 0);
        assert_eq!(port.freight.market.stock, [40; 5]);
        assert_eq!(port.advance(9_000), 41);
        assert_eq!(port.yard.completed, 1);
        assert_eq!(port.ledger.flows[0].landed, 5);
        assert_eq!(port.inventory(0), 6);
    }

    #[test]
    fn specified_lots_wallet_limits_and_churn_never_mint_charters() {
        let mut port = funded(CHARTER_UNIT - 1);
        for g in 0..3 {
            for quantity in [0, 1, 2, 7, 19, u16::MAX] {
                let buy = port.quote_lot(g, true, quantity);
                assert_eq!(port.trade_lot(g, true, quantity), buy);
                assert_eq!(port.inventory(g), buy.qty);
                let sell = port.quote_lot(g, false, quantity);
                assert_eq!(port.trade_lot(g, false, quantity), sell);
                assert_eq!(port.inventory(g), 0);
                assert_eq!(port.freight.market.stock[g], 40);
                if buy.qty > 0 {
                    assert!(sell.coins < buy.coins);
                }
            }
        }
        assert!(port.earned > CHARTER_UNIT);
        assert_eq!(port.charter_offer(), 0);
        assert_eq!(port.charter_earned(), CHARTER_UNIT - 1);
        port.ring();
        assert_eq!(port.charter_offer(), 1);
        let mut poor = funded(13);
        assert_eq!(poor.trade_lot(0, true, 4), Quote { qty: 1, coins: 7 });
        assert_eq!(poor.trade_lot(0, true, 4), Quote::default());
        assert_eq!(poor.trade_lot(4, true, 4), Quote::default());
    }

    #[test]
    fn parallel_factories_reconcile_orders_inputs_output_and_cash() {
        let mut port = funded(10_000);
        assert!(port.open_factory(0));
        assert!(port.open_factory(1));
        assert!(!port.open_factory(1));
        for (g, q) in [5, 2, 2].into_iter().enumerate() {
            assert_eq!(port.trade_lot(g, true, q).qty, q);
        }
        assert!(port.reserve_order());
        let purchases = port.ledger.purchases;
        assert!(port.start_batch(0));
        assert!(port.start_batch(1));
        assert_eq!(port.freight.stock, [0, 0, 0]);
        let snapshot = port.encode(100);
        let mut live = port.clone();
        live.advance(30_000);
        let (offline, earnings) = Economy::decode(&snapshot, 130).unwrap();
        assert_eq!(earnings, 42);
        assert_eq!(live.encode(130), offline.encode(130));
        assert_eq!(live.inventory(3), 1);
        assert_eq!(live.inventory(4), 1);
        let sales = live.sell(3, true) + live.sell(4, true);
        assert_eq!(live.ledger.sales, sales);
        assert_eq!(live.ledger.orders, 42);
        assert_eq!(live.ledger.fees, 22);
        assert_eq!(live.ledger.construction, 2_220);
        assert_eq!(live.cash, 10_000 - purchases - 2_220 - 22 + 42 + sales);
        for i in 0..5 {
            let f = live.ledger.flows[i];
            assert_eq!(
                u64::from(f.opening) + f.landed + f.bought + f.made,
                u64::from(live.inventory(i)) + f.used + f.delivered + f.sold
            );
        }
        assert_eq!(
            live.charter_earned(),
            10_000 + (sales + 42).saturating_sub(purchases + 22)
        );
        assert!(Economy::decode(&live.encode(130), 130).is_some());
    }

    #[test]
    fn competing_repeat_batches_resume_and_stop_at_the_hold_limit() {
        let mut port = funded(60_000);
        for g in 0..3 {
            port.buy(g, Bulk::One);
            port.hire(g);
        }
        for i in 0..2 {
            port.open_factory(i);
            port.set_repeat(i, true);
        }
        let mut bulk = port.clone();
        bulk.advance(43_200_000);
        for _ in 0..10_800 {
            port.advance(4_000);
        }
        assert_eq!(bulk, port);
        assert_eq!(port.inventory(3), 24);
        assert_eq!(port.inventory(4), 24);
        assert!(!port.factories[0].running && !port.factories[1].running);
        assert!(port.sell(3, false) > 0);
        assert!(port.factories[0].running);
        assert!(!port.factories[1].running);
        port.advance(u64::MAX);
        assert_eq!(port.inventory(3), 24);
        assert_eq!(port.ledger.flows[3].made, 25);
    }

    #[test]
    fn v5_rejects_inconsistent_books_factory_flags_and_depth() {
        let mut port = funded(2_000);
        port.trade_lot(0, true, 3);
        let save = port.encode(100);
        for offset in [92usize, 102, 183, 253] {
            let mut invalid = save;
            invalid[offset] = 250;
            let crc = crc32(&invalid[..SAVE_LEN - 4]);
            invalid[SAVE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
            assert!(Economy::decode(&invalid, 100).is_none());
        }
        port.ledger.flows[0].sold += 1;
        assert!(Economy::decode(&port.encode(100), 100).is_none());
    }
    #[test]
    fn sale_ticket_marginal_floor_partials_cancellation_and_expiry() {
        let mut port = funded(10_000);
        assert_eq!(port.trade_lot(2, true, 24).qty, 24);
        let mut next = port.clone();
        next.advance(30_000);
        let high = next.freight.price(2);
        let floor = (1..=high)
            .rev()
            .find(|&f| {
                let q = next.ticket_quote(2, 24, f as u16);
                q.qty > 0 && q.qty < 24
            })
            .unwrap() as u16;
        let expected = next.ticket_quote(2, 24, floor);
        assert!(port.place_sale(2, floor, 24));
        assert!(!port.place_sale(1, 1, 2));
        assert_eq!(port.inventory(2), 24);
        assert_eq!(port.advance(29_999), 0);
        assert_eq!(port.advance(1), expected.coins);
        assert_eq!(port.ticket.filled, expected.qty);
        assert_eq!(port.ticket.proceeds, expected.coins);
        assert_eq!(port.ticket.tides_left, 11);
        assert!(expected.coins >= u64::from(floor) * u64::from(expected.qty));
        assert_eq!(port.ledger.flows[2].sold, u64::from(expected.qty));
        assert_eq!(port.ticket.status, TicketStatus::Pending);
        assert!(port.cancel_sale());
        assert_eq!(port.ticket.status, TicketStatus::Cancelled);
        assert_eq!(port.ticket.filled, expected.qty);
        assert!(!port.cancel_sale());
        assert!(port.place_sale(2, u16::MAX, 24));
        port.advance(11 * 30_000);
        assert_eq!(port.ticket.tides_left, 1);
        assert_eq!(port.ticket.status, TicketStatus::Pending);
        port.advance(30_000);
        assert_eq!(port.ticket.status, TicketStatus::Expired);
        assert_eq!(port.ticket.filled, 0);
        assert!(!port.place_sale(5, 1, 1));
        assert!(!port.place_sale(2, 0, 1));
        assert!(!port.place_sale(2, 1, 97));
    }

    #[test]
    fn ticket_settles_factory_output_completed_on_the_same_millisecond() {
        let mut port = funded(2_000);
        assert!(port.open_factory(0));
        port.trade_lot(0, true, 1);
        port.trade_lot(1, true, 2);
        port.advance(22_000);
        assert!(port.start_batch(0));
        assert!(port.place_sale(3, 1, 2));
        assert_eq!(port.ticket_quote(3, 1, 1), Quote::default());
        let before = port.clone();
        let save = port.encode(100);
        port.advance(7_999);
        assert_eq!(port.ticket.filled, 0);
        let mut expected_market = before.freight.market.clone();
        expected_market.advance(before.freight.tide, 1);
        let estimated = expected_market.bid_lot(3, 1, 1);
        assert_eq!(port.advance(1), estimated.coins);
        assert_eq!(port.ticket.filled, 1);
        assert_eq!(port.ticket.remaining, 1);
        assert_eq!(port.inventory(3), 0);
        assert_eq!(port.ledger.flows[3].made, 1);
        assert_eq!(port.ledger.flows[3].sold, 1);
        let (restored, paid) = Economy::decode(&save, 108).unwrap();
        assert_eq!(paid, estimated.coins);
        assert_eq!(port.encode(108), restored.encode(108));
        let mut jump = port.clone();
        let mut fragments = port;
        let reward = jump.advance(u64::MAX);
        assert_eq!(
            reward,
            fragments
                .advance(360_000)
                .saturating_add(fragments.advance(u64::MAX - 360_000))
        );
        assert_eq!(jump, fragments);
        assert_eq!(jump.ticket.status, TicketStatus::Expired);
        assert_eq!(jump.ticket.proceeds, estimated.coins);
    }

    #[test]
    fn live_ticket_does_not_reserve_stock_and_old_books_have_no_tickets() {
        let mut port = funded(CHARTER_UNIT);
        port.trade_lot(0, true, 8);
        assert!(port.place_sale(0, 1, 8));
        assert!(port.reserve_order());
        assert!(port.build(0));
        assert_eq!(port.inventory(0), 1);
        port.advance(30_000);
        assert_eq!(port.ticket.filled, 1);
        assert_eq!(port.ticket.remaining, 7);
        assert_eq!(port.ledger.orders, 42);
        let snapshot = port.encode(150);
        let mut old = [0u8; V5_INITIAL_SAVE_LEN];
        let cut = V5_INITIAL_SAVE_LEN - 4;
        old[..cut].copy_from_slice(&snapshot[..cut]);
        old[..4].copy_from_slice(b"BWH5");
        let crc = crc32(&old[..cut]);
        old[cut..].copy_from_slice(&crc.to_le_bytes());
        let (migrated, _) = Economy::decode(&old, 150).unwrap();
        assert_eq!(migrated.ticket.status, TicketStatus::Empty);
        assert_eq!(migrated.ledger, port.ledger);
        for (index, val) in [(503, 5), (504, 5), (521, 250), (518, 255)] {
            let mut bad = snapshot;
            bad[index] = val;
            let crc = crc32(&bad[..SAVE_LEN - 4]);
            bad[SAVE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
            assert!(Economy::decode(&bad, 150).is_none());
        }
        assert!(port.charter() > 0);
        assert_eq!(port.ticket.status, TicketStatus::Empty);
    }
    fn solve_manifest(port: &mut Economy, ending: u8) {
        for (chapter, pair) in crate::manifest_text::PAIRS.iter().enumerate() {
            let [a, b] = pair.map(usize::from);
            assert!(port.inspect_lead(chapter, a));
            assert!(port.inspect_lead(chapter, b));
            assert!(port.prove_pair(chapter, a, b));
        }
        assert!(port.choose_ending(ending));
    }

    #[test]
    fn first_landing_reveals_the_case_and_progress_outlives_cargo_and_charters() {
        let mut port = funded(CHARTER_UNIT);
        port.ring();
        port.trade_lot(0, true, 2);
        port.advance(1_999);
        assert!(!port.manifest.unlocked);
        let before = port.encode(1);
        assert!(!Economy::decode(&before, 1).unwrap().0.manifest.unlocked);
        port.advance(1);
        assert!(port.manifest.unlocked);
        port.sell(0, true);
        assert_eq!(port.inventory(0), 0);
        let [a, b] = crate::manifest_text::PAIRS[0].map(usize::from);
        assert!(port.inspect_lead(0, a));
        let (mut restored, _) = Economy::decode(&port.encode(2), 2).unwrap();
        assert!(restored.charter() > 0);
        assert!(restored.inspect_lead(0, b));
        assert!(restored.prove_pair(0, b, a));
        assert_eq!(restored.manifest.solved, 1);
        assert_eq!(
            Economy::decode(&restored.encode(3), 3).unwrap().0.manifest,
            restored.manifest
        );
        let (away, coins) = Economy::decode(&before, 2).unwrap();
        assert!(away.manifest.unlocked);
        assert_eq!(coins, 8);
    }

    #[test]
    fn each_ending_pays_exactly_its_own_permanent_effect_in_live_and_offline_play() {
        for choice in [1, 2] {
            let mut port = funded(CHARTER_UNIT);
            port.dispatch(0);
            port.advance(2_000);
            solve_manifest(&mut port, choice);
            port.charter();
            assert!(!port.choose_ending(if choice == 1 { 2 } else { 1 }));
            let tip = if choice == 1 { 2 } else { 1 };
            let fare = if choice == 1 { 10 } else { 11 };
            assert_eq!(port.ring(), tip);
            assert_eq!(port.cash, tip);
            assert_eq!(port.opening_payout(0), fare);
            assert_eq!(port.payout(1), 0);
            assert_eq!(port.opening_payout(1), 70);
            port.credit(96);
            port.hire(0);
            let snapshot = port.encode(200);
            assert_eq!(port.advance(10_000), 5 * fare);
            let (restored, paid) = Economy::decode(&snapshot, 210).unwrap();
            assert_eq!(paid, 5 * fare);
            assert_eq!(restored.encode(210), port.encode(210));
        }
    }

    #[test]
    fn predecessor_ticket_and_gross_survive_and_fresh_old_ports_stay_sealed() {
        fn older(port: &Economy) -> [u8; V5_SAVE_LEN] {
            let snapshot = port.encode(123);
            let mut old = [0u8; V5_SAVE_LEN];
            let cut = V5_SAVE_LEN - 4;
            old[..cut].copy_from_slice(&snapshot[..cut]);
            old[..4].copy_from_slice(b"BWH5");
            let crc = crc32(&old[..cut]);
            old[cut..].copy_from_slice(&crc.to_le_bytes());
            old
        }
        let mut fresh = Economy::new();
        fresh.ring();
        fresh.advance(1_500);
        assert!(
            !Economy::decode(&older(&fresh), 123)
                .unwrap()
                .0
                .manifest
                .unlocked
        );
        assert!(
            Economy::decode(&older(&fresh), 124)
                .unwrap()
                .0
                .manifest
                .unlocked
        );
        let mut old_port = funded(10_000);
        old_port.trade_lot(0, true, 7);
        old_port.place_sale(0, 16, 5);
        let (loaded, paid) = Economy::decode(&older(&old_port), 123).unwrap();
        assert_eq!(paid, 0);
        assert!(loaded.manifest.unlocked);
        assert_eq!(loaded.ticket, old_port.ticket);
        assert_eq!(loaded.ledger, old_port.ledger);
        assert_eq!(loaded.cash, old_port.cash);
        let valid = fresh.encode(123);
        for (index, value) in [(522, 2), (523, 1), (524, 1), (527, 9)] {
            let mut bad = valid;
            bad[index] = value;
            let crc = crc32(&bad[..SAVE_LEN - 4]);
            bad[SAVE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
            assert!(Economy::decode(&bad, 123).is_none());
        }
    }
    #[test]
    fn a_quay_visit_checks_terms_and_pays_the_contract_ledger_once() {
        let mut port = funded(200);
        let nell = port.current_favor();
        assert_eq!(
            (nell.neighbor, nell.tier, nell.cargo, nell.reward),
            (0, 0, [1, 0, 0, 0, 0], 12)
        );
        assert!(!port.can_help());
        assert!(!port.help_neighbor(nell));
        assert_eq!(port.trade_lot(0, true, 1), Quote { qty: 1, coins: 7 });
        assert!(port.can_help());
        let before = port.clone();
        assert!(!port.help_neighbor(Favor {
            reward: 1_000_000,
            ..nell
        }));
        assert_eq!(port, before);
        assert!(port.help_neighbor(nell));
        assert_eq!(port.current_favor(), nell);
        assert!(!port.help_neighbor(nell));
        assert_eq!(port.cash, 205);
        assert_eq!(port.charter_earned(), 205);
        assert_eq!(port.earned, 212);
        assert_eq!(port.ledger.orders, 12);
        assert_eq!(port.ledger.flows[0].delivered, 1);
        assert_eq!(port.inventory(0), 0);
        port.advance(30_000);
        let ada = port.current_favor();
        port.trade_lot(0, true, 2);
        assert!(!port.help_neighbor(nell));
        assert!(port.help_neighbor(ada));
        assert!(Economy::decode(&port.encode(0), 0).is_some());
    }

    #[test]
    fn the_lanterns_return_each_dusk_and_the_charter_keeps_appointments() {
        let mut port = funded(CHARTER_UNIT);
        port.trade_lot(0, true, 12);
        for i in 0..3 {
            let favor = port.current_favor();
            assert_eq!(favor.neighbor, i);
            assert!(port.help_neighbor(favor));
            if i < 2 {
                port.advance(30_000);
            }
        }
        assert!(port.neighbors.lanterns_unlocked());
        assert!(!port.lanterns_sailing());
        port.advance(13_000);
        let snapshot = port.encode(100);
        let (mut same, _) = Economy::decode(&snapshot, 100).unwrap();
        assert_eq!(same.charter(), 1);
        assert_eq!(same.freight.tide, 2);
        assert_eq!(same.freight.elapsed_ms, 13_000);
        assert!(same.neighbors.served);
        assert_eq!(same.neighbors.helped, [1, 1, 1]);
        assert!(!same.can_help());
        same.advance(17_000);
        assert!(!same.neighbors.served);
        assert_eq!(same.current_favor().tier, 1);
        same.advance(90_000);
        assert!(same.lanterns_sailing());
        same.advance(90_000);
        assert!(!same.lanterns_sailing());
        same.advance(270_000);
        assert!(same.lanterns_sailing());
        let (offline, delta) = Economy::decode(&snapshot, 100 + 12 * 3600).unwrap();
        assert_eq!(delta, 0);
        assert_eq!(offline.neighbors.helped, [1, 1, 1]);
        assert!(!offline.neighbors.served);
        assert_eq!(offline.ledger.orders, 45);
    }

    #[test]
    fn higher_tiers_take_real_products_and_the_last_terms_repeat() {
        let mut port = funded(10_000);
        port.trade_lot(0, true, 20);
        port.trade_lot(1, true, 10);
        port.open_factory(0);
        assert!(port.help_neighbor(port.current_favor()));
        port.advance(90_000);
        let second = port.current_favor();
        assert_eq!(second.tier, 1);
        assert!(port.help_neighbor(second));
        port.advance(90_000);
        let third = port.current_favor();
        assert_eq!(third.cargo, [3, 0, 0, 1, 0]);
        assert!(!port.can_help());
        assert!(port.start_batch(0));
        port.advance(8_000);
        assert_eq!(port.inventory(3), 1);
        assert!(port.help_neighbor(third));
        assert_eq!(port.inventory(3), 0);
        assert_eq!(port.ledger.flows[3].delivered, 1);
        port.advance(82_000);
        let again = port.current_favor();
        assert_eq!(
            (again.tier, again.reward, again.cargo),
            (third.tier, third.reward, third.cargo)
        );
        assert!(Economy::decode(&port.encode(0), 0).is_some());
    }

    #[test]
    fn v6_keeps_a_signed_manifest_and_imports_no_imaginary_relationships() {
        let mut port = funded(CHARTER_UNIT);
        port.dispatch(0);
        port.advance(2_000);
        solve_manifest(&mut port, 2);
        assert!(port.place_sale(0, 19, 1));
        let latest = port.encode(9);
        let mut old = [0; V6_SAVE_LEN];
        let cut = V6_SAVE_LEN - 4;
        old[..cut].copy_from_slice(&latest[..cut]);
        old[..4].copy_from_slice(b"BWH6");
        let crc = crc32(&old[..cut]);
        old[cut..].copy_from_slice(&crc.to_le_bytes());
        let (migrated, paid) = Economy::decode(&old, 9).unwrap();
        assert_eq!(paid, 0);
        assert_eq!(migrated.manifest, port.manifest);
        assert_eq!(migrated.neighbors, Neighbors::new());
        assert_eq!(migrated.ticket, port.ticket);
        for flag in [1, 2] {
            let mut corrupt = latest;
            corrupt[V7_SAVE_LEN - 5] = flag;
            let crc = crc32(&corrupt[..SAVE_LEN - 4]);
            corrupt[SAVE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
            assert!(Economy::decode(&corrupt, 9).is_none());
        }
    }
    fn invited_port() -> Economy {
        let mut port = funded(10_000);
        port.yard.completed = 7;
        port.neighbors.helped = [1, 1, 1];
        port
    }

    #[test]
    fn invitation_requires_both_milestones_in_either_order_and_never_auto_signs() {
        let mut port = Economy::new();
        assert!(!port.committee_invited());
        assert!(!port.sign_order_thirteen());
        port.yard.completed = 3;
        port.neighbors.helped = [1, 1, 1];
        assert!(!port.committee_invited());
        port.yard.completed = 7;
        assert!(port.committee_invited());
        port.neighbors.helped = [1, 0, 1];
        assert!(!port.sign_order_thirteen());
        port.neighbors.helped[1] = 1;
        assert!(port.committee_invited());
        port.advance(u64::MAX);
        assert_eq!(port.committee, Committee::new());
        assert!(port.sign_order_thirteen());
        assert!(!port.sign_order_thirteen());
        assert!(!port.committee_invited());
        assert!(!port.adjourn_committee());
        assert_eq!(port.claim_tea_tin(), 0);
    }

    #[test]
    fn six_personal_motions_take_exactly_three_salt_and_three_provisions() {
        let mut port = invited_port();
        port.open_factory(0);
        port.trade_lot(0, true, 6);
        port.trade_lot(1, true, 6);
        assert!(!port.submit_motion(0));
        port.sign_order_thirteen();
        for i in 0..3 {
            assert_eq!(port.committee.motion_good(), Some(0));
            assert!(port.submit_motion(i));
            assert!(!port.submit_motion(i));
        }
        assert_eq!(port.committee.stage, Stage::Quorum);
        assert!(!port.can_submit_motion());
        assert!(!port.submit_motion(3));
        assert!(!port.adjourn_committee());
        for i in 3..6 {
            assert_eq!(port.committee.motion_good(), Some(3));
            assert!(port.start_batch(0));
            port.advance(8_000);
            assert!(port.submit_motion(i));
        }
        assert_eq!(port.committee.stage, Stage::HighTea);
        assert!(!port.submit_motion(6));
        assert!(port.committee.active());
        assert_eq!(port.ledger.flows[0].delivered, 3);
        assert_eq!(port.ledger.flows[3].delivered, 3);
        assert_eq!(port.ledger.flows[0].used, 3);
        assert_eq!(port.inventory(0), 0);
        assert_eq!(port.inventory(3), 0);
        port.dispatch(0);
        port.advance(2_000);
        let tin = port.committee.tin;
        assert!(tin > 0);
        let (cash, gross, eligible) = (port.cash, port.earned, port.charter_earned());
        assert!(port.adjourn_committee());
        assert_eq!(
            (port.cash, port.earned, port.charter_earned()),
            (cash + tin + 13, gross + 13, eligible + 13)
        );
        assert!(!port.adjourn_committee());
        assert!(!port.sign_order_thirteen());
        assert!(port.committee.souvenir());
        assert_eq!(port.claim_tea_tin(), 0);
        port.dispatch(0);
        assert_eq!(port.advance(2_000), port.payout(0));
        assert!(Economy::decode(&port.encode(0), 0).is_some());
    }

    #[test]
    fn pending_escrow_survives_charter_and_reclaim_never_rebooks_fares() {
        let mut port = invited_port();
        port.credit(CHARTER_UNIT);
        port.hire(0);
        port.sign_order_thirteen();
        let gross = port.payout(0) * 20;
        assert_eq!(port.advance(40_000), gross - gross / 8);
        let (held, remainder) = (port.committee.tin, port.committee.carry);
        assert_eq!((held, remainder), (gross / 8, (gross % 8) as u8));
        assert!(port.charter() > 0);
        assert_eq!(port.earned, 0);
        assert_eq!(port.charter_earned(), 0);
        assert_eq!(port.committee.stage, Stage::TeaService);
        assert_eq!(
            (port.committee.tin, port.committee.carry),
            (held, remainder)
        );
        assert_eq!(port.claim_tea_tin(), held);
        assert_eq!(port.cash, held);
        assert_eq!(port.earned, 0);
        assert_eq!(port.charter_earned(), 0);
        let (reopened, _) = Economy::decode(&port.encode(100), 100).unwrap();
        assert_eq!(reopened, port);
        assert_eq!(port.claim_tea_tin(), 0);
    }

    #[test]
    fn fares_only_withholding_is_partition_invariant_offline_and_at_numeric_limits() {
        let mut port = invited_port();
        port.hire(0);
        port.buy(1, Bulk::One);
        port.hire(1);
        port.sign_order_thirteen();
        let before = (port.cash, port.earned, port.charter_earned());
        assert_eq!(port.ring(), 1);
        port.trade_lot(0, true, 7);
        let sale = port.sell(0, false);
        assert!(sale > 0);
        assert_eq!(port.committee.tin, 0);
        assert!(port.reserve_order());
        let snapshot = port.encode(100);
        let (offline, available) = Economy::decode(&snapshot, 100 + 43_200).unwrap();
        let mut fragmented = port.clone();
        let mut reported = 0u64;
        for _ in 0..43_200 {
            reported += fragmented.advance(1_000);
        }
        assert_eq!(fragmented.encode(43_300), offline.encode(43_300));
        let fares = 21_600 * port.payout(0) + 7_200 * port.payout(1);
        assert_eq!(available, fares - fares / 8 + 42);
        assert_eq!(available, reported);
        assert_eq!(offline.earned - before.1, fares + 1 + sale + 42);
        assert_eq!(offline.committee.tin, fares / 8);
        assert_eq!(offline.committee.motions, 0);
        let mut single = port.clone();
        let mut split = port;
        single.advance(u64::MAX);
        split.advance(u64::MAX / 3 * 2);
        split.advance(u64::MAX - u64::MAX / 3 * 2);
        assert_eq!(single, split);
    }

    #[test]
    fn bwh7_migrates_an_unopened_invitation_and_rejects_corrupt_minutes() {
        let eligible = invited_port();
        let snapshot = eligible.encode(0);
        let cut = V7_SAVE_LEN - 4;
        let mut old = [0; V7_SAVE_LEN];
        old[..cut].copy_from_slice(&snapshot[..cut]);
        old[..4].copy_from_slice(b"BWH7");
        let crc = crc32(&old[..cut]);
        old[cut..].copy_from_slice(&crc.to_le_bytes());
        let (migrated, _) = Economy::decode(&old, 0).unwrap();
        assert!(migrated.committee_invited());
        assert_eq!(migrated.committee, Committee::new());
        assert_eq!(migrated.neighbors, eligible.neighbors);
        let mut signed = migrated;
        signed.sign_order_thirteen();
        let valid = signed.encode(0);
        for (byte, val) in [(cut, 7), (cut + 1, 4), (cut + 2, 8), (cut + 11, 10)] {
            let mut invalid = valid;
            invalid[byte] = val;
            let crc = crc32(&invalid[..SAVE_LEN - 4]);
            invalid[SAVE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
            assert!(Economy::decode(&invalid, 0).is_none());
        }
        signed.neighbors.helped = [0; 3];
        assert!(Economy::decode(&signed.encode(0), 0).is_none());
    }
    fn high_tea_port() -> Economy {
        let mut port = invited_port();
        port.committee.stage = Stage::HighTea;
        port.committee.motions = 6;
        port
    }

    #[test]
    fn six_pocket_deals_are_explicit_cashless_and_reconcile_through_closed_works() {
        let mut port = high_tea_port();
        for (g, n) in [(0, 3), (1, 2), (2, 2)] {
            assert_eq!(port.trade_lot(g, true, n).qty, n);
        }
        let (cash, gross, eligible, purchases, sales, orders) = (
            port.cash,
            port.earned,
            port.charter_earned(),
            port.ledger.purchases,
            port.ledger.sales,
            port.ledger.orders,
        );
        let terms = [
            (0, 3, 2, 1),
            (1, 2, 4, 1),
            (2, 1, 3, 1),
            (4, 1, 1, 2),
            (3, 1, 2, 2),
            (2, 1, 0, 3),
        ];
        for (tide, (give, give_qty, take, take_qty)) in terms.into_iter().enumerate() {
            let tide = tide as u64;
            assert_eq!(
                port.sea_offer(),
                Some(Barter {
                    tide,
                    give,
                    give_qty,
                    take,
                    take_qty
                })
            );
            let before = port.clone();
            assert!(!port.barter(tide + 6));
            assert_eq!(port, before);
            assert!(port.barter(tide));
            assert!(!port.barter(tide));
            assert_eq!(port.inventory(give), before.inventory(give) - give_qty);
            assert_eq!(port.inventory(take), before.inventory(take) + take_qty);
            assert_eq!(
                port.ledger.flows[give].delivered,
                before.ledger.flows[give].delivered + u64::from(give_qty)
            );
            assert_eq!(
                port.ledger.flows[take].landed,
                before.ledger.flows[take].landed + u64::from(take_qty)
            );
            let (mut restored, paid) = Economy::decode(&port.encode(100), 100).unwrap();
            assert_eq!(paid, 0);
            assert!(!restored.barter(tide));
            assert_eq!(restored, port);
            assert_eq!(port.advance(30_000), 0);
        }
        assert_eq!(
            (
                port.cash,
                port.earned,
                port.charter_earned(),
                port.ledger.purchases,
                port.ledger.sales,
                port.ledger.orders
            ),
            (cash, gross, eligible, purchases, sales, orders)
        );
        assert!(!port.factories[0].built && !port.factories[1].built);
    }

    #[test]
    fn pending_production_and_a_partial_raw_hold_can_block_the_entire_exchange() {
        let mut port = high_tea_port();
        port.freight.tide = 2;
        port.trade_lot(2, true, 1);
        port.open_factory(0);
        port.factories[0].output = 23;
        port.ledger.flows[3].made = 23;
        port.trade_lot(0, true, 1);
        port.trade_lot(1, true, 2);
        port.start_batch(0);
        assert_eq!(port.cargo_room(3), 0);
        let before = port.clone();
        assert!(!port.barter(2));
        assert_eq!(port, before);
        port.sell(3, false);
        assert_eq!(port.cargo_room(3), 1);
        assert!(port.barter(2));
        port.advance(8_000);
        assert_eq!(port.inventory(3), 24);
        port.advance(82_000);
        assert_eq!(port.freight.tide, 5);
        port.trade_lot(2, true, 1);
        port.trade_lot(0, true, 22);
        assert_eq!(port.cargo_room(0), 2);
        assert!(!port.barter(5));
        port.sell(0, false);
        assert!(port.barter(5));
        assert_eq!(port.freight.stock[0], 24);
        assert!(Economy::decode(&port.encode(0), 0).is_some());
    }

    #[test]
    fn pocket_lock_is_per_live_tide_across_charters_offline_and_adjournment() {
        let mut port = high_tea_port();
        port.credit(CHARTER_UNIT);
        port.trade_lot(0, true, 6);
        assert!(port.barter(0));
        port.advance(29_999);
        assert!(!port.barter(0));
        assert!(port.charter() > 0);
        assert!(port.committee.bartered);
        assert!(!port.barter(0));
        let flows = port.ledger.flows;
        let (massive, paid) = Economy::decode(&port.encode(100), u64::MAX).unwrap();
        assert_eq!(paid, 0);
        assert_eq!(massive.ledger.flows, flows);
        assert!(!massive.committee.bartered);
        port.advance(1);
        assert!(!port.committee.bartered);
        assert!(port.adjourn_committee());
        assert!(port.sea_offer().is_none());
        assert!(!port.barter(port.freight.tide));
        assert!(Economy::decode(&port.encode(100), 100).is_some());
    }

    #[test]
    fn initial_bwh8_gains_an_unused_pocket_offer_and_invalid_locks_fail() {
        let game = high_tea_port();
        let latest = game.encode(88);
        let mut old = [0; V8_INITIAL_SAVE_LEN];
        let cut = V8_INITIAL_SAVE_LEN - 4;
        old[..cut].copy_from_slice(&latest[..cut]);
        let crc = crc32(&old[..cut]);
        old[cut..].copy_from_slice(&crc.to_le_bytes());
        let (loaded, _) = Economy::decode(&old, 88).unwrap();
        assert_eq!(loaded, game);
        assert_eq!(loaded.sea_offer(), game.sea_offer());
        let mut pre = invited_port();
        assert!(pre.sign_order_thirteen());
        assert!(pre.sea_offer().is_none());
        pre.committee.bartered = true;
        assert!(Economy::decode(&pre.encode(88), 88).is_none());
    }
}
