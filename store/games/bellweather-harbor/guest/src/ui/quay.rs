use super::*;
use crate::quay::{QuayDesk, Reply};
use bellweather::{
    market, neighbor_art,
    neighbors::{self, NAMES, WATCHES, WEATHER},
    quay_text::{self, FAVORS, KEEPSAKES, PLACES, ROLES},
};

pub(super) fn watch(c: &mut Canvas<'_>, g: &Economy, x: usize, y: usize, color: u32) {
    let tide = g.freight.tide;
    let m = neighbors::minutes(tide, g.freight.elapsed_ms);
    c.label(
        x,
        y,
        1,
        color,
        format_args!(
            "{:02}:{:02}  {} / {}",
            m / 60,
            m % 60,
            WATCHES[neighbors::watch(tide)],
            WEATHER[neighbors::weather(tide)]
        ),
    );
}

fn paragraph(c: &mut Canvas<'_>, x: usize, y: usize, cols: usize, color: u32, text: &str) -> usize {
    let mut row = y;
    super::journal::lines(text, cols, |line| {
        c.label(x, row, 1, color, format_args!("{line}"));
        row += 12;
    });
    row
}

fn portrait(c: &mut Canvas<'_>, person: usize, tide: u64) {
    let mut face = [0; neighbor_art::PORTRAIT_W * neighbor_art::PORTRAIT_H];
    neighbor_art::draw_neighbor(&mut face, person, neighbors::watch(tide));
    c.rect(21, 96, 166, 134, BRASS);
    for (y, row) in face.chunks_exact(neighbor_art::PORTRAIT_W).enumerate() {
        for (x, &color) in row.iter().enumerate() {
            c.rect(24 + x * 2, 99 + y * 2, 2, 2, color);
        }
    }
}

