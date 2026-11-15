mod freight;
mod journal;
mod order;
mod quay;
mod tea;
mod wire;
mod works;
mod yard;
use crate::{
    glyph::glyph,
    mail::{Catch, MailRun},
};
use bellweather::{
    art,
    economy::{Bulk, Economy, CHARTER_BONUS_PERCENT, MAX_UNITS, SHOP_NAMES},
};
use core::fmt::{self, Write};

pub const W: usize = 640;
pub const H: usize = 416;
pub type Screen = [u32; W * H];
pub type Harbor = [u32; 480 * 192];

const INK: u32 = 0x142e38;
const NIGHT: u32 = 0x102933;
const TEAL: u32 = 0x205764;
const PALE: u32 = 0xf4e9d0;
const PAPER: u32 = 0xfff6df;
const RULE: u32 = 0xccbea4;
const BRASS: u32 = 0xecbb64;
const BUOY: u32 = 0xf58b66;
const FOAM: u32 = 0x9adcd1;
const MUTED: u32 = 0x556d6d;

pub enum Notice {
    Welcome,
    Bell,
    Restored(u64),
    Bought(u16),
    Unaffordable,
    Dispatch,
    Busy,
    NoFleet,
    Hired,
    Staffed,
    Paid(u64),
    MaxFleet,
    Chartered(u32),
    SaveFailed,
    Launched(usize),
}
pub enum Book {
    Harbor,
    Tea(crate::tea::Meeting),
    Quay(crate::quay::QuayDesk),
    Charter,
    Journal(crate::journal::Journal),
    Mail(MailRun),
    Yard(crate::yard::Slipway),
    Freight(crate::freight::Exchange),
    Works(crate::works::WorksDesk),
}
pub struct Desk {
    pub selected: usize,
    pub bulk: Bulk,
    pub book: Book,
    pub notice: Notice,
    pub notice_until: u64,
    pub store_ok: bool,
    pub bell_reply: bellweather::neighbors::BellReply,
    pub answer: Option<(usize, u64)>,
    pub sea_echo: bellweather::committee::SeaEcho,
    pub sea_caption_until: u64,
}

pub struct Money(pub u64);
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [&str; 7] = ["", "k", "m", "b", "t", "q", "Q"];
        let mut div = 1u64;
        let mut unit = 0;
        while unit + 1 < UNITS.len() && self.0 / div >= 10_000 {
            div *= 1000;
            unit += 1;
        }
        if div == 1 {
            write!(f, "${}", self.0)
        } else {
            write!(
                f,
                "${}.{}{}",
                self.0 / div,
                self.0 / (div / 10) % 10,
                UNITS[unit]
            )
        }
    }
}

struct Canvas<'a>(&'a mut Screen);
impl Canvas<'_> {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: u32) {
        let end = x.saturating_add(w).min(W);
        if x >= end {
            return;
        }
        for row in y..y.saturating_add(h).min(H) {
            self.0[row * W + x..row * W + end].fill(c);
        }
    }
    fn label(&mut self, x: usize, y: usize, size: usize, color: u32, text: fmt::Arguments<'_>) {
        struct Pen<'a, 'b> {
            canvas: &'a mut Canvas<'b>,
            x: usize,
            y: usize,
            size: usize,
            color: u32,
        }
        impl Write for Pen<'_, '_> {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                for b in text.bytes() {
                    for (y, bits) in glyph(b).iter().enumerate() {
                        for x in 0..5 {
                            if bits & (16 >> x) != 0 {
                                self.canvas.rect(
                                    self.x + x * self.size,
                                    self.y + y * self.size,
                                    self.size,
                                    self.size,
                                    self.color,
                                );
                            }
                        }
                    }
                    self.x += 6 * self.size;
                }
                Ok(())
            }
        }
        let _ = (Pen {
            canvas: self,
            x,
            y,
            size,
            color,
        })
        .write_fmt(text);
    }
    fn key(&mut self, x: usize, y: usize, key: &str, action: &str) {
        let width = key.len() * 6 + 6;
        self.rect(x, y - 2, width, 12, TEAL);
        self.label(x + 3, y, 1, FOAM, format_args!("{key}"));
        self.label(x + width + 5, y, 1, PAPER, format_args!("{action}"));
    }
}

