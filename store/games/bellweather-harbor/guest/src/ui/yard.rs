use super::*;
use crate::yard::{Notice, Slipway};
use bellweather::{
    freight::GOODS,
    yard::PLANS,
    yard_art::{draw_yard, YARD_H, YARD_W},
};

pub(super) fn draw(
    c: &mut Canvas<'_>,
    frame: &mut [u32; YARD_W * YARD_H],
    game: &Economy,
    slipway: &Slipway,
    now: u64,
) {
    let i = slipway.selected;
    let plan = &PLANS[i];
    let yard = &game.yard;
    let launched = yard.completed.count_ones();
    let next = yard.next();
    let complete = yard.completed & (1 << i) != 0;
    let active = yard.building && next == Some(i);
    let launch_age = match slipway.notice {
        Notice::Launched(j) if i == j => now.checked_sub(slipway.notice_until.saturating_sub(6000)),
        _ => None,
    };
    let stage = if let Some(age) = launch_age.filter(|&elapsed| elapsed < 2400) {
        1001 + (age * 999 / 2400) as u16
    } else if active {
        (u64::from(yard.elapsed_ms) * 1000 / u64::from(plan.duration_ms)).clamp(1, 1000) as u16
    } else {
        0
    };
    draw_yard(
        frame,
        (now.saturating_mul(30) / 1000) as u32,
        i,
        launched as u8,
        stage,
    );
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.rect(0, 38, W, 1, TEAL);
    c.rect(14, 15, 15, 3, BRASS);
    c.rect(16, 18, 11, 2, BRASS);
    c.rect(18, 20, 7, 2, BRASS);
    c.rect(19, 8, 2, 7, FOAM);
    for s in 0..5 {
        c.rect(21, 8 + s, s + 1, 1, BUOY);
    }
    c.label(38, 12, 2, PAPER, format_args!("Bellweather Shipyard"));
    c.label(322, 17, 1, FOAM, format_args!("FLEET REGISTER"));
    c.label(487, 6, 1, BRASS, format_args!("TREASURY"));
    c.label(487, 19, 2, PAPER, format_args!("{}", Money(game.cash)));

    for (y, row) in frame.chunks_exact(YARD_W).enumerate() {
        let start = (48 + y) * W + 12;
        c.0[start..start + YARD_W].copy_from_slice(row);
    }
    c.rect(12, 160, 360, 22, TEAL);
    c.label(
        21,
        168,
        1,
        PAPER,
        format_args!("{}  /  {}", plan.name, plan.kind),
    );
    c.rect(382, 48, 246, 134, INK);
    c.label(394, 58, 1, BRASS, format_args!("LAUNCH REGISTER"));
    c.label(394, 74, 2, PAPER, format_args!("{launched} of 3 afloat"));
    c.label(
        394,
        99,
        1,
        FOAM,
        format_args!("Permanent voyage bonus   +{}%", yard.bonus_percent()),
    );
    c.rect(394, 114, 222, 1, TEAL);
    c.label(394, 124, 1, BRASS, format_args!("ON THE WAYS"));
    if let Some(build) = next.filter(|_| yard.building) {
        let working = &PLANS[build];
        let remain = working
            .duration_ms
            .saturating_sub(yard.elapsed_ms)
            .div_ceil(1000);
        c.label(
            394,
            141,
            1,
            PAPER,
            format_args!("{}   /   {}s to launch", working.name, remain),
        );
        c.rect(394, 159, 220, 7, NIGHT);
        c.rect(
            394,
            159,
            (220 * u64::from(yard.elapsed_ms) / u64::from(working.duration_ms)).max(1) as usize,
            7,
            BRASS,
        );
    } else if let Some(build) = next {
        c.label(
            394,
            141,
            1,
            PAPER,
            format_args!("Next keel   /   {}", PLANS[build].name),
        );
        c.label(
            394,
            160,
            1,
            FOAM,
            format_args!("Construction continues while away."),
        );
    } else {
        c.label(
            394,
            141,
            1,
            PAPER,
            format_args!("All three vessels commissioned."),
        );
        c.label(
            394,
            160,
            1,
            FOAM,
            format_args!("The fleet sails on every charter."),
        );
    }
    c.rect(12, 191, 616, 86, PAPER);
    c.rect(12, 191, 616, 2, BRASS);
    for j in 0..3 {
        let v = &PLANS[j];
        let x = 12 + j * 206;
        let selected = i == j;
        c.rect(
            x,
            193,
            204,
            84,
            if selected {
                TEAL
            } else if j % 2 == 0 {
                PALE
            } else {
                PAPER
            },
        );
        let fg = if selected { PAPER } else { INK };
        let minor = if selected { FOAM } else { MUTED };
        c.label(
            x + 10,
            201,
            1,
            minor,
            format_args!("0{}   /   {}", j + 1, v.kind),
        );
        c.label(x + 10, 217, 2, fg, format_args!("{}", v.name));
        let state = if yard.completed & (1 << j) != 0 {
            "Afloat"
        } else if next == Some(j) && yard.building {
            "Fitting out"
        } else if next == Some(j) {
            "Next keel"
        } else {
            "Plans on file"
        };
        c.label(x + 10, 243, 1, fg, format_args!("{state}"));
        c.label(
            x + 10,
            259,
            1,
            minor,
            format_args!(
                "{}s fit-out   /   +{}% pay",
                v.duration_ms / 1000,
                v.bonus_percent
            ),
        );
    }
    c.rect(12, 287, 616, 67, INK);
    c.label(
        21,
        297,
        1,
        BRASS,
        format_args!("BUILD REQUISITION  /  {}", plan.name),
    );
    c.label(432, 297, 1, FOAM, format_args!("On hand / required"));
    c.rect(21, 310, 596, 1, TEAL);
    for j in 0..4 {
        let x = 21 + j * 148;
        let (material, need, have) = if j == 0 {
            ("Treasury", plan.cost, game.cash)
        } else {
            (
                GOODS[j - 1],
                u64::from(plan.cargo[j - 1]),
                u64::from(game.freight.stock[j - 1]),
            )
        };
        c.label(x, 319, 1, FOAM, format_args!("{material}"));
        let status = if have >= need { PAPER } else { BUOY };
        if j == 0 {
            c.label(
                x,
                336,
                1,
                status,
                format_args!("{} / {}", Money(have), Money(need)),
            );
        } else {
            c.label(x, 336, 1, status, format_args!("{have} / {need}"));
        }
    }
    c.rect(12, 361, 616, 21, INK);
    c.rect(21, 368, 4, 7, BRASS);
    let msg = |c: &mut Canvas<'_>, txt: fmt::Arguments<'_>| c.label(32, 368, 1, PAPER, txt);
    if now < slipway.notice_until {
        match slipway.notice {
            Notice::Paid => msg(
                c,
                format_args!(
                    "Keel laid. Cash and materials booked. The crew is fitting out the hull."
                ),
            ),
            Notice::Busy => msg(
                c,
                format_args!("The slipway is occupied. Its current vessel launches automatically."),
            ),
            Notice::Missing => msg(
                c,
                format_args!("Gather the missing cash or crates shown in the requisition."),
            ),
            Notice::Prior => msg(c, format_args!("Launch the preceding vessel first.")),
            Notice::Finished => msg(
                c,
                format_args!("This vessel already sails in your permanent fleet."),
            ),
            Notice::Launched(v) => msg(
                c,
                format_args!(
                    "{} launched! The fleet now adds {}% to voyages.",
                    PLANS[v].name,
                    yard.bonus_percent()
                ),
            ),
            Notice::Plan => msg(
                c,
                format_args!("Shipyard progress carries over when signing a charter."),
            ),
        }
    } else if complete {
        msg(
            c,
            format_args!(
                "{} sails under every charter. Permanent voyage bonus  +{}%.",
                plan.name, plan.bonus_percent
            ),
        );
    } else if active {
        msg(
            c,
            format_args!(
                "Fitting out. You may leave the slipway or sign a charter while the crew works."
            ),
        );
    } else if next != Some(i) {
        msg(
            c,
            format_args!(
                "Commission {} before starting {}.",
                PLANS[i.saturating_sub(1)].name,
                plan.name
            ),
        );
    } else if game.can_build(i) {
        msg(
            c,
            format_args!("Ready to lay the keel. Enter spends the listed cash and materials."),
        );
    } else {
        msg(c,format_args!("Earn cash on voyages. Rowboats bring Salt, Crab Pots bring Crab, Ferries bring Timber."));
    }
    c.key(12, 398, "1-3", "Select plans");
    c.key(170, 398, "ENTER", "Lay keel");
    c.key(325, 398, "F", "Freight");
    c.key(501, 398, "ESC", "Back to harbor");
}
