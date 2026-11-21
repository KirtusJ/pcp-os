//! Devices: the bus that says which devices there are, a display, a keyboard, and sound. This is the second of the
//! two kernel units in which `unsafe` is allowed. This file reads and writes device ports. Its opt-in
//! Ethernet probes poll device-owned completions in reserved physical frames. Display memory is reached through `arch`, which checks every address against the
//! extent it mapped.
//!
//! What is here was written for the devices an emulator presents (QEMU's standard display adapter, which
//! speaks the Bochs display interface; the PS/2 keyboard controller every PC emulator has; the Intel AC'97
//! sound device), and for a display mode that a loader has already set. It has not been tried on hardware.
//!
//! The sound device reads frames reserved for it. The Ethernet fixture reserves its rings and packet
//! frames until reset. The RX experiment also grants the device write access to one header and frame.
#![allow(unsafe_code)]
use crate::arch;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "virtio-tx-probe")]
pub mod virtio_tx;
#[cfg(feature = "vm-store")]
pub mod store_disk;

fn in8(port: u16) -> u8 {
    // Safety: the callers name ports of devices this file drives; reading one has no effect on memory.
    unsafe { arch::port_in8(port) }
}
fn out8(port: u16, value: u8) {
    // Safety: device-specific callers own the selected I/O ports. DMA probes only publish reserved frames.
    unsafe { arch::port_out8(port, value) }
}
fn in16(port: u16) -> u16 {
    // Safety: as `in8`.
    unsafe { arch::port_in16(port) }
}
fn out16(port: u16, value: u16) {
    // Safety: as `out8`.
    unsafe { arch::port_out16(port, value) }
}
fn in32(port: u16) -> u32 {
    // Safety: as `in8`.
    unsafe { arch::port_in32(port) }
}
fn out32(port: u16, value: u32) {
    // Safety: as `out8`.
    unsafe { arch::port_out32(port, value) }
}

// ---- the bus -------------------------------------------------------------------------------------------

/// One device the bus reports.
#[derive(Clone, Copy, Debug)]
pub struct BusDevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor: u16,
    pub device: u16,
    pub class: u8,
    pub subclass: u8,
    /// The six base address registers, as read.
    pub bars: [u32; 6],
    /// The interrupt line the firmware assigned, 255 if none.
    pub line: u8,
}

fn bus_read(bus: u8, slot: u8, function: u8, register: u8) -> u32 {
    out32(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (slot as u32) << 11 | (function as u32) << 8 | (register & 0xFC) as u32);
    in32(0xCFC)
}

fn bus_write(d: &BusDevice, register: u8, value: u32) {
    out32(0xCF8, 0x8000_0000 | (d.bus as u32) << 16 | (d.slot as u32) << 11 | (d.function as u32) << 8 | (register & 0xFC) as u32);
    out32(0xCFC, value);
}

/// Ask the bus what devices there are. The first eight buses are read, which is every bus an emulated PC has.
pub fn bus_scan() -> Vec<BusDevice> {
    let mut found = Vec::new();
    for bus in 0..8u8 {
        for slot in 0..32u8 {
            if bus_read(bus, slot, 0, 0) & 0xFFFF == 0xFFFF {
                continue;
            }
            let functions = if bus_read(bus, slot, 0, 0x0C) >> 16 & 0x80 != 0 { 8 } else { 1 };
            for function in 0..functions {
                let id = bus_read(bus, slot, function, 0);
                if id & 0xFFFF == 0xFFFF {
                    continue;
                }
                let class = bus_read(bus, slot, function, 0x08);
                let mut bars = [0u32; 6];
                for (n, b) in bars.iter_mut().enumerate() {
                    *b = bus_read(bus, slot, function, 0x10 + 4 * n as u8);
                }
                found.push(BusDevice { bus, slot, function, vendor: id as u16, device: (id >> 16) as u16, class: (class >> 24) as u8, subclass: (class >> 16) as u8,
                                       bars, line: bus_read(bus, slot, function, 0x3C) as u8 });
            }
        }
    }
    found
}

