use super::*;
use crate::tea::{Meeting, Page, Receipt};
use bellweather::{
    committee::Stage,
    market,
    tea_text::{
        ADJOURN_TEXT, GRANNIES, KEEP_TEXT, MOTIONS, PROGRAMME_NOTE, SOUVENIR_TEXT, SOUVENIR_TITLE,
        TIDE_MINUTES, TREATY_TITLE,
    },
};

fn paragraph(c: &mut Canvas<'_>, x: usize, y: usize, cols: usize, color: u32, text: &str) -> usize {
    let mut row = y;
    super::journal::lines(text, cols, |line| {
        c.label(x, row, 1, color, format_args!("{line}"));
        row += 12;
    });
    row
}

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Dormant => "Invitation",
        Stage::TeaService => "Tea Service",
        Stage::Quorum => "Quorum",
        Stage::HighTea => "High Tea",
        Stage::Adjourned => "Adjourned",
    }
}

fn barter(c: &mut Canvas<'_>, game: &Economy, offer: bellweather::committee::Barter) {
    let stale = offer.tide != game.freight.tide;
    let served = !stale && game.committee.bartered;
    let give = market::NAMES[offer.give];
    let take = market::NAMES[offer.take];
    let on_hand = game.inventory(offer.give);
    let reserved = u16::from(offer.take >= 3 && game.factories[offer.take - 3].running);
    let room = game.cargo_room(offer.take);
    c.label(251, 78, 1, TEAL, format_args!("HIGH TEA   /   BARTER SLIP"));
    c.label(251, 101, 2, INK, format_args!("The sea's offer"));
    c.label(
        251,
        132,
        1,
        TEAL,
        format_args!(
            "{} {} {give}",
            if served { "Gave" } else { "Give" },
            offer.give_qty
        ),
    );
    c.label(
        251,
        150,
        1,
        TEAL,
        format_args!(
            "{} {} {take}",
            if served { "Received" } else { "Receive" },
            offer.take_qty
        ),
    );
    c.label(
        251,
        176,
        1,
        INK,
        format_args!("In your hold: {on_hand} {give}."),
    );
    c.label(251, 194, 1, INK, format_args!("Free space: {room} {take}."));
    if reserved != 0 {
        c.label(
            251,
            209,
            1,
            MUTED,
            format_args!("One space is reserved for a running batch."),
        );
    }
    c.rect(251, 229, 362, 1, RULE);
    if !stale {
        c.label(
            251,
            242,
            1,
            TEAL,
            format_args!(
                "New terms in {}s. One exchange per tide.",
                (30_000 - u32::from(game.freight.elapsed_ms)).div_ceil(1000)
            ),
        );
    }
    let action = if stale {
        "Offer expired. R fetches the new slip."
    } else if served {
        "Exchange recorded. Wait for the next tide."
    } else if on_hand < offer.give_qty {
        "More cargo is needed to complete this exchange."
    } else if !game.can_barter() {
        "Make room for the sea's cargo in your hold."
    } else {
        "ENTER  Exchange these crates"
    };
    paragraph(
        c,
        251,
        282,
        60,
        if stale || served { MUTED } else { INK },
        action,
    );
}

fn minutes(c: &mut Canvas<'_>, game: &Economy, meeting: &Meeting) {
    let leaf = meeting.minute;
    c.label(
        251,
        78,
        1,
        TEAL,
        format_args!(
            "MINUTE BOOK   /   PAGE {} OF {}",
            leaf + 1,
            game.committee.motions + 4
        ),
    );
    if leaf == 0 {
        c.label(251, 101, 2, INK, format_args!("This tide"));
        paragraph(
            c,
            251,
            138,
            60,
            INK,
            TIDE_MINUTES[(game.freight.tide % 12) as usize],
        );
    } else if leaf <= 3 {
        let member = &GRANNIES[leaf - 1];
        c.label(251, 101, 2, INK, format_args!("{}", member.name));
        c.label(251, 128, 1, TEAL, format_args!("{}", member.role));
        paragraph(c, 251, 155, 60, INK, member.line);
    } else {
        let i = leaf - 4;
        let minute = &MOTIONS[i];
        c.label(251, 101, 2, INK, format_args!("{}", minute.title));
        c.label(
            251,
            130,
            1,
            TEAL,
            format_args!("Motion {}   /   {}", i + 1, GRANNIES[minute.sponsor].name),
        );
        let row = paragraph(c, 251, 152, 60, INK, minute.proposal);
        paragraph(c, 251, row + 19, 60, INK, minute.receipt);
    }
}

