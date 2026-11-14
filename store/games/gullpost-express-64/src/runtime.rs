static mut PIXELS: [u32; (W * H) as usize] = [0; (W * H) as usize];
static mut DEPTH: [i32; (W * H) as usize] = [0; (W * H) as usize];
fn micros() -> i64 {
    let mut t = [0i64; 2];
    call(228, 1, t.as_mut_ptr() as usize, 0, 0, 0, 0);
    t[0] * 1_000_000 + t[1] / 1000
}
fn number(mut n: u64) {
    let mut buf = [b'0'; 20];
    let mut i = 19;
    loop {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
        i -= 1
    }
    log(&buf[i..])
}
#[derive(Default)]
struct Stats {
    start: i64,
    frames: u64,
    us: u64,
    worst: u64,
    hist: [u32; 32],
    late: u64,
    dropped: u64,
}
impl Stats {
    fn add(&mut self, at: i64, cost: i64) {
        if self.frames == 0 {
            self.start = at
        }
        self.frames += 1;
        let c = cost.max(0) as u64;
        self.us += c;
        self.worst = self.worst.max(c);
        self.hist[((c / 1000) as usize).min(31)] += 1;
        if c > 25_000 {
            self.late += 1
        }
    }
    fn finish(&self, at: i64) {
        if self.frames == 0 {
            return;
        }
        let mut prefix = 0;
        let bucket = self
            .hist
            .iter()
            .position(|&n| {
                prefix += n as u64;
                prefix * 100 >= self.frames * 95
            })
            .unwrap_or(31);
        log(b"G64 PERF");
        for (k, n) in [
            (b" frames=".as_slice(), self.frames),
            (b" span_us=", (at - self.start).max(1) as u64),
            (b" render_sum_us=", self.us),
            (b" render_p95_bucket_ms=", bucket as u64),
            (b" render_max_us=", self.worst),
            (b" render_over_25ms=", self.late),
            (b" dropped_ticks=", self.dropped),
        ] {
            log(k);
            number(n)
        }
        log(b"\n")
    }
}
#[no_mangle]
pub extern "C" fn gull_entry() -> ! {
    let fb = open(b"/dev/fb0\0", 2);
    let kb = open(b"/dev/kbd\0", 0x800);
    let mut d = [W as u32, H as u32, 0, 0, 0, 0];
    if kb < 0
        || fb < 0
        || ioctl(fb, 0x5002, d.as_mut_ptr() as usize) != 0
        || d[0] != W as u32
        || d[1] != H as u32
        || d[2] < W as u32 * 4
        || d[3] == 0
    {
        log(b"G64: display or keyboard unavailable\n");
        exit(2)
    }
    let pitch = d[2] as usize / 4;
    let page = pitch * H as usize;
    let mem = call(9, 0, page * d[3] as usize * 4, 3, 1, fb as usize, 0);
    if mem < 0 {
        log(b"G64: display mapping failed\n");
        exit(2)
    }
    let pixels = unsafe { &mut *(&raw mut PIXELS) };
    let depth = unsafe { &mut *(&raw mut DEPTH) };
    let mut held = [false; 128];
    let mut pending = 0;
    let mut g = Game::new();
    let mut next = ms();
    let mut frame_deadline = next;
    let mut stats = Stats::default();
    let mut speaker = Speaker {
        fd: open(b"/dev/audio\0", 1),
        sfx: Sfx::Quiet,
        at: 0,
        phase: 0,
    };
    log(b"G64: cover ready\n");
    loop {
        loop {
            let mut ev = [0u8; 2];
            if call(0, kb as usize, ev.as_mut_ptr() as usize, 2, 0, 0, 0) != 2 {
                break;
            }
            let i = (ev[0] & 127) as usize;
            let down = ev[1] & 1 == 0;
            if i == 1 && down {
                stats.finish(micros());
                log(b"G64: exit to PCP OS\n");
                exit(0)
            }
            if down && !held[i] {
                pending |= bind(i)
            }
            held[i] = down
        }
        let mut keys = 0;
        for (i, &v) in held.iter().enumerate() {
            if v {
                keys |= bind(i)
            }
        }
        let mut effect = Sfx::Quiet;
        let now = ms();
        let mut steps = 0;
        while now >= next && steps < 4 {
            let old = g.mode;
            let fx = g.step(keys | pending);
            pending = 0;
            next += TICK;
            steps += 1;
            if old != g.mode {
                if old == Mode::Run {
                    stats.finish(micros());
                    stats = Stats::default()
                }
                log(match g.mode {
                    Mode::Run if g.demo => b"G64: guided run\n",
                    Mode::Run => b"G64: free play\n",
                    Mode::Pause => b"G64: paused\n",
                    Mode::Delivered => b"G64: letter delivered\n",
                    Mode::Cover => b"G64: cover ready\n",
                })
            }
            if fx != Sfx::Quiet {
                effect = fx;
                match fx {
                    Sfx::Hop => log(b"G64: jump\n"),
                    Sfx::Wing => log(b"G64: flap\n"),
                    Sfx::Splash => log(b"G64: retry\n"),
                    Sfx::Stamp => {
                        log(b"G64: postmarks mask=");
                        number(g.mask as u64);
                        log(b"\n")
                    }
                    Sfx::Flag if g.checkpoint => log(b"G64: shore checkpoint\n"),
                    _ => {}
                }
            }
        }
        if now >= next {
            let drop = (now - next) / TICK + 1;
            next += drop * TICK;
            if g.mode == Mode::Run {
                stats.dropped += drop as u64
            }
        }
        speaker.pump(effect);
        let begin = micros();
        scene::draw(
            &mut ink::Frame {
                px: pixels,
                pitch: W as usize,
            },
            &g,
            depth,
        );
        let dest = if d[3] > 1 { 1 - d[4].min(1) } else { 0 };
        let surface =
            unsafe { slice::from_raw_parts_mut((mem as *mut u32).add(page * dest as usize), page) };
        for (a, b) in pixels
            .chunks_exact(W as usize)
            .zip(surface.chunks_exact_mut(pitch))
        {
            b[..W as usize].copy_from_slice(a)
        }
        if d[3] > 1 {
            if ioctl(fb, 0x5003, dest as usize) != 0 {
                log(b"G64: page flip failed\n");
                exit(2)
            }
            d[4] = dest
        }
        if g.mode == Mode::Run {
            stats.add(begin, micros() - begin)
        }
        frame_deadline += TICK;
        let end = ms();
        if frame_deadline < end {
            frame_deadline = end
        }
        sleep(frame_deadline - end);
    }
}