pub fn render(out: &mut Screen, scene: &mut Harbor, game: &Economy, desk: &mut Desk, now: u64) {
    render_desk(out, scene, game, desk, now);
    let caption = if now < desk.sea_caption_until
        && game.committee.stage == bellweather::committee::Stage::HighTea
    {
        Some("The sea votes for sugar.")
    } else {
        desk.answer
            .filter(|&(_, until)| now < until)
            .map(|(person, _)| bellweather::quay_text::ANSWERS[person])
    };
    if let Some(text) = caption {
        let mut c = Canvas(out);
        c.rect(8, 361, 624, 23, INK);
        c.rect(8, 361, 3, 23, BRASS);
        c.label(18, 369, 1, PAPER, format_args!("{text}"));
    }
}

fn render_desk(out: &mut Screen, scene: &mut Harbor, game: &Economy, desk: &mut Desk, now: u64) {
    if let Book::Quay(quay) = &desk.book {
        quay::draw(&mut Canvas(out), game, quay);
        return;
    }
    if let Book::Journal(journal) = &mut desk.book {
        journal::draw(&mut Canvas(out), game, journal);
        return;
    }
    if let Book::Yard(slipway) = &desk.book {
        let frame = (&mut scene[..bellweather::yard_art::YARD_W * bellweather::yard_art::YARD_H])
            .try_into()
            .expect("shipyard frame fits harbor");
        yard::draw(&mut Canvas(out), frame, game, slipway, now);
        return;
    }
    if let Book::Works(works) = &desk.book {
        works::draw(&mut Canvas(out), game, works, now);
        return;
    }
    if let Book::Freight(exchange) = &desk.book {
        freight::draw(&mut Canvas(out), game, exchange, now);
        return;
    }
    let i = desk.selected;
    let mut units = [0; 5];
    let mut managers = 0;
    let mut running = 0;
    for (n, shop) in game.shops.iter().enumerate() {
        units[n] = u32::from(shop.owned);
        managers |= (shop.manager as u8) << n;
        running |= (shop.running as u8) << n;
    }
    art::draw_harbor(
        scene,
        (now.saturating_mul(30) / 1000) as u32,
        units,
        managers,
        running,
        i,
    );
    art::draw_home_fleet(
        scene,
        (now.saturating_mul(30) / 1000) as u32,
        game.yard.completed,
    );
    art::draw_quay_room(
        scene,
        (now.saturating_mul(30) / 1000) as u32,
        game.manifest.ending,
    );
    art::draw_harbor_watch(
        scene,
        game.freight.tide,
        u32::from(game.freight.elapsed_ms),
        game.neighbors.lanterns_unlocked(),
    );
    let stage = game.committee.stage as u8;
    let art_stage = if stage == 0 {
        u8::from(game.committee_invited())
    } else {
        stage + 1
    };
    art::draw_tea_committee(scene, (now.saturating_mul(30) / 1000) as u32, art_stage);
    let mut c = Canvas(out);
    if let Book::Tea(meeting) = &desk.book {
        tea::draw(&mut c, scene, game, meeting);
        return;
    }
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    c.rect(12, 8, 2, 22, BRASS);
    for k in 0..8 {
        c.rect(14, 9 + k, 13 - k, 1, BUOY);
    }
    c.label(34, 12, 2, PAPER, format_args!("Bellweather Harbor"));
    if art_stage != 0 {
        c.rect(251, 9, 45, 22, TEAL);
        c.label(255, 17, 1, BRASS, format_args!("T Tea"));
    }
    if game.manifest.unlocked {
        c.rect(301, 9, 160, 23, TEAL);
        c.rect(308, 12, 11, 15, PALE);
        c.rect(311, 16, 5, 1, INK);
        c.rect(311, 19, 4, 1, INK);
        let label = if game.manifest.solved == 0 && game.manifest.read[0] == 0 {
            "J  Odd receipt"
        } else {
            "J  Casebook"
        };
        c.label(327, 17, 1, BRASS, format_args!("{label}"));
    } else {
        c.label(315, 16, 1, FOAM, format_args!("HARBORMASTER'S OFFICE"));
    }

    c.label(487, 6, 1, BRASS, format_args!("TREASURY"));
    c.label(487, 19, 2, PAPER, format_args!("{}", Money(game.cash)));
    for (y, row) in scene.chunks_exact(480).enumerate() {
        let start = (y + 39) * W;
        c.0[start..start + 480].copy_from_slice(row);
    }

    c.rect(480, 39, 160, 192, INK);
    c.rect(480, 39, 2, 192, BRASS);
    c.label(490, 48, 1, BRASS, format_args!("BERTH 0{}", i + 1));
    c.label(490, 65, 2, PAPER, format_args!("{}", SHOP_NAMES[i]));
    c.rect(490, 88, 138, 1, TEAL);
    let duration = game.duration_ms(i);
    let shop = &game.shops[i];
    let (caption, payout) = if shop.owned == 0 {
        ("FIRST UNIT / VOYAGE", game.opening_payout(i))
    } else {
        ("PER VOYAGE", game.payout(i))
    };
    c.label(490, 98, 1, FOAM, format_args!("{caption}"));
    c.label(
        490,
        111,
        1,
        PAPER,
        format_args!(
            "{} / {}.{}s",
            Money(payout),
            duration / 1000,
            duration / 100 % 10
        ),
    );
    c.label(490, 129, 1, FOAM, format_args!("MANAGER"));
    if shop.manager {
        c.label(490, 142, 1, PAPER, format_args!("On duty"));
    } else {
        c.label(
            490,
            142,
            1,
            PAPER,
            format_args!("M to hire  {}", Money(game.manager_cost(i))),
        );
    }
    c.label(490, 160, 1, FOAM, format_args!("NEXT MILESTONE"));
    if let Some(target) = game.next_milestone(i) {
        c.label(
            490,
            173,
            1,
            PAPER,
            format_args!("{} units  /  2x pay", target),
        );
    } else {
        c.label(490, 173, 1, PAPER, format_args!("Fleet complete"));
    }
    c.rect(490, 190, 138, 1, TEAL);
    c.label(490, 200, 1, BRASS, format_args!("HARBOR CHARTERS"));
    c.label(
        490,
        214,
        1,
        PAPER,
        format_args!("{} held  /  C to view", game.charters),
    );

    c.rect(0, 231, W, 132, PAPER);
    c.rect(0, 231, W, 2, BRASS);
    c.label(12, 238, 1, INK, format_args!("FLEET REGISTER"));
    quay::watch(&mut c, game, 132, 238, TEAL);
    c.label(
        315,
        238,
        1,
        TEAL,
        format_args!(
            "Q  Talk to {}",
            bellweather::neighbors::NAMES[bellweather::neighbors::visitor(game.freight.tide)]
        ),
    );
    let mode = match desk.bulk {
        Bulk::One => "1",
        Bulk::Ten => "10",
        Bulk::Max => "MAX",
    };
    c.label(492, 238, 1, TEAL, format_args!("X  BUY QUANTITY  {mode}"));
    c.rect(12, 249, 616, 1, RULE);
    for &(x, s) in &[
        (12, "BERTH / BUSINESS"),
        (170, "OWNED"),
        (226, "VOYAGE"),
        (353, "CREW"),
        (463, "PURCHASE QUOTE"),
    ] {
        c.label(x, 254, 1, MUTED, format_args!("{s}"));
    }
    for n in 0..5 {
        let y = 266 + n * 19;
        let sel = n == i;
        let fg = if sel { PAPER } else { INK };
        let sub = if sel { FOAM } else { MUTED };
        c.rect(
            0,
            y,
            W,
            18,
            if sel {
                TEAL
            } else if n % 2 == 0 {
                PALE
            } else {
                PAPER
            },
        );
        if sel {
            c.rect(0, y, 4, 18, BRASS);
        }
        let shop = &game.shops[n];
        c.label(12, y + 5, 1, sub, format_args!("0{}", n + 1));
        c.label(35, y + 5, 1, fg, format_args!("{}", SHOP_NAMES[n]));
        c.label(171, y + 5, 1, fg, format_args!("{}", shop.owned));
        let status = if shop.owned == 0 {
            "Closed"
        } else if shop.running {
            "At sea"
        } else {
            "Ready"
        };
        c.label(226, y + 5, 1, fg, format_args!("{status}"));
        if shop.running {
            c.rect(273, y + 7, 64, 4, if sel { NIGHT } else { RULE });
            let amount = 64 * u64::from(shop.elapsed_ms) / u64::from(game.duration_ms(n)).max(1);
            c.rect(
                273,
                y + 7,
                (amount as usize).clamp(1, 64),
                4,
                if sel { BRASS } else { TEAL },
            );
        }
        c.label(
            353,
            y + 5,
            1,
            sub,
            format_args!(
                "{}",
                if shop.manager {
                    "Manager"
                } else if shop.owned > 0 {
                    "Manual"
                } else {
                    "-"
                }
            ),
        );
        let (qty, cost) = game.quote(n, desk.bulk);
        if shop.owned == MAX_UNITS {
            c.label(463, y + 5, 1, sub, format_args!("Complete"));
        } else if qty == 0 {
            c.label(
                463,
                y + 5,
                1,
                sub,
                format_args!("Need {}", Money(game.quote(n, Bulk::One).1)),
            );
        } else {
            c.label(
                463,
                y + 5,
                1,
                if cost <= game.cash { fg } else { sub },
                format_args!("{} for {}", qty, Money(cost)),
            );
        }
    }
    c.rect(0, 363, W, 25, INK);
    c.rect(12, 371, 4, 7, if desk.store_ok { BRASS } else { BUOY });
    let msg = |c: &mut Canvas<'_>, a: fmt::Arguments<'_>| c.label(23, 371, 1, PAPER, a);
    if !desk.store_ok && matches!(desk.notice, Notice::SaveFailed) {
        msg(
            &mut c,
            format_args!("Save failed. Esc again leaves without saving."),
        );
    } else if !desk.store_ok {
        msg(
            &mut c,
            format_args!("Store unavailable. Your fleet is in memory."),
        );
    } else if now < desk.notice_until {
        match desk.notice {
            Notice::Bell => msg(
                &mut c,
                format_args!(
                    "The harbor bell rings. Treasury  +{}.",
                    Money(game.bell_tip())
                ),
            ),
            Notice::Welcome => msg(
                &mut c,
                format_args!("Ring R for a coin and send out your rowboat."),
            ),
            Notice::Restored(0) => msg(
                &mut c,
                format_args!("Welcome back, captain. Fleet restored."),
            ),
            Notice::Restored(amount) => msg(
                &mut c,
                format_args!(
                    "Welcome back. The harbor earned {} while you were away.",
                    Money(amount)
                ),
            ),
            Notice::Bought(n) => msg(&mut c, format_args!("Added {n} to the fleet.")),
            Notice::Unaffordable => msg(
                &mut c,
                format_args!("Treasury short. Run a voyage to earn more."),
            ),
            Notice::Dispatch => msg(
                &mut c,
                format_args!("Cast off! Earnings arrive when the voyage finishes."),
            ),
            Notice::Busy => msg(&mut c, format_args!("Voyage under way.")),
            Notice::NoFleet => msg(
                &mut c,
                format_args!("Buy the first unit before dispatching a voyage or hiring."),
            ),
            Notice::Hired => msg(
                &mut c,
                format_args!("Manager on duty. Voyages now repeat automatically."),
            ),
            Notice::Staffed => msg(&mut c, format_args!("Your manager is already on duty.")),
            Notice::Paid(amount) => {
                msg(&mut c, format_args!("Voyage receipts  +{}", Money(amount)))
            }
            Notice::MaxFleet => msg(&mut c, format_args!("This fleet is fully built.")),
            Notice::Chartered(n) => msg(
                &mut c,
                format_args!("Charter signed. Permanent shares awarded  +{n}."),
            ),
            Notice::Launched(i) => msg(
                &mut c,
                format_args!(
                    "{} launched! All voyages now earn a fleet bonus.",
                    bellweather::yard::PLANS[i].name
                ),
            ),
            Notice::SaveFailed => msg(&mut c, format_args!("Save failed. Press Esc to retry.")),
        }
    } else {
        msg(
            &mut c,
            format_args!("R rings the bell  /  Space sends the selected fleet out."),
        );
    }
    c.key(4, 398, "1-5", "Berth");
    c.key(65, 398, "SPACE", "Sail");
    c.key(132, 398, "R", "Bell");
    c.key(176, 398, "B", "Buy");
    c.key(215, 398, "X", "Qty");
    c.key(253, 398, "M", "Hire");
    c.key(298, 398, "P", "Mail");
    c.key(343, 398, "C", "Charter");
    c.key(405, 398, "F", "Market");
    c.key(463, 398, "Y", "Yard");
    c.key(507, 398, "W", "Works");
    c.key(561, 398, "ESC", "Exit");
    if let Book::Mail(run) = &desk.book {
        mail_overlay(&mut c, game, desk.selected, run, now);
    }
    if matches!(desk.book, Book::Charter) {
        charter_overlay(&mut c, game);
    }
}

