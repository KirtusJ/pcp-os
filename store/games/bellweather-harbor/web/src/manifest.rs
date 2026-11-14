//! The case file and the browser display use the same Rust text as the native guest.
use super::{game, update};
use bellweather::manifest_text::{CASES, ENDINGS, PAIRS};

#[no_mangle]
pub extern "C" fn manifest_unlocked() -> u32 {
    u32::from(game().manifest.unlocked)
}

#[no_mangle]
pub extern "C" fn manifest_solved() -> u32 {
    u32::from(game().manifest.solved)
}

#[no_mangle]
pub extern "C" fn manifest_read(chapter: usize) -> u32 {
    u32::from(game().manifest.read.get(chapter).copied().unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn manifest_ending() -> u32 {
    u32::from(game().manifest.ending as u8)
}

#[no_mangle]
pub extern "C" fn manifest_inspect(chapter: usize, lead: usize) -> u32 {
    update(|port| u32::from(port.inspect_lead(chapter, lead)))
}

#[no_mangle]
pub extern "C" fn manifest_prove(chapter: usize, first: usize, second: usize) -> u32 {
    update(|port| u32::from(port.prove_pair(chapter, first, second)))
}

#[no_mangle]
pub extern "C" fn manifest_choose(choice: u32) -> u32 {
    u8::try_from(choice).map_or(0, |choice| {
        update(|port| u32::from(port.choose_ending(choice)))
    })
}

#[no_mangle]
pub extern "C" fn manifest_evidence(chapter: usize) -> u32 {
    if chapter >= usize::from(game().manifest.solved) {
        return 0;
    }
    PAIRS
        .get(chapter)
        .map_or(0, |pair| (1 << pair[0]) | (1 << pair[1]))
}

#[no_mangle]
pub extern "C" fn bell_tip() -> u32 {
    game().bell_tip() as u32
}

fn story_text(field: u32, chapter: usize, i: usize) -> Option<&'static str> {
    if field >= 8 {
        let ending = ENDINGS.get(i)?;
        return match field {
            8 => Some(ending.title),
            9 => Some(ending.benefit),
            10 => Some(ending.epilogue),
            _ => None,
        };
    }
    let case = CASES.get(chapter)?;
    match field {
        0 => Some(case.title),
        1 => Some(case.scene),
        2 => Some(case.question),
        3 => Some(case.hint),
        4 => Some(case.reveal),
        5 => Some(case.leads.get(i)?.title),
        6 => Some(case.leads.get(i)?.source),
        7 => Some(case.leads.get(i)?.body),
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn manifest_text_ptr(field: u32, chapter: usize, i: usize) -> *const u8 {
    story_text(field, chapter, i).map_or(core::ptr::null(), str::as_ptr)
}

#[no_mangle]
pub extern "C" fn manifest_text_len(field: u32, chapter: usize, i: usize) -> usize {
    story_text(field, chapter, i).map_or(0, str::len)
}

/// Return the bit pattern of a three-pulse call, high bit first. A set bit
/// represents a dash. Eight is a sentinel because only three bits are valid.
#[no_mangle]
pub extern "C" fn wire_call(good: usize, group: usize) -> u32 {
    let Some(&(first, second)) = bellweather::wire::CALLS.get(good) else {
        return 8;
    };
    let code = match group {
        0 => first,
        1 => second,
        _ => return 8,
    };
    code.bytes().fold(0, |pattern, pulse| {
        (pattern << 1) | u32::from(pulse == b'-')
    })
}

#[no_mangle]
pub extern "C" fn wire_group(group: usize) -> u32 {
    if manifest_ending() == 0 {
        return 8;
    }
    wire_call(bellweather::wire::wire_good(game().freight.tide), group)
}

#[no_mangle]
pub extern "C" fn wire_decode(good: usize) -> u32 {
    u32::from(bellweather::wire::intercept(game(), good).is_some())
}

#[no_mangle]
pub extern "C" fn wire_bid(good: usize, offset: usize) -> f64 {
    bellweather::wire::intercept(game(), good)
        .and_then(|quotes| quotes.get(offset).copied())
        .map_or(-1.0, |coins| coins as f64)
}
