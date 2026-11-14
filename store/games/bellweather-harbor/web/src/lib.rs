#![no_std]

mod manifest;
mod market;
mod quay;
mod tea;

use bellweather::art::{
    draw_harbor, draw_harbor_watch, draw_home_fleet, draw_quay_room, draw_tea_committee, ART_H,
    ART_W,
};
use bellweather::economy::{
    Bulk, Economy, Shop, LEGACY_SAVE_LEN, SAVE_LEN, V2_SAVE_LEN, V3_SAVE_LEN, V4_SAVE_LEN,
    V5_INITIAL_SAVE_LEN, V5_SAVE_LEN, V6_SAVE_LEN, V7_SAVE_LEN, V8_INITIAL_SAVE_LEN,
};
use bellweather::freight::TIDE_MS;
use bellweather::yard::PLANS;
use bellweather::yard_art::{draw_yard, YARD_H, YARD_W};

static mut GAME: Economy = Economy::new();
static mut RESTORED_LAUNCH: usize = PLANS.len();
static mut FRAME: [u32; ART_W * ART_H] = [0; ART_W * ART_H];
static mut YARD_FRAME: [u32; YARD_W * YARD_H] = [0; YARD_W * YARD_H];
static mut SAVE: [u8; SAVE_LEN] = [0; SAVE_LEN];

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    loop {
        core::hint::spin_loop();
    }
}

// This WASM instance has one thread and imports no host functions. Each
// operation finishes, dropping its references, before JavaScript can re-enter.
fn game() -> &'static Economy {
    unsafe { &*(&raw const GAME) }
}

fn update<T>(operation: impl FnOnce(&mut Economy) -> T) -> T {
    unsafe { operation(&mut *(&raw mut GAME)) }
}

fn shop(i: usize) -> Shop {
    game().shops.get(i).copied().unwrap_or_default()
}