fn charter_overlay(c: &mut Canvas<'_>, game: &Economy) {
    let offer = game.charter_offer();
    c.rect(99, 121, 452, 168, NIGHT);
    c.rect(94, 115, 452, 168, BRASS);
    c.rect(96, 117, 448, 164, PAPER);
    c.rect(96, 117, 448, 24, INK);
    c.label(
        108,
        126,
        1,
        BRASS,
        format_args!("CHARTER OFFICE   /   BELLWEATHER HARBOR"),
    );
    c.label(108, 150, 2, INK, format_args!("Sign a new charter"));
    c.label(
        108,
        175,
        1,
        INK,
        format_args!(
            "Each permanent share adds {}% to every future payout.",
            CHARTER_BONUS_PERCENT
        ),
    );
    c.label(
        108,
        190,
        1,
        INK,
        format_args!("Fleet, cash, cargo, freight orders and works reset."),
    );
    c.label(
        108,
        202,
        1,
        TEAL,
        format_args!("Ships, yard work, friendships and keepsakes carry over."),
    );
    c.label(
        108,
        218,
        1,
        TEAL,
        format_args!(
            "Shares held  {}   /   {} toward a charter",
            game.charters,
            Money(game.charter_earned())
        ),
    );
    if offer > 0 {
        c.label(108, 230, 1, INK, format_args!("Shares available  +{offer}"));
        c.rect(96, 250, 448, 31, TEAL);
        c.key(108, 262, "ENTER", "Sign and restart");
        c.key(320, 262, "ESC", "Keep sailing");
    } else {
        c.label(
            108,
            234,
            1,
            INK,
            format_args!(
                "Next share at {} in eligible earnings.",
                Money(game.charter_target())
            ),
        );
        c.rect(96, 250, 448, 31, TEAL);
        c.key(108, 262, "ESC", "Back to harbor");
    }
}

