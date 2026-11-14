//! Market and factory views of the shared Rust simulation. JavaScript carries
//! the quote shown to the player into the same synchronous call that fills it.

use super::{game, update};
use bellweather::factory::PLANS;
use bellweather::market::{Quote, NAMES};

#[no_mangle]
pub extern "C" fn market_goods() -> usize {
    NAMES.len()
}

#[no_mangle]
pub extern "C" fn market_held(good: usize) -> u32 {
    u32::from(game().inventory(good))
}

#[no_mangle]
pub extern "C" fn market_stock(good: usize) -> u32 {
    u32::from(game().freight.market.stock.get(good).copied().unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn market_bid(good: usize) -> f64 {
    game().freight.price(good) as f64
}

#[no_mangle]
pub extern "C" fn market_ask(good: usize) -> f64 {
    game().freight.ask(good) as f64
}

#[no_mangle]
pub extern "C" fn market_forecast(good: usize) -> f64 {
    game().freight.projected_price(good) as f64
}

/// Whether each town workshop would run at the next tide, if the player made
/// no intervening trades. Index 0 is the cannery and 1 is the chandler.
#[no_mangle]
pub extern "C" fn market_rival(i: usize) -> u32 {
    let freight = &game().freight;
    let projected = freight.market.project_rival(freight.tide);
    u32::from(projected.get(i).copied().unwrap_or(false))
}

// The UI may ask for one, as many as possible, or a specific count. Each route
// goes through the same Rust quote and fill so the displayed trade can be
// matched to the current inventory curve and the money/cargo actually available.
fn lots(all: u32) -> u32 {
    match all {
        0 => 1,
        1 => u32::from(u16::MAX),
        _ => 0,
    }
}

fn trade_quote(good: usize, buy: u32, requested: u32) -> Quote {
    if buy > 1 || good >= NAMES.len() || requested > u32::from(u16::MAX) {
        return Quote::default();
    }
    game().quote_lot(good, buy == 1, requested as u16)
}

#[no_mangle]
pub extern "C" fn market_qty(good: usize, buy: u32, all: u32) -> u32 {
    market_lot_qty(good, buy, lots(all))
}

#[no_mangle]
pub extern "C" fn market_total(good: usize, buy: u32, all: u32) -> f64 {
    market_lot_total(good, buy, lots(all))
}

#[no_mangle]
pub extern "C" fn market_trade(
    good: usize,
    buy: u32,
    all: u32,
    shown_qty: u32,
    shown_total: f64,
) -> f64 {
    market_trade_lot(good, buy, lots(all), shown_qty, shown_total)
}

#[no_mangle]
pub extern "C" fn market_lot_qty(good: usize, buy: u32, requested: u32) -> u32 {
    u32::from(trade_quote(good, buy, requested).qty)
}

#[no_mangle]
pub extern "C" fn market_lot_total(good: usize, buy: u32, requested: u32) -> f64 {
    trade_quote(good, buy, requested).coins as f64
}

#[no_mangle]
pub extern "C" fn market_trade_lot(
    good: usize,
    buy: u32,
    requested: u32,
    shown_qty: u32,
    shown_total: f64,
) -> f64 {
    let quote = trade_quote(good, buy, requested);
    if quote.qty == 0 || u32::from(quote.qty) != shown_qty || quote.coins as f64 != shown_total {
        return 0.0;
    }
    update(|port| port.trade_lot(good, buy == 1, requested as u16).coins as f64)
}

#[no_mangle]
pub extern "C" fn factory_open(i: usize) -> u32 {
    u32::from(game().factories.get(i).is_some_and(|works| works.built))
}

#[no_mangle]
pub extern "C" fn factory_good(i: usize) -> usize {
    PLANS.get(i).map_or(NAMES.len(), |plan| plan.good)
}

#[no_mangle]
pub extern "C" fn factory_unlock_cost(i: usize) -> f64 {
    PLANS.get(i).map_or(0.0, |plan| plan.build_cost as f64)
}

#[no_mangle]
pub extern "C" fn factory_unlock(i: usize) -> u32 {
    update(|port| u32::from(port.open_factory(i)))
}

#[no_mangle]
pub extern "C" fn factory_input(i: usize, good: usize) -> u32 {
    PLANS
        .get(i)
        .and_then(|plan| plan.inputs.get(good))
        .copied()
        .map_or(0, u32::from)
}

#[no_mangle]
pub extern "C" fn factory_fee(i: usize) -> f64 {
    PLANS.get(i).map_or(0.0, |plan| plan.fee as f64)
}

#[no_mangle]
pub extern "C" fn factory_duration(i: usize) -> u32 {
    PLANS.get(i).map_or(0, |plan| plan.duration_ms)
}

#[no_mangle]
pub extern "C" fn factory_elapsed(i: usize) -> u32 {
    game().factories.get(i).map_or(0, |works| works.elapsed_ms)
}

#[no_mangle]
pub extern "C" fn factory_running(i: usize) -> u32 {
    u32::from(game().factories.get(i).is_some_and(|works| works.running))
}

#[no_mangle]
pub extern "C" fn factory_repeat(i: usize) -> u32 {
    u32::from(game().factories.get(i).is_some_and(|works| works.repeat))
}

#[no_mangle]
pub extern "C" fn factory_set_repeat(i: usize, on: u32) -> u32 {
    if on > 1 {
        return 0;
    }
    update(|port| u32::from(port.set_repeat(i, on == 1)))
}

#[no_mangle]
pub extern "C" fn factory_can_start(i: usize) -> u32 {
    u32::from(game().can_start_batch(i))
}

#[no_mangle]
pub extern "C" fn factory_start(i: usize) -> u32 {
    update(|port| u32::from(port.start_batch(i)))
}

#[no_mangle]
pub extern "C" fn factory_stock(i: usize) -> u32 {
    game()
        .factories
        .get(i)
        .map_or(0, |works| u32::from(works.output))
}

/// Each completed batch turns its recipe into exactly one finished crate.
#[no_mangle]
pub extern "C" fn factory_output(i: usize) -> u32 {
    u32::from(i < PLANS.len())
}

#[no_mangle]
pub extern "C" fn factory_capacity(i: usize) -> u32 {
    if i < PLANS.len() {
        u32::from(game().freight.capacity())
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn factory_bid(i: usize) -> f64 {
    market_bid(factory_good(i))
}

#[no_mangle]
pub extern "C" fn factory_margin(i: usize) -> f64 {
    game().conversion_margin(i) as f64
}

#[no_mangle]
pub extern "C" fn factory_sell_qty(i: usize, all: u32) -> u32 {
    market_qty(factory_good(i), 0, all)
}

#[no_mangle]
pub extern "C" fn factory_sell_total(i: usize, all: u32) -> f64 {
    market_total(factory_good(i), 0, all)
}

#[no_mangle]
pub extern "C" fn factory_sell(i: usize, all: u32, shown_qty: u32, shown_total: f64) -> f64 {
    market_trade(factory_good(i), 0, all, shown_qty, shown_total)
}

#[no_mangle]
pub extern "C" fn ledger_value(index: usize) -> f64 {
    let ledger = &game().ledger;
    (match index {
        0 => ledger.sales,
        1 => ledger.purchases,
        2 => ledger.fees,
        3 => ledger.construction,
        4 => ledger.orders,
        _ => 0,
    }) as f64
}

#[no_mangle]
pub extern "C" fn charter_earned() -> f64 {
    game().charter_earned() as f64
}

#[no_mangle]
pub extern "C" fn ledger_flow(good: usize, field: usize) -> f64 {
    let Some(flow) = game().ledger.flows.get(good) else {
        return 0.0;
    };
    (match field {
        0 => u64::from(flow.opening),
        1 => flow.landed,
        2 => flow.bought,
        3 => flow.made,
        4 => flow.sold,
        5 => flow.used,
        6 => flow.delivered,
        _ => 0,
    }) as f64
}

#[no_mangle]
pub extern "C" fn market_capacity(good: usize) -> u32 {
    if good < NAMES.len() {
        u32::from(bellweather::market::MARKET_CAP)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn market_history_len() -> u32 {
    u32::from(game().freight.market.history_len)
}

/// The history excludes the current live quote. Index zero is the oldest actual
/// closing quote retained by this harbor. A legacy save starts with no closes.
#[no_mangle]
pub extern "C" fn market_close(good: usize, index: u32) -> f64 {
    let length = market_history_len();
    if index >= length {
        return 0.0;
    }
    game()
        .freight
        .relative_price(good, index as i32 - length as i32) as f64
}

fn exact_lot(good: usize, qty: u32, buy: bool) -> f64 {
    let Ok(qty) = u16::try_from(qty) else {
        return 0.0;
    };
    if good >= NAMES.len() || qty == 0 {
        return 0.0;
    }
    let market = &game().freight;
    let quote = if buy {
        market.ask_lot(good, qty)
    } else {
        market.bid_lot(good, qty)
    };
    if quote.qty == qty {
        quote.coins as f64
    } else {
        0.0
    }
}

#[no_mangle]
pub extern "C" fn market_bid_lot(good: usize, qty: u32) -> f64 {
    exact_lot(good, qty, false)
}

#[no_mangle]
pub extern "C" fn market_ask_lot(good: usize, qty: u32) -> f64 {
    exact_lot(good, qty, true)
}

/// Opportunity cost is the amount the market would actually pay now, even if
/// some of the inputs currently have no buyer. It excludes the processing fee.
#[no_mangle]
pub extern "C" fn factory_input_value(i: usize) -> f64 {
    let Some(plan) = PLANS.get(i) else { return 0.0 };
    plan.inputs
        .iter()
        .enumerate()
        .map(|(good, &qty)| game().freight.bid_lot(good, qty).coins)
        .sum::<u64>() as f64
}

#[no_mangle]
pub extern "C" fn factory_product_value(i: usize) -> f64 {
    market_bid_lot(factory_good(i), factory_output(i))
}

#[no_mangle]
pub extern "C" fn ticket_status() -> u32 {
    u32::from(game().ticket.status as u8)
}

#[no_mangle]
pub extern "C" fn ticket_good() -> usize {
    if ticket_status() == 0 {
        NAMES.len()
    } else {
        usize::from(game().ticket.good)
    }
}

#[no_mangle]
pub extern "C" fn ticket_requested() -> u32 {
    u32::from(game().ticket.requested)
}

#[no_mangle]
pub extern "C" fn ticket_remaining() -> u32 {
    u32::from(game().ticket.remaining)
}

#[no_mangle]
pub extern "C" fn ticket_filled() -> u32 {
    u32::from(game().ticket.filled)
}

#[no_mangle]
pub extern "C" fn ticket_floor() -> u32 {
    u32::from(game().ticket.floor)
}

#[no_mangle]
pub extern "C" fn ticket_proceeds() -> f64 {
    game().ticket.proceeds as f64
}

#[no_mangle]
pub extern "C" fn ticket_tides_left() -> u32 {
    u32::from(game().ticket.tides_left)
}

#[no_mangle]
pub extern "C" fn ticket_place(good: usize, requested: u32, floor: u32) -> u32 {
    let (Ok(requested), Ok(floor)) = (u16::try_from(requested), u16::try_from(floor)) else {
        return 0;
    };
    update(|port| u32::from(port.place_sale(good, floor, requested)))
}

#[no_mangle]
pub extern "C" fn ticket_cancel() -> u32 {
    update(|port| u32::from(port.cancel_sale()))
}

fn ticket_quote(good: usize, requested: u32, floor: u32) -> Quote {
    let (Ok(requested), Ok(floor)) = (u16::try_from(requested), u16::try_from(floor)) else {
        return Quote::default();
    };
    game().ticket_quote(good, requested, floor)
}

#[no_mangle]
pub extern "C" fn ticket_preview_qty(good: usize, requested: u32, floor: u32) -> u32 {
    u32::from(ticket_quote(good, requested, floor).qty)
}

#[no_mangle]
pub extern "C" fn ticket_preview_total(good: usize, requested: u32, floor: u32) -> f64 {
    ticket_quote(good, requested, floor).coins as f64
}
