use crate::{call, exit, ioctl, log, open};
pub fn time_us() -> u64 {
    let mut t = [0i64; 2];
    call(228, 1, t.as_mut_ptr() as usize, 0, 0, 0, 0);
    (t[0] * 1_000_000 + t[1] / 1000) as u64
}
pub fn sleep_us(n: u64) {
    if n >= 1000 {
        let t = [0i64, (n.min(50000) * 1000) as i64];
        call(35, t.as_ptr() as usize, 0, 0, 0, 0, 0);
    }
}
pub struct Clock {
    next: u64,
    pub dropped: u32,
}
impl Clock {
    pub fn new() -> Self {
        Self {
            next: time_us(),
            dropped: 0,
        }
    }
    pub fn steps(&mut self) -> usize {
        let now = time_us();
        let mut n = 0;
        while self.next <= now && n < 8 {
            n += 1;
            self.next += 16667;
        }
        if self.next <= now {
            self.dropped += 1;
            self.next = now + 16667;
        }
        n
    }
}
pub struct Display {
    fd: isize,
    kb: isize,
    base: usize,
    page: usize,
    pitch: usize,
    w: usize,
    h: usize,
    pages: usize,
    shown: usize,
    present_required: bool,
    pub held: [bool; 128],
    pub pressed: [bool; 128],
}
impl Display {
    pub fn new(w: usize, h: usize) -> Self {
        Self::open_mode(Some((w, h)))
    }

    /// Borrow the desktop mode without issuing a mode-setting ioctl.
    pub fn current() -> Self {
        Self::open_mode(None)
    }

    fn open_mode(requested: Option<(usize, usize)>) -> Self {
        let fd = open(b"/dev/fb0\0", 2);
        let kb = open(b"/dev/kbd\0", 0x800);
        let mut d = [0u32; 6];
        let request = if let Some((w, h)) = requested {
            d[0] = u32::try_from(w).unwrap_or_else(|_| exit(2));
            d[1] = u32::try_from(h).unwrap_or_else(|_| exit(2));
            0x5002
        } else {
            0x5001
        };
        if fd < 0 || kb < 0 || ioctl(fd, request, d.as_mut_ptr() as usize) != 0 {
            log(b"display setup failed\n");
            exit(2);
        }
        let (w, h) = (d[0] as usize, d[1] as usize);
        let pitch = d[2] as usize / 4;
        if w == 0 || h == 0 || d[2] % 4 != 0 || pitch < w || !(1..=2).contains(&d[3])
            || d[4] >= d[3] || requested.is_some_and(|size| size != (w, h))
        {
            log(b"invalid display geometry\n");
            exit(2);
        }
        let page = pitch.checked_mul(h).unwrap_or_else(|| exit(2));
        let bytes = page.checked_mul(d[3] as usize * 4).unwrap_or_else(|| exit(2));
        // Older kernels return -ENOTTY for the opt-in and keep direct mappings.
        const UNSUPPORTED_IOCTL: isize = -25;
        let shadow = match ioctl(fd, 0x5004, 0) {
            0 | UNSUPPORTED_IOCTL => false,
            1 => true,
            _ => {
                log(b"display presentation setup failed\n");
                exit(2);
            }
        };
        let base = call(9, 0, bytes, 3, 1, fd as usize, 0);
        if base < 0 {
            exit(2);
        }
        Self {
            fd,
            kb,
            base: base as usize,
            page,
            pitch,
            w,
            h,
            pages: d[3] as usize,
            shown: d[4] as usize,
            present_required: shadow || d[3] > 1,
            held: [false; 128],
            pressed: [false; 128],
        }
    }
    /// Read one new key press in event order. Held modifiers reflect this
    /// press, even when their releases are already waiting in the queue.
    pub fn next_press(&mut self) -> Option<usize> {
        loop {
            let mut event = [0u8; 2];
            if call(0, self.kb as usize, event.as_mut_ptr() as usize, 2, 0, 0, 0) != 2 {
                return None;
            }
            let key = (event[0] & 127) as usize;
            let down = event[1] & 1 == 0;
            let pressed = down && !self.held[key];
            self.held[key] = down;
            if pressed {
                return Some(key);
            }
        }
    }

    pub fn poll(&mut self) {
        self.pressed.fill(false);
        while let Some(key) = self.next_press() {
            self.pressed[key] = true;
        }
    }
    pub fn show(&mut self, px: &[u32]) {
        assert_eq!(px.len(), self.w * self.h);
        self.draw(|surface, w, _, pitch| {
            if pitch == w {
                surface.copy_from_slice(px);
            } else {
                for (s, d) in px
                    .chunks_exact(w)
                    .zip(surface.chunks_exact_mut(pitch))
                {
                    d[..w].copy_from_slice(s);
                }
            }
        });
    }

    /// Paint directly into the next display page, using its actual row stride.
    pub fn draw(&mut self, paint: impl FnOnce(&mut [u32], usize, usize, usize)) {
        let dest = if self.pages > 1 { 1 - self.shown } else { 0 };
        let surface = unsafe {
            core::slice::from_raw_parts_mut(
                (self.base as *mut u32).add(self.page * dest),
                self.page,
            )
        };
        paint(surface, self.w, self.h, self.pitch);
        if self.present_required && ioctl(self.fd, 0x5003, dest) != 0 {
            exit(2);
        }
        self.shown = dest;
    }
}
pub fn load(path: &[u8], data: &mut [u8]) -> bool {
    let f = open(path, 0);
    if f < 0 {
        return false;
    }
    let n = call(
        0,
        f as usize,
        data.as_mut_ptr() as usize,
        data.len(),
        0,
        0,
        0,
    );
    let mut extra = 0u8;
    let end = call(0, f as usize, &mut extra as *mut _ as usize, 1, 0, 0, 0);
    call(3, f as usize, 0, 0, 0, 0, 0);
    n == data.len() as isize && end == 0
}
pub fn save(temp: &[u8], path: &[u8], data: &[u8]) -> bool {
    let f = open(temp, 1 | 64 | 512);
    if f < 0 {
        return false;
    }
    let n = call(1, f as usize, data.as_ptr() as usize, data.len(), 0, 0, 0);
    let closed = call(3, f as usize, 0, 0, 0, 0, 0);
    n == data.len() as isize
        && closed == 0
        && call(
            82,
            temp.as_ptr() as usize,
            path.as_ptr() as usize,
            0,
            0,
            0,
            0,
        ) == 0
}
