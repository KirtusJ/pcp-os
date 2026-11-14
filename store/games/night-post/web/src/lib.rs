#![no_std]
use night_post::{audio, Game, Pixels, Sound, H, W};

static mut GAME: Game = Game::new();
static mut FRAME: Pixels = [0; W * H];
static mut PCM: [i16; 24000] = [0; 24000];
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
#[no_mangle]
pub extern "C" fn frame() -> *mut u32 {
    (&raw mut FRAME).cast()
}
#[no_mangle]
pub extern "C" fn pcm() -> *mut i16 {
    (&raw mut PCM).cast()
}
#[no_mangle]
pub extern "C" fn render() {
    let frame = unsafe { &mut *(&raw mut FRAME) };
    unsafe { (&*(&raw const GAME)).draw(frame) };
    // The artist works in RGB. Little-endian ImageData wants RGBA.
    for p in frame {
        let c = *p;
        *p = 0xff00_0000 | ((c & 255) << 16) | (c & 0xff00) | ((c >> 16) & 255);
    }
}
#[no_mangle]
pub extern "C" fn step(keys: u8) -> usize {
    let effect = unsafe { (&mut *(&raw mut GAME)).step(keys) };
    if effect == Sound::Quiet {
        return 0;
    }
    let buf = unsafe { &mut *(&raw mut PCM) };
    let len = audio::length(effect).min(buf.len());
    for (i, sample) in buf[..len].iter_mut().enumerate() {
        *sample = audio::sample(effect, i);
    }
    len
}

#[no_mangle]
pub extern "C" fn album() -> u32 {
    unsafe { (&*(&raw const GAME)).album }
}
#[no_mangle]
pub extern "C" fn restore_album(album: u32) {
    unsafe {
        (&mut *(&raw mut GAME)).restore_album(album);
    }
}

/// Zero while flying or at the chart. Nonzero values identify both card and side.
#[no_mangle]
pub extern "C" fn mail_view() -> usize {
    use night_post::{mail::Side, Scene};
    let game = unsafe { &*(&raw const GAME) };
    if game.scene == Scene::Mail {
        1 + 2 * game.mailbox.card + usize::from(game.mailbox.side == Side::Message)
    } else {
        0
    }
}

/// One static UTF-8 line, exposed so the browser can offer native, selectable text.
fn mail_line(which: usize) -> Option<&'static str> {
    let game = unsafe { &*(&raw const GAME) };
    if game.scene != night_post::Scene::Mail {
        return None;
    }
    let card = night_post::mail::Card::for_index(game.mailbox.card);
    match which {
        0 => Some(card.title),
        1 => Some(card.place),
        2 => Some(card.sender),
        n => card.lines.get(n - 3).copied(),
    }
}
#[no_mangle]
pub extern "C" fn mail_line_ptr(which: usize) -> *const u8 {
    mail_line(which).map_or(core::ptr::null(), str::as_ptr)
}
#[no_mangle]
pub extern "C" fn mail_line_len(which: usize) -> usize {
    mail_line(which).map_or(0, str::len)
}
