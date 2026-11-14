//! Harbor time, neighbor visits and favors are shared with the native guest.
use super::{game, update};
use bellweather::neighbor_art::{draw_neighbor, PORTRAIT_H, PORTRAIT_W};
use bellweather::{neighbors, quay_text};

static mut BELL: neighbors::BellReply = neighbors::BellReply::new();
static mut PORTRAIT: [u32; PORTRAIT_W * PORTRAIT_H] = [0; PORTRAIT_W * PORTRAIT_H];

#[no_mangle]
pub extern "C" fn quay_minutes() -> u32 {
    let freight = &game().freight;
    u32::from(neighbors::minutes(freight.tide, freight.elapsed_ms))
}

#[no_mangle]
pub extern "C" fn quay_watch() -> usize {
    neighbors::watch(game().freight.tide)
}

#[no_mangle]
pub extern "C" fn quay_weather() -> usize {
    neighbors::weather(game().freight.tide)
}

#[no_mangle]
pub extern "C" fn quay_visitor() -> usize {
    neighbors::visitor(game().freight.tide)
}

/// Packed center of the resident on the native 480-by-192 waterfront canvas.
#[no_mangle]
pub extern "C" fn quay_contact_point() -> u32 {
    let (x, y) = bellweather::art::quay_contact(game().freight.tide);
    (x as u32) | ((y as u32) << 16)
}

#[no_mangle]
pub extern "C" fn quay_visits(person: usize) -> f64 {
    neighbors::visits(game().freight.tide, person) as f64
}

#[no_mangle]
pub extern "C" fn quay_helped(person: usize) -> u32 {
    game().neighbors.helped.get(person).copied().unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn quay_served() -> u32 {
    u32::from(game().neighbors.served)
}

#[no_mangle]
pub extern "C" fn quay_tier() -> usize {
    game().current_favor().tier
}

#[no_mangle]
pub extern "C" fn quay_cargo(good: usize) -> u32 {
    u32::from(game().current_favor().cargo.get(good).copied().unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn quay_reward() -> f64 {
    game().current_favor().reward as f64
}

#[no_mangle]
pub extern "C" fn quay_can_help() -> u32 {
    u32::from(game().can_help())
}

/// Stale offers must be declined even when both requests happen to use the
/// same cargo. All three values come from the last quote shown to the player.
#[no_mangle]
pub extern "C" fn quay_help(tide: f64, person: usize, tier: usize) -> u32 {
    update(|port| {
        let offer = port.current_favor();
        if tide != offer.tide as f64 || person != offer.neighbor || tier != offer.tier {
            return 0;
        }
        u32::from(port.help_neighbor(offer))
    })
}

#[no_mangle]
pub extern "C" fn quay_lanterns() -> u32 {
    u32::from(game().neighbors.lanterns_unlocked())
}

#[no_mangle]
pub extern "C" fn quay_sailing() -> u32 {
    u32::from(game().lanterns_sailing())
}

#[no_mangle]
pub extern "C" fn quay_portrait(person: usize) -> *const u32 {
    let frame = unsafe { &mut *(&raw mut PORTRAIT) };
    draw_neighbor(frame, person, quay_watch());
    frame.as_ptr()
}

#[no_mangle]
pub extern "C" fn quay_bell_strike(now_ms: f64) -> u32 {
    super::tea::strike(now_ms as u64);
    let visitor = quay_visitor();
    unsafe { u32::from((&mut *(&raw mut BELL)).ring(now_ms as u64, visitor)) }
}

#[no_mangle]
pub extern "C" fn quay_bell_poll(now_ms: f64) -> usize {
    unsafe {
        (&mut *(&raw mut BELL))
            .poll(now_ms as u64)
            .unwrap_or(neighbors::NAMES.len())
    }
}

#[no_mangle]
pub extern "C" fn quay_bell_clear() {
    super::tea::clear_echo();
    unsafe { (&mut *(&raw mut BELL)).clear() }
}

// Names, duties, remarks, requests and keepsakes are all native Rust strings.
// The host reads them through two scalar functions, without copying out a heap.
fn line(field: u32, person: usize, slot: usize) -> Option<&'static str> {
    if person >= neighbors::NAMES.len() {
        return None;
    }
    let port = game();
    let tide = port.freight.tide;
    match field {
        0 => Some(neighbors::NAMES[person]),
        1 => Some(quay_text::ROLES[person]),
        2 => Some(quay_text::PLACES[person]),
        3 => Some(quay_text::greeting(port, person)),
        4 => Some(quay_text::duty(tide, person)),
        5 => quay_text::FAVORS[person].get(slot).copied(),
        6 => quay_text::THANKS[person].get(slot).copied(),
        7 => Some(quay_text::KEEPSAKES[person].get(slot)?.title),
        8 => Some(quay_text::KEEPSAKES[person].get(slot)?.body),
        9 => Some(quay_text::exchange(tide).0),
        10 => Some(quay_text::exchange(tide).1),
        11 => Some(neighbors::WATCHES[quay_watch()]),
        12 => Some(neighbors::WEATHER[quay_weather()]),
        13 => Some(quay_text::ANSWERS[person]),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn quay_text_ptr(field: u32, person: usize, slot: usize) -> *const u8 {
    line(field, person, slot).map_or(core::ptr::null(), str::as_ptr)
}

#[no_mangle]
pub extern "C" fn quay_text_len(field: u32, person: usize, slot: usize) -> usize {
    line(field, person, slot).map_or(0, str::len)
}
