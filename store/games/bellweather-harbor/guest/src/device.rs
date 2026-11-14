//! PCP OS application devices and the session's file store.
use crate::ui::{Screen, H, W};
use bellweather::economy::{Economy, SAVE_LEN};
use core::{
    ffi::{c_char, c_int, c_long, c_void},
    ptr, slice,
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
struct Timespec {
    sec: c_long,
    nsec: c_long,
}

extern "C" {
    fn open(name: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buffer: *mut c_void, size: usize) -> isize;
    fn write(fd: c_int, buffer: *const c_void, size: usize) -> isize;
    fn ioctl(fd: c_int, request: usize, ...) -> c_int;
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn rename(old: *const c_char, new: *const c_char) -> c_int;
    fn clock_gettime(id: c_int, t: *mut Timespec) -> c_int;
    fn nanosleep(t: *const Timespec, remaining: *mut Timespec) -> c_int;
    pub fn _exit(code: c_int) -> !;
}
pub fn log(text: &str) {
    unsafe {
        write(1, text.as_ptr().cast(), text.len());
    }
}

pub fn clock_ms(id: i32) -> u64 {
    let mut t = Timespec::default();
    if unsafe { clock_gettime(id, &mut t) } < 0 {
        return 0;
    }
    (t.sec.max(0) as u64).saturating_mul(1000) + t.nsec.max(0) as u64 / 1_000_000
}

pub fn sleep_ms(ms: u64) {
    let t = Timespec {
        sec: (ms / 1000) as c_long,
        nsec: ((ms % 1000) * 1_000_000) as c_long,
    };
    unsafe {
        nanosleep(&t, ptr::null_mut());
    }
}

pub struct Console {
    fb: c_int,
    kb: c_int,
    g: Display,
    mapped: *mut u32,
    held: [bool; 128],
}
impl Console {
    pub fn open() -> Option<Self> {
        let fb = unsafe { open(b"/dev/fb0\0".as_ptr().cast(), 2) };
        let kb = unsafe { open(b"/dev/kbd\0".as_ptr().cast(), 0x800) };
        let mut g = Display {
            width: W as u32,
            height: H as u32,
            ..Display::default()
        };
        if fb < 0
            || kb < 0
            || unsafe { ioctl(fb, 0x5002, &mut g) } != 0
            || g.width != W as u32
            || g.height != H as u32
            || g.pages == 0
            || g.pitch < W as u32 * 4
        {
            return None;
        }
        let bytes = g.pitch as usize * H * g.pages as usize;
        let mem = unsafe { mmap(ptr::null_mut(), bytes, 3, 1, fb, 0) };
        if mem as isize == -1 {
            return None;
        }
        Some(Self {
            fb,
            kb,
            g,
            mapped: mem.cast(),
            held: [false; 128],
        })
    }
    pub fn next_key(&mut self) -> Option<usize> {
        loop {
            let mut event = [0u8; 2];
            if unsafe { read(self.kb, event.as_mut_ptr().cast(), 2) } != 2 {
                return None;
            }
            let code = usize::from(event[0] & 127);
            let down = event[1] & 1 == 0;
            let tapped = down && !self.held[code];
            self.held[code] = down;
            if tapped {
                return Some(code);
            }
        }
    }
    pub fn show(&mut self, pixels: &Screen) -> bool {
        let next = if self.g.pages > 1 {
            1 - self.g.shown.min(1)
        } else {
            0
        };
        let pitch = self.g.pitch as usize / 4;
        let page_words = pitch * H;
        // The entire device mapping was checked in open(); each row has room for W pixels.
        let page = unsafe {
            slice::from_raw_parts_mut(self.mapped.add(next as usize * page_words), page_words)
        };
        for (dst, src) in page.chunks_exact_mut(pitch).zip(pixels.chunks_exact(W)) {
            dst[..W].copy_from_slice(src);
        }
        if self.g.pages > 1 && unsafe { ioctl(self.fb, 0x5003, next as usize) } != 0 {
            return false;
        }
        self.g.shown = next;
        true
    }
}

const LEDGER: &[u8] = b"bellweather.sav\0";
const TEMP: &[u8] = b"bellweather.tmp\0";

pub fn load() -> Option<(Economy, u64)> {
    let fd = unsafe { open(LEDGER.as_ptr().cast(), 0) };
    if fd < 0 {
        return None;
    }
    let mut bytes = [0u8; SAVE_LEN + 1];
    let mut count = 0;
    while count < bytes.len() {
        let part = &mut bytes[count..];
        let received = unsafe { read(fd, part.as_mut_ptr().cast(), part.len()) };
        if received <= 0 {
            break;
        }
        count += received as usize;
    }
    unsafe {
        close(fd);
    }
    let ledger = Economy::decode(&bytes[..count], clock_ms(0) / 1000);
    if ledger.is_none() {
        log("Bellweather: unreadable ledger, opening fresh harbor\n");
    }
    ledger
}

pub fn save(game: &Economy) -> bool {
    let fd = unsafe { open(TEMP.as_ptr().cast(), 1 | 64 | 512, 0o600) };
    if fd < 0 {
        return false;
    }
    let bytes = game.encode(clock_ms(0) / 1000);
    let mut count = 0;
    while count < bytes.len() {
        let part = &bytes[count..];
        let wrote = unsafe { write(fd, part.as_ptr().cast(), part.len()) };
        if wrote <= 0 {
            break;
        }
        count += wrote as usize;
    }
    let closed = unsafe { close(fd) };
    count == bytes.len()
        && closed == 0
        && unsafe { rename(TEMP.as_ptr().cast(), LEDGER.as_ptr().cast()) } == 0
}
