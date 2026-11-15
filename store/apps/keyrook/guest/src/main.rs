#![no_std]
#![no_main]

mod profile;
mod screen;

use keyrook_core::{fixture, run_episode, Episode};
use pcp_game_guest::{exit, log, sleep_us, Display};
use profile::{Profile, WIDTHS};
use screen::{H, W};

static mut PIXELS: [u32; W * H] = [0; W * H];

fn selected(p: &Profile, mode: u8) -> u8 {
    if mode == 2 { p.cells[p.index()].choose() } else { mode }
}

fn run(p: &Profile, mode: u8) -> Episode {
    run_episode(WIDTHS[p.width], p.seed, p.depth, selected(p, mode))
}

fn metric(name: &[u8], n: u64) {
    log(name);
    log(screen::decimal(n, &mut [0; 20]).as_bytes());
}

fn report(p: &Profile, e: &Episode) {
    log(b"KEYROOK RESULT");
    metric(b" bits=", WIDTHS[p.width] as u64);
    metric(b" depth=", p.depth as u64);
    metric(b" seed=", p.seed as u64);
    metric(b" method=", e.strategy as u64);
    metric(b" verified=", u64::from(e.verified));
    metric(b" work=", e.work);
    metric(b" branches=", e.branches as u64);
    log(b"\n");
}

pcp_game_guest::entry!(keyrook_main);
#[no_mangle]
pub extern "C" fn keyrook_main() -> ! {
    let mut display = Display::new(W, H);
    let mut restored = Profile::read();
    let mut saved = if restored.is_some() { "PROFILE LOADED" } else { "NO SAVED TRIALS" };
    let mut p = restored.take().unwrap_or_default();
    let mut mode = 2;
    let mut last = run(&p, mode);
    let px = unsafe { &mut *(&raw mut PIXELS) };
    let mut dirty = true;
    let mut first = true;
    loop {
        if dirty {
            let (public, _) = fixture(WIDTHS[p.width], p.seed).unwrap();
            screen::draw(px, &p, &public, &last, mode, saved);
            display.show(px);
            report(&p, &last);
            if first {
                log(b"KEYROOK READY ");
                log(saved.as_bytes());
                log(b"\n");
                first = false;
            }
            dirty = false;
        }
        display.poll();
        let keys = &display.pressed;
        if keys[1] { exit(0); }
        if keys[20] { // T runs both methods on the same new public fixture.
            p.next_seed();
            let bits = WIDTHS[p.width];
            let a = run_episode(bits, p.seed, p.depth, 0);
            let b = run_episode(bits, p.seed, p.depth, 1);
            let cell = p.index();
            let memory = &mut p.cells[cell];
            memory.observe(&a);
            memory.observe(&b);
            log(b"KEYROOK TRAIN");
            metric(b" exhaustive=", a.work);
            metric(b" meet=", b.work);
            metric(b" choice=", u64::from(memory.choose()));
            metric(b" exhaustive_trials=", u64::from(memory.runs[0]));
            metric(b" meet_trials=", u64::from(memory.runs[1]));
            saved = p.persist();
            log(b" ");
            log(saved.as_bytes());
            log(b"\n");
            last = if selected(&p, mode) == 0 { a } else { b };
            dirty = true;
        } else if keys[28] || keys[48] || keys[32] || keys[50] { // Enter, B, D, M
            if keys[28] { p.next_seed(); }
            if keys[48] { p.width = (p.width + 1) % WIDTHS.len(); }
            if keys[32] { p.depth = (p.depth + 1) % 4; }
            if keys[50] { mode = (mode + 1) % 3; }
            last = run(&p, mode);
            dirty = true;
        }
        sleep_us(10_000);
    }
}