// ---- the display -----------------------------------------------------------------------------------------

/// How the display is driven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayKind {
    /// The Bochs display interface: any mode, and two pages to draw on, one shown while the other is drawn.
    Bochs,
    /// A mode the loader set before the kernel started: it cannot be changed, and there is one page.
    Loader,
}

/// The display as it is now: 32 bits to a pixel, blue in the lowest byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Display {
    pub kind: DisplayKind,
    pub width: u32,
    pub height: u32,
    /// Bytes from one row to the next.
    pub pitch: u32,
    /// Pages a program may draw on: 2 where the device can show one while the other is drawn, else 1.
    pub pages: u32,
    /// Where the display memory is, and how much of it was mapped.
    pub memory: usize,
    pub mapped: usize,
    /// The page being shown.
    pub shown: u32,
}

static DISPLAY: arch::Single<Option<Display>> = arch::Single::new(None);
/// Times a program has asked for a page to be shown since the start: each is one finished picture put before the person.
static SHOWS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Pictures shown since the start.
pub fn display_shows() -> u64 {
    SHOWS.load(Ordering::Relaxed)
}

/// Count one picture a program has finished and asked to be shown.
pub fn display_count_show() {
    SHOWS.fetch_add(1, Ordering::Relaxed);
}

const DISPI_INDEX: u16 = 0x1CE;
const DISPI_DATA: u16 = 0x1CF;

fn dispi_read(index: u16) -> u16 {
    out16(DISPI_INDEX, index);
    in16(DISPI_DATA)
}
fn dispi_write(index: u16, value: u16) {
    out16(DISPI_INDEX, index);
    out16(DISPI_DATA, value);
}

/// Find a display and take it over. Returns what was found, in words, for the start-up report.
pub fn display_init(bus: &[BusDevice], loader: Option<arch::LoaderDisplay>, requested: Option<(u32, u32)>) -> Option<Display> {
    // The Bochs interface: a display-class device of one of the two makers that implement it, answering with a known version.
    let adapter = bus.iter().find(|d| d.class == 3 && ((d.vendor == 0x1234 && d.device == 0x1111) || (d.vendor == 0x80EE && d.device == 0xBEEF)));
    if let Some(d) = adapter {
        let version = dispi_read(0);
        let memory = (d.bars[0] & 0xFFFF_FFF0) as usize;
        if (0xB0C0..=0xB0C5).contains(&version) && d.bars[0] & 1 == 0 && memory != 0 {
            let size = if version >= 0xB0C4 { dispi_read(0x0A) as usize * 65536 } else { 4 << 20 };
            let mapped = size & !((2 << 20) - 1);
            if mapped >= 4 << 20 && arch::map_device(memory, mapped) {
                let display = Display { kind: DisplayKind::Bochs, width: 0, height: 0, pitch: 0, pages: 2, memory, mapped, shown: 0 };
                arch::without_interrupts(|| DISPLAY.with(|s| *s = Some(display)));
                let (w, h) = requested.unwrap_or((1920, 1080));
                if let Some(display) = display_set(w, h) {
                    return Some(display);
                }
                for (width, height) in [(1920, 1080), (1280, 720), (1024, 768), (800, 600), (640, 480)] {
                    if let Some(display) = display_set(width, height) {
                        return Some(display);
                    }
                }
            }
        }
    }
    // Otherwise, a mode the loader set, if it is one this kernel can draw in.
    if let Some(l) = loader {
        let bytes = (l.pitch as usize * l.height as usize + (2 << 20) - 1) & !((2 << 20) - 1);
        if l.bits == 32 && l.width >= 320 && l.height >= 200 && l.address % (2 << 20) == 0 && arch::map_device(l.address, bytes) {
            let display = Display { kind: DisplayKind::Loader, width: l.width, height: l.height, pitch: l.pitch, pages: 1, memory: l.address, mapped: bytes, shown: 0 };
            arch::without_interrupts(|| DISPLAY.with(|s| *s = Some(display)));
            return Some(display);
        }
    }
    None
}