pub(super) fn draw(c: &mut Canvas<'_>, game: &Economy, desk: &QuayDesk) {
    let tide = game.freight.tide;
    let visitor = neighbors::visitor(tide);
    let person = desk.person;
    let helped = game.neighbors.helped[person];
    let letters = helped.min(3);
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.label(14, 13, 2, PAPER, format_args!("The Quay"));
    watch(c, game, 292, 16, FOAM);
    c.label(
        499,
        16,
        1,
        BRASS,
        format_args!("Treasury {}", Money(game.cash)),
    );
    for (i, &name) in NAMES.iter().enumerate() {
        let x = 12 + 207 * i;
        c.rect(x, 43, 202, 36, if person == i { TEAL } else { INK });
        if i == person {
            c.rect(x, 43, 202, 2, BRASS);
        }
        c.label(x + 7, 50, 1, PAPER, format_args!("{}  {name}", i + 1));
        c.label(
            x + 7,
            65,
            1,
            if i == visitor { BRASS } else { FOAM },
            format_args!(
                "{}",
                if i == visitor {
                    "AT THE COUNTER"
                } else {
                    PLACES[i]
                }
            ),
        );
    }
    c.rect(12, 87, 185, 243, INK);
    portrait(c, person, tide);
    c.label(22, 239, 2, PAPER, format_args!("{}", NAMES[person]));
    paragraph(c, 22, 264, 28, FOAM, ROLES[person]);
    c.label(
        22,
        291,
        1,
        BRASS,
        format_args!(
            "{helped} favor{} / {letters} keepsake{}",
            if helped == 1 { "" } else { "s" },
            if letters == 1 { "" } else { "s" }
        ),
    );
    c.label(
        22,
        309,
        1,
        FOAM,
        format_args!(
            "{}",
            if game.lanterns_sailing() {
                "Lantern flotilla sailing"
            } else if game.neighbors.lanterns_unlocked() {
                "Lantern flotilla at dusk"
            } else {
                "Help all 3 for dusk lanterns"
            }
        ),
    );

    c.rect(203, 87, 425, 243, PAPER);
    c.rect(203, 87, 425, 2, RULE);
    if let Some(i) = desk.letter {
        let keepsake = &KEEPSAKES[person][i];
        c.label(
            215,
            102,
            1,
            TEAL,
            format_args!(
                "FROM {}   /   KEEPSAKE {} OF {letters}",
                NAMES[person],
                i + 1
            ),
        );
        c.label(215, 126, 2, INK, format_args!("{}", keepsake.title));
        paragraph(c, 215, 162, 65, INK, keepsake.body);
        c.label(
            215,
            305,
            1,
            TEAL,
            format_args!("Saved in your harbor ledger."),
        );
    } else {
        c.label(215, 101, 1, TEAL, format_args!("{}", PLACES[person]));
        let greeting = quay_text::greeting(game, person);
        let after_greeting = paragraph(c, 215, 121, 65, INK, greeting);
        let y = (after_greeting + 8).max(164);
        c.rect(215, y - 7, 401, 1, RULE);
        let favor = desk.offer;
        if desk.person != favor.neighbor {
            c.label(215, y + 2, 1, TEAL, format_args!("AWAY FROM THE COUNTER"));
            paragraph(c, 215, y + 22, 65, INK, quay_text::duty(tide, person));
            c.label(
                215,
                297,
                1,
                TEAL,
                format_args!("V  Visit {} at the counter.", NAMES[visitor]),
            );
        } else if favor.tide != tide || matches!(desk.reply, Reply::Changed) {
            c.label(215, y + 2, 1, TEAL, format_args!("THE WATCH HAS MOVED ON"));
            c.label(
                215,
                y + 24,
                1,
                INK,
                format_args!("{} is at the counter now.", NAMES[visitor]),
            );
            c.label(
                215,
                y + 46,
                1,
                TEAL,
                format_args!("V  Hear the current request."),
            );
        } else if game.neighbors.served {
            c.label(
                215,
                y + 2,
                1,
                TEAL,
                format_args!("DELIVERY RECEIPT  /  +{} PAID", Money(favor.reward)),
            );
            let mut row = y + 23;
            for (i, &qty) in favor.cargo.iter().enumerate() {
                if qty > 0 {
                    c.label(
                        215,
                        row,
                        1,
                        INK,
                        format_args!("Delivered {qty} {}", market::NAMES[i]),
                    );
                    row += 14;
                }
            }
            if let Reply::Helped = desk.reply {
                c.label(
                    215,
                    284,
                    1,
                    TEAL,
                    format_args!("Friendship recorded for future charters."),
                );
            }
            c.label(
                215,
                305,
                1,
                TEAL,
                format_args!("L  Read the keepsakes from {}.", NAMES[person]),
            );
        } else if matches!(desk.reply, Reply::Declined) {
            c.label(215, y + 2, 1, TEAL, format_args!("ANOTHER TIME"));
            c.label(
                215,
                y + 25,
                1,
                INK,
                format_args!("You keep your cargo. The request lasts until departure."),
            );
            c.label(
                215,
                y + 48,
                1,
                TEAL,
                format_args!("V  Return to the counter."),
            );
        } else {
            c.label(
                215,
                y + 2,
                1,
                TEAL,
                format_args!(
                    "REQUEST   /   {}s TO DEPARTURE",
                    (30_000 - u32::from(game.freight.elapsed_ms)).div_ceil(1000)
                ),
            );
            let text_end = paragraph(c, 215, y + 20, 65, INK, FAVORS[person][favor.tier]);
            let mut line = text_end + 11;
            for (i, &qty) in favor.cargo.iter().enumerate() {
                if qty != 0 {
                    let own = game.inventory(i);
                    c.label(
                        215,
                        line,
                        1,
                        if own >= qty { TEAL } else { 0xa04731 },
                        format_args!("Give {qty} {}   /   {own} in your hold", market::NAMES[i]),
                    );
                    line += 14;
                }
            }
            c.label(
                215,
                line + 4,
                1,
                INK,
                format_args!("Receive {} in cash.", Money(favor.reward)),
            );
            c.label(
                215,
                306,
                1,
                TEAL,
                format_args!(
                    "{}",
                    if game.can_help() {
                        "ENTER  Deliver the cargo"
                    } else if matches!(desk.reply, Reply::Short) {
                        "Waiting for the missing cargo. Nothing spent."
                    } else {
                        "Bring the missing cargo before departure."
                    }
                ),
            );
        }
    }

    c.rect(12, 340, 616, 43, INK);
    let (a, b) = quay_text::exchange(tide);
    c.label(22, 347, 1, BRASS, format_args!("OVERHEARD"));
    paragraph(c, 94, 347, 86, PAPER, a);
    paragraph(c, 94, 366, 86, FOAM, b);
    if desk.letter.is_some() {
        c.key(12, 397, "LEFT/RIGHT", "Turn page");
        c.key(185, 397, "1-3", "Choose letter");
        c.key(331, 397, "ESC", "Back to resident");
        c.key(526, 397, "Q", "Harbor");
    } else {
        c.key(12, 397, "1-3", "Residents");
        c.key(119, 397, "V", "Visit counter");
        c.key(243, 397, "D", "Decline");
        c.key(328, 397, "L", "Keepsakes");
        c.key(430, 397, "ESC", "Harbor");
    }
}
