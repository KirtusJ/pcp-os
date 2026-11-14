//! Short harbor signals, mixed directly into the optional stereo PCM device.
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Default)]
struct AudioInfo {
    rate: u32,
    channels: u32,
    bits: u32,
    room: u32,
    waiting: u32,
}
extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn ioctl(fd: c_int, request: usize, ...) -> c_int;
    fn write(fd: c_int, buffer: *const c_void, size: usize) -> isize;
}

#[derive(Clone, Copy)]
pub enum Cue {
    Bell,
    Answer(usize),
    Sea,
    CastOff,
    Purchase,
    Manager,
    Charter,
    Wire(usize),
}

struct Tone {
    hz: u32,
    offset_ms: u32,
    level: i32,
}
const fn tone(hz: u32, offset_ms: u32, level: i32) -> Tone {
    Tone {
        hz,
        offset_ms,
        level,
    }
}

// Inharmonic partials give the bell a brass attack.
const BELL: &[Tone] = &[tone(784, 0, 5200), tone(1205, 0, 2300), tone(1865, 0, 1000)];
const CAST_OFF: &[Tone] = &[tone(329, 0, 4200), tone(494, 58, 3500)];
const PURCHASE: &[Tone] = &[tone(880, 0, 3500), tone(1175, 55, 3500)];
const MANAGER: &[Tone] = &[
    tone(523, 0, 4200),
    tone(659, 55, 4100),
    tone(784, 110, 4300),
];
const CHARTER: &[Tone] = &[
    tone(392, 0, 4100),
    tone(523, 85, 4300),
    tone(659, 150, 4300),
    tone(784, 220, 4700),
];
const SEA: &[Tone] = &[
    tone(143, 0, 4500),
    tone(292, 0, 1600),
    tone(1732, 360, 1700),
];
impl Cue {
    fn score(self) -> (u32, &'static [Tone]) {
        match self {
            // Silence keeps the DMA ring moving between the caller and the reply.
            // Restarting an empty AC'97 queue can replay an old buffer first.
            Self::Bell => (1700, BELL),
            // A full silent DMA ring after the reply also clears its last note.
            Self::Answer(_) => (1000, &[]),
            Self::Sea => (1200, SEA),
            Self::CastOff => (180, CAST_OFF),
            Self::Purchase => (180, PURCHASE),
            Self::Manager => (270, MANAGER),
            Self::Charter => (440, CHARTER),
            Self::Wire(_) => (17 * WIRE_UNIT_MS, &[]),
        }
    }
}
pub const WIRE_UNIT_MS: u32 = 95;

/// Marks are 1 or 3 units long, with one unit between marks and three between groups.
pub fn wire_mark(good: usize, elapsed: u32) -> Option<usize> {
    let (a, b) = bellweather::wire::CALLS[good];
    let mut at = 0;
    for (i, pulse) in a.bytes().chain(b.bytes()).enumerate() {
        let len = if pulse == b'-' {
            3 * WIRE_UNIT_MS
        } else {
            WIRE_UNIT_MS
        };
        if (at..at + len).contains(&elapsed) {
            return Some(i);
        }
        at += len
            + if i == 2 {
                3 * WIRE_UNIT_MS
            } else {
                WIRE_UNIT_MS
            };
    }
    None
}
fn wire_sample(good: usize, at: u32) -> i16 {
    if wire_mark(good, at / 48).is_none() {
        return 0;
    }
    // A 600 Hz key tone crosses zero at every mark boundary (95 ms = 57 cycles).
    let phase = (at % 80) as i32;
    (if phase < 20 {
        phase
    } else if phase < 60 {
        40 - phase
    } else {
        phase - 80
    }) as i16
        * 210
}