/// The display as it is now, if there is one.
pub fn display() -> Option<Display> {
    arch::without_interrupts(|| DISPLAY.with(|s| *s))
}

/// Change the mode. `None`, and nothing changes, if there is no display, it cannot change mode, or the mode
/// is one it does not have memory for. Display memory is cleared by the device.
pub fn display_set(width: u32, height: u32) -> Option<Display> {
    arch::without_interrupts(|| {
        DISPLAY.with(|s| {
            let d = s.as_mut()?;
            if d.kind != DisplayKind::Bochs {
                return if d.width == width && d.height == height { Some(*d) } else { None };
            }
            if !(320..=2560).contains(&width) || !(200..=1600).contains(&height) || width % 8 != 0 {
                return None;
            }
            if width as usize * height as usize * 4 * 2 > d.mapped {
                return None;
            }
            dispi_write(4, 0);
            dispi_write(1, width as u16);
            dispi_write(2, height as u16);
            dispi_write(3, 32);
            dispi_write(6, width as u16);
            dispi_write(7, (height * 2) as u16);
            dispi_write(8, 0);
            dispi_write(9, 0);
            dispi_write(4, 0x41); // enabled, with the linear frame buffer
            if dispi_read(1) as u32 != width || dispi_read(2) as u32 != height || dispi_read(3) != 32 {
                return None;
            }
            d.width = width;
            d.height = height;
            d.pitch = width * 4;
            d.shown = 0;
            Some(*d)
        })
    })
}

/// Show one of the pages. False if there is no such page.
pub fn display_show(page: u32) -> bool {
    arch::without_interrupts(|| {
        DISPLAY.with(|s| match s.as_mut() {
            Some(d) if page < d.pages => {
                if d.kind == DisplayKind::Bochs {
                    dispi_write(9, (page * d.height) as u16);
                }
                d.shown = page;
                true
            }
            _ => false,
        })
    })
}

/// Fill a rectangle of page 0 with one colour. What falls outside the display is left out.
pub fn display_fill(x: u32, y: u32, w: u32, h: u32, colour: u32) {
    if let Some(d) = display() {
        let (x1, y1) = ((x + w).min(d.width), (y + h).min(d.height));
        if x >= x1 {
            return;
        }
        for row in y..y1 {
            arch::device_fill32(d.memory + row as usize * d.pitch as usize + x as usize * 4, (x1 - x) as usize, colour);
        }
    }
}

/// Write a row of pixels on page 0, starting at (x, y). What falls outside the display is left out.
pub fn display_row(x: u32, y: u32, pixels: &[u32]) {
    if let Some(d) = display() {
        if y < d.height && x < d.width {
            let n = pixels.len().min((d.width - x) as usize);
            arch::device_write32(d.memory + y as usize * d.pitch as usize + x as usize * 4, &pixels[..n]);
        }
    }
}

// ---- the keyboard ------------------------------------------------------------------------------------------

/// A key going down or coming up: the key's number in the first scan code set, whether it was preceded by
/// the prefix that marks the second group of keys (arrows, the right-hand Ctrl and Alt, and others), and
/// whether it was released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub code: u8,
    pub extended: bool,
    pub released: bool,
    /// Whether a Ctrl key was held when this key was pressed or released.
    pub ctrl: bool,
}

struct Keyboard {
    present: bool,
    events: [Key; 256],
    head: usize,
    count: usize,
    extended: bool,
    /// Bytes of a longer sequence still to be passed over.
    skip: u8,
    ctrl: bool,
    alt: bool,
    /// Whether each Enter key is down now, by its last press or release that was seen: the main one, and
    /// the one on the number pad.
    enter: [bool; 2],
    /// Events dropped because nothing took them in time.
    pub dropped: u64,
}

static KEYBOARD: arch::Single<Keyboard> = arch::Single::new(Keyboard { present: false, events: [Key { code: 0, extended: false, released: false, ctrl: false }; 256], head: 0, count: 0,
                                                                         extended: false, skip: 0, ctrl: false, alt: false, enter: [false; 2], dropped: 0 });