fn mail_overlay(c: &mut Canvas<'_>, game: &Economy, berth: usize, run: &MailRun, now: u64) {
    c.rect(99, 121, 452, 180, NIGHT);
    c.rect(94, 115, 452, 180, BRASS);
    c.rect(96, 117, 448, 176, PAPER);
    c.rect(96, 117, 448, 24, INK);
    c.label(
        108,
        126,
        1,
        BRASS,
        format_args!("BELLWEATHER MAIL   /   {}", SHOP_NAMES[berth]),
    );
    let count = game.postmarks.count_ones();
    let title = match run.outcome {
        Catch::Sailing => "Catch the mailbag",
        Catch::Missed => "Just missed the sack",
        _ if count == 5 => "Album complete!",
        Catch::Stamped => "Postmark collected!",
        Catch::AlreadyHeld => "Already postmarked",
        Catch::Closed => "The berth is closed",
    };
    c.label(108, 152, 2, INK, format_args!("{title}"));
    match run.outcome {
        Catch::Sailing => {
            c.label(
                108,
                174,
                1,
                TEAL,
                format_args!("Press Space when the mailbag reaches the gold landing."),
            );
            c.rect(119, 199, 402, 10, TEAL);
            c.rect(280, 197, 80, 14, BRASS);
            let x = 120 + (run.position(now) / 3) as usize;
            c.rect(x - 4, 193, 9, 6, INK);
            c.rect(x - 2, 191, 5, 2, INK);
            c.rect(x - 1, 189, 3, 2, BUOY);
            c.rect(x - 1, 200, 3, 12, BUOY);
        }
        Catch::Missed => c.label(
            108,
            177,
            1,
            TEAL,
            format_args!("Time the catch in the gold landing. Space to try again."),
        ),
        Catch::Closed => c.label(
            108,
            177,
            1,
            TEAL,
            format_args!("Open this business before picking up its postmark."),
        ),
        _ => c.label(
            108,
            177,
            1,
            TEAL,
            format_args!("Postmarks travel with you into every new charter."),
        ),
    }
    c.label(
        108,
        223,
        1,
        INK,
        format_args!("PORT POSTMARKS    {count} / 5"),
    );
    for n in 0..5 {
        let x = 108 + 84 * n;
        let held = game.postmarks & (1 << n) != 0;
        c.rect(x, 239, 80, 17, if held { TEAL } else { PALE });
        c.rect(x + 1, 240, 78, 1, if held { FOAM } else { RULE });
        c.label(
            x + 5,
            244,
            1,
            if held { PAPER } else { MUTED },
            format_args!("{}", SHOP_NAMES[n]),
        );
    }
    c.rect(96, 263, 448, 30, TEAL);
    if matches!(run.outcome, Catch::Sailing | Catch::Missed) {
        let action = if matches!(run.outcome, Catch::Missed) {
            "Retry"
        } else {
            "Catch"
        };
        c.key(108, 275, "SPACE", action);
    }
    c.key(320, 275, "ESC", "Back to harbor");
}
