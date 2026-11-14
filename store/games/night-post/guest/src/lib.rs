#![no_std]
use core::{
    ffi::{c_char, c_int, c_long, c_void},
    ptr, slice,
};
use night_post::{
    audio, Game, Pixels, Scene, Sound, ACTION, BRAKE, DOWN, H, LEFT, PAUSE, RIGHT, STEP_MS, UP, W,
};

#[repr(C)]
#[derive(Default)]
struct Display {
    width: u32,
    height: u32,
    pitch: u32,
    pages: u32,
    shown: u32,
    changeable: u32,
}
#[repr(C)]
#[derive(Default)]
struct AudioInfo {
    rate: u32,
    channels: u32,
    bits: u32,
    room: u32,
    waiting: u32,
}
#[repr(C)]
#[derive(Default)]
struct Timespec {
    sec: c_long,
    nsec: c_long,
}

extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn ioctl(fd: c_int, request: usize, ...) -> c_int;
    fn mmap(
        address: *mut c_void,
        size: usize,
        protection: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn read(fd: c_int, p: *mut c_void, size: usize) -> isize;
    fn write(fd: c_int, p: *const c_void, size: usize) -> isize;
    fn close(fd: c_int) -> c_int;
    fn rename(old: *const c_char, new: *const c_char) -> c_int;
    fn clock_gettime(clock: c_int, t: *mut Timespec) -> c_int;
    fn nanosleep(req: *const Timespec, remaining: *mut Timespec) -> c_int;
    fn _exit(code: c_int) -> !;
}
fn log(line: &[u8]) {
    unsafe {
        write(1, line.as_ptr().cast(), line.len());
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    log(b"Night Post: internal error\n");
    unsafe { _exit(3) }
}
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}
fn ms() -> u64 {
    let mut ts = Timespec::default();
    unsafe {
        clock_gettime(1, &mut ts);
    }
    (ts.sec as u64) * 1000 + ts.nsec as u64 / 1_000_000
}
const BOOK: &[u8] = b"nightpost.stamps\0";
const TEMP_BOOK: &[u8] = b"nightpost.new\0";

fn load_album() -> u32 {
    let fd = unsafe { open(BOOK.as_ptr().cast(), 0) };
    if fd < 0 {
        return 0;
    }
    let mut buf = [0u8; 9];
    let n = unsafe { read(fd, buf.as_mut_ptr().cast(), buf.len()) };
    unsafe {
        close(fd);
    }
    if n == 8 && &buf[..4] == b"NP02" {
        u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]])
    } else {
        0
    }
}
fn save_album(album: u32) -> bool {
    let fd = unsafe { open(TEMP_BOOK.as_ptr().cast(), 1 | 64 | 512, 0o600) };
    if fd < 0 {
        return false;
    }
    let mut buf = *b"NP02....";
    buf[4..].copy_from_slice(&album.to_le_bytes());
    let n = unsafe { write(fd, buf.as_ptr().cast(), buf.len()) };
    let closed = unsafe { close(fd) };
    n == 8 && closed == 0 && unsafe { rename(TEMP_BOOK.as_ptr().cast(), BOOK.as_ptr().cast()) } == 0
}