/// Set when the person presses Ctrl, Alt and Delete together: the kernel's own key, which no program can take.
static STOP: AtomicBool = AtomicBool::new(false);

fn controller_wait_write() {
    for _ in 0..100_000 {
        if in8(0x64) & 2 == 0 {
            return;
        }
    }
}
fn controller_read() -> Option<u8> {
    for _ in 0..100_000 {
        if in8(0x64) & 1 != 0 {
            return Some(in8(0x60));
        }
    }
    None
}

/// Prepare the keyboard controller and let the keyboard interrupt. Returns whether a controller answered.
pub fn keyboard_init() -> bool {
    // A machine with no controller reads all ones from its status port.
    if in8(0x64) == 0xFF {
        return false;
    }
    controller_wait_write();
    out8(0x64, 0xAD); // first port off while it is set up
    controller_wait_write();
    out8(0x64, 0xA7); // second port off: there is no driver for a mouse
    for _ in 0..64 {
        if in8(0x64) & 1 == 0 {
            break;
        }
        in8(0x60);
    }
    controller_wait_write();
    out8(0x64, 0x20);
    let config = match controller_read() {
        Some(c) => c,
        None => return false,
    };
    controller_wait_write();
    out8(0x64, 0x60);
    controller_wait_write();
    out8(0x60, (config | 0x01 | 0x40) & !0x02 & !0x10); // interrupt for the keyboard, translation to the first scan code set, no interrupt for the second port, keyboard clock on
    controller_wait_write();
    out8(0x64, 0xAE);
    controller_wait_write();
    out8(0x60, 0xF4); // the keyboard itself: begin sending keys
    let _ = controller_read(); // its acknowledgement
    arch::without_interrupts(|| KEYBOARD.with(|k| k.present = true));
    arch::interrupt_line_on(1);
    true
}

fn keyboard_byte(k: &mut Keyboard, byte: u8) {
    if k.skip > 0 {
        k.skip -= 1;
        return;
    }
    match byte {
        0xE0 => k.extended = true,
        0xE1 => k.skip = 2, // the Pause key sends a sequence of its own, which is passed over
        0xFA | 0xFE | 0xEE | 0x00 | 0xFF => {} // acknowledgements and errors from the keyboard, not keys
        _ => {
            let mut key = Key { code: byte & 0x7F, extended: k.extended, released: byte & 0x80 != 0, ctrl: false };
            k.extended = false;
            match key.code {
                0x1D => k.ctrl = !key.released,
                0x38 => k.alt = !key.released,
                0x1C => k.enter[key.extended as usize] = !key.released,
                0x53 if !key.released && k.ctrl && k.alt => STOP.store(true, Ordering::SeqCst),
                _ => {}
            }
            key.ctrl = k.ctrl;
            if k.count == k.events.len() {
                k.head = (k.head + 1) % k.events.len(); // the oldest is lost
                k.count -= 1;
                k.dropped += 1;
            }
            let at = (k.head + k.count) % k.events.len();
            k.events[at] = key;
            k.count += 1;
        }
    }
}

/// Entered from `arch` when a device line interrupts, with interrupts off.
pub fn interrupt(line: u8) {
    if line == 1 {
        KEYBOARD.with(|k| {
            for _ in 0..16 {
                let status = in8(0x64);
                if status & 1 == 0 {
                    break;
                }
                let byte = in8(0x60);
                if status & 0x20 == 0 {
                    keyboard_byte(k, byte);
                }
            }
        });
    }
}

/// How the boot fixture times simulated keyboard input.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyPacing {
    Timed,
    BlockedRead,
    /// Send the Stop chord once the sleeper has idled for the requested number of timer periods.
    /// At the test deadline, send it regardless so a broken sleeper can finish with a failing result.
    Sleeping { idle_ticks: u64, timeout: u64 },
}
struct Script {
    at: u64,
    pacing: KeyPacing,
    /// A key reached the queue but the idle reader was still waiting on the next timer interrupt.
    late_wake: bool,
    bytes: [u8; 8],
    len: usize,
    next: usize,
}