pub(super) fn draw(c: &mut Canvas<'_>, scene: &Harbor, game: &Economy, meeting: &Meeting) {
    let committee = &game.committee;
    let stage = committee.stage;
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.label(14, 14, 2, PAPER, format_args!("Emergency Tea Committee"));
    c.label(451, 13, 1, BRASS, format_args!("HARBOR OFFICE / FILE 13"));
    c.rect(12, 53, 217, 270, INK);
    c.rect(12, 53, 217, 2, BRASS);
    // Crop the waterfront at native resolution so the changing committee can be seen.
    for row in 0..106 {
        let left = if stage == Stage::HighTea { 118 } else { 4 };
        let start = (row + 83) * 480 + left;
        c.0[(59 + row) * W + 16..(59 + row) * W + 225].copy_from_slice(&scene[start..start + 209]);
    }
    c.label(22, 181, 2, PAPER, format_args!("{}", stage_name(stage)));
    c.label(
        22,
        211,
        1,
        FOAM,
        format_args!("Motions carried   {} / 6", committee.motions),
    );
    for i in 0..6 {
        c.rect(
            22 + i * 32,
            228,
            26,
            5,
            if i < usize::from(committee.motions) {
                BRASS
            } else {
                TEAL
            },
        );
    }
    c.label(22, 251, 1, FOAM, format_args!("TREASURY"));
    c.label(22, 267, 1, PAPER, format_args!("{} coins", game.cash));
    c.label(22, 288, 1, FOAM, format_args!("TEA TIN"));
    c.label(22, 304, 1, BRASS, format_args!("{} coins", committee.tin));
    c.rect(237, 53, 391, 270, PAPER);
    c.rect(237, 53, 391, 2, RULE);
    if let Page::Barter(offer) = meeting.page {
        barter(c, game, offer);
    } else if meeting.page == Page::Minutes {
        minutes(c, game, meeting);
    } else if meeting.page == Page::Adjourn {
        c.label(251, 78, 1, TEAL, format_args!("FINAL RESOLUTION"));
        c.label(251, 101, 2, INK, format_args!("Adjourn for good?"));
        paragraph(c, 251, 141, 60, INK, "Release the entire tin and receive thirteen coins. Barter closes and future fares go straight to your treasury. The minutes and souvenir stay on file.");
        c.label(
            251,
            215,
            1,
            TEAL,
            format_args!("Due now: {} in the tin + 13 coins.", committee.tin),
        );
        c.label(
            251,
            294,
            1,
            INK,
            format_args!("ENTER  Sign the final resolution"),
        );
    } else if stage == Stage::Dormant {
        c.label(251, 78, 1, TEAL, format_args!("INVITATION TO SIGN"));
        c.label(251, 101, 2, INK, format_args!("{TREATY_TITLE}"));
        let row = paragraph(c, 251, 136, 60, INK, PROGRAMME_NOTE);
        let row = paragraph(c, 251, row + 12, 60, INK, "Signing sets aside one coin in eight from voyage fares. Collect the tin whenever you like. Signing lasts across charters.");
        paragraph(c, 251, row + 12, 60, INK, "Bring one salt crate for each of the first three motions and one provisions crate for each of the next three. After High Tea you can adjourn for good.");
        c.label(
            251,
            294,
            1,
            TEAL,
            format_args!("S  Sign Standing Order Thirteen"),
        );
    } else if let Some(good) = committee.motion_good() {
        let n = usize::from(committee.motions);
        let motion = &MOTIONS[n];
        c.label(
            251,
            78,
            1,
            TEAL,
            format_args!("ORDER OF BUSINESS   /   MOTION {} OF 6", n + 1),
        );
        c.label(251, 101, 2, INK, format_args!("{}", motion.title));
        c.label(
            251,
            130,
            1,
            TEAL,
            format_args!("Proposed by {}", GRANNIES[motion.sponsor].name),
        );
        paragraph(c, 251, 151, 60, INK, motion.proposal);
        c.rect(251, 216, 362, 1, RULE);
        c.label(
            251,
            233,
            1,
            INK,
            format_args!(
                "Motion requires 1 {}   /   {} in hold",
                market::NAMES[good],
                game.inventory(good)
            ),
        );
        c.label(
            251,
            256,
            1,
            TEAL,
            format_args!(
                "{}",
                if good == 0 {
                    "Three salt motions establish Quorum."
                } else {
                    "Three provisions motions bring High Tea."
                }
            ),
        );
        c.label(
            251,
            294,
            1,
            INK,
            format_args!(
                "{}",
                if game.can_submit_motion() {
                    "ENTER  Supply one crate and carry this motion"
                } else {
                    "Bring a crate to carry this motion."
                }
            ),
        );
    } else if stage == Stage::HighTea {
        c.label(251, 78, 1, TEAL, format_args!("ALL SIX MOTIONS CARRIED"));
        c.label(251, 101, 2, INK, format_args!("A place at High Tea"));
        let row = paragraph(c, 251, 141, 60, INK, KEEP_TEXT);
        paragraph(c, 251, row + 14, 60, INK, "Keep High Tea in session to barter cargo with the sea once each tide and collect the tin. Adjourn to take the remaining tin and thirteen coins, and restore ordinary fares.");
        c.label(
            251,
            268,
            1,
            TEAL,
            format_args!("B  Read the sea's barter slip"),
        );
        c.label(
            251,
            294,
            1,
            INK,
            format_args!("A  Consider permanent adjournment"),
        );
    } else {
        c.label(251, 78, 1, TEAL, format_args!("COMMITTEE SOUVENIR"));
        c.label(251, 101, 2, INK, format_args!("{SOUVENIR_TITLE}"));
        let row = paragraph(c, 251, 141, 60, INK, ADJOURN_TEXT);
        let row = paragraph(c, 251, row + 14, 60, INK, SOUVENIR_TEXT);
        c.label(
            251,
            row + 12,
            1,
            TEAL,
            format_args!("The sea's barter desk is closed."),
        );
        c.label(
            251,
            294,
            1,
            TEAL,
            format_args!("Voyage fares now go in full to the treasury."),
        );
    }
    c.rect(12, 334, 616, 47, INK);
    let status = match meeting.receipt {
        Receipt::Filed(i) => MOTIONS[i].receipt,
        Receipt::Signed => "The chair accepts the signed order. Tea service begins.",
        Receipt::Short => {
            "The motion is held over. Bring the requested crate to the harbor office."
        }
        Receipt::Kept => "The meeting stays in session.",
        Receipt::Claimed(0) => "The tin is empty.",
        Receipt::Claimed(_) => "",
        Receipt::None => TIDE_MINUTES[(game.freight.tide % 12) as usize],
    };
    if let Receipt::Claimed(amount) = meeting.receipt {
        if amount > 0 {
            c.label(
                22,
                348,
                1,
                BRASS,
                format_args!("Treasurer's receipt   /   {amount} coins paid from the tin."),
            );
        } else {
            paragraph(c, 22, 347, 98, FOAM, status);
        }
    } else {
        paragraph(c, 22, 347, 98, FOAM, status);
    }
    match meeting.page {
        Page::Barter(_) => {
            c.key(12, 397, "R", "Current offer");
            c.key(199, 397, "ESC", "High Tea");
        }
        Page::Minutes => {
            c.key(12, 397, "LEFT/RIGHT", "Turn page");
            c.key(190, 397, "ESC", "Meeting");
        }
        Page::Adjourn => {
            c.key(12, 397, "ENTER", "Adjourn for good");
            c.key(202, 397, "ESC", "Keep High Tea");
        }
        Page::Business => {
            c.key(12, 397, "M", "Minutes");
            if committee.active() {
                c.key(123, 397, "C", "Collect tin");
            }
            c.key(332, 397, "ESC", "Harbor");
        }
    }
}