fn answer_sample(person: usize, frame: u32) -> i16 {
    let (period_ms, burst_ms, notes, hz, overtones) = match person {
        0 => (230, 215, 2, 988, 1697),
        1 => (110, 75, 3, 228, 571),
        _ => (165, 90, 2, 1175, 2350),
    };
    let second_whistle = person == 2 && frame >= period_ms * 48;
    let note = if second_whistle {
        1
    } else {
        frame / (period_ms * 48)
    };
    let at = frame - note * period_ms * 48;
    let length = if second_whistle { 315 } else { burst_ms } * 48;
    if note >= notes || at >= length {
        return 0;
    }
    let tones = [
        tone(hz, 0, 3700),
        tone(overtones, 0, if person == 2 { 350 } else { 1650 }),
    ];
    sample(&tones, at, length)
}
fn sample(tones: &[Tone], at: u32, duration: u32) -> i16 {
    let mut value = 0i32;
    for tone in tones {
        let start = tone.offset_ms * 48;
        if at < start {
            continue;
        }
        let t = at - start;
        let length = duration - start;
        let phase = (u64::from(t) * u64::from(tone.hz) * 256 / 48_000 % 256) as i32;
        let tri = 64 - (phase - 128).abs();
        let attack = t.min(96) as i32;
        let decay = ((length - t) * 256 / length) as i32;
        value += (((tri * tone.level / 64) * attack / 96) * decay / 256) * decay / 256;
    }
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

pub struct Speaker {
    fd: c_int,
    playing: Option<Cue>,
    at: u32,
}
impl Speaker {
    pub fn open() -> Self {
        let mut s = Self {
            fd: unsafe { open(b"/dev/audio\0".as_ptr().cast(), 1) },
            playing: None,
            at: 0,
        };
        if s.info().is_none() {
            s.fd = -1;
        }
        s
    }
    fn info(&self) -> Option<AudioInfo> {
        let mut info = AudioInfo::default();
        if self.fd < 0
            || unsafe { ioctl(self.fd, 0x5011, &mut info) } != 0
            || (info.rate, info.bits, info.channels) != (48_000, 16, 2)
        {
            return None;
        }
        Some(info)
    }
    pub fn play(&mut self, cue: Cue) {
        if matches!(cue, Cue::Answer(_) | Cue::Sea) && matches!(self.playing, Some(Cue::Wire(_))) {
            return;
        }
        if self.fd >= 0 {
            self.playing = Some(cue);
            self.at = 0;
        }
    }
    pub fn frame_ms(&self) -> u64 {
        // AC'97 queues at most seven 10.7 ms pieces, including one currently
        // being played. A 40 ms render interval can starve and replay that DMA ring.
        if self.playing.is_some() {
            10
        } else {
            40
        }
    }
    pub fn pump(&mut self) {
        let Some(cue) = self.playing else {
            return;
        };
        let Some(info) = self.info() else {
            self.playing = None;
            return;
        };
        // Keep up to 80 ms queued. Later key presses can replace a ringing tail.
        let (ms, tones) = cue.score();
        let duration = ms * 48;
        let frames = (info.room / 4)
            .min(3840u32.saturating_sub(info.waiting / 4))
            .min(duration - self.at);
        if frames == 0 {
            return;
        }
        let mut samples = [0i16; 7680];
        for (i, lr) in samples[..(frames * 2) as usize]
            .chunks_exact_mut(2)
            .enumerate()
        {
            let at = self.at + i as u32;
            lr.fill(match cue {
                Cue::Bell if at < 240 * 48 => sample(BELL, at, 240 * 48),
                Cue::Bell => 0,
                Cue::Wire(good) => wire_sample(good, at),
                Cue::Answer(person) => answer_sample(person, at),
                Cue::Sea if at < 630 * 48 => sample(SEA, at, 630 * 48),
                Cue::Sea => 0,
                _ => sample(tones, at, duration),
            });
        }
        let sent = unsafe { write(self.fd, samples.as_ptr().cast(), (frames * 4) as usize) };
        if sent < 0 {
            self.playing = None;
            return;
        }
        self.at += (sent as u32 / 4).min(frames);
        if self.at == duration {
            self.playing = None;
        }
    }
}