struct Speaker {
    fd: i32,
    effect: Sound,
    at: usize,
}
impl Speaker {
    fn pump(&mut self) {
        if self.fd < 0 || self.at >= audio::length(self.effect) {
            return;
        }
        let mut info = AudioInfo::default();
        if unsafe { ioctl(self.fd, 0x5011, &mut info) } != 0
            || info.rate != 48_000
            || info.channels != 2
        {
            return;
        }
        let mut samples = [0i16; 2400];
        let frames = (info.room as usize / 4)
            .min(samples.len() / 2)
            .min(audio::length(self.effect) - self.at);
        for i in 0..frames {
            let v = audio::sample(self.effect, self.at + i);
            samples[2 * i] = v;
            samples[2 * i + 1] = v;
        }
        if frames > 0 {
            let n = unsafe { write(self.fd, samples.as_ptr().cast(), frames * 4) };
            if n > 0 {
                self.at += n as usize / 4;
            }
        }
    }
}
// The kernel supplies these paths and ioctls to normal application processes.
// Only the adapter below touches device memory. The game itself uses safe Rust.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    let fb = unsafe { open(b"/dev/fb0\0".as_ptr().cast(), 2) };
    let kb = unsafe { open(b"/dev/kbd\0".as_ptr().cast(), 0x800) };
    let mut d = Display {
        width: W as u32,
        height: H as u32,
        ..Display::default()
    };
    if kb < 0
        || fb < 0
        || unsafe { ioctl(fb, 0x5002, &mut d) } != 0
        || d.width != W as u32
        || d.height != H as u32
        || d.pages == 0
        || d.pitch < 4 * W as u32
    {
        log(b"Night Post needs a 512x288 display and keyboard\n");
        return 2;
    }
    let pitch = d.pitch as usize / 4;
    let page = pitch * H;
    let mapped = unsafe { mmap(ptr::null_mut(), page * d.pages as usize * 4, 3, 1, fb, 0) };
    if mapped as isize == -1 {
        log(b"Night Post: framebuffer mapping failed\n");
        return 2;
    }
    static mut FRAME: Pixels = [0; W * H];
    let pixels = unsafe { &mut *(&raw mut FRAME) };
    let mut keys = [false; 128];
    let mut game = Game::new();
    game.restore_album(load_album());
    if game.album != 0 {
        log(b"Night Post: postmark album restored\n");
    }
    let mut speaker = Speaker {
        fd: unsafe { open(b"/dev/audio\0".as_ptr().cast(), 1) },
        effect: Sound::Quiet,
        at: 0,
    };
    let mut next = ms();
    log(b"Night Post: lighthouse route ready\n");
    loop {
        let mut taps = 0;
        let bind = |code: usize| match code {
            17 | 72 => UP,
            31 | 80 => DOWN,
            30 | 75 => LEFT,
            32 | 77 => RIGHT,
            57 | 28 => ACTION,
            25 => PAUSE,
            50 => night_post::MAIL,
            45 | 42 | 54 => BRAKE,
            _ => 0,
        };
        loop {
            let mut event = [0u8; 2];
            if unsafe { read(kb, event.as_mut_ptr().cast(), 2) } != 2 {
                break;
            }
            let i = (event[0] & 127) as usize;
            let down = event[1] & 1 == 0;
            if i == 1 && down {
                log(b"Night Post: back to the launcher\n");
                return 0;
            }
            if down && !keys[i] {
                taps |= bind(i);
            }
            keys[i] = down;
        }
        let mut mask = taps;
        for (i, down) in keys.iter().enumerate() {
            if *down {
                mask |= bind(i);
            }
        }
        let old = game.scene;
        let old_album = game.album;
        let effect = game.step(mask);
        if game.album != old_album {
            if save_album(game.album) {
                log(b"Night Post: postmark album saved\n");
            } else {
                log(b"Night Post: album retained in game memory only\n");
            }
        }
        if game.scene == Scene::Flying && old == Scene::Title {
            log(game.route.title().as_bytes());
            log(b"\n");
        }
        if game.scene != old {
            log(match game.scene {
                Scene::Mail => b"Night Post: reading return mail\n",
                _ if old == Scene::Mail => b"Night Post: postbox closed\n",
                Scene::Title => b"Title\n",
                Scene::Flying => b"Night Post: in flight\n",
                Scene::Paused => b"Night Post: perched\n",
                Scene::Arrived => b"Night Post: all three delivered\n",
            });
        }
        if effect == Sound::Drop {
            log(b"Night Post: letter away\n");
        }
        if effect == Sound::Delivered {
            log(b"Night Post: station lit\n");
        }
        if effect != Sound::Quiet {
            speaker.effect = effect;
            speaker.at = 0;
        }
        speaker.pump();
        game.draw(pixels);
        let drawing = if d.pages > 1 { 1 - d.shown.min(1) } else { 0 };
        let surface = unsafe {
            slice::from_raw_parts_mut((mapped as *mut u32).add(page * drawing as usize), page)
        };
        for (src, dst) in pixels.chunks_exact(W).zip(surface.chunks_exact_mut(pitch)) {
            dst[..W].copy_from_slice(src);
        }
        if d.pages > 1 {
            if unsafe { ioctl(fb, 0x5003, drawing as usize) } != 0 {
                log(b"Night Post: page flip failed\n");
                return 2;
            }
            d.shown = drawing;
        }
        next += STEP_MS as u64;
        let now = ms();
        if next > now {
            let delay = Timespec {
                sec: 0,
                nsec: ((next - now) * 1_000_000) as c_long,
            };
            unsafe {
                nanosleep(&delay, ptr::null_mut());
            }
        } else if now - next > STEP_MS as u64 {
            next = now;
        }
    }
}
