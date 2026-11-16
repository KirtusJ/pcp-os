use super::*;
use crate::journal::Journal;
use bellweather::{
    freight::TIDE_MS,
    market::NAMES,
    wire::{intercept, wire_good, CALLS},
};

const GLOW: u32 = 0xd98943;
const RAIL: u32 = 0x968467;

fn group(
    c: &mut Canvas<'_>,
    x: usize,
    y: usize,
    pattern: &str,
    big: bool,
    color: u32,
    active: Option<usize>,
) {
    let s = if big { 3 } else { 1 };
    let space = 12 * s;
    for (i, symbol) in pattern.bytes().enumerate() {
        let w = if symbol == b'-' { 8 * s } else { 3 * s };
        let xx = x + i * space + (8 * s - w) / 2;
        c.rect(xx, y, w, 3 * s, color);
        if active == Some(i) {
            c.rect(xx, y + 4 * s, w, 2, BUOY);
        }
    }
}
pub(super) fn draw(c: &mut Canvas<'_>, g: &Economy, j: &mut Journal) {
    j.tune(g.freight.tide);
    let good = wire_good(g.freight.tide);
    let (first, second) = CALLS[good];
    let decoded = j.wire_guess.and_then(|guess| intercept(g, guess));
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.rect(13, 24, 17, 2, BRASS);
    c.rect(16, 21, 6, 3, BRASS);
    c.rect(18, 17, 3, 4, BRASS);
    c.rect(20, 16, 7, 2, BUOY);
    c.label(39, 12, 2, PAPER, format_args!("Circuit Eight"));
    c.label(
        359,
        16,
        1,
        FOAM,
        format_args!("BELLWEATHER  /  QUAYSIDE RECEIVER"),
    );

    c.rect(12, 46, 616, 112, INK);
    c.rect(12, 46, 616, 2, BRASS);
    c.label(
        23,
        55,
        1,
        BRASS,
        format_args!(
            "INCOMING CALL   /   TIDE {}",
            g.freight.tide.saturating_add(1)
        ),
    );
    let seconds = (u64::from(TIDE_MS) - u64::from(g.freight.elapsed_ms)).div_ceil(1000);
    c.label(
        444,
        55,
        1,
        FOAM,
        format_args!("Next transmission in {seconds}s"),
    );
    c.rect(22, 73, 596, 75, RULE);
    c.rect(23, 74, 594, 73, PAPER);
    for n in 0..40 {
        c.rect(30 + n * 15, 76, 3, 2, RAIL);
        c.rect(30 + n * 15, 143, 3, 2, RAIL);
    }
    c.label(43, 98, 1, MUTED, format_args!("READ"));
    c.label(43, 115, 1, MUTED, format_args!("LEFT TO RIGHT"));
    let frame_ms = u32::from(g.freight.elapsed_ms);
    let age = j
        .wire_replay_ms
        .map(|start| frame_ms.saturating_sub(u32::from(start)));
    let elapsed = age
        .filter(|&age| age < 17 * crate::sound::WIRE_UNIT_MS)
        .unwrap_or(frame_ms % (23 * crate::sound::WIRE_UNIT_MS));
    let active = crate::sound::wire_mark(good, elapsed);
    group(
        c,
        157,
        105,
        first,
        true,
        INK,
        active.filter(|&mark| mark < 3),
    );
    c.rect(285, 93, 1, 38, RULE);
    group(
        c,
        310,
        105,
        second,
        true,
        INK,
        active.and_then(|mark| mark.checked_sub(3)),
    );
    c.label(156, 86, 1, TEAL, format_args!("FIRST GROUP"));
    c.label(309, 86, 1, TEAL, format_args!("SECOND GROUP"));
    let energized = active.is_some();
    c.rect(492, 99, 13, 13, RAIL);
    c.rect(494, 101, 9, 9, if energized { GLOW } else { INK });
    c.label(513, 102, 1, MUTED, format_args!("RELAY"));

    c.rect(12, 169, 291, 177, PAPER);
    c.rect(12, 169, 291, 3, BRASS);
    c.label(23, 180, 1, INK, format_args!("PORT CALL DIRECTORY"));
    c.label(23, 197, 1, MUTED, format_args!("CARGO"));
    c.label(159, 197, 1, MUTED, format_args!("FIRST"));
    c.label(231, 197, 1, MUTED, format_args!("SECOND"));
    for (i, &(a, b)) in CALLS.iter().enumerate() {
        let y = 211 + i * 25;
        let selected = j.wire_guess == Some(i);
        let fg = if selected { PAPER } else { INK };
        c.rect(
            12,
            y,
            291,
            24,
            if selected {
                TEAL
            } else if i % 2 == 0 {
                PALE
            } else {
                PAPER
            },
        );
        c.label(23, y + 8, 1, fg, format_args!("{}  {}", i + 1, NAMES[i]));
        group(c, 158, y + 10, a, false, fg, None);
        group(c, 230, y + 10, b, false, fg, None);
    }
    c.rect(313, 169, 315, 177, INK);
    c.rect(
        313,
        169,
        315,
        3,
        if decoded.is_some() { FOAM } else { BUOY },
    );
    c.label(325, 180, 1, BRASS, format_args!("INTERCEPTED BID SHEET"));
    if let Some(bids) = decoded {
        c.label(325, 200, 2, PAPER, format_args!("{}", NAMES[good]));
        for (n, bid) in bids.into_iter().enumerate() {
            let y = 230 + n * 23;
            if n == 0 {
                c.label(326, y, 1, BRASS, format_args!("LIVE  /  NOW"));
            } else {
                c.label(326, y, 1, FOAM, format_args!("Tide +{n}  estimate"));
            }
            if bid == 0 {
                c.label(529, y, 1, PAPER, format_args!("No bid"));
            } else {
                c.label(529, y, 1, PAPER, format_args!("{}", Money(bid)));
            }
            c.rect(325, y + 13, 288, 1, TEAL);
        }
    } else {
        let wrong = j.wire_guess.is_some();
        c.label(
            325,
            206,
            2,
            PAPER,
            format_args!(
                "{}",
                if wrong {
                    "Try another call"
                } else {
                    "Identify the cargo"
                }
            ),
        );
        c.label(
            325,
            239,
            1,
            FOAM,
            format_args!("Match both groups in the directory."),
        );
        c.label(
            325,
            258,
            1,
            FOAM,
            format_args!("A short mark is a dot. A long mark is a dash."),
        );
        if wrong {
            c.label(
                325,
                291,
                1,
                BRASS,
                format_args!("That pair identifies another cargo."),
            );
        }
        c.label(
            325,
            317,
            1,
            PAPER,
            format_args!("Press its number to decode the sheet."),
        );
    }
    c.rect(12, 355, 616, 29, TEAL);
    c.label(
        22,
        361,
        1,
        PAPER,
        format_args!(
            "All bids are for one crate. Estimates include town demand and rival production."
        ),
    );
    c.label(
        22,
        373,
        1,
        FOAM,
        format_args!("The forecast assumes no player or broker trades. Zero means no bid."),
    );
    c.key(12, 398, "1-5", "Identify cargo");
    c.key(176, 398, "SPACE", "Replay pulses");
    c.key(354, 398, "ESC", "Filed conclusion");
    c.key(558, 398, "J", "Harbor");
}
