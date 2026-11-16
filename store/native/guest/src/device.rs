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
    pub held: [bool; 128],
    pub pressed: [bool; 128],
}
impl Display {
    pub fn new(w: usize, h: usize) -> Self {
        let fd = open(b"/dev/fb0\0", 2);
        let kb = open(b"/dev/kbd\0", 0x800);
        let mut d = [w as u32, h as u32, 0, 0, 0, 0];
        if fd < 0
            || kb < 0
            || ioctl(fd, 0x5002, d.as_mut_ptr() as usize) != 0
            || d[0] != w as u32
            || d[1] != h as u32
            || d[2] < w as u32 * 4
            || d[3] == 0
        {
            log(b"display setup failed\n");
            exit(2);
        }
        let pitch = d[2] as usize / 4;
        let page = pitch * h;
        let base = call(9, 0, page * d[3] as usize * 4, 3, 1, fd as usize, 0);
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
            held: [false; 128],
            pressed: [false; 128],
        }
    }
    pub fn poll(&mut self) {
        self.pressed.fill(false);
        loop {
            let mut e = [0u8; 2];
            if call(0, self.kb as usize, e.as_mut_ptr() as usize, 2, 0, 0, 0) != 2 {
                break;
            }
            let k = (e[0] & 127) as usize;
            let down = e[1] & 1 == 0;
            self.pressed[k] |= down && !self.held[k];
            self.held[k] = down;
        }
    }
    pub fn show(&mut self, px: &[u32]) {
        assert_eq!(px.len(), self.w * self.h);
        let dest = if self.pages > 1 { 1 - self.shown } else { 0 };
        let surface = unsafe {
            core::slice::from_raw_parts_mut(
                (self.base as *mut u32).add(self.page * dest),
                self.page,
            )
        };
        if self.pitch == self.w {
            surface.copy_from_slice(px);
        } else {
            for (s, d) in px
                .chunks_exact(self.w)
                .zip(surface.chunks_exact_mut(self.pitch))
            {
                d[..self.w].copy_from_slice(s);
            }
        }
        if self.pages > 1 && ioctl(self.fd, 0x5003, dest) != 0 {
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
