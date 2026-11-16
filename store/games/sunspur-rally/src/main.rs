#![no_std]
#![no_main]
mod scene;
use pcp_game_guest::{exit, log, sleep_us, time_us, Clock, Display};
use sunspur_rally::{Input, Race};
const W: i32 = 384;
const H: i32 = 216;
static mut PIX: [u32; (W * H) as usize] = [0; (W * H) as usize];
static mut SKY: [u32; (W * H) as usize] = [0; (W * H) as usize];
pcp_game_guest::entry!(sunspur_main);
#[no_mangle]
pub extern "C" fn sunspur_main() -> ! {
    let mut d = Display::new(W as usize, H as usize);
    let (p, s) = unsafe { (&mut *(&raw mut PIX), &mut *(&raw mut SKY)) };
    scene::sky(s);
    let mut race = Race::default();
    let mut cover = true;
    let mut demo = false;
    let mut clock = Clock::new();
    log(b"SUNSPUR ready\n");
    loop {
        let now = time_us();
        d.poll();
        if d.pressed[1] {
            exit(0);
        }
        if d.pressed[60] || d.pressed[19] || d.pressed[28] && (cover || race.finished) {
            demo = d.pressed[60];
            cover = false;
            race = Race::default();
            log(b"SUNSPUR start\n");
        }
        let input = Input {
            gas: d.held[17] || d.held[72],
            brake: d.held[31] || d.held[80],
            steer: (d.held[32] || d.held[77]) as i32 - (d.held[30] || d.held[75]) as i32,
        };
        let n = clock.steps();
        for _ in 0..n {
            if !cover && !race.finished {
                race.step(if demo { race.demo() } else { input });
                if race.finished {
                    log(b"SUNSPUR finish\n");
                }
            }
        }
        scene::draw(p, s, &race, input.brake, cover, demo);
        d.show(p);
        sleep_us(16667u64.saturating_sub(time_us() - now));
    }
}
