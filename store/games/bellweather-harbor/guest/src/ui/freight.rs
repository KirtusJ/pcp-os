use super::*;
use crate::freight::{Exchange, Receipt};
use bellweather::{freight::TIDE_MS, market::NAMES};

fn crate_mark(c: &mut Canvas<'_>, x: usize, y: usize, col: u32) {
    c.rect(x, y, 13, 11, INK);
    c.rect(x + 1, y + 1, 11, 9, col);
    c.rect(x + 3, y + 1, 1, 9, INK);
    c.rect(x + 9, y + 1, 1, 9, INK);
    for n in 0..6 {
        c.rect(x + 4 + n, y + 7 - n, 1, 1, INK);
    }
}
pub(super) fn draw(c: &mut Canvas<'_>, game: &Economy, desk: &Exchange, now: u64) {
    if let Some(ticket) = &desk.order {
        super::order::draw(c, game, ticket);
        return;
    }
    let f = &game.freight;
    let i = desk.selected;
    let cap = f.capacity();
    let rem = (u64::from(TIDE_MS) - u64::from(f.elapsed_ms)).div_ceil(1000);
    let order = f.offer();
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    crate_mark(c, 13, 13, BRASS);
    c.label(38, 12, 2, PAPER, format_args!("Freight Exchange"));
    c.label(322, 17, 1, FOAM, format_args!("BELLWEATHER HARBOR"));
    c.label(487, 6, 1, BRASS, format_args!("TREASURY"));
    c.label(487, 19, 2, PAPER, format_args!("{}", Money(game.cash)));
    c.rect(12, 42, 300, 39, INK);
    c.rect(320, 42, 308, 39, INK);
    c.label(21, 49, 1, BRASS, format_args!("MARKET TIDE"));
    c.label(
        21,
        64,
        1,
        PAPER,
        format_args!(
            "Tide {}   /   next clearing in {rem}s",
            f.tide.saturating_add(1)
        ),
    );
    c.rect(21, 77, 280, 2, TEAL);
    c.rect(
        21,
        77,
        (u64::from(f.elapsed_ms) * 280 / u64::from(TIDE_MS)) as usize,
        2,
        FOAM,
    );
    c.label(330, 49, 1, BRASS, format_args!("WAREHOUSE"));
    c.label(330, 64, 1, PAPER, format_args!("{cap} crates per good"));
    if let Some(cost) = f.upgrade_cost() {
        c.label(478, 64, 1, BRASS, format_args!("U Expand {}", Money(cost)));
    } else {
        c.label(510, 64, 1, FOAM, format_args!("Fully expanded"));
    }

    c.rect(12, 90, 616, 146, PAPER);
    c.rect(12, 90, 616, 2, BRASS);
    c.label(22, 99, 1, INK, format_args!("QUAYSIDE ORDER BOOK"));
    c.label(454, 99, 1, MUTED, format_args!("Bid = sell. Ask = buy."));
    for &(x, title) in &[
        (23, "CARGO"),
        (142, "ON HAND"),
        (222, "PORT STOCK"),
        (323, "BID"),
        (395, "ASK"),
        (475, "VS CLOSE"),
        (554, "EST. BID"),
    ] {
        c.label(x, 117, 1, MUTED, format_args!("{title}"));
    }
    const COLORS: [u32; 5] = [0xf2e1ac, 0xd67956, 0xaf8b58, 0xcaaa63, 0x83aaa9];
    for (g, col) in COLORS.iter().enumerate() {
        let y = 131 + g * 20;
        let sel = g == i;
        let fg = if sel { PAPER } else { INK };
        let sub = if sel { FOAM } else { MUTED };
        c.rect(
            12,
            y,
            616,
            19,
            if sel {
                TEAL
            } else if g % 2 == 0 {
                PALE
            } else {
                PAPER
            },
        );
        if sel {
            c.rect(12, y, 4, 19, BRASS);
        }
        c.label(23, y + 6, 1, sub, format_args!("{}", g + 1));
        crate_mark(c, 38, y + 4, *col);
        c.label(58, y + 6, 1, fg, format_args!("{}", NAMES[g]));
        c.label(
            142,
            y + 6,
            1,
            fg,
            format_args!("{} / {}", game.inventory(g), cap),
        );
        c.label(222, y + 6, 1, sub, format_args!("{}", f.market.stock[g]));
        let bid = f.price(g);
        if bid == 0 {
            c.label(323, y + 6, 1, sub, format_args!("-"));
        } else {
            c.label(323, y + 6, 1, fg, format_args!("{}", Money(bid)));
        }
        if g < 3 && f.ask(g) > 0 {
            c.label(395, y + 6, 1, fg, format_args!("{}", Money(f.ask(g))));
        } else {
            c.label(395, y + 6, 1, sub, format_args!("-"));
        }
        let prev = f.relative_price(g, -1);
        if f.market.history_len == 0 {
            c.label(475, y + 6, 1, sub, format_args!("-"));
        } else if bid > prev {
            c.label(475, y + 6, 1, sub, format_args!("+{}", Money(bid - prev)));
        } else if prev > bid {
            c.label(475, y + 6, 1, sub, format_args!("-{}", Money(prev - bid)));
        } else {
            c.label(475, y + 6, 1, sub, format_args!("Steady"));
        }
        c.label(
            554,
            y + 6,
            1,
            fg,
            format_args!("{}", Money(f.projected_price(g))),
        );
    }
    c.rect(12, 245, 302, 100, INK);
    c.rect(323, 245, 305, 100, PALE);
    if desk.history {
        c.label(
            22,
            254,
            1,
            BRASS,
            format_args!("CLOSING BIDS  /  {}", NAMES[i]),
        );
        let n = usize::from(f.market.history_len);
        if n == 0 {
            c.label(
                22,
                279,
                1,
                PAPER,
                format_args!("The first close posts next tide."),
            );
        } else {
            let high = (1..=n)
                .map(|j| f.relative_price(i, -(j as i32)))
                .max()
                .unwrap_or(1)
                .max(1);
            for j in 0..n {
                let price = f.relative_price(i, -((n - j) as i32));
                let h = (price * 32 / high) as usize;
                let x = 22 + j * 35;
                c.rect(x, 306 - h, 23, h, if j + 1 == n { BRASS } else { TEAL });
                c.label(x, 312, 1, FOAM, format_args!("{}", price));
            }
        }
        c.label(
            22,
            329,
            1,
            FOAM,
            format_args!("Oldest to newest. H returns to quote."),
        );
    } else {
        c.label(
            22,
            254,
            1,
            BRASS,
            format_args!("DEAL TICKET  /  {}", NAMES[i]),
        );
        c.label(22, 270, 1, FOAM, format_args!("Trade"));
        c.label(97, 270, 1, FOAM, format_args!("One crate"));
        c.label(184, 270, 1, FOAM, format_args!("Maximum lot"));
        for n in 0..2 {
            let buy = n == 0;
            let one = game.quote_trade(i, buy, false);
            let max = game.quote_trade(i, buy, true);
            let y = 286 + n * 15;
            c.label(
                22,
                y,
                1,
                PAPER,
                format_args!("{}", if buy { "Buy" } else { "Sell" }),
            );
            if buy && i >= 3 {
                c.label(97, y, 1, FOAM, format_args!("Produced at works"));
                continue;
            }
            if one.qty > 0 {
                c.label(97, y, 1, PAPER, format_args!("{}", Money(one.coins)));
            } else {
                c.label(97, y, 1, FOAM, format_args!("-"));
            }
            if max.qty > 0 {
                c.label(
                    184,
                    y,
                    1,
                    PAPER,
                    format_args!("{} for {}", max.qty, Money(max.coins)),
                );
            } else {
                c.label(184, y, 1, FOAM, format_args!("0 crates"));
            }
        }
        c.label(
            22,
            326,
            1,
            FOAM,
            format_args!("H History   /   Lots walk the book."),
        );
    }
    c.label(334, 254, 1, INK, format_args!("BILL OF LADING"));
    c.label(
        334,
        270,
        2,
        INK,
        format_args!("{} x {}", order.qty, NAMES[order.good]),
    );
    c.label(
        334,
        291,
        1,
        INK,
        format_args!("Fixed payment   {}", Money(order.reward)),
    );
    if f.reserved {
        c.label(
            334,
            310,
            1,
            TEAL,
            format_args!("SIGNED  /  Payment due in {rem}s."),
        );
    } else if f.stock[order.good] >= order.qty {
        c.label(
            334,
            310,
            1,
            TEAL,
            format_args!("R  Reserve cargo for the next tide."),
        );
    } else {
        c.label(
            334,
            310,
            1,
            MUTED,
            format_args!(
                "Need {} more {} to reserve.",
                order.qty - f.stock[order.good],
                NAMES[order.good]
            ),
        );
    }
    let next_rivals = f.market.project_rival(f.tide);
    let rival = match next_rivals {
        [true, true] => "cannery + chandler",
        [true, false] => "cannery",
        [false, true] => "chandler",
        _ => "no batch",
    };
    c.label(334, 330, 1, MUTED, format_args!("Rival outlook   {rival}"));
    c.rect(12, 355, 616, 27, INK);
    c.rect(21, 365, 4, 7, BRASS);
    let msg = |c: &mut Canvas<'_>, txt: fmt::Arguments<'_>| c.label(32, 365, 1, PAPER, txt);
    if now < desk.receipt_until {
        match desk.receipt {
            Receipt::Trade {
                good,
                qty,
                coins,
                buy,
            } => msg(
                c,
                format_args!(
                    "{} {qty} {} for {}.",
                    if buy { "Bought" } else { "Sold" },
                    NAMES[good],
                    Money(coins)
                ),
            ),
            Receipt::NoTrade(true) if i >= 3 => msg(
                c,
                format_args!(
                    "Produce {} at the works, then sell the finished crates here.",
                    NAMES[i]
                ),
            ),
            Receipt::NoTrade(true) => msg(
                c,
                format_args!("No fill. Check treasury, available port stock and warehouse room."),
            ),
            Receipt::NoTrade(false) => msg(
                c,
                format_args!("No sale available for this stock and current market depth."),
            ),
            Receipt::Accepted => msg(
                c,
                format_args!("Order signed. Cargo is reserved at the posted rate."),
            ),
            Receipt::Delivered(coins) => msg(
                c,
                format_args!(
                    "Freight delivered. Fixed payment received  +{}.",
                    Money(coins)
                ),
            ),
            Receipt::AlreadyReserved => {
                msg(c, format_args!("Your signed order pays at the next tide."))
            }
            Receipt::MissingGoods => {
                msg(c, format_args!("Stock the requested cargo before signing."))
            }
            Receipt::Expanded => msg(
                c,
                format_args!("Warehouse expanded to {cap} crates per good."),
            ),
            Receipt::InsufficientFunds => msg(
                c,
                format_args!("Treasury short. Earn or sell cargo to expand the warehouse."),
            ),
            Receipt::Full => msg(c, format_args!("The warehouse is fully expanded.")),
            Receipt::Open => msg(
                c,
                format_args!("Trades move prices. The estimated bid assumes no further trades."),
            ),
        }
    } else {
        msg(
            c,
            format_args!("Trades move prices. The estimated bid assumes no further trades."),
        );
    }
    c.key(6, 398, "1-5", "Cargo");
    c.key(70, 398, "B", "Buy 1");
    c.key(126, 398, "X", "Buy max");
    c.key(194, 398, "S", "Sell 1");
    c.key(256, 398, "A", "Sell max");
    c.key(330, 398, "R", "Reserve");
    c.key(398, 398, "U", "Grow");
    c.key(445, 398, "L", "Limit");
    c.key(499, 398, "W", "Works");
    c.key(556, 398, "ESC", "Harbor");
}