static SCRIPT: arch::Single<Script> = arch::Single::new(Script { at: 0, pacing: KeyPacing::Timed, late_wake: false, bytes: [0; 8], len: 0, next: 0 });

/// For the kernel's self-tests, which have nobody at the keyboard: from timer tick `at`, hand the controller one
/// of these scan codes (at most eight) per eligible tick, with its command "put this byte in the output buffer as if the
/// keyboard had sent it" (0xD2). The controller then interrupts on the keyboard's line, so each byte arrives the
/// way a key does: asynchronously, through `interrupt` and `keyboard_byte`.
pub fn keyboard_script(at: u64, bytes: &[u8], pacing: KeyPacing) {
    arch::without_interrupts(|| SCRIPT.with(|s| {
        let n = bytes.len().min(s.bytes.len());
        s.bytes[..n].copy_from_slice(&bytes[..n]);
        *s = Script { at, pacing, late_wake: false, bytes: s.bytes, len: n, next: 0 };
    }))
}

/// Release any trailing key-up bytes after the test program exits so the modifiers return to rest.
/// True if no blocked reader was left waiting for the timer to deliver an already queued key.
pub fn keyboard_script_finish() -> bool {
    arch::without_interrupts(|| SCRIPT.with(|s| {
        s.pacing = KeyPacing::Timed;
        !s.late_wake
    }))
}

/// How many scan codes of the script have not been handed to the controller yet.
pub fn keyboard_script_left() -> usize {
    arch::without_interrupts(|| SCRIPT.with(|s| s.len - s.next))
}

/// Entered from `tick`, with interrupts off.
fn script_tick(ticks: u64) {
    SCRIPT.with(|s| {
        // One byte a tick, and only when the controller holds nothing: neither a byte to be read nor one being taken.
        if s.next < s.len && ticks >= s.at && in8(0x64) & 3 == 0 {
            if let KeyPacing::Sleeping { idle_ticks, timeout } = s.pacing {
                if ticks.saturating_sub(s.at) < timeout &&
                    !crate::user::sleeping_idle_ticks(ticks).is_some_and(|idle| idle >= idle_ticks) {
                    return;
                }
            }
            if s.pacing == KeyPacing::BlockedRead {
                if !crate::user::idle_in_key_read() {
                    return;
                }
                // IRQ1 queued a key without resuming the reader. The timer would otherwise conceal the missed wake.
                if keys_waiting() {
                    s.late_wake = true;
                    return;
                }
            }
            out8(0x64, 0xD2);
            controller_wait_write();
            out8(0x60, s.bytes[s.next]);
            s.next += 1;
        }
    })
}

/// Forget every key event that is waiting and say, as of that same moment, whether each Enter key is down:
/// (the main one, the one on the number pad). Taken together with interrupts off, so that every event taken
/// afterwards is later than the answer and none is counted twice.
pub fn keys_forget_enter() -> (bool, bool) {
    arch::without_interrupts(|| KEYBOARD.with(|k| {
        k.head = 0;
        k.count = 0;
        (k.enter[0], k.enter[1])
    }))
}

/// The oldest key event not yet taken, if any.
pub fn key_take() -> Option<Key> {
    arch::without_interrupts(|| {
        KEYBOARD.with(|k| {
            if k.count == 0 {
                return None;
            }
            let key = k.events[k.head];
            k.head = (k.head + 1) % k.events.len();
            k.count -= 1;
            Some(key)
        })
    })
}

/// True if a key event is waiting.
pub fn keys_waiting() -> bool {
    arch::without_interrupts(|| KEYBOARD.with(|k| k.count > 0))
}

/// Forget every key event that is waiting: what was typed at one program is not handed to the next.
pub fn keys_forget() {
    arch::without_interrupts(|| KEYBOARD.with(|k| {
        k.head = 0;
        k.count = 0;
    }))
}

/// (whether a keyboard controller answered, key events lost because nothing took them in time).
pub fn keyboard_state() -> (bool, u64) {
    arch::without_interrupts(|| KEYBOARD.with(|k| (k.present, k.dropped)))
}

