use super::*;
use crate::works::{Report, WorksDesk};
use bellweather::{factory::PLANS, freight::GOODS};

fn header(c: &mut Canvas<'_>, game: &Economy, title: &str) {
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.rect(0, 38, W, 1, TEAL);
    for k in 0..3 {
        c.rect(13 + k * 5, 11 + k, 3, 12 - k, BRASS);
    }
    c.rect(12, 23, 17, 2, BUOY);
    c.label(38, 12, 2, PAPER, format_args!("{title}"));
    c.label(302, 17, 1, FOAM, format_args!("BELLWEATHER HARBOR"));
    c.label(487, 6, 1, BRASS, format_args!("TREASURY"));
    c.label(487, 19, 2, PAPER, format_args!("{}", Money(game.cash)));
}
fn ledger(c: &mut Canvas<'_>, g: &Economy, show_cargo: bool) {
    if show_cargo {
        header(c, g, "Cargo Ledger");
        c.rect(12, 53, 616, 230, PAPER);
        c.rect(12, 53, 616, 3, BRASS);
        c.label(23, 66, 2, INK, format_args!("Warehouse movements"));
        c.label(
            23,
            96,
            1,
            MUTED,
            format_args!("Crates in and out during this charter."),
        );
        let columns = [
            (22, "CARGO"),
            (112, "OPEN"),
            (168, "LANDED"),
            (228, "BOUGHT"),
            (290, "MADE"),
            (340, "SOLD"),
            (393, "USED"),
            (447, "DELIVERED"),
            (540, "ON HAND"),
        ];
        for (x, title) in columns {
            c.label(x, 119, 1, MUTED, format_args!("{title}"));
        }
        for (i, v) in g.ledger.flows.iter().enumerate() {
            let y = 134 + i * 27;
            c.rect(12, y, 616, 26, if i % 2 == 0 { PALE } else { PAPER });
            c.label(
                22,
                y + 9,
                1,
                INK,
                format_args!("{}", bellweather::market::NAMES[i]),
            );
            for ((x, _), qty) in columns[1..].iter().zip([
                u64::from(v.opening),
                v.landed,
                v.bought,
                v.made,
                v.sold,
                v.used,
                v.delivered,
                u64::from(g.inventory(i)),
            ]) {
                if qty < 100000 {
                    c.label(*x, y + 9, 1, TEAL, format_args!("{qty}"));
                } else {
                    c.label(*x, y + 9, 1, TEAL, format_args!("{}k", qty / 1000));
                }
            }
        }
        c.label(22,308,1,FOAM,format_args!("Opening stock plus arrivals and purchases and production, less sales, use and deliveries."));
        c.key(12, 398, "C", "Cash ledger");
        c.key(154, 398, "L", "Works");
        c.key(258, 398, "F", "Market");
        c.key(525, 398, "ESC", "Harbor");
        return;
    }
    header(c, g, "Cash Ledger");
    c.rect(22, 55, 596, 255, PAPER);
    c.rect(22, 55, 596, 3, BRASS);
    c.label(38, 72, 2, INK, format_args!("Port accounts"));
    c.label(
        38,
        100,
        1,
        MUTED,
        format_args!("Cash flows for this charter"),
    );
    c.rect(38, 120, 564, 1, RULE);
    let l = &g.ledger;
    for (n, (label, amount, credit)) in [
        ("Cargo sales", l.sales, true),
        ("Freight orders and quay favors", l.orders, true),
        ("Cargo purchases", l.purchases, false),
        ("Processing fees", l.fees, false),
        ("Construction", l.construction, false),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 130 + n * 28;
        c.rect(30, y, 579, 25, if n % 2 == 0 { PALE } else { PAPER });
        c.label(42, y + 8, 1, INK, format_args!("{label}"));
        c.label(
            399,
            y + 8,
            1,
            TEAL,
            format_args!("{}", if credit { "RECEIVED" } else { "PAID" }),
        );
        c.label(503, y + 8, 1, INK, format_args!("{}", Money(amount)));
    }
    c.rect(38, 277, 564, 1, RULE);
    c.label(
        42,
        289,
        1,
        TEAL,
        format_args!("Voyage fares are tracked outside these port accounts."),
    );
    c.label(30, 332, 1, FOAM, format_args!("GROSS RECEIPTS"));
    c.label(30, 349, 2, PAPER, format_args!("{}", Money(g.earned)));
    c.label(322, 332, 1, FOAM, format_args!("TOWARD NEXT CHARTER"));
    c.label(
        322,
        349,
        2,
        PAPER,
        format_args!("{}", Money(g.charter_earned())),
    );
    c.key(12, 398, "C", "Cargo ledger");
    c.key(154, 398, "L", "Works");
    c.key(258, 398, "F", "Market");
    c.key(525, 398, "ESC", "Harbor");
}
fn works_mark(c: &mut Canvas<'_>, x: usize, y: usize, i: usize, running: bool, tick: u64) {
    // Tiny production-line emblems: stacked tins and a spool of rigging.
    if i == 0 {
        for (dx, dy) in [(1, 12), (17, 12), (9, 1)] {
            c.rect(x + dx, y + dy, 14, 13, INK);
            c.rect(x + dx + 1, y + dy + 1, 12, 11, BUOY);
            c.rect(x + dx + 1, y + dy + 1, 12, 2, PAPER);
            c.rect(x + dx + 1, y + dy + 9, 12, 2, BRASS);
        }
    } else {
        c.rect(x + 2, y + 8, 30, 12, BRASS);
        c.rect(x, y + 5, 5, 18, FOAM);
        c.rect(x + 29, y + 5, 5, 18, FOAM);
        let phase = if running {
            (tick / 140 % 4) as usize
        } else {
            0
        };
        for k in 0..6 {
            for j in 0..8 {
                c.rect(x + 5 + (k * 4 + j / 2 + phase) % 23, y + 10 + j, 1, 1, INK);
            }
        }
    }
}
pub(super) fn draw(c: &mut Canvas<'_>, g: &Economy, desk: &WorksDesk, now: u64) {
    if desk.ledger {
        ledger(c, g, desk.cargo_log);
        return;
    }
    header(c, g, "Harbor Works");
    let i = desk.selected;
    let plant = &g.factories[i];
    let plan = &PLANS[i];
    let cap = g.freight.capacity();
    for (j, v) in PLANS.iter().enumerate() {
        let x = 12 + 312 * j;
        let bg = if j == i { TEAL } else { INK };
        let f = &g.factories[j];
        c.rect(x, 48, 304, 81, bg);
        c.rect(x, 48, 304, 2, if j == i { BRASS } else { TEAL });
        works_mark(c, x + 12, 69, j, f.running, now);
        c.label(
            x + 56,
            58,
            1,
            FOAM,
            format_args!(
                "0{}   /   {}",
                j + 1,
                if f.built { "OPEN" } else { "CLOSED" }
            ),
        );
        c.label(x + 56, 75, 2, PAPER, format_args!("{}", v.name));
        c.label(
            x + 56,
            103,
            1,
            FOAM,
            format_args!("{}    {} / {} ready", v.product, f.output, cap),
        );
        if f.running {
            c.rect(x + 12, 121, 280, 3, NIGHT);
            c.rect(
                x + 12,
                121,
                (280 * u64::from(f.elapsed_ms) / u64::from(v.duration_ms)).max(1) as usize,
                3,
                BRASS,
            );
        }
    }
    c.rect(12, 140, 298, 144, PAPER);
    c.rect(12, 140, 298, 3, BRASS);
    c.label(23, 152, 1, INK, format_args!("BATCH REQUISITION"));
    c.label(23, 174, 1, MUTED, format_args!("INPUT"));
    c.label(133, 174, 1, MUTED, format_args!("HAVE / NEED"));
    c.label(230, 174, 1, MUTED, format_args!("RESALE"));
    let mut raw_value = 0;
    for n in 0..3 {
        let y = 190 + n * 16;
        let need = plan.inputs[n];
        let have = g.inventory(n);
        let value = g.freight.bid_lot(n, need).coins;
        raw_value += value;
        c.label(23, y, 1, INK, format_args!("{}", GOODS[n]));
        c.label(
            133,
            y,
            1,
            if have >= need { TEAL } else { INK },
            format_args!("{have} / {need}"),
        );
        c.label(230, y, 1, INK, format_args!("{}", Money(value)));
    }
    c.rect(22, 242, 277, 1, RULE);
    c.label(23, 254, 1, INK, format_args!("Operating fee"));
    c.label(230, 254, 1, INK, format_args!("{}", Money(plan.fee)));
    c.rect(320, 140, 308, 144, INK);
    c.label(331, 152, 1, BRASS, format_args!("CONVERSION QUOTE"));
    c.label(
        331,
        173,
        1,
        FOAM,
        format_args!("1 {} at current bid", plan.product),
    );
    c.label(
        542,
        173,
        1,
        PAPER,
        format_args!("{}", Money(g.freight.bid_lot(plan.good, 1).coins)),
    );
    c.label(331, 191, 1, FOAM, format_args!("Less raw resale + fee"));
    c.label(
        542,
        191,
        1,
        PAPER,
        format_args!("{}", Money(raw_value + plan.fee)),
    );
    c.rect(331, 213, 285, 1, TEAL);
    c.label(331, 226, 1, FOAM, format_args!("Margin per batch"));
    let m = g.conversion_margin(i);
    c.label(
        467,
        223,
        2,
        if m >= 0 { BRASS } else { BUOY },
        format_args!(
            "{}{}",
            if m >= 0 { "+" } else { "-" },
            Money(m.unsigned_abs())
        ),
    );
    c.label(
        331,
        259,
        1,
        FOAM,
        format_args!("Spot prices can change before shipping."),
    );
    c.rect(12, 293, 616, 58, PALE);
    c.rect(12, 293, 5, 58, if plant.running { BRASS } else { TEAL });
    let state = if !plant.built {
        "Unopened"
    } else if plant.running {
        "Batch in progress"
    } else if plant.repeat {
        "Waiting for supplies"
    } else {
        "Ready for a batch"
    };
    c.label(24, 303, 1, INK, format_args!("{state}"));
    if !plant.built {
        c.label(
            24,
            326,
            1,
            TEAL,
            format_args!("O  Open for {}", Money(plan.build_cost)),
        );
    } else if plant.running {
        c.label(
            24,
            326,
            1,
            TEAL,
            format_args!(
                "{}s to completion",
                (plan.duration_ms - plant.elapsed_ms).div_ceil(1000)
            ),
        );
    } else {
        c.label(
            24,
            326,
            1,
            TEAL,
            format_args!("{}s per batch", plan.duration_ms / 1000),
        );
    }
    c.label(237, 303, 1, MUTED, format_args!("AUTOMATION"));
    c.label(
        237,
        326,
        1,
        INK,
        format_args!(
            "{}",
            if plant.repeat {
                "Repeat on"
            } else {
                "Repeat off"
            }
        ),
    );
    let lot = g.quote_trade(plan.good, false, true);
    c.label(414, 303, 1, MUTED, format_args!("OUTBOUND QUOTE"));
    c.label(
        414,
        326,
        1,
        INK,
        format_args!("{} crates for {}", lot.qty, Money(lot.coins)),
    );
    c.rect(12, 360, 616, 23, INK);
    c.rect(22, 368, 4, 7, BRASS);
    let text = |c: &mut Canvas<'_>, a: fmt::Arguments<'_>| c.label(32, 368, 1, PAPER, a);
    if now < desk.until {
        match desk.report {
            Report::Opened => text(
                c,
                format_args!("Works opened. Enter pays the fee and loads a batch."),
            ),
            Report::Started => text(
                c,
                format_args!("Batch paid. The crew is processing the materials."),
            ),
            Report::AutoOn => text(
                c,
                format_args!(
                    "Repeat enabled. Batches start when inputs, cash and storage are available."
                ),
            ),
            Report::AutoOff => text(c, format_args!("Repeat off. Any paid batch will finish.")),
            Report::Sold(q, coins) => text(
                c,
                format_args!("Shipped {q} {} for {}.", plan.product, Money(coins)),
            ),
            Report::NoSale => text(
                c,
                format_args!("No finished goods can be sold at the current market depth."),
            ),
            Report::Unavailable => text(
                c,
                format_args!("Check your treasury, raw stock and finished-goods storage."),
            ),
            Report::Busy => text(c, format_args!("A prepaid batch is already running.")),
            Report::Closed => text(
                c,
                format_args!("Open the works before placing its first job."),
            ),
            Report::AlreadyOpen => {
                text(c, format_args!("These works are open. Enter runs a batch."))
            }
            Report::Idle => text(
                c,
                format_args!("Select a line. Each batch spends inputs and the cash fee."),
            ),
        }
    } else {
        text(
            c,
            format_args!("Select a line. Each batch spends inputs and the cash fee."),
        );
    }
    c.key(8, 398, "1-2", "Line");
    c.key(72, 398, "O", "Open");
    c.key(125, 398, "ENTER", "Batch");
    c.key(207, 398, "T", "Repeat");
    c.key(278, 398, "S", "Sell 1");
    c.key(344, 398, "A", "Sell max");
    c.key(424, 398, "L", "Ledger");
    c.key(489, 398, "F", "Market");
    c.key(559, 398, "ESC", "Harbor");
}
