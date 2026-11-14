//! The order book presents the same terms, minutes and signatures as the native game.
use super::{game, update};
use bellweather::committee::SeaEcho;
use bellweather::tea_text;

static mut SEA: SeaEcho = SeaEcho::new();

pub(crate) fn strike(now_ms: u64) {
    unsafe { (&mut *(&raw mut SEA)).ring(now_ms, game().committee.stage) };
}

pub(crate) fn clear_echo() {
    unsafe { (&mut *(&raw mut SEA)).clear() };
}

#[no_mangle]
pub extern "C" fn tea_echo_poll(now_ms: f64) -> u32 {
    unsafe { u32::from((&mut *(&raw mut SEA)).poll(now_ms as u64, game().committee.stage)) }
}

#[no_mangle]
pub extern "C" fn tea_invited() -> u32 {
    u32::from(game().committee_invited())
}

#[no_mangle]
pub extern "C" fn tea_stage() -> u32 {
    game().committee.stage as u32
}

#[no_mangle]
pub extern "C" fn tea_motions() -> u32 {
    u32::from(game().committee.motions)
}

#[no_mangle]
pub extern "C" fn tea_tin() -> f64 {
    game().committee.tin as f64
}

#[no_mangle]
pub extern "C" fn tea_good() -> usize {
    game().committee.motion_good().unwrap_or(5)
}

#[no_mangle]
pub extern "C" fn tea_can_submit() -> u32 {
    u32::from(game().can_submit_motion())
}

#[no_mangle]
pub extern "C" fn tea_sign() -> u32 {
    update(|port| u32::from(port.sign_order_thirteen()))
}

#[no_mangle]
pub extern "C" fn tea_submit(expected: u32) -> u32 {
    u8::try_from(expected).map_or(0, |expected| {
        update(|port| u32::from(port.submit_motion(expected)))
    })
}

#[no_mangle]
pub extern "C" fn tea_claim() -> f64 {
    update(|port| port.claim_tea_tin() as f64)
}

#[no_mangle]
pub extern "C" fn tea_adjourn() -> u32 {
    let paid = update(|port| port.adjourn_committee());
    if paid {
        clear_echo();
    }
    u32::from(paid)
}

#[no_mangle]
pub extern "C" fn tea_barter_good(incoming: u32) -> usize {
    game().sea_offer().map_or(5, |offer| {
        if incoming == 0 {
            offer.give
        } else {
            offer.take
        }
    })
}

#[no_mangle]
pub extern "C" fn tea_barter_qty(incoming: u32) -> u32 {
    game().sea_offer().map_or(0, |offer| {
        u32::from(if incoming == 0 {
            offer.give_qty
        } else {
            offer.take_qty
        })
    })
}

#[no_mangle]
pub extern "C" fn tea_barter_room(good: usize) -> u32 {
    u32::from(game().cargo_room(good))
}

#[no_mangle]
pub extern "C" fn tea_barter_done() -> u32 {
    u32::from(game().committee.bartered)
}

#[no_mangle]
pub extern "C" fn tea_can_barter() -> u32 {
    u32::from(game().can_barter())
}

/// Preserve all 64 ticket bits on browsers with a number-only WASM boundary.
#[no_mangle]
pub extern "C" fn tea_tide_lo() -> u32 {
    game().freight.tide as u32
}

#[no_mangle]
pub extern "C" fn tea_tide_hi() -> u32 {
    (game().freight.tide >> 32) as u32
}

#[no_mangle]
pub extern "C" fn tea_barter(tide_lo: u32, tide_hi: u32) -> u32 {
    let expected = u64::from(tide_hi) << 32 | u64::from(tide_lo);
    update(|port| u32::from(port.barter(expected)))
}

#[no_mangle]
pub extern "C" fn tea_sponsor(motion: usize) -> usize {
    tea_text::MOTIONS
        .get(motion)
        .map_or(3, |entry| entry.sponsor)
}

fn copy(field: u32, index: usize) -> Option<&'static str> {
    use tea_text::*;
    match field {
        0 => Some(TREATY_TITLE),
        1 => Some(PROGRAMME_NOTE),
        2 => Some(KEEP_TEXT),
        3 => Some(ADJOURN_TEXT),
        4 => Some(GRANNIES.get(index)?.name),
        5 => Some(GRANNIES.get(index)?.role),
        6 => Some(GRANNIES.get(index)?.line),
        7 => Some(MOTIONS.get(index)?.title),
        8 => Some(MOTIONS.get(index)?.proposal),
        9 => Some(MOTIONS.get(index)?.receipt),
        10 => TIDE_MINUTES.get(index).copied(),
        11 => Some(SOUVENIR_TITLE),
        12 => Some(SOUVENIR_TEXT),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn tea_text_ptr(field: u32, index: usize) -> *const u8 {
    copy(field, index).map_or(core::ptr::null(), str::as_ptr)
}

#[no_mangle]
pub extern "C" fn tea_text_len(field: u32, index: usize) -> usize {
    copy(field, index).map_or(0, str::len)
}