/// True, once, if the person has pressed Ctrl, Alt and Delete together since this was last asked.
pub fn stop_requested() -> bool {
    STOP.swap(false, Ordering::SeqCst)
}

// ---- sound ---------------------------------------------------------------------------------------------------

/// The sound device plays 16-bit samples, left then right, 48000 pairs a second.
pub const SOUND_RATE: u32 = 48000;
/// Sound memory is a ring of 32 pieces of 512 pairs each (2048 bytes, about a hundredth of a second).
const PIECES: usize = 32;
const PIECE_BYTES: usize = 2048;
/// No more than this many pieces are ever waiting to be played: what is written is heard within about 75 ms.
const WAITING_MOST: usize = 7;

struct Sound {
    /// The two port ranges of the device: its mixer and its transfer engine.
    mixer: u16,
    engine: u16,
    /// The frames that hold the ring (two pieces to a frame) and the frame that holds the list of pieces.
    frames: [usize; PIECES / 2],
    /// The piece being filled, and how many bytes of it are filled.
    filling: usize,
    filled: usize,
    /// Whether the engine has ever been started.
    started: bool,
    /// Bytes accepted since the start, and times the device ran out of sound to play.
    pub accepted: u64,
    pub ran_dry: u64,
}

static SOUND: arch::Single<Option<Sound>> = arch::Single::new(None);

/// Find an AC'97 sound device and prepare it to play. Returns whether one was found and prepared.
pub fn sound_init(bus: &[BusDevice]) -> bool {
    let d = match bus.iter().find(|d| d.class == 4 && d.subclass == 1 && d.vendor == 0x8086 && d.device == 0x2415) {
        Some(d) => d,
        None => return false,
    };
    if d.bars[0] & 1 == 0 || d.bars[1] & 1 == 0 {
        return false; // its registers are expected at device ports
    }
    let (mixer, engine) = ((d.bars[0] & 0xFFFC) as u16, (d.bars[1] & 0xFFFC) as u16);
    // Let the device answer at its ports and read memory by itself.
    bus_write(d, 0x04, bus_read(d.bus, d.slot, d.function, 0x04) | 0x5);
    out32(engine + 0x2C, 0x2); // out of reset
    out16(mixer, 1); // the mixer's own reset
    out16(mixer + 0x02, 0x0000); // master volume: full
    out16(mixer + 0x18, 0x0808); // the level of what programs play: unchanged
    out8(engine + 0x1B, 0x02); // reset the output engine
    for _ in 0..100_000 {
        if in8(engine + 0x1B) & 0x02 == 0 {
            break;
        }
    }
    // The ring and the list of its pieces.
    let mut frames = [0usize; PIECES / 2];
    for f in frames.iter_mut() {
        *f = match crate::mem::take() {
            Some(x) => x,
            None => return false,
        };
    }
    let list = match crate::mem::take() {
        Some(x) => x,
        None => return false,
    };
    let (_, pool_end) = arch::pool();
    if pool_end > 1 << 32 {
        return false; // the device is told 32-bit addresses
    }
    for n in 0..PIECES {
        let address = (frames[n / 2] + (n % 2) * PIECE_BYTES) as u32;
        let mut entry = [0u8; 8];
        entry[..4].copy_from_slice(&address.to_le_bytes());
        entry[4..6].copy_from_slice(&((PIECE_BYTES / 2) as u16).to_le_bytes()); // its length, in samples
        arch::frame_write(list, n * 8, &entry);
    }
    out32(engine + 0x10, list as u32);
    out16(engine + 0x16, 0x1C); // clear what the engine reports
    arch::without_interrupts(|| SOUND.with(|s| *s = Some(Sound { mixer, engine, frames, filling: 0, filled: 0, started: false, accepted: 0, ran_dry: 0 })));
    true
}

