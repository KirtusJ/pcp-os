#![no_std]
#![no_main]
mod arena;
mod link;
mod pavilion;
use flux_rink::{Match, Settings};
use heliograph::raster::Cached;
use link::{Link, Role};
use pcp_game_guest::{
    digits, exit, ink::Frame, load, log, save, sleep_us, time_us, Clock, Display,
};
const W: i32 = 640;
const H: i32 = 360;
const N: usize = (W * H) as usize;
static mut PIX: [u32; N] = [0; N];
static mut BG: [u32; N] = [0; N];
static mut DEPTH: [i32; N] = [0; N];
const PATH: &[u8] = b"flux-rink.cfg\0";
const TEMP: &[u8] = b"flux-rink.tmp\0";
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Title,
    Play,
    Options,
}
fn choice(f: &mut Frame, x: i32, y: i32, label: &str, value: &str, selected: bool) {
    if selected {
        f.rect(x - 5, y - 4, 226, 15, 0x30585f);
        f.rect(x - 5, y - 4, 3, 15, 0xffb967);
    }
    let color = if selected { 0xffdfa5 } else { 0xb9cbd0 };
    f.text(x + 3, y, label, 1, color);
    f.text(x + 132, y, value, 1, 0xe1eee3);
}
fn keys(d: &Display, map: [usize; 4]) -> u8 {
    let mut v = 0;
    for (i, code) in map.iter().enumerate() {
        v |= (d.held[*code] as u8) << i;
    }
    v
}
pcp_game_guest::entry!(flux_main);
#[no_mangle]
pub extern "C" fn flux_main() -> ! {
    let mut d = Display::new(W as usize, H as usize);
    let mut cfg = Settings::default();
    let mut bytes = [0; 5];
    if load(PATH, &mut bytes) {
        if let Some(s) = Settings::decode(bytes) {
            cfg = s;
            log(b"RINK settings restored\n");
        }
    }
    let (p, bg, z) = unsafe {
        (
            &mut *(&raw mut PIX),
            &mut *(&raw mut BG),
            &mut *(&raw mut DEPTH),
        )
    };
    arena::background(bg, &cfg);
    let mut settings = cfg;
    let mut screen = Screen::Title;
    let mut back = screen;
    let mut cursor = 0usize;
    let mut menu = 0usize;
    let mut note = "";
    let mut game = Match::default();
    let mut net = Link::new();
    let mut clock = Clock::new();
    let mut cache = [Cached::ZERO; 280];
    let mut tick = 0u32;
    let (mut stamp, mut frames, mut fps) = (time_us(), 0u32, 0u32);
    let mut bench = 0;
    let mut moved = [false; 2];
    log(b"RINK ready\n");
    loop {
        let begin = time_us();
        d.poll();
        net.poll();
        match screen {
            Screen::Title => {
                if d.pressed[1] {
                    exit(0);
                }
                if d.pressed[72] {
                    menu = (menu + 3) % 4;
                }
                if d.pressed[80] {
                    menu = (menu + 1) % 4;
                }
                if d.pressed[24] {
                    menu = 3;
                }
                if d.pressed[28] || d.pressed[24] {
                    if menu == 3 {
                        settings = cfg;
                        cursor = 0;
                        back = screen;
                        screen = Screen::Options;
                        note = "";
                        log(b"RINK settings\n");
                    } else {
                        let role = [Role::Local, Role::Host, Role::Peer][menu];
                        if role == Role::Local || net.start(role) {
                            game = Match::default();
                            moved = [false; 2];
                            screen = Screen::Play;
                            note = "";
                            log(match role {
                                Role::Local => b"RINK local\n",
                                Role::Host => b"RINK host\n",
                                Role::Peer => b"RINK client\n",
                            });
                        } else {
                            note = "LINK NEEDS THE CABLE KERNEL AND GRANT";
                            log(b"RINK cable unavailable\n");
                        }
                    }
                }
            }
            Screen::Play => {
                if d.pressed[1] {
                    net.stop();
                    screen = Screen::Title;
                    note = "";
                } else if d.pressed[24] {
                    settings = cfg;
                    cursor = 0;
                    back = screen;
                    screen = Screen::Options;
                    log(b"RINK settings\n");
                } else if d.pressed[28] && net.role != Role::Peer && game.score.contains(&5) {
                    game = Match::default();
                }
                if d.pressed[48] {
                    bench = 121;
                    log(b"RINK timing start\n");
                }
            }
            Screen::Options => {
                if d.pressed[1] {
                    screen = back;
                    note = "";
                } else if d.pressed[72] {
                    cursor = (cursor + 5) % 6;
                } else if d.pressed[80] {
                    cursor = (cursor + 1) % 6;
                } else if d.pressed[75] || d.pressed[77] || d.pressed[28] {
                    let delta = if d.pressed[75] { 2 } else { 1 };
                    match cursor {
                        0 => settings.fps = !settings.fps,
                        1 => settings.cap = (settings.cap + delta) % 3,
                        2 => settings.detail = !settings.detail,
                        3 => settings.wide = !settings.wide,
                        4 if d.pressed[28] => {
                            settings = Settings::default();
                            note = "DEFAULTS SELECTED";
                        }
                        5 if d.pressed[28] => {
                            if save(TEMP, PATH, &settings.encode()) {
                                cfg = settings;
                                arena::background(bg, &cfg);
                                screen = back;
                                note = "SETTINGS SAVED";
                                log(b"RINK settings saved\n");
                            } else {
                                note = "SAVE FAILED / TRY AGAIN";
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let a = keys(&d, [31, 17, 32, 30]);
        let b = keys(&d, [80, 72, 77, 75]);
        let play = screen == Screen::Play;
        let linked = net.role != Role::Local;
        for _ in 0..clock.steps() {
            tick = tick.wrapping_add(1);
            if play || linked {
                let active = if play { a } else { 0 };
                let old = game.player;
                if net.role == Role::Local {
                    game.step([a, b]);
                } else if net.role == Role::Host && net.heard {
                    game.step([active, net.remote]);
                }
                net.tick(&game, active, tick);
                if play {
                    for i in 0..2 {
                        if !moved[i] && game.player[i] != old[i] {
                            moved[i] = true;
                            log(if i == 0 {
                                b"RINK p1 moved\n"
                            } else {
                                b"RINK p2 moved\n"
                            });
                        }
                    }
                }
            }
        }
        let view = if net.role == Role::Peer {
            net.view()
        } else {
            game
        };
        p.copy_from_slice(bg);
        z.fill(0);
        arena::actors(p, z, &mut cache, &view, &cfg);
        let f = &mut Frame {
            px: p,
            pitch: W as usize,
        };
        let mut buf = [0; 10];
        match screen {
            Screen::Title => {
                f.rect(16, 16, 241, 171, 0x142b3a);
                f.rect(16, 16, 4, 171, 0xf4b66a);
                f.text(29, 28, "FLUX RINK", 3, 0xffdf9f);
                f.text(30, 57, "TWO MALLETS / FIRST TO FIVE", 1, 0x9fd5ce);
                for (i, name) in [
                    "LOCAL DUEL",
                    "HOST CABLE DUEL",
                    "JOIN CABLE DUEL",
                    "SETTINGS",
                ]
                .iter()
                .enumerate()
                {
                    choice(f, 29, 81 + i as i32 * 20, name, "", menu == i);
                }
                f.text(30, 168, "UP DOWN  SELECT   ENTER  OPEN", 1, 0xbed0d1);
                if !note.is_empty() {
                    f.rect(0, 201, W, 23, 0x102532);
                    f.text(12, 208, note, 1, 0xffc67e);
                }
            }
            Screen::Options => {
                f.rect(16, 9, 255, 187, 0x142b3a);
                f.text(30, 20, "SETTINGS", 2, 0xffdfa5);
                for (i, (name, value)) in [
                    ("FPS COUNTER", if settings.fps { "ON" } else { "OFF" }),
                    (
                        "FRAME LIMIT",
                        ["30 FPS", "60 FPS", "UNCAPPED"][settings.cap as usize],
                    ),
                    (
                        "ICE DETAIL",
                        if settings.detail { "ETCHED" } else { "PLAIN" },
                    ),
                    ("CAMERA", if settings.wide { "WIDE" } else { "STANDARD" }),
                    ("RESTORE DEFAULTS", ""),
                    ("APPLY AND SAVE", ""),
                ]
                .iter()
                .enumerate()
                {
                    choice(f, 30, 47 + i as i32 * 20, name, value, cursor == i);
                }
                f.text(30, 177, "LEFT / RIGHT  EDIT   ESC  CANCEL", 1, 0xb5cfca);
                if !note.is_empty() {
                    f.text(18, 204, note, 1, 0xffc67e);
                }
            }
            Screen::Play => {
                f.rect(195, 0, 250, 34, 0x111e2c);
                f.rect(195, 32, 116, 2, 0xc88c54);
                f.rect(329, 32, 116, 2, 0x65c4be);
                f.text(204, 6, "AMBER", 1, 0xf4c085);
                f.text(205, 18, "WEST", 1, 0xa7aaa8);
                f.text(261, 9, digits(view.score[0] as u32, &mut buf), 2, 0xffdea4);
                f.rect(289, 3, 59, 27, 0x253342);
                f.text(306, 6, "FLUX", 1, 0xe8e3cd);
                f.text(294, 18, "FIRST TO 5", 1, 0xa0b5b5);
                f.text(358, 9, digits(view.score[1] as u32, &mut buf), 2, 0xa1f6e1);
                f.text(395, 6, "JADE", 1, 0xa1ddd0);
                f.text(395, 18, "EAST", 1, 0xa2bec1);
                f.rect(10, 8, 96, 15, 0x142635);
                f.rect(14, 12, 3, 6, 0xe79862);
                f.text(23, 12, "NORTH DOCK", 1, 0xc7cfca);
                if cfg.fps {
                    f.rect(W - 67, 7, 59, 16, 0x132b37);
                    f.text(W - 61, 12, "FPS", 1, 0xaae0d2);
                    f.text(W - 35, 12, digits(fps, &mut buf), 1, 0xffffff);
                }
                if linked && !net.heard {
                    f.rect(74, 90, 236, 28, 0x142b39);
                    f.text(87, 100, "WAITING FOR THE OTHER MACHINE", 1, 0xffdf9f);
                }
                if view.score.contains(&5) {
                    f.rect(83, 86, 218, 38, 0x112835);
                    f.text(
                        95,
                        93,
                        if view.score[0] == 5 {
                            "AMBER WINS"
                        } else {
                            "JADE WINS"
                        },
                        2,
                        0xffdea1,
                    );
                    f.text(
                        96,
                        114,
                        if net.role == Role::Peer {
                            "HOST STARTS NEXT MATCH"
                        } else {
                            "ENTER  REMATCH"
                        },
                        1,
                        0xb3d7d5,
                    );
                }
                f.rect(0, H - 13, W, 13, 0x102532);
                f.text(
                    8,
                    H - 10,
                    match net.role {
                        Role::Local => "AMBER WASD / JADE ARROWS",
                        Role::Host => "HOST / AMBER WASD",
                        Role::Peer => "PEER / JADE WASD",
                    },
                    1,
                    0xb6d2cb,
                );
                f.text(W - 119, H - 10, "O OPTIONS  ESC MENU", 1, 0xe8c796);
            }
        }
        d.show(p);
        frames += 1;
        let now = time_us();
        if now - stamp >= 500000 {
            fps = (frames as u64 * 1_000_000 / (now - stamp)) as u32;
            stamp = now;
            frames = 0;
        }
        if bench > 0 {
            log(b"F\n");
            bench -= 1;
            if bench == 0 {
                log(b"RINK timing done\n");
            }
        }
        sleep_us(cfg.interval().saturating_sub(time_us() - begin));
    }
}
