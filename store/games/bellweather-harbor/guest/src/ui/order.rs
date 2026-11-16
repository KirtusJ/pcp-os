use super::*;
use crate::order::{Message, Ticket};
use bellweather::{economy::TicketStatus, freight::TIDE_MS, market::NAMES};
pub(super) fn draw(c: &mut Canvas<'_>, g: &Economy, d: &Ticket) {
    let t = &g.ticket;
    let pending = t.status == TicketStatus::Pending;
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.rect(0, 38, W, 1, TEAL);
    c.rect(13, 8, 15, 20, BRASS);
    for n in 0..3 {
        c.rect(16, 12 + n * 4, 9 - (n % 2) * 3, 1, INK);
    }
    c.label(38, 12, 2, PAPER, format_args!("Standing Sell Order"));
    c.label(323, 17, 1, FOAM, format_args!("FREIGHT EXCHANGE"));
    c.label(487, 6, 1, BRASS, format_args!("TREASURY"));
    c.label(487, 19, 2, PAPER, format_args!("{}", Money(g.cash)));
    for (i, name) in NAMES.iter().enumerate() {
        let x = 12 + 124 * i;
        c.rect(x, 48, 120, 31, if i == d.good { TEAL } else { INK });
        if i == d.good {
            c.rect(x, 48, 120, 2, BRASS);
        }
        c.label(
            x + 9,
            60,
            1,
            if i == d.good { PAPER } else { FOAM },
            format_args!("{} {}", i + 1, name),
        );
    }
    c.rect(12, 89, 307, 190, PAPER);
    c.rect(12, 89, 307, 3, BRASS);
    c.label(
        24,
        102,
        1,
        TEAL,
        format_args!(
            "{}",
            if pending {
                "PLACED INSTRUCTION"
            } else {
                "NEW INSTRUCTION"
            }
        ),
    );
    c.label(24, 122, 2, INK, format_args!("{}", NAMES[d.good]));
    c.label(24, 156, 1, MUTED, format_args!("Quantity"));
    c.label(150, 152, 2, INK, format_args!("{} crates", d.qty));
    c.label(24, 184, 1, MUTED, format_args!("Minimum / crate"));
    c.label(
        150,
        180,
        2,
        INK,
        format_args!("{}", Money(u64::from(d.floor))),
    );
    c.rect(24, 207, 281, 1, RULE);
    c.label(
        24,
        219,
        1,
        INK,
        format_args!(
            "On hand   {}   /   Spot bid   {}",
            g.inventory(d.good),
            Money(g.freight.price(d.good))
        ),
    );
    if pending {
        c.label(
            24,
            241,
            1,
            TEAL,
            format_args!("Floor applies to every crate sold."),
        );
        c.label(
            24,
            260,
            1,
            MUTED,
            format_args!("C cancels the unfilled quantity."),
        );
    } else {
        c.label(
            24,
            241,
            1,
            TEAL,
            format_args!("X   Adjust by {} per keypress", d.step),
        );
        c.label(
            24,
            260,
            1,
            MUTED,
            format_args!("Left / Right  qty    Up / Down  floor"),
        );
    }
    c.rect(328, 89, 300, 190, INK);
    c.label(341, 102, 1, BRASS, format_args!("BROKER'S RECORD"));
    let status = match t.status {
        TicketStatus::Empty => "No instruction",
        TicketStatus::Pending => "Open",
        TicketStatus::Filled => "Filled",
        TicketStatus::Expired => "Expired",
        TicketStatus::Cancelled => "Cancelled",
    };
    c.label(341, 122, 2, PAPER, format_args!("{status}"));
    if t.status != TicketStatus::Empty {
        c.label(
            341,
            153,
            1,
            FOAM,
            format_args!(
                "{} crates of {} at {} or better",
                t.requested,
                NAMES[t.good as usize],
                Money(u64::from(t.floor))
            ),
        );
        c.label(
            341,
            175,
            1,
            PAPER,
            format_args!("Filled  {}   /   Remaining  {}", t.filled, t.remaining),
        );
        c.label(
            341,
            198,
            1,
            PAPER,
            format_args!("Receipts   {}", Money(t.proceeds)),
        );
        c.rect(341, 218, 271, 1, TEAL);
        if pending {
            let seconds = (u64::from(TIDE_MS) - u64::from(g.freight.elapsed_ms)).div_ceil(1000);
            c.label(
                341,
                232,
                1,
                BRASS,
                format_args!("{} clearings left", t.tides_left),
            );
            c.label(
                341,
                253,
                1,
                FOAM,
                format_args!("Next clearing in {seconds}s"),
            );
        } else {
            c.label(
                341,
                237,
                1,
                FOAM,
                format_args!("Enter submits a new instruction."),
            );
        }
    } else {
        c.label(
            341,
            158,
            1,
            FOAM,
            format_args!("A placed ticket records each fill"),
        );
        c.label(341, 176, 1, FOAM, format_args!("and the money received."));
    }
    c.rect(12, 289, 616, 65, TEAL);
    c.label(24, 300, 1, BRASS, format_args!("GOOD FOR TWELVE TIDES"));
    c.label(
        24,
        318,
        1,
        PAPER,
        format_args!("At each clearing, sell available crates while each crate meets the minimum."),
    );
    c.label(
        24,
        333,
        1,
        FOAM,
        format_args!(
            "Unfilled quantity carries forward. Your stock remains available for other use."
        ),
    );
    let quoted = g.ticket_quote(d.good, if pending { t.remaining } else { d.qty }, d.floor);
    c.label(
        24,
        346,
        1,
        FOAM,
        format_args!(
            "Current eligible lot  {} crates / {}   (orders execute at clearing)",
            quoted.qty,
            Money(quoted.coins)
        ),
    );
    c.rect(12, 359, 616, 24, INK);
    c.rect(22, 367, 4, 7, BRASS);
    let line = match d.message {
        Message::Placed if !pending => {
            "Instruction settled. The broker's record shows its final status."
        }
        Message::Placed => "Instruction placed. It will first trade at the next clearing.",
        Message::Cancelled => "Instruction cancelled. Completed sales remain booked.",
        Message::NoPending => "There is no open instruction to cancel.",
        Message::Busy if !pending => "No pending instruction. Edit terms or place another.",
        Message::Busy => "An order is already open. Cancel it before placing another.",
        Message::Draft if pending => {
            "Open instruction. You can return to the harbor while it works."
        }
        Message::Draft => "Set a quantity and a minimum price, then press Enter to submit.",
    };
    c.label(32, 367, 1, PAPER, format_args!("{line}"));
    if !pending {
        c.key(12, 398, "1-5", "Cargo");
        c.key(100, 398, "X", "Step");
        c.key(184, 398, "ENTER", "Place order");
    } else {
        c.key(12, 398, "C", "Cancel remaining");
    }
    c.key(500, 398, "ESC", "Market");
}