impl Sound {
    /// (pieces waiting to be played, counting the one being played; whether the engine has stopped for want of sound)
    fn waiting(&mut self) -> (usize, bool) {
        if !self.started {
            return (0, false);
        }
        let status = in16(self.engine + 0x16);
        let playing = in8(self.engine + 0x14) as usize % PIECES;
        if status & 1 != 0 {
            // Stopped: everything given to it has been played. What comes next goes in the piece after the last.
            out16(self.engine + 0x16, 0x1C);
            if self.filling != (playing + 1) % PIECES {
                self.filling = (playing + 1) % PIECES;
                self.filled = 0;
            }
            return (0, true);
        }
        ((self.filling + PIECES - playing) % PIECES, false)
    }
}

/// How many bytes the sound device will take now, and how many are waiting to be played. (0, 0) if there is none.
pub fn sound_room() -> (usize, usize) {
    arch::without_interrupts(|| {
        SOUND.with(|s| match s.as_mut() {
            Some(s) => {
                let (waiting, _) = s.waiting();
                ((WAITING_MOST.saturating_sub(waiting) * PIECE_BYTES).saturating_sub(s.filled), waiting * PIECE_BYTES + s.filled)
            }
            None => (0, 0),
        })
    })
}

/// Give the device sound to play: 16-bit samples, left then right. Returns how many bytes were taken,
/// which is fewer than offered when enough is already waiting. A piece is played once it is full.
pub fn sound_write(bytes: &[u8]) -> usize {
    arch::without_interrupts(|| {
        SOUND.with(|s| {
            let s = match s.as_mut() {
                Some(s) => s,
                None => return 0,
            };
            let (mut waiting, dry) = s.waiting();
            if dry {
                s.ran_dry += 1;
            }
            let mut done = 0;
            while done < bytes.len() && waiting < WAITING_MOST {
                let n = (PIECE_BYTES - s.filled).min(bytes.len() - done);
                let at = s.filling * PIECE_BYTES + s.filled;
                arch::frame_write(s.frames[at / 4096], at % 4096, &bytes[done..done + n]);
                s.filled += n;
                done += n;
                if s.filled == PIECE_BYTES {
                    out8(s.engine + 0x15, s.filling as u8); // the last piece the engine may play
                    if !s.started || dry {
                        out8(s.engine + 0x1B, 0x01); // run
                        s.started = true;
                    }
                    s.filling = (s.filling + 1) % PIECES;
                    s.filled = 0;
                    waiting += 1;
                }
            }
            s.accepted += done as u64;
            done
        })
    })
}

/// Stop the device and forget what was waiting: for when the program that was playing ends.
pub fn sound_quiet() {
    arch::without_interrupts(|| {
        SOUND.with(|s| {
            if let Some(s) = s.as_mut() {
                if s.started {
                    out8(s.engine + 0x1B, 0x00);
                    out8(s.engine + 0x1B, 0x02);
                    for _ in 0..100_000 {
                        if in8(s.engine + 0x1B) & 0x02 == 0 {
                            break;
                        }
                    }
                    let list = in32(s.engine + 0x10);
                    out32(s.engine + 0x10, list);
                    out16(s.engine + 0x16, 0x1C);
                    for f in s.frames.iter() {
                        arch::frame_zero(*f);
                    }
                    s.started = false;
                    s.filling = 0;
                    s.filled = 0;
                }
                let _ = s.mixer;
            }
        })
    })
}

/// (whether there is a sound device, bytes it has taken since the start, times it ran out of sound to play)
pub fn sound_state() -> (bool, u64, u64) {
    arch::without_interrupts(|| SOUND.with(|s| s.as_ref().map(|s| (true, s.accepted, s.ran_dry)).unwrap_or((false, 0, 0))))
}

// ---- music: an FM synthesiser chip ---------------------------------------------------------------------------

/// A register of the synthesiser to be written at a time: the time is in milliseconds on the clock programs read.
#[derive(Clone, Copy)]
struct FmWrite {
    when: u32,
    register: u8,
    value: u8,
}

const FM_QUEUE: usize = 8192;

struct Fm {
    present: bool,
    queue: [FmWrite; FM_QUEUE],
    head: usize,
    count: usize,
    /// Registers written since the start, and the most that were ever late, in milliseconds.
    written: u64,
    latest: u32,
}