fn bulk(code: u32) -> Option<Bulk> {
    match code {
        0 => Some(Bulk::One),
        1 => Some(Bulk::Ten),
        2 => Some(Bulk::Max),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn cash() -> f64 {
    game().cash as f64
}

#[no_mangle]
pub extern "C" fn earned() -> f64 {
    game().earned as f64
}

#[no_mangle]
pub extern "C" fn charters() -> u32 {
    game().charters
}

#[no_mangle]
pub extern "C" fn postmarks() -> u32 {
    u32::from(game().postmarks)
}

#[no_mangle]
pub extern "C" fn mark_berth(i: usize) -> u32 {
    update(|port| u32::from(port.mark_berth(i)))
}

#[no_mangle]
pub extern "C" fn charter_offer() -> u32 {
    game().charter_offer()
}

#[no_mangle]
pub extern "C" fn charter_target() -> f64 {
    game().charter_target() as f64
}

#[no_mangle]
pub extern "C" fn shop_owned(i: usize) -> u32 {
    u32::from(shop(i).owned)
}

#[no_mangle]
pub extern "C" fn shop_managed(i: usize) -> u32 {
    u32::from(shop(i).manager)
}

#[no_mangle]
pub extern "C" fn shop_running(i: usize) -> u32 {
    u32::from(shop(i).running)
}

#[no_mangle]
pub extern "C" fn shop_elapsed(i: usize) -> u32 {
    shop(i).elapsed_ms
}

#[no_mangle]
pub extern "C" fn shop_duration(i: usize) -> u32 {
    game().duration_ms(i)
}

#[no_mangle]
pub extern "C" fn shop_payout(i: usize) -> f64 {
    game().payout(i) as f64
}

/// Preview the receipts of the first hull at the harbor's current charter bonus.
#[no_mangle]
pub extern "C" fn shop_first_payout(i: usize) -> f64 {
    game().opening_payout(i) as f64
}

#[no_mangle]
pub extern "C" fn manager_cost(i: usize) -> f64 {
    game().manager_cost(i) as f64
}

#[no_mangle]
pub extern "C" fn next_milestone(i: usize) -> u32 {
    u32::from(game().next_milestone(i).unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn automated_rate() -> f64 {
    let port = game();
    port.shops
        .iter()
        .enumerate()
        .filter(|(_, shop)| shop.manager)
        .map(|(i, _)| port.payout(i) as f64 * 1000.0 / f64::from(port.duration_ms(i)))
        .sum()
}

#[no_mangle]
pub extern "C" fn shop_cost(i: usize, mode: u32) -> f64 {
    bulk(mode).map_or(0.0, |b| game().quote(i, b).1 as f64)
}

#[no_mangle]
pub extern "C" fn shop_count(i: usize, mode: u32) -> u32 {
    bulk(mode).map_or(0, |b| u32::from(game().quote(i, b).0))
}

#[no_mangle]
pub extern "C" fn ring(_i: usize) -> u32 {
    update(|port| port.ring() as u32)
}

#[no_mangle]
pub extern "C" fn dispatch(i: usize) -> u32 {
    update(|port| u32::from(port.dispatch(i)))
}

#[no_mangle]
pub extern "C" fn buy(i: usize, mode: u32) -> u32 {
    bulk(mode).map_or(0, |b| update(|port| u32::from(port.buy(i, b))))
}

#[no_mangle]
pub extern "C" fn hire(i: usize) -> u32 {
    update(|port| u32::from(port.hire(i)))
}

#[no_mangle]
pub extern "C" fn charter() -> u32 {
    update(Economy::charter)
}

#[no_mangle]
pub extern "C" fn advance(ms: f64) -> f64 {
    update(|port| port.advance(ms as u64) as f64)
}

#[no_mangle]
pub extern "C" fn freight_open() -> u32 {
    u32::from(game().shops[0].owned > 0)
}

#[no_mangle]
pub extern "C" fn freight_goods() -> usize {
    game().freight.stock.len()
}

#[no_mangle]
pub extern "C" fn freight_stock(good: usize) -> u32 {
    u32::from(game().freight.stock.get(good).copied().unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn freight_level() -> u32 {
    u32::from(game().freight.hold_level)
}

#[no_mangle]
pub extern "C" fn freight_capacity() -> u32 {
    u32::from(game().freight.capacity())
}

#[no_mangle]
pub extern "C" fn freight_spot(good: usize) -> f64 {
    game().freight.price(good) as f64
}

#[no_mangle]
pub extern "C" fn freight_last(good: usize) -> f64 {
    game().freight.relative_price(good, -1) as f64
}

/// Next-tide estimate assuming current market conditions hold.
#[no_mangle]
pub extern "C" fn freight_next(good: usize) -> f64 {
    game().freight.projected_price(good) as f64
}

/// Eight spot quotes, oldest at index zero and the current tide at index seven.
#[no_mangle]
pub extern "C" fn freight_history(good: usize, index: usize) -> f64 {
    if index >= 8 {
        return 0.0;
    }
    game().freight.relative_price(good, index as i32 - 7) as f64
}

#[no_mangle]
pub extern "C" fn freight_tide() -> f64 {
    game().freight.tide as f64
}

#[no_mangle]
pub extern "C" fn freight_tide_ms() -> u32 {
    TIDE_MS - u32::from(game().freight.elapsed_ms)
}

#[no_mangle]
pub extern "C" fn freight_offer_good() -> usize {
    game().freight.offer().good
}

#[no_mangle]
pub extern "C" fn freight_offer_units() -> u32 {
    u32::from(game().freight.offer().qty)
}

#[no_mangle]
pub extern "C" fn freight_offer_pay() -> f64 {
    game().freight.offer().reward as f64
}

#[no_mangle]
pub extern "C" fn freight_pending() -> u32 {
    u32::from(game().freight.reserved)
}

#[no_mangle]
pub extern "C" fn freight_pending_ms() -> u32 {
    if game().freight.reserved {
        freight_tide_ms()
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn freight_pending_pay() -> f64 {
    if game().freight.reserved {
        freight_offer_pay()
    } else {
        0.0
    }
}

#[no_mangle]
pub extern "C" fn freight_upgrade_cost() -> f64 {
    game().freight.upgrade_cost().unwrap_or(0) as f64
}

#[no_mangle]
pub extern "C" fn freight_sell(good: usize, all: u32) -> f64 {
    if all > 1 {
        return 0.0;
    }
    update(|port| port.sell(good, all == 1) as f64)
}

#[no_mangle]
pub extern "C" fn freight_send() -> u32 {
    update(|port| u32::from(port.reserve_order()))
}

#[no_mangle]
pub extern "C" fn freight_upgrade() -> u32 {
    update(|port| u32::from(port.upgrade_hold()))
}

#[no_mangle]
pub extern "C" fn yard_count(i: usize) -> u32 {
    u32::from(i < PLANS.len() && (game().yard.completed & (1 << i)) != 0)
}

#[no_mangle]
pub extern "C" fn yard_next() -> usize {
    game().yard.next().unwrap_or(PLANS.len())
}

#[no_mangle]
pub extern "C" fn yard_active() -> usize {
    if game().yard.building {
        yard_next()
    } else {
        PLANS.len()
    }
}

#[no_mangle]
pub extern "C" fn yard_elapsed() -> u32 {
    game().yard.elapsed_ms
}

#[no_mangle]
pub extern "C" fn yard_duration(i: usize) -> u32 {
    PLANS.get(i).map_or(0, |plan| plan.duration_ms)
}

#[no_mangle]
pub extern "C" fn yard_cost(i: usize) -> f64 {
    PLANS.get(i).map_or(0.0, |plan| plan.cost as f64)
}

#[no_mangle]
pub extern "C" fn yard_cargo(i: usize, good: usize) -> u32 {
    PLANS
        .get(i)
        .and_then(|plan| plan.cargo.get(good))
        .copied()
        .map_or(0, u32::from)
}

#[no_mangle]
pub extern "C" fn yard_award(i: usize) -> u32 {
    PLANS.get(i).map_or(0, |plan| plan.bonus_percent)
}

#[no_mangle]
pub extern "C" fn yard_bonus() -> u32 {
    game().yard.bonus_percent()
}

#[no_mangle]
pub extern "C" fn yard_can_begin(i: usize) -> u32 {
    u32::from(game().can_build(i))
}

#[no_mangle]
pub extern "C" fn yard_begin(i: usize) -> u32 {
    update(|port| u32::from(port.build(i)))
}

/// Return an RGB frame of the selected hull at its current commissioning stage.
#[no_mangle]
pub extern "C" fn render_yard(tick: u32, selected: usize, launch_stage: u32) -> *const u32 {
    let selected = selected.min(PLANS.len() - 1);
    let yard = &game().yard;
    let stage = if yard_count(selected) != 0 && (1001..=2000).contains(&launch_stage) {
        launch_stage as u16
    } else if yard.building && yard.next() == Some(selected) {
        // Zero is reserved for the empty slipway. One marks a just-paid keel.
        1 + ((u64::from(yard.elapsed_ms) * 999) / u64::from(PLANS[selected].duration_ms)) as u16
    } else {
        0
    };
    let frame = unsafe { &mut *(&raw mut YARD_FRAME) };
    draw_yard(
        frame,
        tick,
        selected,
        yard.completed.count_ones() as u8,
        stage,
    );
    frame.as_ptr()
}

/// The renderer and JavaScript share a buffer of little-endian 0x00RRGGBB words.
#[no_mangle]
pub extern "C" fn render(tick: u32, selected: usize) -> *const u32 {
    render_waterfront(tick, selected, u32::from(game().freight.elapsed_ms))
}

#[no_mangle]
pub extern "C" fn render_still(selected: usize) -> *const u32 {
    render_waterfront(0, selected, TIDE_MS / 2)
}

fn render_waterfront(tick: u32, selected: usize, elapsed: u32) -> *const u32 {
    let shops = &game().shops;
    let units = shops.map(|shop| u32::from(shop.owned));
    let mut managers = 0;
    let mut running = 0;
    for (i, shop) in shops.iter().enumerate() {
        managers |= u8::from(shop.manager) << i;
        running |= u8::from(shop.running) << i;
    }
    let frame = unsafe { &mut *(&raw mut FRAME) };
    draw_harbor(frame, tick, units, managers, running, selected);
    draw_home_fleet(frame, tick, game().yard.completed);
    draw_quay_room(frame, tick, game().manifest.ending);
    draw_harbor_watch(
        frame,
        game().freight.tide,
        elapsed,
        game().neighbors.lanterns_unlocked(),
    );
    let committee = game().committee.stage as u8;
    let scene = if committee == 0 {
        u8::from(game().committee_invited())
    } else {
        committee + 1
    };
    draw_tea_committee(frame, tick, scene);
    frame.as_ptr()
}

#[no_mangle]
pub extern "C" fn save_ptr() -> *mut u8 {
    (&raw mut SAVE).cast()
}

#[no_mangle]
pub extern "C" fn save_size() -> usize {
    SAVE_LEN
}

#[no_mangle]
pub extern "C" fn valid_save_size(len: usize) -> u32 {
    u32::from(matches!(
        len,
        LEGACY_SAVE_LEN
            | V2_SAVE_LEN
            | V3_SAVE_LEN
            | V4_SAVE_LEN
            | V5_INITIAL_SAVE_LEN
            | V5_SAVE_LEN
            | V6_SAVE_LEN
            | V7_SAVE_LEN
            | V8_INITIAL_SAVE_LEN
            | SAVE_LEN
    ))
}

#[no_mangle]
pub extern "C" fn snapshot(now_s: f64) {
    unsafe { SAVE = game().encode(now_s as u64) };
}

/// A rejected save leaves the live port alone. A valid save pays accrued income.
#[no_mangle]
pub extern "C" fn restore(now_s: f64) -> f64 {
    restore_sized(now_s, SAVE_LEN)
}

/// Pass the exact byte count to allow older save formats. Bounds are checked
/// before constructing the slice or replacing any live state.
#[no_mangle]
pub extern "C" fn restore_sized(now_s: f64, len: usize) -> f64 {
    unsafe { RESTORED_LAUNCH = PLANS.len() };
    if valid_save_size(len) == 0 {
        return -1.0;
    }
    let buffer = unsafe { &*(&raw const SAVE) };
    let bytes = &buffer[..len];
    if let Some((port, gain)) = Economy::decode(bytes, now_s as u64) {
        // A restore can finish at most the single paid hull already on the ways.
        // A second, zero-time decode compares the validated yard before settlement
        // without duplicating any binary-format offsets here.
        if len >= V4_SAVE_LEN && port.yard.completed != 0 && !port.yard.building {
            if let Some((before, _)) = Economy::decode(bytes, 0) {
                if before.yard.building && before.yard.completed != port.yard.completed {
                    unsafe { RESTORED_LAUNCH = before.yard.next().unwrap_or(PLANS.len()) };
                }
            }
        }
        quay::quay_bell_clear();
        unsafe { GAME = port };
        gain as f64
    } else {
        -1.0
    }
}

/// Hull launched while restoring the latest save, or 3 if no vessel launched.
#[no_mangle]
pub extern "C" fn restored_launch() -> usize {
    unsafe { RESTORED_LAUNCH }
}
