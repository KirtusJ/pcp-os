//! A program that stops in the middle of drawing (C3's probe, written again here).
//!
//! It shows one picture, all of one colour. Then it draws the top half of a second picture in
//! another colour and waits for a key; draws the bottom half and waits for a key; and only then
//! asks for the second picture to be shown. What is on the screen at each wait says whether a
//! picture is seen being made: the wait is as long as the one watching likes, so the answer does
//! not depend on how fast the machine writes its screen.
//!
//! It asks, as Tailorbird does, to draw through the kernel where the display needs that (display
//! request 0x5004), and says on the log what it was told.
#![no_std]
#![no_main]
use pcp_game_guest::{call, entry, exit, ioctl, log, open, sleep_us};

/// The two pictures. Neither is the kernel's ground colour or black, so neither is what an
/// untouched or cleared screen holds.
const FIRST: u32 = 0x0020_6080;
const SECOND: u32 = 0x00E0_A030;

entry!(main);

fn wait_for_key(keys: isize) {
    loop {
        let mut event = [0u8; 2];
        if call(0, keys as usize, event.as_mut_ptr() as usize, 2, 0, 0, 0) == 2 {
            if event[1] & 1 == 0 {
                return;
            }
        } else {
            sleep_us(10_000);
        }
    }
}

#[no_mangle]
extern "C" fn main() -> ! {
    let screen = open(b"/dev/fb0\0", 2);
    let keys = open(b"/dev/kbd\0", 0x800);
    let mut d = [0u32; 6];
    if screen < 0 || keys < 0 || ioctl(screen, 0x5001, d.as_mut_ptr() as usize) != 0 {
        log(b"probe: no display\n");
        exit(2);
    }
    let (width, height, across, pages) = (d[0] as usize, d[1] as usize, d[2] as usize / 4, d[3] as usize);
    let own = match ioctl(screen, 0x5004, 0) {
        1 => true,
        _ => false,
    };
    log(if own { b"probe: told to draw to memory of its own\n" as &[u8] } else { b"probe: draws on the display itself\n" });
    let at = call(9, 0, across * height * pages * 4, 3, 1, screen as usize, 0);
    if at < 0 || width == 0 || across < width {
        log(b"probe: the display could not be mapped\n");
        exit(2);
    }
    // Safety: the kernel mapped that many bytes there for this program, writable.
    let picture = unsafe { core::slice::from_raw_parts_mut(at as *mut u32, across * height) };
    let show = |what: &[u8]| {
        if (own || pages > 1) && ioctl(screen, 0x5003, 0) != 0 {
            log(b"probe: the picture was refused\n");
            exit(2);
        }
        log(what);
    };
    picture.fill(FIRST);
    show(b"probe: first picture shown\n");
    picture[..across * (height / 2)].fill(SECOND);
    log(b"probe: top half of the second drawn, not shown\n");
    wait_for_key(keys);
    picture[across * (height / 2)..].fill(SECOND);
    log(b"probe: all of the second drawn, not shown\n");
    wait_for_key(keys);
    show(b"probe: second picture shown\n");
    wait_for_key(keys);
    exit(0)
}
