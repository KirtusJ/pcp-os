use crate::{
    islands::{destination, horizon, star, target_art},
    paint::*,
    scenery::*,
    *,
};
use core::fmt::{self, Write};

struct Label {
    bytes: [u8; 64],
    len: usize,
}
impl Label {
    fn new() -> Self {
        Self {
            bytes: [0; 64],
            len: 0,
        }
    }
    fn value(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}
impl Write for Label {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.len() > self.bytes.len() - self.len {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}

fn border(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, col: Color) {
    c.rect(x, y, w, 1, col);
    c.rect(x, y + h - 1, w, 1, col);
    c.rect(x, y, 1, h, col);
    c.rect(x + w - 1, y, 1, h, col);
}
fn panel(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    c.rect(x + 3, y + 3, w, h, 0x302a44);
    c.rect(x, y, w, h, INK);
    border(c, x, y, w, h, 0x937e8b);
}
fn caption(c: &mut Canvas, x: i32, y: i32, label: &str) {
    let width = label.len() as i32 * 6 + 12;
    c.rect(x, y, width, 14, AMBER);
    c.text(x + 6, y + 4, label, 1, INK);
}
pub(crate) fn bottom(c: &mut Canvas, s: &str, right: &str) {
    c.rect(0, 266, 512, 22, INK);
    c.rect(0, 266, 512, 1, 0x776680);
    c.text(12, 274, s, 1, PEARL);
    c.text(500 - right.len() as i32 * 6, 274, right, 1, MIST);
}
fn stamp(c: &mut Canvas, x: i32, y: i32) {
    c.rect(x, y, 20, 16, PEARL);
    for k in 0..6 {
        c.dot(x + 2 + 3 * k, y, INK);
        c.dot(x + 2 + 3 * k, y + 15, INK);
    }
    for k in 0..4 {
        c.dot(x, y + 2 + 3 * k, INK);
        c.dot(x + 19, y + 2 + 3 * k, INK);
    }
    c.rect(x + 3, y + 3, 14, 10, 0x496879);
    c.poly(
        &[
            (x + 4, y + 11),
            (x + 8, y + 6),
            (x + 11, y + 9),
            (x + 14, y + 5),
            (x + 16, y + 11),
        ],
        JADE,
    );
}
pub(crate) fn postmark(
    c: &mut Canvas,
    x: i32,
    y: i32,
    route: Route,
    id: usize,
    owned: bool,
    gold: bool,
) {
    let (paper, field) = if owned {
        (PEARL, [0x497e7e, 0xa56463, 0x526b94][route as usize])
    } else {
        (0x878498, 0x44445d)
    };
    c.rect(x, y, 27, 23, paper);
    for k in 0..7 {
        c.rect(x + 2 + k * 4, y, 1, 2, INK);
        c.rect(x + 2 + k * 4, y + 21, 1, 2, INK);
    }
    for k in 0..5 {
        c.rect(x, y + 3 + k * 4, 2, 1, INK);
        c.rect(x + 25, y + 3 + k * 4, 2, 1, INK);
    }
    c.rect(x + 3, y + 3, 21, 17, field);
    let ink = if owned { PEARL } else { 0xa6a4b6 };
    match (route, id) {
        (Route::Coast, 0) => {
            pine(c, x + 14, y + 19, 16, ink);
            c.rect(x + 7, y + 17, 14, 1, ink);
        }
        (Route::Coast, 1) => {
            c.poly(&[(x + 6, y + 7), (x + 14, y + 11), (x + 10, y + 17)], ink);
            c.poly(&[(x + 21, y + 7), (x + 14, y + 11), (x + 17, y + 17)], ink);
            c.rect(x + 13, y + 8, 2, 8, ink);
        }
        (Route::Coast, _) => {
            c.line(x + 5, y + 9, x + 12, y + 12, ink);
            c.line(x + 12, y + 12, x + 15, y + 14, ink);
            c.line(x + 15, y + 14, x + 20, y + 7, ink);
            c.line(x + 5, y + 10, x + 13, y + 13, ink);
        }
        (Route::Flotilla, 0) => {
            c.poly(
                &[
                    (x + 5, y + 12),
                    (x + 10, y + 8),
                    (x + 17, y + 9),
                    (x + 20, y + 12),
                    (x + 14, y + 15),
                    (x + 10, y + 15),
                ],
                ink,
            );
            c.poly(&[(x + 17, y + 12), (x + 22, y + 8), (x + 22, y + 16)], ink);
            c.dot(x + 9, y + 11, field);
        }
        (Route::Flotilla, 1) => {
            c.disc(x + 14, y + 11, 6, ink);
            c.disc(x + 17, y + 8, 5, field);
            star(c, x + 20, y + 6, 1, ink);
        }
        (Route::Flotilla, _) => {
            c.poly(
                &[
                    (x + 5, y + 14),
                    (x + 22, y + 14),
                    (x + 18, y + 18),
                    (x + 8, y + 18),
                ],
                ink,
            );
            c.rect(x + 8, y + 10, 12, 3, ink);
            c.rect(x + 11, y + 7, 6, 3, ink);
        }
        (Route::Aurora, 0) => {
            c.line(x + 8, y + 17, x + 18, y + 5, ink);
            for k in 0..3 {
                c.line(
                    x + 10 + 2 * k,
                    y + 14 - 3 * k,
                    x + 7 + 2 * k,
                    y + 11 - 3 * k,
                    ink,
                );
                c.line(
                    x + 11 + 2 * k,
                    y + 14 - 3 * k,
                    x + 16 + 2 * k,
                    y + 13 - 3 * k,
                    ink,
                );
            }
        }
        (Route::Aurora, 1) => {
            for (a, b) in [(7, 10), (12, 8), (15, 13), (19, 7), (20, 15)] {
                star(c, x + a, y + b, 1, ink);
            }
            c.line(x + 7, y + 10, x + 12, y + 8, ink);
            c.line(x + 12, y + 8, x + 15, y + 13, ink);
        }
        (Route::Aurora, _) => {
            star(c, x + 14, y + 11, 5, ink);
            c.line(x + 12, y + 9, x + 16, y + 13, ink);
            c.line(x + 12, y + 13, x + 16, y + 9, ink);
        }
    }
    if gold {
        star(c, x + 24, y + 3, 3, AMBER);
        c.dot(x + 24, y + 3, PEARL);
    }
}
fn title(g: &Game, c: &mut Canvas) {
    let t = (g.tick % 100000) as i32;
    horizon(c, g.route, 0, t);
    let y = match g.route {
        Route::Coast => 198,
        Route::Flotilla => 215,
        Route::Aurora => 175,
    };
    target_art(c, g.route, 337, y, 0, false, t);
    bird(c, 376, 101 + triangle(t / 3, 26) / 3, 3, t / 2);
    caption(c, 21, 21, "COASTAL AIRMAIL");
    c.text(22, 44, "NIGHT", 5, INK);
    c.text(21, 42, "NIGHT", 5, PEARL);
    c.text(22, 83, "POST", 5, INK);
    c.text(21, 81, "POST", 5, AMBER);
    panel(c, 20, 129, 243, 121);
    c.text(32, 140, "SELECT A MAIL RUN", 1, JADE);
    c.text(35, 155, g.route.title(), 2, PEARL);
    c.text(32, 178, g.route.description(), 1, MIST);
    for i in 0..3 {
        let bit = g.route.bit(i);
        postmark(
            c,
            32 + i as i32 * 34,
            195,
            g.route,
            i,
            g.album & bit != 0,
            g.album & (bit << 9) != 0,
        );
    }
    let mut album = Label::new();
    let _ = write!(album, "ALBUM {}/9", (g.album & 511).count_ones());
    c.text(144, 200, album.value(), 1, AMBER);
    c.text(144, 212, "CENTER FOR GOLD", 1, MIST);
    c.text(32, 233, "LEFT / RIGHT   CHANGE ROUTE", 1, PEARL);
    bottom(c, "SPACE  TAKE FLIGHT", "M  RETURN MAIL");
}
fn flight(g: &Game, c: &mut Canvas) {
    let t = (g.tick % 100000) as i32;
    horizon(c, g.route, g.distance, t);
    for (x, y) in g.clouds() {
        cloud(c, x, y, t);
    }
    for i in 0..3 {
        destination(c, g, i, t);
    }
    if g.braking {
        let mut trace = g.drop_origin();
        for n in 1..=42 {
            trace.advance(g.scroll_speed());
            if trace.y > 246 {
                break;
            }
            if n % 3 == 0 {
                c.rect(trace.x, trace.y, 2, 1, JADE);
            }
        }
        for k in 0..3 {
            let xx = g.x - 32 - k * 6;
            let yy = g.y + 3 + (k % 2) * 3;
            c.line(xx, yy, xx - 3, yy + 1, 0x9ba7af);
        }
    }
    if let Some(letter) = g.letter {
        envelope(c, letter.x, letter.y, 1);
    }
    bird(c, g.x, g.y, 1, t);
    if g.gust > 34 && g.gust % 6 < 3 {
        for (x, y) in [(-32, -4), (-38, 2), (25, 3)] {
            c.line(g.x + x, g.y + y, g.x + x + 5, g.y + y - 2, PEARL);
        }
    }
    c.rect(0, 0, 512, 23, INK);
    c.rect(0, 22, 512, 1, 0x776680);
    c.text(12, 8, g.route.title(), 1, PEARL);
    let mut score = Label::new();
    let _ = write!(score, "MAIL {}/3", g.count());
    c.text(123, 8, score.value(), 1, AMBER);
    let mut lap = Label::new();
    let _ = write!(lap, "LAP {}", g.laps);
    c.text(189, 8, lap.value(), 1, MIST);
    c.line(285, 12, 486, 12, 0x605972);
    for (i, station) in g.route.berths().iter().enumerate() {
        let x = 285 + station.0 * 201 / g.route.length();
        c.rect(
            x - 1,
            9,
            3,
            7,
            if g.delivered & (1 << i) != 0 {
                JADE
            } else {
                AMBER
            },
        );
    }
    let marker = 285 + (g.distance + g.x).rem_euclid(g.route.length()) * 201 / g.route.length();
    c.poly(&[(marker - 3, 5), (marker + 3, 5), (marker, 9)], PEARL);
    if g.celebration > 0 {
        if let Notice::Landed(i) = g.notice {
            let a = 38 - g.celebration as i32;
            let (x, y) = (g.post_x(i), g.post_y(i));
            for (vx, vy) in [(-4, -3), (-3, -5), (0, -6), (2, -5), (4, -3)] {
                let dx = x + vx * a / 3;
                let dy = y - 13 + vy * a / 3 + a * a / 65;
                c.rect(dx, dy, 2, 2, if vx % 2 == 0 { PEARL } else { JADE });
            }
        }
    }
    let next = (0..3)
        .filter(|&i| g.delivered & (1 << i) == 0)
        .min_by_key(|&i| (g.post_x(i) - g.x + 24).rem_euclid(g.route.length()))
        .unwrap_or(0);
    let mut status = Label::new();
    if g.notice_time > 0 {
        match g.notice {
            Notice::Landed(i) => {
                let _ = write!(
                    status,
                    "{}   {}",
                    if g.centered & (1 << i) != 0 {
                        "GOLD POSTMARK"
                    } else {
                        "DELIVERED"
                    },
                    g.route.names()[i]
                );
            }
            Notice::Gust => {
                let _ = status.write_str("STORM GUST!  STEER UP");
            }
            Notice::Miss => {
                let _ = status.write_str("MISSED.  ANOTHER LETTER IS READY");
            }
            Notice::Again => {
                let _ = status.write_str("ANOTHER LAP.  THE LIGHTS ARE STILL ON");
            }
            _ => {}
        }
    }
    if status.len == 0 {
        if g.flight_ticks < 125 {
            let _ = status.write_str("FLY ABOVE A NET. DROP ON APPROACH.");
        } else {
            let _ = write!(status, "NEXT  {}", g.route.names()[next]);
        }
    }
    bottom(
        c,
        status.value(),
        if g.notice_time > 0 && matches!(g.notice, Notice::Landed(_)) {
            "M  READ REPLY"
        } else {
            "X  AIRBRAKE   SPACE  DROP"
        },
    );
}
pub fn draw(g: &Game, pixels: &mut Pixels) {
    let mut c = Canvas(pixels);
    match g.scene {
        Scene::Mail => crate::postcard::reader(&mut c, g),
        Scene::Title => title(g, &mut c),
        Scene::Flying => flight(g, &mut c),
        Scene::Paused => {
            flight(g, &mut c);
            panel(&mut c, 160, 100, 193, 80);
            c.text(177, 118, "ON THE PERCH", 2, PEARL);
            c.text(178, 149, "P      KEEP FLYING", 1, JADE);
            c.text(178, 163, "SPACE  ROUTE CHART", 1, MIST);
            bottom(&mut c, "M  RETURN MAIL", "P  KEEP FLYING");
        }
        Scene::Arrived => {
            let t = (g.tick % 100000) as i32;
            horizon(&mut c, g.route, 350, t);
            target_art(&mut c, g.route, 470, g.post_y(2), 2, true, t);
            panel(&mut c, 76, 37, 360, 199);
            c.text(92, 53, g.route.title(), 1, JADE);
            stamp(&mut c, 398, 49);
            c.text(92, 74, "MAIL CALL!", 3, PEARL);
            c.text(
                92,
                109,
                match g.route {
                    Route::Coast => "ALL THREE LIGHTS ARE ON.",
                    Route::Flotilla => "THE LAST FERRY HAS HER MAIL.",
                    Route::Aurora => "POST RECEIVED ABOVE THE CLOUDS.",
                },
                1,
                AMBER,
            );
            for (i, name) in g.route.names().iter().enumerate() {
                let xx = 92 + 110 * i as i32;
                postmark(
                    &mut c,
                    xx,
                    132,
                    g.route,
                    i,
                    true,
                    g.centered & (1 << i) != 0,
                );
                c.text(xx, 165, name, 1, PEARL);
            }
            let mut stats = Label::new();
            let _ = write!(
                stats,
                "{} DROPS / {} LAP{} / {}S",
                g.drops,
                g.laps,
                if g.laps == 1 { "" } else { "S" },
                g.flight_ticks / 40
            );
            c.text(92, 189, stats.value(), 1, MIST);
            let mut album = Label::new();
            let _ = write!(
                album,
                "POSTMARKS {}/9     GOLD {}/9",
                (g.album & 511).count_ones(),
                (g.album >> 9).count_ones()
            );
            c.text(92, 210, album.value(), 1, JADE);
            bottom(&mut c, "M  READ REPLIES   L/R  ROUTE", "SPACE  FLY AGAIN");
        }
    }
}