static FM: arch::Single<Fm> = arch::Single::new(Fm { present: false, queue: [FmWrite { when: 0, register: 0, value: 0 }; FM_QUEUE], head: 0, count: 0, written: 0, latest: 0 });

fn fm_status() -> u8 {
    in8(0x388)
}

fn fm_register(register: u8, value: u8) {
    out8(0x388, register);
    fm_status(); // the chip needs a moment after each write; reading its status is that moment
    out8(0x389, value);
    fm_status();
    fm_status();
}

/// Look for an FM synthesiser of the AdLib kind at its usual ports, by the test that has always been used:
/// its first timer, once started, must be seen to run out. Returns whether one answered.
pub fn fm_init() -> bool {
    fm_register(0x04, 0x60); // stop both timers
    fm_register(0x04, 0x80); // clear what they reported
    let before = fm_status();
    fm_register(0x02, 0xFF); // the first timer: one step of 80 microseconds
    fm_register(0x04, 0x21); // start it
    let mut after = 0;
    for _ in 0..200_000 {
        after = fm_status();
        if after & 0xE0 == 0xC0 {
            break;
        }
    }
    fm_register(0x04, 0x60);
    fm_register(0x04, 0x80);
    let present = before & 0xE0 == 0 && after & 0xE0 == 0xC0;
    if present {
        for r in 1..=0xF5u8 {
            fm_register(r, 0);
        }
    }
    arch::without_interrupts(|| FM.with(|f| f.present = present));
    present
}

/// Milliseconds on the clock programs read, for a count of timer ticks.
fn clock_ms(ticks: u64) -> u32 {
    (ticks * 999_847 / 1_000_000) as u32
}

/// Entered from `arch` on every timer tick, with interrupts off: write the registers whose time has come.
pub fn tick(ticks: u64) {
    script_tick(ticks);
    FM.with(|f| {
        if f.count == 0 {
            return;
        }
        let now = clock_ms(ticks);
        let mut done = 0;
        while f.count > 0 && done < 256 {
            let w = f.queue[f.head];
            if w.when > now {
                break;
            }
            fm_register(w.register, w.value);
            f.latest = f.latest.max(now - w.when);
            f.head = (f.head + 1) % FM_QUEUE;
            f.count -= 1;
            f.written += 1;
            done += 1;
        }
    })
}

/// Take register writes for the synthesiser, each eight bytes: the time in milliseconds (four bytes), the
/// register (two), the value (one), and one unused. Returns how many bytes were taken.
pub fn fm_write(bytes: &[u8]) -> usize {
    arch::without_interrupts(|| {
        FM.with(|f| {
            if !f.present {
                return 0;
            }
            let mut done = 0;
            while done + 8 <= bytes.len() && f.count < FM_QUEUE {
                let b = &bytes[done..done + 8];
                let register = u16::from_le_bytes([b[4], b[5]]);
                if register < 0x100 {
                    // The second bank of registers belongs to a later chip than this driver is for.
                    let at = (f.head + f.count) % FM_QUEUE;
                    f.queue[at] = FmWrite { when: u32::from_le_bytes([b[0], b[1], b[2], b[3]]), register: register as u8, value: b[6] };
                    f.count += 1;
                }
                done += 8;
            }
            done
        })
    })
}

/// Silence the synthesiser and forget what was waiting: for when the program that was playing ends.
pub fn fm_quiet() {
    arch::without_interrupts(|| {
        FM.with(|f| {
            if f.present && (f.count > 0 || f.written > 0) {
                f.head = 0;
                f.count = 0;
                for r in 0xB0..=0xB8u8 {
                    fm_register(r, 0); // every voice: key off
                }
                for r in 0x40..=0x55u8 {
                    fm_register(r, 0x3F); // every operator: no level
                }
            }
        })
    })
}

/// (whether there is a synthesiser, registers written since the start, the most a write was ever late in milliseconds)
pub fn fm_state() -> (bool, u64, u32) {
    arch::without_interrupts(|| FM.with(|f| (f.present, f.written, f.latest)))
}
