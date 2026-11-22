//! What happens between the kernel's verification of itself and its stop: the programs the loader handed
//! over are offered to the person, or one is run unattended, and each run is a task in the kernel's register.
//!
//! With a display and a keyboard the person sees what the machine verified, what programs and files there
//! are with their digests, and chooses. Starting a program is the person's act, and it is the grant of what
//! the program may use: the display, the keyboard, the loader's files, the store. The person can stop a
//! running program at any time with Ctrl, Alt and Delete, which the kernel takes and no program can.
//!
//! The loader's command line can ask for one program to be run with nobody there (`auto=<name>`) and for
//! the machine to switch off instead of showing its menu (`then=off`). With `grant=none` a program run that
//! way is given the console and nothing else, which is how the tests show that the grants are what count.
//!
//! This file has no `unsafe`.
use crate::arch;
use crate::dev;
use crate::exec;
use crate::ui;
use crate::user::{self, End, Grants, Program, World};
use crate::{logged, now, obj, text};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use pcpos_core::acr::Record;
use pcpos_core::ledger;
use pcpos_core::sha256_hex;
use pcpos_core::value::Value;

macro_rules! say {
    ($($arg:tt)*) => { crate::log(format_args!($($arg)*)) };
}

/// What the session is told about the machine and the start.
pub struct Start<'a> {
    pub boot: &'a arch::BootInfo,
    pub display: Option<dev::Display>,
    pub keyboard: bool,
    /// Why there is no keyboard to choose with, in a sentence for the person, if there is none.
    pub no_keyboard: Option<&'static str>,
    /// Why the six self-tests that press keys were not run, if they were not.
    pub not_run: Option<&'static str>,
    pub sound: bool,
    pub passed: u32,
    pub failed: u32,
    /// Tasks already in the register: the next is numbered after them.
    pub tasks: u32,
    pub version: &'static str,
}

struct Entry {
    name: String,
    arguments: String,
    image: &'static [u8],
    digest: String,
}

fn option<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace().find_map(|w| w.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

#[cfg(feature = "guest-ethernet")]
mod ethernet;
#[cfg(feature = "guest-ethernet")]
use ethernet::ethernet_grant;

/// Text made to fit `room` characters. Text that does not fit is never cut in silence: its last two places
/// become "..", so that a person sees that there is more than is shown.
fn cut(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        text.into()
    } else {
        let mut out: String = text.chars().take(room.saturating_sub(2)).collect();
        out.push_str("..");
        out
    }
}

/// A long name on as many lines as it needs, none longer than `across`. Nothing is left out.
fn wrapped(text: &str, across: usize) -> Vec<String> {
    let all: Vec<char> = text.chars().collect();
    all.chunks(across.max(1)).map(|c| c.iter().collect()).collect()
}

/// A sentence on as many lines as it needs, broken between words, none longer than `across`. A word
/// longer than a line is broken where the line ends. Nothing is left out.
fn wrapped_words(text: &str, across: usize) -> Vec<String> {
    let across = across.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let have = line.chars().count();
        if have != 0 && have + 1 + word.chars().count() <= across {
            line.push(' ');
            line.push_str(word);
            continue;
        }
        if have != 0 {
            lines.push(core::mem::take(&mut line));
        }
        let mut parts = wrapped(word, across);
        line = parts.pop().unwrap_or_default();
        lines.append(&mut parts);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The sentence about verification that every surface carries, in the longest wording that fits `room`
/// characters; the last wording is cut with ".." if even it does not fit. Whether it passed is never the
/// part that is lost: it comes first.
fn verified_sentence(start: &Start, room: usize) -> String {
    let all = start.passed + start.failed;
    let wordings = if start.failed != 0 {
        [format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, all), format!("NOT VERIFIED: {} of {} failed", start.failed, all),
         format!("NOT VERIFIED: {} failed", start.failed)]
    } else {
        [format!("Verified at start: all {} self-tests passed.", start.passed), format!("Verified: all {} self-tests", start.passed),
         format!("Verified: all {}", start.passed)]
    };
    let last = cut(&wordings[2], room);
    wordings.into_iter().find(|w| w.chars().count() <= room).unwrap_or(last)
}

/// The whole of what the chosen pinned program is, on a plate that ends just above `bottom`: its name in
/// full, its size and the beginning of its digest. A label in the bar or the dock has room for twelve
/// characters, and two programs can agree in those; this is what says which one Enter will start.
fn chosen_pin(p: &Entry, x: u32, bottom: u32, w: u32, refused: Option<&str>) {
    use ui::{label, BAD, GOOD, SOFT, TEXT};
    let across = ((w.saturating_sub(80)) / 10).min(72) as usize;
    let name = wrapped(&p.name, across);
    let facts = format!("{}   SHA-256 {}...", size_text(p.image.len()), &p.digest[..16]);
    // After Enter was refused for this program, the plate says so in place of 'Enter starts' (B3's patch
    // did this for the small display; the same holds here).
    let caption = match refused { Some(why) => cut(&format!("Not started: {}", why), across), None => String::from("Enter starts") };
    let widest = name.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(facts.chars().count()).max(caption.chars().count()).max(18) as u32;
    let (wide, tall) = (widest * 10 + 40, 80 + 20 * name.len() as u32);
    let x = (x.min(w.saturating_sub(wide + 20))) / 10 * 10;
    let y = (bottom.saturating_sub(tall + 10)) / 20 * 20;
    let on = plate(x, y, wide, tall, PLATE, outline(), false);
    label(x + 20, y + 20, &caption, if refused.is_some() { BAD } else { GOOD }, false, &on);
    for (n, line) in name.iter().enumerate() {
        label(x + 20, y + 40 + n as u32 * 20, line, TEXT, false, &on);
    }
    label(x + 20, y + 40 + name.len() as u32 * 20, &facts, SOFT, false, &on);
}

fn size_text(bytes: usize) -> String {
    if bytes >= 10 << 20 {
        format!("{} MiB", bytes >> 20)
    } else if bytes >= 1 << 20 {
        format!("{}.{} MiB", bytes >> 20, (bytes & 0xfffff) * 10 >> 20)
    } else {
        format!("{} KiB", (bytes + 1023) >> 10)
    }
}

/// Seconds since 1970 for a date and time of the machine's clock.
fn epoch(y: u32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> u64 {
    let (y, m) = if mo <= 2 { (y as i64 - 1, mo as i64 + 9) } else { (y as i64, mo as i64 - 3) };
    let days = y.div_euclid(400) * 146097 + y.rem_euclid(400) * 365 + y.rem_euclid(400) / 4 - y.rem_euclid(400) / 100 + (153 * m + 2) / 5 + d as i64 - 1 - 719468;
    (days * 86400 + h as i64 * 3600 + mi as i64 * 60 + s as i64).max(0) as u64
}

/// Run the session. Returns the number of tasks now in the register.
pub fn run(rec: &mut Record, start: &Start, failed: &mut u32) -> u32 {
    let mut tasks = start.tasks;
    // A kernel that did not verify itself is in SAFE: it shows what it has and starts nothing (K-11). Its register
    // takes no further material either: the participant that would record it is registered only by a kernel that verified.
    let safe = start.failed != 0;
    let mut programs: Vec<Entry> = Vec::new();
    let mut files: Vec<(String, &'static [u8], String)> = Vec::new();
    for m in start.boot.modules.iter() {
        if let Some(bytes) = arch::module_bytes(m) {
            let digest = sha256_hex(bytes);
            // Every file the loader handed over is material the kernel was given: it is recorded by the kernel's own reading of it.
            let ok = safe || logged("ARTIFACT_ADDED", rec.submit(&now(), "kernel", "HOST", "ARTIFACT_ADDED",
                obj(&[("sha256", text(&digest)), ("size", Value::Int(bytes.len() as i64)), ("origin", text("SUPPLIED")), ("stored", Value::Bool(false)),
                      ("name", text(&m.name)), ("filename", text(&m.name)), ("read_by", text("HOST"))]))).is_ok();
            if !ok {
                *failed += 1;
            }
            if exec::looks_like_a_program(bytes) {
                programs.push(Entry { name: m.name.clone(), arguments: m.arguments.clone(), image: bytes, digest: digest.clone() });
            }
            files.push((m.name.clone(), bytes, digest));
        }
    }
    let (y, mo, d, h, mi, s) = arch::clock();
    let mut world = World { files: files.iter().map(|f| (f.0.clone(), f.1)).collect(), store: Default::default(),
        #[cfg(feature = "vm-store")] volume: None,
                            epoch_at_start: epoch(y, mo, d, h, mi, s).saturating_sub(arch::ticks() / arch::TICKS_PER_SECOND), unanswered: Vec::new() };
    let line = start.boot.command_line.as_str();
    #[cfg(feature = "vm-store")]
    if !safe {
        match crate::store::from_command_line(start.boot.exact_command_line()) {
            Ok(Some((volume, files))) => {
                say!("VM checkpoint store mounted: {} file(s)", files.len());
                world.store = files;
                world.volume = Some(volume);
            }
            Ok(None) => {}
            Err(()) => {
                say!("requested VM checkpoint store could not be mounted; no program started");
                *failed += 1;
                return tasks;
            }
        }
    }
    let mut message = String::new();
    if safe {
        say!("the kernel did not verify itself ({} self-test(s) failed): it is in SAFE and starts no program", start.failed);
    }
    // One program, with nobody there.
    if let Some(name) = option(line, "auto") {
        match programs.iter().find(|p| p.name == name) {
            Some(entry) if safe => say!("auto={}: not started", entry.name),
            Some(entry) => {
                let (m, w, _) = run_entry(rec, entry, world, &mut tasks, start, failed);
                world = w;
                message = m;
            }
            None => {
                say!("auto={}: the loader handed over no program of that name", name);
                *failed += 1;
            }
        }
        if option(line, "then") != Some("menu") {
            return tasks;
        }
    }
    let interactive = start.display.is_some() && start.keyboard && option(line, "then") != Some("off");
    if !interactive {
        if !programs.is_empty() && option(line, "auto").is_none() {
            say!("{} program(s) from the loader were not started: there is {}. Name one with auto=<name> to run it unattended.",
                 programs.len(), if option(line, "then") == Some("off") { "nobody to choose (then=off)" } else { "no display and keyboard to choose with" });
        }
        return tasks;
    }
    // The menu.
    // The desktop is the home surface. It is empty until the person puts something on it: a shortcut to a
    // program, placed from the launcher, or a widget. The launcher is opened from it and left with Esc.
    // What is placed lasts for the session; nothing about the desktop is written to a disk.
    let mut chosen = 0usize;
    let mut place = 0usize;
    let mut last_refusal = None;
    // Which part of the chosen pin's name the small-display desktop shows, and how many parts it has.
    let (mut name_page, mut name_pages) = (0usize, 1usize);
    // `first=details` on the loader's command line starts on the plain list of everything the loader gave,
    // which is the screen this kernel showed first before it had a desktop. Harnesses that drive that
    // screen by its keys and compare its pixels ask for it; a person gets the desktop.
    let mut surface = if option(line, "first") == Some("details") { Surface::Details } else { Surface::Home };
    let mut shortcuts: Vec<usize> = Vec::new();
    // The person's programs sit at the foot of the desktop in one of two forms, chosen in the
    // settings (F2) and by no key of the desktop itself: a bar across the whole width (the default, the
    // form most desktops have), or a dock in the middle. `bar=dock` on the loader's command line starts with the dock.
    let mut whole_bar = option(line, "bar") != Some("dock");
    // The edition whose theme the desktop wears: `edition=personal`, `commercial` or `enterprise` on the
    // loader's command line. With none named it is Enterprise, the edition this source is published in.
    if let Some(name) = option(line, "edition") {
        if !ui::set_edition(name) {
            say!("edition: '{}' is not an edition; the desktop stays in the {} theme", name, ui::edition().name);
        }
    }
    let mut widgets = [false; WIDGETS.len()];
    let display = start.display.unwrap();
    ui::set_scale(if option(line, "ui_scale") == Some("2") && display.width >= 1280 && display.height >= 960 { 2 } else { 1 });
    say!("desktop: {}x{} at {}x UI scale", display.width, display.height, ui::scale());
    // Whether the desktop is drawn in full, or as the plain screens of a small display.
    let graphic = display.width >= 1280 && display.height >= 720 && ui::scale() == 1;
    // How many places the desktop has for the way to all the programs and what is pinned. Every place
    // can be reached and is drawn; a pin for which there is no place is refused, and said to be.
    let room = if graphic { foot_room(display.width) } else { programs.len() + 1 };
    arch::interrupts_on();
    dev::keys_forget();
    let mut drawn = None;
    loop {
        #[cfg(feature = "vm-store")]
        let mounted = world.volume.is_some();
        #[cfg(not(feature = "vm-store"))]
        let mounted = false;
        // The dock holds the way to all the programs first, then what is pinned.
        place = place.min(shortcuts.len()).min(room - 1);
        // A surface is drawn when what it shows has changed or another screen was drawn over it, and at
        // no other time: a key that changes nothing writes nothing to the display (B3 traced the stores).
        let shows = (surface, chosen, place, name_page, shortcuts.clone(), whole_bar, widgets, message.clone(), last_refusal, tasks, world.store.len(), mounted);
        if drawn.as_ref() != Some(&(shows.clone(), ui::clears())) {
            match surface {
                Surface::Home => name_pages = draw_home(start, &programs, rec, &shortcuts, place, whole_bar, &widgets, &world.store, mounted, tasks - start.tasks, &message, last_refusal, name_page),
                Surface::Launcher => draw_desktop(start, &programs, rec, chosen, &message, &shortcuts),
                Surface::Details => draw_menu(start, &programs, &files, rec, chosen, &message, world.store.len(),
                                              #[cfg(feature = "vm-store")] mounted),
            }
            drawn = Some((shows, ui::clears()));
        }
        let (code, extended, ctrl) = wait_chord();
        let key = (code, extended);
        // Which program Enter starts, if any: the chosen shortcut on the desktop, the chosen row elsewhere.
        let to_start = match surface {
            Surface::Home => place.checked_sub(1).and_then(|at| shortcuts.get(at)).copied(),
            _ => if programs.is_empty() { None } else { Some(chosen) },
        };
        // The rules of the desktop's keys. No key that is typed (a letter) does anything on the desktop or
        // in the launcher: screens are opened with function keys, and a change to what is chosen takes
        // Ctrl. Nothing that stops the machine happens at one press: F10 asks, and a fresh Enter answers.
        // That holds on a display of any size (directors' debrief, 21 November 2026). Only the plain list
        // (`first=details`, or Ctrl+D on a display wide enough for it) keeps the letters it has always had,
        // since it names them on the screen and harnesses drive it by them; its P asks in the same way.
        let letters = surface == Surface::Details;
        let desk = surface != Surface::Details;
        match (surface, key) {
            (_, (0x44, false)) => if power_prompt() { break },
            (_, (0x19, false)) if letters && !ctrl => if power_prompt() { break },
            (_, (0x3E, false)) => register_view(rec),
            (_, (0x13, false)) if letters => register_view(rec),
            (_, (0x3F, false)) if desk => verification_view(start, rec),
            (_, (0x2F, false)) if desk && letters => verification_view(start, rec),
            (Surface::Home, (0x3B, false)) => keys_view("KEYS ON THE DESKTOP", if graphic { &HOME_KEYS } else { &SMALL_HOME_KEYS }),
            (Surface::Launcher, (0x3B, false)) => keys_view("KEYS IN THE PROGRAMS", &LAUNCHER_KEYS),
            (Surface::Home, (0x25, false)) if letters => keys_view("KEYS ON THE DESKTOP", if graphic { &HOME_KEYS } else { &SMALL_HOME_KEYS }),
            (Surface::Launcher, (0x25, false)) if letters => keys_view("KEYS IN THE PROGRAMS", &LAUNCHER_KEYS),
            // On the small-display desktop Left and Right turn the parts of a long name; Up and Down choose.
            (Surface::Home, (0x4B, _)) if !graphic => name_page = name_page.saturating_sub(1),
            (Surface::Home, (0x4D, _)) if !graphic => name_page = (name_page + 1).min(name_pages.saturating_sub(1)),
            (Surface::Home, (0x4B, _)) | (Surface::Home, (0x48, _)) if place > 0 => { place -= 1; name_page = 0; }
            (Surface::Home, (0x4D, _)) | (Surface::Home, (0x50, _)) if place < shortcuts.len().min(room - 1) => { place += 1; name_page = 0; }
            (Surface::Home, (0x39, false)) => surface = Surface::Launcher,
            (Surface::Home, (0x1C, _)) if to_start.is_none() => surface = Surface::Launcher,
            (Surface::Home, (0x3D, false)) => files_view(&world.store, mounted),
            (Surface::Home, (0x21, false)) if letters => files_view(&world.store, mounted),
            (Surface::Home, (0x3C, false)) => settings_view(&mut whole_bar, &mut widgets),
            (Surface::Home, (0x53, _)) | (Surface::Home, (0x0E, false)) if place > 0 => { shortcuts.remove(place - 1); name_page = 0; }
            (Surface::Launcher, (0x48, _)) | (Surface::Details, (0x48, _)) if chosen > 0 => chosen -= 1,
            (Surface::Launcher, (0x50, _)) | (Surface::Details, (0x50, _)) if chosen + 1 < programs.len() => chosen += 1,
            (_, (0x1C, _)) if to_start.is_some() && safe => {
                last_refusal = to_start.map(|index| (index, "the kernel did not verify itself"));
                message = String::from("Not started: the kernel did not verify itself, and starts no program.");
            }
            (_, (0x1C, _)) if to_start.is_some() => {
                arch::interrupts_off();
                let index = to_start.unwrap();
                let (m, w, refusal) = run_entry(rec, &programs[index], world, &mut tasks, start, failed);
                last_refusal = refusal.map(|why| (index, why));
                world = w;
                message = m;
                arch::interrupts_on();
                dev::keys_forget();
            }
            (Surface::Launcher, (0x19, false)) if ctrl && !programs.is_empty() => match shortcuts.iter().position(|p| *p == chosen) {
                Some(at) => { shortcuts.remove(at); }
                None if shortcuts.len() + 1 < room => shortcuts.push(chosen),
                None => message = format!("Not pinned: the desktop has room for {} pinned program(s) on this display. Take one out first.", room - 1),
            },
            // The plain list is laid out for ninety-six columns. On a narrower display Ctrl+D shows the
            // same facts as a screen of text that is read and left, whole, and the choosing stays here.
            (Surface::Launcher, (0x20, false)) if ctrl && ui::grid().0 < 96 => details_view(start, &programs, &files, rec, world.store.len(), mounted),
            (Surface::Launcher, (0x20, false)) if ctrl => surface = Surface::Details,
            (Surface::Details, (0x20, false)) => surface = Surface::Launcher,
            (Surface::Launcher, (0x01, false)) | (Surface::Details, (0x01, false)) => surface = Surface::Home,
            _ => {}
        }
    }
    arch::interrupts_off();
    ui::clear(ui::GROUND);
    ui::at(2, 2, "PCP OS is stopping. Its register is being closed and verified.", ui::TEXT, ui::GROUND);
    tasks
}

#[derive(Clone, Copy, PartialEq)]
enum Surface {
    Home,
    Launcher,
    Details,
}

/// The widgets a person can put on the desktop, in the order they are drawn.
const WIDGETS: [&str; 3] = ["Recent activity", "This machine", "Session"];
/// The outline of what is chosen and of the emblem's plate: the Core module's accent in this edition.
fn outline() -> u32 {
    ui::edition().core_accent
}
/// The quieter edge of a plate that is not chosen: the same accent, half way to the plate.
fn edge() -> u32 {
    ui::blend(PLATE, ui::edition().core_accent, 110)
}
const PLATE: u32 = ui::CORE_PLATE;
const PLATE_CHOSEN: u32 = 0x1B4A63;

/// What sits in the two lower corners of every desktop surface, small and out of the way: at the left the
/// one or two keys worth naming here, and at the right whether the kernel verified itself. A kernel that did
/// not verify says so in full, in red; one that did says a single word. V shows the whole of it. Each piece
/// is on a flat plate on the character grid, so that a check reading a picture of the screen can read it.
fn corners(start: &Start, w: u32, h: u32, keys: &[(&str, &str)]) {
    use ui::{label, label_width, BAD, GOOD, SOFT, TEXT};
    let row = h / 20 * 20 - 40;
    let mut x = 30;
    for (key, does) in keys {
        let width = label_width(key, false) + label_width(does, false) + 50;
        let on = plate(x - 10, row - 6, width, 32, PLATE, edge(), false);
        let after = label(x, row, key, TEXT, false, &on);
        label(after + 10, row, does, SOFT, false, &on);
        x += width + 10;
    }
    let _ = verified_word(start, w, row);
}

/// Whether the kernel verified itself, on its plate at the right of text row `row`: one word when it did,
/// the whole sentence in red when it did not. Gives back where the plate begins.
fn verified_word(start: &Start, w: u32, row: u32) -> u32 {
    use ui::{label, label_width, BAD, GOOD};
    let (word, colour) = if start.failed != 0 { (format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed), BAD) }
                         else { (String::from("verified"), GOOD) };
    let width = label_width(&word, false) + 20;
    let at = (w - 20 - width) / 10 * 10;
    let on = plate(at, row - 6, width, 32, PLATE, if colour == BAD { BAD } else { outline() }, false);
    label(at + 10, row, &word, colour, false, &on);
    at
}

/// The bar: the same things as the dock and the corners, in the form most desktops have, across the whole
/// foot of the desktop. At the left the way to all the programs and each pinned program, one of them
/// chosen; at the right the key that lists the keys, and whether the kernel verified itself.
fn whole_bar(start: &Start, programs: &[Entry], pinned: &[usize], place: usize, w: u32, h: u32, refusal: Option<(usize, &str)>) {
    use ui::{label, label_width, AMBER, GOOD, SOFT, TEAL, TEXT};
    let y0 = h / 20 * 20 - 60;
    let row = y0 + 20;
    let on_bar = plate(0, y0, w, 60, PLATE, edge(), false);
    let right = verified_word(start, w, row);
    let keys_width = label_width("F1", false) + label_width("keys", false) + 30;
    let keys_at = right.saturating_sub(keys_width + 20) / 10 * 10;
    let after = label(keys_at, row, "F1", TEXT, false, &on_bar);
    label(after + 10, row, "keys", SOFT, false, &on_bar);
    let room = foot_room(w);
    for i in 0..(1 + pinned.len()).min(room) {
        let x = 10 + i as u32 * 190;
        let chosen = i == place;
        let on_item = plate(x, y0 + 6, 180, 48, if chosen { PLATE_CHOSEN } else { PLATE }, if chosen { outline() } else { PLATE }, chosen);
        if chosen {
            dev::display_fill(x + 50, y0 + 46, 80, 3, AMBER);
        }
        // The same marks as the dock's, smaller: a grid of four for all the programs, three lines for one.
        let (mx, my) = (x + 14, y0 + 20);
        let bars: &[(u32, u32, u32, u32, u32)] = if i == 0 { &[(0, 0, 10, 8, TEAL), (14, 0, 10, 8, GOOD), (0, 12, 10, 8, GOOD), (14, 12, 10, 8, AMBER)] }
                                                 else { &[(0, 1, 24, 4, TEAL), (0, 8, 24, 4, GOOD), (0, 15, 16, 4, AMBER)] };
        for (bx, by, bw, bh, colour) in bars {
            dev::display_fill(mx + bx, my + by, *bw, *bh, *colour);
        }
        let name: String = if i == 0 { String::from("Programs") } else { cut(&programs[pinned[i - 1]].name, 12) };
        label(x + 50, row, &name, TEXT, false, &on_item);
    }
    if place > 0 && place < (1 + pinned.len()).min(room) {
        chosen_pin(&programs[pinned[place - 1]], 10 + place as u32 * 190, y0, w, refusal.filter(|(i, _)| *i == pinned[place - 1]).map(|(_, why)| why));
    }
}

/// The dock: the person's programs, along the foot of the desktop, in the middle. First the way to all the
/// programs; then each program the person has pinned there. One is chosen; Enter opens it.
fn dock(programs: &[Entry], pinned: &[usize], place: usize, w: u32, h: u32, refusal: Option<(usize, &str)>) {
    use ui::{label, label_width, AMBER, GOOD, TEAL, TEXT};
    let room = foot_room(w);
    let shown = (1 + pinned.len()).min(room);
    let width = shown as u32 * 140 + 20;
    let (x0, y0) = ((w - width) / 20 * 10, h / 20 * 20 - 120);
    let _ = plate(x0, y0, width, 100, PLATE, edge(), true);
    for i in 0..shown {
        let x = x0 + 10 + i as u32 * 140;
        let chosen = i == place;
        let on_item = plate(x + 5, y0 + 10, 130, 80, if chosen { PLATE_CHOSEN } else { PLATE }, if chosen { outline() } else { PLATE }, chosen);
        if chosen {
            dev::display_fill(x + 25, y0 + 86, 90, 3, AMBER);
        }
        // The mark: a grid of four for all the programs, three lines for a program.
        let (mx, my) = (x + 45, y0 + 18);
        let bars: &[(u32, u32, u32, u32, u32)] = if i == 0 { &[(4, 0, 18, 14, TEAL), (28, 0, 18, 14, GOOD), (4, 18, 18, 14, GOOD), (28, 18, 18, 14, AMBER)] }
                                                 else { &[(0, 2, 50, 6, TEAL), (0, 13, 50, 6, GOOD), (0, 24, 34, 6, AMBER)] };
        for (bx, by, bw, bh, colour) in bars {
            dev::display_fill(mx + bx, my + by, *bw, *bh, *colour);
        }
        let name: String = if i == 0 { String::from("Programs") } else { cut(&programs[pinned[i - 1]].name, 12) };
        label(x + (140 - label_width(&name, false)) / 20 * 10, y0 + 60, &name, TEXT, false, &on_item);
    }
    if place > 0 && place < shown {
        chosen_pin(&programs[pinned[place - 1]], x0 + 10 + place as u32 * 140, y0, w, refusal.filter(|(i, _)| *i == pinned[place - 1]).map(|(_, why)| why));
    }
}

/// One line of a text screen: pieces of text, each at its column and in its colour.
type Line = Vec<(u32, String, u32)>;

/// A sentence as lines at one column, broken between words to fit the display.
fn lines_of(col: u32, text: &str, colour: u32) -> Vec<Line> {
    let (cols, _) = ui::grid();
    let across = (cols.saturating_sub(col + 2) as usize).max(1);
    if text.is_empty() {
        return alloc::vec![Vec::new()];
    }
    wrapped_words(text, across).into_iter().map(|part| alloc::vec![(col, part, colour)]).collect()
}

/// A screen of text that is read and left: a title, lines, and on the row of the keys how it is left.
/// With fourteen rows the title is on row 1, the lines begin on row 3 (a `header`, if any, on row 2) and
/// the keys are two rows from the bottom; with fewer the title is on the first row, the lines follow it
/// and the keys are on the last. If the lines do not all fit, Up and Down move them by half a screenful,
/// the keys' row says so, and Esc or Enter leaves; `from_end` begins at the last of them. If they fit,
/// `foot` is on the keys' row; `any_key` then says whether any key leaves or only Esc and Enter.
fn text_screen(title: &str, title_colour: u32, header: Option<&str>, lines: &[Line], foot: &str, any_key: bool, from_end: bool) {
    use ui::{at, GROUND, SOFT};
    let (cols, rows) = ui::grid();
    let across = (cols as usize).saturating_sub(4).max(1);
    let roomy = rows >= 14;
    let (title_row, first, foot_row) = if roomy { (1, 3, rows - 2) } else { (0, 1, rows.saturating_sub(1)) };
    let per = ((if roomy { rows - 6 } else { rows.saturating_sub(2) }) as usize).max(1);
    let paged = lines.len() > per;
    let last_top = lines.len().saturating_sub(per);
    let mut top = if from_end { last_top } else { 0 };
    // The screen is drawn when it is opened and when the lines move, and at no other time: a key that
    // does nothing here changes nothing on it.
    let mut shown = None;
    loop {
        if shown != Some(top) {
            ui::clear(GROUND);
            at(2, title_row, &cut(title, across), title_colour, GROUND);
            if let Some(header) = header.filter(|h| roomy && h.len() <= across) {
                at(2, 2, header, SOFT, GROUND);
            }
            for (n, line) in lines.iter().skip(top).take(per).enumerate() {
                for (col, part, colour) in line {
                    at(*col, first + n as u32, part, *colour, GROUND);
                }
            }
            at(2, foot_row, &cut(if paged { "Up/Down scroll    Esc back" } else { foot }, across), SOFT, GROUND);
            shown = Some(top);
        }
        match wait_key() {
            (0x48, _) if paged => top = top.saturating_sub((per / 2).max(1)),
            (0x50, _) if paged => top = (top + (per / 2).max(1)).min(last_top),
            (0x01, _) | (0x1C, _) => return,
            _ if any_key && !paged => return,
            _ => {}
        }
    }
}

/// Whether the kernel verified itself, in full, and the state of its register. Any key goes back, or Esc
/// if the display is too small to show it all at once.
fn verification_view(start: &Start, rec: &Record) {
    use ui::{SOFT, TEXT};
    let mut lines: Vec<Line> = Vec::new();
    if start.failed != 0 {
        lines.extend(lines_of(2, &format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed), ui::BAD));
    } else {
        lines.extend(lines_of(2, &format!("Verified at start: all {} self-tests passed.", start.passed), ui::GOOD));
    }
    lines.extend(lines_of(2, "", TEXT));
    lines.extend(lines_of(2, &format!("kernel {} | x86-64", start.version), TEXT));
    let head = rec.ledger().head();
    lines.extend(lines_of(2, &format!("Register: {} events, head {}...", rec.ledger().len(), &head[..head.len().min(16)]), TEXT));
    lines.extend(lines_of(2, if ledger::verify(rec.ledger().bytes()).is_ok() { "The register's chain is intact." } else { "The register's chain does NOT verify." }, TEXT));
    lines.extend(lines_of(2, "", TEXT));
    lines.extend(lines_of(2, "F4 shows every event of the register.", SOFT));
    // What was not tested is said beside what was. It is below the lines that have always been here.
    if let Some(why) = start.not_run.filter(|_| start.failed == 0) {
        lines.extend(lines_of(2, "", TEXT));
        lines.extend(lines_of(2, &format!("Not run at start: the six tests that press keys, because {}.", why), ui::AMBER));
    }
    // The whole head, which the register's own title cannot hold on every display. It is below the lines
    // that have always been here, which keep their rows.
    lines.extend(lines_of(2, "", TEXT));
    lines.extend(lines_of(2, "The whole head of the register's chain:", SOFT));
    let across = (ui::grid().0 as usize).saturating_sub(4).max(1);
    lines.extend(wrapped(head, across).into_iter().map(|part| alloc::vec![(2, part, TEXT)]));
    text_screen("VERIFICATION", ui::TEAL, None, &lines, "Any key goes back", true, false);
}

/// Every key of a desktop surface and what it does. Any key goes back, or Esc if the display is too small
/// to show them all at once. With sixty columns a key and what it does are side by side; with fewer, what
/// it does is under the key.
fn keys_view(title: &str, keys: &[(&str, &str)]) {
    use ui::{SOFT, TEXT};
    let (cols, _) = ui::grid();
    let mut lines: Vec<Line> = Vec::new();
    for (key, does) in keys {
        let beside = cols >= 60;
        let mut parts = lines_of(if beside { 20 } else { 4 }, does, SOFT).into_iter();
        if beside {
            let mut first = alloc::vec![(2, key.to_string(), TEXT)];
            first.extend(parts.next().unwrap_or_default());
            lines.push(first);
        } else {
            lines.push(alloc::vec![(2, key.to_string(), TEXT)]);
        }
        lines.extend(parts);
    }
    text_screen(title, ui::TEAL, None, &lines, "Any key goes back", true, false);
}

const HOME_KEYS: [(&str, &str); 11] = [("Left/Right", "choose among the programs at the foot of the desktop"), ("Enter", "open what is chosen: all the programs, or a pinned program"),
    ("Space", "open all the programs"), ("Del", "take the chosen program out of the foot of the desktop"),
    ("F1", "this list"), ("F2", "settings: how the desktop looks, and its widgets"), ("F3", "the files programs have written"),
    ("F4", "the register"), ("F5", "verification, in full"), ("F10", "power off, after asking"),
    ("Ctrl+Alt+Delete", "stops a running program; no program can take that key")];
/// The keys of the desktop of a small display, which are not those of the full one: Up and Down choose,
/// Left and Right turn the parts of a long name. Letters do nothing on it, as on the full desktop.
const SMALL_HOME_KEYS: [(&str, &str); 12] = [("Up/Down", "choose among all the programs and the pinned programs"), ("Left/Right", "turn the parts of a long name"),
    ("Enter", "open what is chosen: all the programs, or a pinned program"),
    ("Space", "open all the programs"), ("Del", "take the chosen program out of the pinned programs"),
    ("F1", "this list"), ("F2", "settings: how the desktop looks, and its widgets"), ("F3", "the files programs have written"),
    ("F4", "the register"), ("F5", "verification, in full"), ("F10", "power off, after asking"),
    ("Ctrl+Alt+Delete", "stops a running program; no program can take that key")];
const LAUNCHER_KEYS: [(&str, &str); 10] = [("Up/Down", "choose a program"), ("Enter", "start it"), ("Ctrl+P", "pin it to the foot of the desktop, or take it out"),
    ("Esc", "back to the desktop"), ("Ctrl+D", "details: everything the loader gave"), ("F1", "this list"), ("F4", "the register"), ("F5", "verification, in full"), ("F10", "power off, after asking"),
    ("Ctrl+Alt+Delete", "stops a running program; no program can take that key")];

/// A flat plate with an outline: a tile, a card, a row. Text on it is read back from a picture of the screen
/// because its ground is one colour.
fn plate(x: u32, y: u32, w: u32, h: u32, colour: u32, outline: u32, lit: bool) -> impl Fn(u32, u32) -> u32 {
    let flat = move |_: u32, _: u32| colour;
    let _ = ui::panel(x, y, w, h, &flat, colour, 0, outline, lit);
    flat
}

/// The emblem on its plate where the field's light is, with the product's words beneath.
fn emblem_plate(field: &ui::Field, h: u32) {
    let over_field = |x: u32, y: u32| field.colour(x, y);
    let e = ui::edition();
    // The plate of the default backgrounds: 280 by 302 where the picture is 900 high.
    let (wide, tall) = (h * 280 / 900, h * 302 / 900);
    let (px, py) = (field.cx - wide / 2, field.cy - tall / 2);
    let on_plate = ui::panel(px, py, wide, tall, &over_field, ui::CORE_PLATE, 255, outline(), true);
    let bird = wide * 70 / 100;
    ui::emblem_over(px + (wide - bird * 121 / 132) / 2, py + (tall - bird) / 2, bird, &on_plate);
    // The edition is carried by the field and is also said in words, as on every cover.
    let words = format!("Parrot Council Protocol | {} Edition", e.name);
    ui::label(field.cx - ui::label_width(&words, false) / 2, py + tall + 30, &words, e.soft, false, &over_field);
    let motto = "Evidence before agreement.";
    ui::label(field.cx - ui::label_width(motto, false) / 2, py + tall + 60, motto, e.soft, false, &over_field);
}

/// The last programs that ran, newest last, as (time, name, outcome, note), read from the verified register.
fn recent_tasks(rec: &Record, programs: &[Entry], most: usize) -> Vec<(String, String, String, String)> {
    let events = match ledger::verify(rec.ledger().bytes()) {
        Ok((events, _)) => events,
        Err(_) => return Vec::new(),
    };
    let body = |e: &Value, k: &str| e.get("body").and_then(|b| b.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
    let kind = |e: &Value| e.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
    let mut labels: Vec<(String, String)> = Vec::new();
    let mut ended = Vec::new();
    for e in events.iter() {
        if kind(e) == "TASK_STARTED" && programs.iter().any(|p| p.name == body(e, "label")) {
            labels.push((body(e, "id"), body(e, "label")));
        } else if kind(e) == "TASK_ENDED" {
            if let Some((_, label)) = labels.iter().find(|(id, _)| *id == body(e, "id")) {
                let ts = e.get("ts").and_then(Value::as_str).unwrap_or("");
                ended.push((ts.get(11..19).unwrap_or("").to_string(), label.clone(), body(e, "outcome"), body(e, "note")));
            }
        }
    }
    let skip = ended.len().saturating_sub(most);
    ended.split_off(skip)
}

/// The desktop: the home surface. A status bar, the field with the emblem, and the keys; and beyond that only
/// what the person has put there: shortcuts to programs, one of them chosen, and widgets. With nothing placed
/// it is empty. On a display under 1280 by 720 the same things are said as plain lines.
#[allow(clippy::too_many_arguments)]
fn draw_home(start: &Start, programs: &[Entry], rec: &Record, shortcuts: &[usize], place: usize, whole: bool, widgets: &[bool; 3], store: &crate::regular::Files, mounted: bool, run: u32, message: &str, last_refusal: Option<(usize, &str)>, name_page: usize) -> usize {
    use ui::{label, AMBER, BAD, GOOD, SOFT, TEAL, TEXT};
    let safe = start.failed != 0;
    let recent = if widgets[0] { recent_tasks(rec, programs, 3) } else { Vec::new() };
    let (pool, free, _, _) = crate::mem::census();
    let files_line = if mounted { "VM files | saved when an app syncs" } else { "Session files | cleared at power-off" };
    let display = match dev::display() {
        Some(d) if d.width >= 1280 && d.height >= 720 && ui::scale() == 1 => d,
        _ => {
            use ui::{at, GROUND};
            ui::clear(GROUND);
            let (_, rows) = ui::grid();
            ui::emblem(20, 16, 36, GROUND);
            at(7, 1, "PCP OS | DESKTOP", TEXT, GROUND);
            at(2, 3, &verified_sentence(start, (ui::grid().0 as usize).saturating_sub(4)), if safe { BAD } else { GOOD }, GROUND);
            // The rows between the heading and the keys are shared out so that nothing is drawn over
            // anything else and nothing that is left out is left out in silence (after B3's windowed list).
            // First what must be there: one row of the list, holding the chosen place, for a chosen pin
            // its whole name and its size and digest, and what the last program left to say (up to three
            // rows). Then, as far as the rows go: a blank row above
            // the keys, the words 'Enter starts', a blank row above them, the widgets' lines, and more
            // rows of the list. Row 4 says which places are shown of how many, and how many widget lines
            // had no room. Every place is reached by moving the choice: the list follows it.
            let (cols, _) = ui::grid();
            let across = (cols as usize).saturating_sub(4).max(1);
            let selected = place.checked_sub(1).and_then(|at| shortcuts.get(at)).copied();
            let chosen_pin = selected.map(|i| &programs[i]);
            let name: Vec<String> = chosen_pin.map(|p| wrapped(&p.name, across)).unwrap_or_default();
            let mut widget_lines: Vec<String> = recent.iter().map(|(time, name, outcome, _)| format!("{} {} {}", time, outcome, name)).collect();
            if widgets[1] { widget_lines.push(format!("memory {} MiB, {} MiB free", pool * 4 / 1024, free * 4 / 1024)); }
            // On a display too narrow for the sentence the files' line has a shorter wording, whole.
            if widgets[2] { widget_lines.push(String::from(if files_line.len() <= across { files_line } else if mounted { "VM files: saved on sync" } else { "Files last until power-off" })); }
            let places = shortcuts.len() + 1;
            let body = (rows as usize).saturating_sub(7);
            let mut left = body.saturating_sub(1);
            // The name gives way only when the display has fewer rows than the name alone needs, and says so.
            let name_rows = if chosen_pin.is_some() { name.len().min(left.saturating_sub(1).max(1)) } else { 0 };
            // A name with more rows than there is room for is read a part at a time: Left and Right turn
            // its parts, and row 4 says which part of how many is shown. Every character can be read
            // before Enter (the ending alone does not tell two names apart that differ in the middle).
            let name_pages = if name_rows == 0 { 1 } else { (name.len() + name_rows - 1) / name_rows };
            let name_page = name_page.min(name_pages - 1);
            let facts = chosen_pin.is_some() && left > name_rows;
            left = left.saturating_sub(name_rows + facts as usize);
            // What the last program left to say, or why one was not started, comes next: a person who
            // pressed Enter and sees the desktop again must be told what happened.
            let said = wrapped(message.lines().next().unwrap_or(""), across);
            let said_rows = if message.is_empty() { 0 } else { said.len().min(3).min(left) };
            left -= said_rows;
            fn take(left: &mut usize, want: bool) -> bool { let got = want && *left > 0; if got { *left -= 1; } got }
            let gap_keys = take(&mut left, true);
            let caption = take(&mut left, chosen_pin.is_some());
            let gap_identity = take(&mut left, chosen_pin.is_some());
            let gap_widgets = !widget_lines.is_empty() && left >= 2 && take(&mut left, true);
            let widgets_shown = if gap_widgets { widget_lines.len().min(left) } else { 0 };
            left -= widgets_shown;
            let list_rows = (1 + left).min(places);
            let first = (place + 1).saturating_sub(list_rows);
            let mut notes: Vec<String> = Vec::new();
            // Row 4 says which places are shown and which part of the name, in the longest wording that fits
            // the row it has: the width decides, not a class of display (A3 found a width where the long
            // wording was cut and the short one would have fitted). A wording is never cut where a shorter
            // one says the same; the arrows and the numbers are the last things to go.
            let _ = &mut notes;
            let paged = name_pages > 1;
            let windowed = list_rows < places;
            let (at_place, of, part, parts) = (place + 1, places, name_page + 1, name_pages);
            let mut wordings: Vec<String> = Vec::new();
            match (windowed, paged) {
                (true, true) => {
                    wordings.push(format!("Places {} to {} of {} | name part {} of {}: Left/Right", first + 1, first + list_rows, of, part, parts));
                    wordings.push(format!("Place {}/{} | name {}/{} <- ->", at_place, of, part, parts));
                    wordings.push(format!("{}/{} | name {}/{} <- ->", at_place, of, part, parts));
                    wordings.push(format!("{}/{} | {}/{} <- ->", at_place, of, part, parts));
                }
                (true, false) => {
                    wordings.push(format!("Places {} to {} of {}", first + 1, first + list_rows, of));
                    wordings.push(format!("Place {}/{}", at_place, of));
                }
                (false, true) => {
                    wordings.push(format!("name part {} of {}: Left/Right", part, parts));
                    wordings.push(format!("name {}/{} <- ->", part, parts));
                    wordings.push(format!("{}/{} <- ->", part, parts));
                }
                (false, false) => {}
            }
            let fits = |text: &String| text.chars().count() <= across;
            let mut line = wordings.iter().find(|w| fits(w)).or(wordings.last()).cloned().unwrap_or_default();
            // How many widget lines had no room is never the part that is cut (B3): where row 4 cannot hold
            // it beside the rest, it has the last row of the display, below the keys.
            if widgets_shown < widget_lines.len() {
                let count = format!("{} widget line(s) not shown", widget_lines.len() - widgets_shown);
                let both = if line.is_empty() { count.clone() } else { format!("{} | {}", line, count) };
                if fits(&both) { line = both; } else { at(2, rows - 1, &cut(&count, across), SOFT, GROUND); }
            }
            if !line.is_empty() { at(2, 4, &cut(&line, across), SOFT, GROUND); }
            let mut row = 5;
            for i in first..first + list_rows {
                let ground = if i == place { ui::PANEL } else { GROUND };
                ui::fill(2 * ui::CELL_W, row * ui::CELL_H, (across as u32).min(44) * ui::CELL_W, ui::CELL_H, ground);
                at(3, row, &if i == 0 { String::from("Programs") } else { cut(&format!("PINNED {}", programs[shortcuts[i - 1]].name), across.min(42)) }, TEXT, ground);
                row += 1;
            }
            if let Some(p) = chosen_pin {
                row += gap_identity as u32;
                if let Some((_, reason)) = last_refusal.filter(|(i, _)| Some(*i) == selected) {
                    // With no caption row, use the space below the desktop title and to the right of the emblem.
                    let (x, y, room) = if caption { (2, row, across) } else { (7, 2, (cols as usize).saturating_sub(7)) };
                    let why = if reason == "the program could not be admitted as a task" && "Not started: ".len() + reason.len() > room {
                        "not admitted"
                    } else { reason };
                    at(x, y, &cut(&format!("Not started: {}", why), room), BAD, GROUND);
                } else if caption {
                    at(2, row, "Enter starts", GOOD, GROUND);
                }
                row += caption as u32;
                for line in name.iter().skip(name_page * name_rows).take(name_rows) {
                    at(2, row, line, TEXT, GROUND);
                    row += 1;
                }
                // The last part may be shorter: its rows stay the name's, so that what follows does not move.
                row += (name_rows - name.iter().skip(name_page * name_rows).take(name_rows).count()) as u32;
                if facts {
                    // Where the row is too short for the usual words, the digest gets the room: as many of
                    // its first sixteen digits as fit, then "..".
                    let usual = format!("{}   SHA-256 {}...", size_text(p.image.len()), &p.digest[..16]);
                    let line = if usual.chars().count() <= across { usual } else {
                        let size = size_text(p.image.len());
                        let digits = across.saturating_sub(size.chars().count() + 7).min(16);
                        format!("{} sha {}..", size, &p.digest[..digits])
                    };
                    at(2, row, &cut(&line, across), SOFT, GROUND);
                    row += 1;
                }
            }
            for (n, line) in said.iter().enumerate().take(said_rows) {
                at(2, row, &if n + 1 == said_rows && said.len() > said_rows { cut(&format!("{}...", line), line.chars().count()) } else { line.clone() }, ui::AMBER, GROUND);
                row += 1;
            }
            row += gap_widgets as u32;
            for line in widget_lines.iter().take(widgets_shown) {
                at(2, row, &cut(line, across), SOFT, GROUND);
                row += 1;
            }
            let _ = gap_keys;
            // On a display too narrow for the whole line, the keys that open the list of keys and that
            // stop the machine are the ones kept.
            at(2, rows - 2, if across >= 47 { "Up/Down choose   Enter open   F1 keys   F10 off" } else { "Enter open  F1 keys  F10 off" }, TEXT, GROUND);
            return name_pages;
        }
    };
    let _ = name_page;
    let (w, h) = (display.width, display.height);
    let field = ui::Field::of(w, h);
    field.paint();
    let over_field = |x: u32, y: u32| field.colour(x, y);
    let left = w / 200 * 10;
    let mut y = 60;

    // Widgets the person placed.
    let wide = 860;
    if widgets[0] {
        let on_card = plate(left, y, wide, 140, PLATE, edge(), false);
        label(left + 20, y + 20, "RECENT ACTIVITY", TEAL, false, &on_card);
        if recent.is_empty() {
            label(left + 20, y + 60, "No program has run in this session.", SOFT, false, &on_card);
        }
        for (i, (time, name, outcome, note)) in recent.iter().enumerate() {
            let line = y + 60 + i as u32 * 20;
            let x = label(left + 20, line, time, SOFT, false, &on_card);
            let x = label(x + 20, line, outcome, if outcome == "DONE" { GOOD } else { AMBER }, false, &on_card);
            let x = label(x + 20, line, name, TEXT, false, &on_card);
            let room = ((left + wide).saturating_sub(x + 40) / 10) as usize;
            label(x + 20, line, &cut(&note, room), SOFT, false, &on_card);
        }
        y += 160;
    }
    let mut x = left;
    if widgets[1] {
        let on_machine = plate(x, y, 420, 140, PLATE, edge(), false);
        label(x + 20, y + 20, "THIS MACHINE", TEAL, false, &on_machine);
        label(x + 20, y + 60, &format!("memory {} MiB, {} MiB free", pool * 4 / 1024, free * 4 / 1024), TEXT, false, &on_machine);
        label(x + 20, y + 80, &format!("display {} x {}", w, h), TEXT, false, &on_machine);
        label(x + 20, y + 100, &format!("keyboard PS/2, sound {}", if start.sound { "present" } else { "absent" }), TEXT, false, &on_machine);
        x += 440;
    }
    if widgets[2] {
        let on_session = plate(x, y, 420, 140, PLATE, edge(), false);
        label(x + 20, y + 20, "SESSION", TEAL, false, &on_session);
        let up = arch::ticks() / arch::TICKS_PER_SECOND;
        label(x + 20, y + 60, &format!("up {} min, {} program(s) run, {} file(s)", up / 60, run, store.len()), TEXT, false, &on_session);
        label(x + 20, y + 80, files_line, TEXT, false, &on_session);
        label(x + 20, y + 100, if safe { "SAFE: programs are disabled" } else { "network: none" }, if safe { BAD } else { TEXT }, false, &on_session);
    }
    if widgets[1] || widgets[2] {
        y += 160;
    }
    // What the last program left to say is shown once, where the person is looking.
    // Each of its lines is wrapped to the width of the column it is in, so that none runs off the screen;
    // up to five rows, and the last says so if there was more.
    let across = ((w * 47 / 100).min(1100) / 10).max(20) as usize;
    let said: Vec<String> = message.lines().flat_map(|line| wrapped(line, across)).collect();
    for (i, line) in said.iter().enumerate().take(5) {
        label(left, y + i as u32 * 20, &if i == 4 && said.len() > 5 { cut(&format!("{}...", line), line.chars().count()) } else { line.clone() }, ui::edition().accent, false, &over_field);
    }

    emblem_plate(&field, h);
    if whole {
        whole_bar(start, programs, shortcuts, place, w, h, last_refusal);
    } else {
        dock(programs, shortcuts, place, w, h, last_refusal);
        corners(start, w, h, &[("F1", "keys")]);
    }
    1
}

/// The settings: how the desktop looks, and which widgets are on it. Nothing here changes by one key from
/// the desktop; a person comes here, chooses, and sees what is chosen. They last for this session.
fn settings_view(whole_bar: &mut bool, widgets: &mut [bool; 3]) {
    use ui::{at, GROUND, PANEL, SOFT, TEXT};
    let (cols, rows) = ui::grid();
    let forms = ["A bar across the screen", "A dock in the middle"];
    // Where there is room the screen is set out with space around its parts: eleven rows and the keys two
    // from the bottom, which takes fourteen rows, and the keys' line takes forty-one columns. On a smaller
    // display every choice still has a row of its own that nothing else is drawn on, and its state on it:
    // the title on the first row, the keys on the last, and between them the two headings and the five
    // choices, moved so that the chosen one is among them if they cannot all be there.
    let roomy = rows >= 14 && cols >= 41;
    let width = cols.saturating_sub(2).min(46);
    // (heading, or which choice it is)
    let items: [(Option<&str>, usize); 7] = [(Some(if roomy { "Programs at the foot of the desktop" } else { "Foot of the desktop" }), 0), (None, 0), (None, 1),
        (Some(if roomy { "Widgets on the desktop" } else { "Widgets" }), 0), (None, 2), (None, 3), (None, 4)];
    let mut at_row = 0usize;
    // Drawn when opened and when a key changed something, and at no other time.
    let mut draw = true;
    loop {
        if draw {
        ui::clear(GROUND);
        let (first_row, room) = if roomy {
            at(2, 1, "SETTINGS", ui::TEAL, GROUND);
            at(2, 2, "They last for this session.", SOFT, GROUND);
            at(2, rows - 2, "Up/Down choose    Enter set    Esc back", SOFT, GROUND);
            (4u32, items.len())
        } else {
            at(2, 0, "SETTINGS (this session)", ui::TEAL, GROUND);
            at(2, rows - 1, "Up/Down  Enter set  Esc back", SOFT, GROUND);
            (1u32, (rows.saturating_sub(2) as usize).clamp(1, items.len()))
        };
        // The chosen choice is always among the rows shown, with its heading above it if there is room.
        let chosen_item = items.iter().position(|(heading, i)| heading.is_none() && *i == at_row).unwrap_or(1);
        let top = if room >= items.len() { 0 } else { (chosen_item + 1).saturating_sub(room).min(items.len() - room) };
        let mut row = first_row;
        for (n, (heading, i)) in items.iter().enumerate().skip(top).take(room) {
            // The roomy screen leaves a row empty above its second heading.
            if roomy && n == 3 {
                row += 1;
            }
            match heading {
                Some(words) => { at(2, row, words, TEXT, GROUND); }
                None => {
                    let ground = if *i == at_row { PANEL } else { GROUND };
                    ui::fill(ui::CELL_W, row * ui::CELL_H, width * ui::CELL_W, ui::CELL_H, ground);
                    if *i < 2 {
                        let on = (*i == 0) == *whole_bar;
                        at(2, row, if on { "(o)" } else { "( )" }, if on { ui::GOOD } else { SOFT }, ground);
                        at(8, row, forms[*i], TEXT, ground);
                    } else {
                        let on = widgets[*i - 2];
                        at(2, row, if on { "[on]" } else { "[  ]" }, if on { ui::GOOD } else { SOFT }, ground);
                        at(8, row, WIDGETS[*i - 2], TEXT, ground);
                    }
                }
            }
            row += 1;
        }
        }
        draw = true;
        match wait_key() {
            (0x48, _) if at_row > 0 => at_row -= 1,
            (0x50, _) if at_row + 1 < forms.len() + WIDGETS.len() => at_row += 1,
            // Choosing the form that is already chosen changes nothing, and so draws nothing (B3's measurement).
            (0x1C, _) if at_row < 2 && *whole_bar != (at_row == 0) => *whole_bar = at_row == 0,
            (0x1C, _) if at_row >= 2 => widgets[at_row - 2] = !widgets[at_row - 2],
            (0x01, _) => return,
            _ => draw = false,
        }
    }
}

/// The files programs have written, by name and size. Every one can be reached: Up and Down move the list
/// by a line, Page Up and Page Down by a screenful, and the heading says which are shown of how many. A
/// name too long for its line is on as many lines as it needs. Esc or Enter goes back.
fn files_view(store: &crate::regular::Files, mounted: bool) {
    use ui::{at, GROUND, SOFT, TEXT};
    let (cols, rows) = ui::grid();
    // The name has the columns up to the one before its size, which begins fourteen from the right.
    let across = (cols as usize).saturating_sub(17).max(8);
    // Each file as the lines it takes: its name, wrapped, with its size on the first.
    let mut lines: Vec<(String, String)> = Vec::new();
    let mut begins: Vec<usize> = Vec::new();
    for (name, bytes) in store.iter() {
        begins.push(lines.len());
        for (n, part) in wrapped(name, across).into_iter().enumerate() {
            lines.push((part, if n == 0 { size_text(bytes.len()) } else { String::new() }));
        }
    }
    let total: usize = store.iter().map(|f| f.1.len()).sum();
    let page = rows.saturating_sub(8).max(1) as usize;
    let last_top = lines.len().saturating_sub(page);
    let mut top = 0usize;
    loop {
        ui::clear(GROUND);
        // The files of which any line is on the screen: a long name that began on the page before still
        // has its last lines here, and it is one of the files shown.
        let shown: Vec<usize> = (0..begins.len()).filter(|i| {
            let end = begins.get(i + 1).copied().unwrap_or(lines.len());
            begins[*i] < top + page && end > top
        }).collect();
        let heading = match (shown.first(), shown.last()) {
            (Some(a), Some(b)) => format!("FILES: showing {} to {} of {}, {}", a + 1, b + 1, store.len(), size_text(total)),
            _ => format!("FILES: {} file(s), {}", store.len(), size_text(total)),
        };
        let available = cols.saturating_sub(2) as usize;
        let heading = match (shown.first(), shown.last()) {
            (Some(a), Some(b)) if heading.len() > available =>
                format!("FILES: {}-{}/{} | {}", a + 1, b + 1, store.len(), size_text(total)),
            _ => heading,
        };
        at(2, 1, &heading, ui::TEAL, GROUND);
        // Row 3 is empty above the list, and takes the end of this line on a display too narrow for it.
        for (n, part) in wrapped_words(if mounted { "VM files | saved when an app syncs" } else { "Session files | cleared at power-off" }, available.max(1)).iter().enumerate().take(2) {
            at(2, 2 + n as u32, part, SOFT, GROUND);
        }
        if store.len() == 0 {
            at(2, 4, "No program has written a file.", SOFT, GROUND);
        }
        for (n, (part, size)) in lines.iter().skip(top).take(page).enumerate() {
            at(2, 4 + n as u32, part, TEXT, GROUND);
            at(cols.saturating_sub(14), 4 + n as u32, size, SOFT, GROUND);
        }
        let page_keys = "Up/Down, Page Up/Page Down move    Esc back";
        let keys = if last_top == 0 { "Esc back" } else if page_keys.len() <= available {
            page_keys
        } else { "Up/Down PgUp/PgDn  Esc back" };
        at(2, rows - 2, keys, SOFT, GROUND);
        match wait_key() {
            (0x01, _) | (0x1C, _) => return,
            (0x48, _) if top > 0 => top -= 1,
            (0x50, _) if top < last_top => top += 1,
            (0x49, _) => top = top.saturating_sub(page),
            (0x51, _) => top = (top + page).min(last_top),
            _ => {}
        }
    }
}

/// The launcher: the programs the loader gave, one chosen, each with what it may use. It is opened from the
/// desktop and left with Esc. On a display under 1280 by 720 the plain list below is drawn instead.
fn draw_desktop(start: &Start, programs: &[Entry], rec: &Record, chosen: usize, message: &str, shortcuts: &[usize]) {
    use ui::{label, label_width, AMBER, BAD, SOFT, TEXT};
    let display = match dev::display() {
        Some(d) if d.width >= 1280 && d.height >= 720 && ui::scale() == 1 => d,
        _ => return draw_desktop_plain(start, programs, chosen, message),
    };
    let (w, h) = (display.width, display.height);
    let safe = start.failed != 0;
    let field = ui::Field::of(w, h);
    field.paint();
    let over_field = |x: u32, y: u32| field.colour(x, y);

    let left = w / 200 * 10;
    let list_width = (w * 47 / 100).min(1100) / 10 * 10;
    label(left, 60, "Programs", ui::edition().text, true, &over_field);
    let on_note = plate(left, 110, list_width, 40, PLATE, edge(), false);
    label(left + 20, 120, if safe { "SAFE: verification failed. Programs are disabled." } else if programs.is_empty() { "The loader handed over no program." }
                               // At 1280 across the plate holds fifty-six characters, four fewer than the longer wording.
                               else if list_width >= 640 { "Choose one to start. Each runs with only what it is granted." }
                               else { "Choose one. Each runs with only what it is granted." }, if safe { BAD } else { SOFT }, false, &on_note);
    let none = option(&start.boot.command_line, "grant") == Some("none");
    let grants = if none { "console only" } else if start.sound { "display, keyboard, sound, loader files, store" } else { "display, keyboard, loader files, store" };
    let foot = h / 20 * 20 - 60;
    let _ = rec;
    // What a program may use is never cut short and never drawn over: it takes as many lines as it needs,
    // and that a program is pinned is said on a line of its own.
    let across = (list_width / 10).saturating_sub(6) as usize;
    let access_of = |p: &Entry| -> Vec<String> {
        #[allow(unused_mut)]
        let mut access = String::from(grants);
        #[cfg(feature = "guest-ethernet")]
        if !none && ethernet_grant(start.boot.exact_command_line(), &p.digest) { access.push_str(", raw Ethernet frames"); }
        #[cfg(feature = "game-link")]
        if !none && option(&start.boot.command_line, "link_image") == Some(p.digest.as_str()) { access.push_str(", second serial port"); }
        let _ = p;
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::from("may use:");
        for (n, part) in access.split(", ").enumerate() {
            let piece = if n == 0 { format!(" {}", part) } else { format!(", {}", part) };
            if line.chars().count() + piece.chars().count() > across && line != "may use:" {
                lines.push(line + ",");
                line = format!("         {}", part);
            } else {
                line.push_str(&piece);
            }
        }
        lines.push(line);
        lines
    };
    let height_of = |i: usize, p: &Entry| 40 + 20 * (access_of(p).len() as u32 + shortcuts.contains(&i) as u32);
    let tallest = programs.iter().enumerate().map(|(i, p)| height_of(i, p)).max().unwrap_or(60);
    let mut y = 170;
    let room = ((foot.saturating_sub(y + 120)) / (tallest + 20)).max(1) as usize;
    let first = (chosen + 1).saturating_sub(room);
    for (i, p) in programs.iter().enumerate().skip(first).take(room) {
        let tall = height_of(i, p);
        let on_row = plate(left, y, list_width, tall, if i == chosen { PLATE_CHOSEN } else { PLATE }, if i == chosen { outline() } else { edge() }, i == chosen);
        if i == chosen {
            dev::display_fill(left, y + 8, 4, tall - 16, AMBER);
        }
        // The row says, on the character grid: which is chosen, its name, its size and digest, and what it may use.
        label(left + 20, y + 10, if i == chosen { ">" } else { " " }, AMBER, false, &on_row);
        let size = size_text(p.image.len());
        let digest = format!("SHA-256 {}...", &p.digest[..16]);
        let size_at = (left + list_width - 60 - label_width(&digest, false) - label_width(&size, false)) / 10 * 10;
        let name_room = (size_at.saturating_sub(left + 60) / 10).max(4) as usize;
        label(left + 40, y + 10, &cut(&p.name, name_room.min(28)), TEXT, false, &on_row);
        label((left + list_width - 20 - label_width(&digest, false)) / 10 * 10, y + 10, &digest, SOFT, false, &on_row);
        label(size_at, y + 10, &size, SOFT, false, &on_row);
        let lines = access_of(p);
        for (n, line) in lines.iter().enumerate() {
            label(left + 40, y + 30 + n as u32 * 20, line, SOFT, false, &on_row);
        }
        if shortcuts.contains(&i) {
            label(left + 40, y + 30 + lines.len() as u32 * 20, "pinned to the desktop", ui::GOOD, false, &on_row);
        }
        y += tall + 20;
    }
    if !message.is_empty() {
        let lines = message.lines().take(3).count() as u32;
        let on_message = plate(left, y + 10, list_width, 20 + lines * 20, PLATE, edge(), false);
        for (i, line) in message.lines().take(3).enumerate() {
            label(left + 20, y + 20 + i as u32 * 20, &cut(line, (list_width / 10 - 4) as usize), AMBER, false, &on_message);
        }
    }

    emblem_plate(&field, h);
    corners(start, w, h, &[("Esc", "desktop"), ("F1", "keys")]);
}

fn draw_desktop_plain(start: &Start, programs: &[Entry], chosen: usize, message: &str) {
    use ui::{at, GROUND, PANEL, SOFT, TEXT};
    ui::clear(GROUND);
    let (cols, rows) = ui::grid();
    let across = (cols as usize).saturating_sub(4).max(1);
    // What the chosen program may use, said only when programs can be started.
    let mut access: Vec<(String, u32)> = Vec::new();
    if let Some(_entry) = programs.get(chosen).filter(|_| start.failed == 0) {
        let none = option(&start.boot.command_line, "grant") == Some("none");
        access.push((String::from(if none { "Access: console only" } else if start.sound {
            "Access: console, display, keyboard, files, store, sound"
        } else { "Access: console, display, keyboard, files and store" }), SOFT));
        #[cfg(feature = "guest-ethernet")]
        if !none && ethernet_grant(start.boot.exact_command_line(), &_entry.digest) {
            access.push((String::from("Access: raw Ethernet frames"), ui::AMBER));
        }
        #[cfg(feature = "game-link")]
        if !none && option(&start.boot.command_line, "link_image") == Some(_entry.digest.as_str()) {
            access.push((String::from("Access: second serial port"), ui::AMBER));
        }
    }
    // The keys named are function keys and Esc, as on every other surface; letters do nothing here.
    let keys = if programs.is_empty() || start.failed != 0 {
        "Esc back   F1 keys   F10 off"
    } else { "Up/Down choose   Enter open   Esc back   F1 keys   F10 off" };
    // The message has two rows. What does not fit on them ends in "..".
    let said: Vec<String> = if message.is_empty() { Vec::new() } else {
        let lines = wrapped(message.lines().next().unwrap_or(""), across);
        lines.iter().enumerate().take(2).map(|(n, line)| if n == 1 && lines.len() > 2 { cut(&format!("{}...", line), line.chars().count()) } else { line.clone() }).collect()
    };
    // With thirteen rows and sixty-four columns the screen is set out with space around its parts, each
    // at a row of its own. With fewer, the same things share the rows from the edges inward, so that none
    // is drawn on another: the sentence about verification on the first row; the keys on the last, on two
    // rows if one does not hold them; above the keys what the chosen program may use, on as many rows as
    // it needs; above that the message; and every row between is the list, which always has one row and
    // always holds the chosen program.
    let (list_row, visible) = if rows >= 13 && cols >= 64 {
        ui::emblem(20, 16, 36, GROUND);
        at(7, 1, "PCP OS", TEXT, GROUND);
        // The same sentence about verification as on every other surface, whichever way it went.
        if start.failed != 0 {
            // Both sentences on row 3 where they fit; where they do not (A3: at 640 across the last period
            // was off the screen), the second has row 4, which is empty above the list.
            let failed = format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed);
            let both = format!("{} Programs are disabled.", failed);
            if both.len() <= across {
                at(2, 3, &both, ui::BAD, GROUND);
            } else {
                at(2, 3, &failed, ui::BAD, GROUND);
                at(2, 4, "Programs are disabled.", ui::BAD, GROUND);
            }
        } else {
            at(2, 3, &format!("Verified at start: all {} self-tests passed.", start.passed), ui::GOOD, GROUND);
        }
        for (n, (line, colour)) in access.iter().enumerate() {
            at(2, rows - 7 + n as u32, line, *colour, GROUND);
        }
        for (n, line) in said.iter().enumerate() {
            at(2, rows - 4 + n as u32, line, ui::AMBER, GROUND);
        }
        at(2, rows - 2, keys, TEXT, GROUND);
        (5u32, rows.saturating_sub(12).max(1) as usize)
    } else {
        at(2, 0, &verified_sentence(start, across), if start.failed != 0 { ui::BAD } else { ui::GOOD }, GROUND);
        let key_lines: Vec<String> = if keys.len() <= across { alloc::vec![String::from(keys)] } else if programs.is_empty() || start.failed != 0 {
            alloc::vec![String::from(keys)]
        } else {
            alloc::vec![String::from("Up/Down choose   Enter open"), String::from("Esc back   F1 keys   F10 off")]
        };
        // After a failed start the list can be read and nothing in it can be started; that is said, in
        // the words the roomy screen uses.
        let mut below: Vec<(String, u32)> = if start.failed != 0 { alloc::vec![(String::from("Programs are disabled."), ui::BAD)] } else { Vec::new() };
        below.extend(said.iter().map(|line| (line.clone(), ui::AMBER)));
        for (line, colour) in &access {
            below.extend(wrapped_words(line, across).into_iter().map(|part| (part, *colour)));
        }
        below.extend(key_lines.into_iter().map(|line| (line, TEXT)));
        // The list keeps one row whatever else there is; then the keys are kept, then what comes above them.
        let room = (rows as usize).saturating_sub(2);
        let skip = below.len().saturating_sub(room);
        let first = rows as usize - (below.len() - skip);
        for (n, (line, colour)) in below.iter().skip(skip).enumerate() {
            at(2, (first + n) as u32, &cut(line, across), *colour, GROUND);
        }
        (1u32, first.saturating_sub(1).max(1))
    };
    let first = (chosen + 1).saturating_sub(visible);
    if programs.is_empty() {
        at(2, list_row, "No programs loaded", SOFT, GROUND);
    }
    for (i, p) in programs.iter().enumerate().skip(first).take(visible) {
        let row = list_row + (i - first) as u32;
        let width = cols.saturating_sub(4).min(44);
        let bg = if i == chosen { PANEL } else { GROUND };
        ui::fill(2 * ui::CELL_W, row * ui::CELL_H, width * ui::CELL_W, ui::CELL_H, bg);
        if i == chosen { ui::fill(2 * ui::CELL_W, row * ui::CELL_H, 3, ui::CELL_H, ui::TEAL); }
        at(3, row, &cut(&p.name, width.saturating_sub(2) as usize), TEXT, bg);
    }
}

/// The next key pressed: (its number, whether it is of the second group). Releases are passed over.
fn wait_key() -> (u8, bool) {
    loop {
        match dev::key_take() {
            Some(k) if !k.released => return (k.code, k.extended),
            Some(_) => {}
            None => arch::wait_for_interrupt(),
        }
    }
}

/// The next key pressed, with whether Ctrl was held: (its number, second group, Ctrl).
fn wait_chord() -> (u8, bool, bool) {
    loop {
        match dev::key_take() {
            Some(k) if !k.released => return (k.code, k.extended, k.ctrl),
            Some(_) => {}
            None => arch::wait_for_interrupt(),
        }
    }
}

/// How many places the foot of the desktop has, in either form, for a display this wide: the way to all
/// the programs and then what is pinned. Both forms show the same places.
fn foot_room(w: u32) -> usize {
    ((w.saturating_sub(700)) / 190).max(1) as usize
}

/// Ask before the machine is stopped. Keys pressed before the question was on the screen are forgotten, so
/// that only an Enter pressed in answer to it switches off. Esc, or any other key, goes back to where the
/// person was, with what was chosen there unchanged.
fn power_prompt() -> bool {
    use ui::{at, GROUND, SOFT, TEXT};
    let (cols, rows) = ui::grid();
    let across = (cols as usize).saturating_sub(4).max(1);
    ui::clear(GROUND);
    // The warning is whole on every display. With 79 columns it is on the two rows it has always had;
    // with fewer it is broken between words. Where its last row would not leave an empty row above the
    // keys, or the keys do not fit their row, the screen begins on the first row and each answer has a
    // row of its own at the bottom.
    let two = ["The register is closed and verified first. Files of this session that were", "not saved by a program are lost."];
    let warning: Vec<String> = if two[0].len() <= across { two.iter().map(|l| l.to_string()).collect() } else { wrapped_words(&two.join(" "), across) };
    let answers = "Enter switch off    Esc go back";
    if 5 + warning.len() as u32 + 1 <= rows.saturating_sub(2) && answers.len() <= across {
        at(2, 1, "POWER OFF", ui::AMBER, GROUND);
        at(2, 3, "Switch PCP OS off?", TEXT, GROUND);
        for (n, line) in warning.iter().enumerate() {
            at(2, 5 + n as u32, line, SOFT, GROUND);
        }
        at(2, rows - 2, answers, TEXT, GROUND);
    } else {
        at(2, 0, "POWER OFF", ui::AMBER, GROUND);
        at(2, 1, "Switch PCP OS off?", TEXT, GROUND);
        // The answers are never given up for the warning: they have the last rows, and the warning the rows between.
        let split = answers.len() > across;
        let answer_rows = if split { 2 } else { 1 };
        let room = (rows.saturating_sub(2 + answer_rows) as usize).max(1);
        for (n, line) in warning.iter().enumerate().take(room) {
            at(2, 2 + n as u32, &if n + 1 == room && warning.len() > room { cut(&format!("{}...", line), across) } else { line.clone() }, SOFT, GROUND);
        }
        if split {
            at(2, rows.saturating_sub(2), "Enter switch off", TEXT, GROUND);
            at(2, rows.saturating_sub(1), "Esc go back", TEXT, GROUND);
        } else {
            at(2, rows.saturating_sub(1), answers, TEXT, GROUND);
        }
    }
    // Only an Enter pressed in answer switches off. A key that is held repeats its press, and those repeats
    // are not an answer, whichever Enter key they come from and whichever is let go first: a press counts
    // only when both Enter keys (the main one and the number pad's) were up before it. Which are down is
    // taken at the moment the waiting keys are forgotten and then followed from the events themselves, in
    // their order. Any other key pressed goes back.
    let (mut main, mut pad) = dev::keys_forget_enter();
    loop {
        match dev::key_take() {
            Some(k) if k.code == 0x1C => {
                let was_clear = !main && !pad;
                if k.extended { pad = !k.released } else { main = !k.released }
                if !k.released && was_clear {
                    return true;
                }
            }
            Some(k) if !k.released && k.code != 0x1D && k.code != 0x38 && k.code != 0x44 => return false,
            Some(_) => {}
            None => arch::wait_for_interrupt(),
        }
    }
}

fn draw_menu(start: &Start, programs: &[Entry], files: &[(String, &'static [u8], String)], rec: &Record, chosen: usize, message: &str, stored: usize,
             #[cfg(feature = "vm-store")] mounted: bool) {
    use ui::{at, GROUND, PANEL, SOFT, TEXT};
    ui::clear(GROUND);
    let (cols, rows) = ui::grid();
    ui::emblem(30, 24, 132, GROUND);
    at(19, 2, "PCP OS", TEXT, GROUND);
    at(26, 2, &format!("kernel {}", start.version), SOFT, GROUND);
    at(19, 3, "Parrot Council Protocol. Evidence before agreement.", SOFT, GROUND);
    let verified = if start.failed == 0 { format!("Verified at start: all {} self-tests passed.", start.passed) } else { format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed) };
    at(19, 5, &verified, if start.failed == 0 { ui::GOOD } else { ui::BAD }, GROUND);
    let head = rec.ledger().head();
    at(19, 6, &format!("Register: {} events, head {}...", rec.ledger().len(), &head[..head.len().min(16)]), SOFT, GROUND);
    let mut row = 9;
    at(2, row, "PROGRAMS", ui::TEAL, GROUND);
    row += 1;
    if programs.is_empty() {
        at(4, row, "The loader handed over no program.", SOFT, GROUND);
        row += 1;
    }
    for (i, p) in programs.iter().enumerate() {
        let ground = if i == chosen { PANEL } else { GROUND };
        ui::fill(ui::CELL_W, row * ui::CELL_H, (cols - 2) * ui::CELL_W, 2 * ui::CELL_H, ground);
        at(2, row, if i == chosen { ">" } else { " " }, ui::AMBER, ground);
        at(4, row, &p.name, TEXT, ground);
        at(26, row, &size_text(p.image.len()), SOFT, ground);
        at(38, row, &format!("SHA-256 {}...", &p.digest[..16]), SOFT, ground);
        if !p.arguments.is_empty() {
            at(66, row, &format!("with: {}", p.arguments.chars().take(30).collect::<String>()), SOFT, ground);
        }
        at(4, row + 1, if start.sound { "starting it lets it use: the display, the keyboard, sound, the loader's files, the store" } else { "starting it lets it use: the display, the keyboard, the loader's files, the store" }, SOFT, ground);
        row += 2;
        #[cfg(feature = "guest-ethernet")]
        if ethernet_grant(start.boot.exact_command_line(), &p.digest) {
            at(4, row, "Ethernet grant: this program can send and receive raw network frames.", SOFT, ground);
            row += 1;
        }
        #[cfg(feature = "game-link")]
        if option(&start.boot.command_line, "grant") != Some("none") && option(&start.boot.command_line, "link_image") == Some(p.digest.as_str()) {
            at(4, row, "Optional game cable: this image can read and write the second serial port.", SOFT, ground);
            row += 1;
        }
    }
    row += 1;
    at(2, row, "FILES FROM THE LOADER", ui::TEAL, GROUND);
    row += 1;
    for f in files.iter().take(8) {
        at(4, row, &f.0, TEXT, GROUND);
        at(26, row, &size_text(f.1.len()), SOFT, GROUND);
        at(38, row, &format!("SHA-256 {}...", &f.2[..16]), SOFT, GROUND);
        row += 1;
    }
    if files.is_empty() {
        at(4, row, "None.", SOFT, GROUND);
        row += 1;
    }
    row += 1;
    at(2, row, "MACHINE", ui::TEAL, GROUND);
    row += 1;
    let (pool, free, _, _) = crate::mem::census();
    at(4, row, &format!("memory for programs: {} MiB, {} MiB free", pool * 4096 >> 20, free * 4096 >> 20), SOFT, GROUND);
    row += 1;
    if let Some(d) = start.display {
        at(4, row, &format!("display: {}, {} page(s); keyboard: PS/2; sound: {}", if d.kind == dev::DisplayKind::Bochs { "Bochs interface, any mode" } else { "mode set by the loader" }, d.pages, if start.sound { "present" } else { "none" }), SOFT, GROUND);
        row += 1;
        let retention = "kept until power off";
        #[cfg(feature = "vm-store")]
        let retention = if mounted { "saved to VM disk on fsync" } else { retention };
        at(4, row, &format!("store: {} file(s) written by programs, {}", stored, retention), SOFT, GROUND);
    }
    row += 2;
    if !message.is_empty() {
        for (n, part) in message.split('\n').enumerate() {
            at(2, row + n as u32, &part.chars().take(cols as usize - 4).collect::<String>(), ui::AMBER, GROUND);
        }
    }
    ui::fill(0, (rows - 2) * ui::CELL_H, cols * ui::CELL_W, 2 * ui::CELL_H, PANEL);
    at(2, rows - 2, "Up/Down choose    Enter start    D desktop    R register    P off", TEXT, PANEL);
    at(2, rows - 1, "Ctrl+Alt+Delete stops a running program and returns here. No program can take that key.", SOFT, PANEL);
}

/// The last screen on a machine this kernel cannot switch off: that it has stopped, whether it verified
/// itself (`passed` and `failed` as they are at the very end), why nothing could be chosen if nothing
/// could, and what the person does now. The display is
/// cleared first, so that nothing a firmware or a program left is read as part of it, and every sentence
/// is whole on a display of any size. Nothing is waited for: the processor is halted after it.
pub fn stopped_screen(start: &Start, failed: u32, passed: u32) {
    use ui::{at, GROUND, SOFT, TEXT};
    let (cols, rows) = ui::grid();
    let across = (cols as usize).saturating_sub(4).max(1);
    // Each thing to say, with how soon it is given up on a display with too few rows (0: never). What
    // the person must do is never given up: it is the one thing they cannot find out any other way.
    let mut parts: Vec<(u8, String, u32)> = Vec::new();
    parts.push((0, String::from("PCP OS has stopped."), ui::AMBER));
    parts.push((3, String::new(), TEXT));
    parts.push((1, if failed != 0 { format!("NOT VERIFIED: {} of {} self-tests failed.", failed, passed + failed) } else { format!("Verified: all {} self-tests it ran passed.", passed) },
                if failed != 0 { ui::BAD } else { ui::GOOD }));
    if let Some(why) = start.not_run.filter(|_| failed == 0) {
        parts.push((2, format!("Not run: the six tests that press keys, because {}.", why), SOFT));
    }
    if let Some(why) = start.no_keyboard.filter(|_| start.display.is_some()) {
        parts.push((3, String::new(), TEXT));
        parts.push((1, String::from(why), TEXT));
    }
    parts.push((3, String::new(), TEXT));
    parts.push((2, String::from("Nothing is running and nothing more will be written."), TEXT));
    parts.push((0, String::from("This kernel cannot switch the computer off. Hold its power button until it goes dark."), TEXT));
    let lines_of_parts = |parts: &[(u8, String, u32)]| -> Vec<(String, u32)> {
        parts.iter().flat_map(|(_, text, colour)| if text.is_empty() { alloc::vec![(String::new(), *colour)] } else {
            wrapped_words(text, across).into_iter().map(|part| (part, *colour)).collect()
        }).collect()
    };
    // Give up, in turn, the empty rows and then what is least needed, until the rest fits. The verdict
    // and the reason there is no keyboard are given up only one at a time and last of all, the reason
    // before the verdict: on the smallest display the kernel has (ten rows of thirty-two columns) all of
    // the title, the verdict, each of the three reasons and what to do fit, and a guest check holds
    // that so (a first form dropped both together when a longer reason needed one row more).
    for giving_up in [3u8, 2] {
        if lines_of_parts(&parts).len() <= rows as usize {
            break;
        }
        parts.retain(|(rank, _, _)| *rank != giving_up);
    }
    while lines_of_parts(&parts).len() > rows as usize {
        match parts.iter().rposition(|(rank, _, _)| *rank == 1) {
            Some(at) => { parts.remove(at); }
            None => break,
        }
    }
    let lines = lines_of_parts(&parts);
    ui::clear(GROUND);
    let first = if rows as usize > lines.len() + 2 { 1 } else { 0 };
    for (n, (line, colour)) in lines.iter().enumerate().take(rows as usize) {
        at(2, first + n as u32, line, *colour, GROUND);
    }
}

/// Everything the loader gave and what the machine has, as a screen of text for a display too narrow for
/// the plain list: nothing is cut, digests are whole, and nothing here starts or changes anything.
fn details_view(start: &Start, programs: &[Entry], files: &[(String, &'static [u8], String)], rec: &Record, stored: usize, mounted: bool) {
    use ui::{SOFT, TEXT};
    let across = (ui::grid().0 as usize).saturating_sub(4).max(1);
    let whole = |col: u32, text: &str, colour: u32| -> Vec<Line> {
        wrapped(text, across.saturating_sub(col as usize - 2).max(1)).into_iter().map(|part| alloc::vec![(col, part, colour)]).collect()
    };
    let mut lines: Vec<Line> = Vec::new();
    lines.extend(lines_of(2, &format!("PCP OS kernel {}", start.version), TEXT));
    lines.extend(lines_of(2, &if start.failed == 0 { format!("Verified at start: all {} self-tests passed.", start.passed) }
                              else { format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed) }, if start.failed == 0 { ui::GOOD } else { ui::BAD }));
    lines.extend(lines_of(2, &format!("Register: {} events. F5 shows the head of its chain.", rec.ledger().len()), SOFT));
    lines.extend(lines_of(2, "", TEXT));
    lines.extend(lines_of(2, "PROGRAMS", ui::TEAL));
    if programs.is_empty() {
        lines.extend(lines_of(4, "The loader handed over no program.", SOFT));
    }
    let none = option(&start.boot.command_line, "grant") == Some("none");
    for p in programs {
        lines.extend(whole(2, &p.name, TEXT));
        lines.extend(lines_of(4, &size_text(p.image.len()), SOFT));
        lines.extend(lines_of(4, "SHA-256", SOFT));
        lines.extend(whole(4, &p.digest, SOFT));
        if !p.arguments.is_empty() {
            lines.extend(lines_of(4, &format!("with: {}", p.arguments), SOFT));
        }
        lines.extend(lines_of(4, if none { "starting it lets it use: the console only" } else if start.sound { "starting it lets it use: the display, the keyboard, sound, the loader's files, the store" }
                                 else { "starting it lets it use: the display, the keyboard, the loader's files, the store" }, SOFT));
        #[cfg(feature = "guest-ethernet")]
        if !none && ethernet_grant(start.boot.exact_command_line(), &p.digest) {
            lines.extend(lines_of(4, "Ethernet grant: this program can send and receive raw network frames.", ui::AMBER));
        }
        #[cfg(feature = "game-link")]
        if !none && option(&start.boot.command_line, "link_image") == Some(p.digest.as_str()) {
            lines.extend(lines_of(4, "Optional game cable: this image can read and write the second serial port.", ui::AMBER));
        }
    }
    lines.extend(lines_of(2, "", TEXT));
    lines.extend(lines_of(2, "FILES FROM THE LOADER", ui::TEAL));
    if files.is_empty() {
        lines.extend(lines_of(4, "None.", SOFT));
    }
    for f in files {
        lines.extend(whole(2, &f.0, TEXT));
        lines.extend(lines_of(4, &size_text(f.1.len()), SOFT));
        lines.extend(lines_of(4, "SHA-256", SOFT));
        lines.extend(whole(4, &f.2, SOFT));
    }
    lines.extend(lines_of(2, "", TEXT));
    lines.extend(lines_of(2, "MACHINE", ui::TEAL));
    let (pool, free, _, _) = crate::mem::census();
    lines.extend(lines_of(4, &format!("memory for programs: {} MiB, {} MiB free", pool * 4096 >> 20, free * 4096 >> 20), SOFT));
    if let Some(d) = start.display {
        lines.extend(lines_of(4, &format!("display: {}, {} page(s)", if d.kind == dev::DisplayKind::Bochs { "Bochs interface, any mode" } else { "mode set by the loader" }, d.pages), SOFT));
        lines.extend(lines_of(4, &format!("keyboard: PS/2; sound: {}", if start.sound { "present" } else { "none" }), SOFT));
        lines.extend(lines_of(4, &format!("store: {} file(s) written by programs, {}", stored, if mounted { "saved to VM disk on fsync" } else { "kept until power off" }), SOFT));
    }
    text_screen("DETAILS", ui::TEAL, None, &lines, "Esc back", false, false);
}

/// The kernel's register, newest events last, a screen at a time.
fn register_view(rec: &Record) {
    use ui::{at, GROUND, SOFT, TEXT};
    let (cols, rows) = ui::grid();
    let (events, head) = match ledger::verify(rec.ledger().bytes()) {
        Ok(verified) => verified,
        Err(error) => {
            ui::clear(GROUND);
            at(2, 1, "THE KERNEL'S REGISTER: verification failed", ui::BAD, GROUND);
            at(2, 2, &format!("{error:?}"), TEXT, GROUND);
            at(2, rows - 2, "Esc back", SOFT, GROUND);
            loop {
                match wait_key() {
                    (0x01, _) | (0x1C, _) => return,
                    _ => {}
                }
            }
        }
    };
    // With room for a row of each event the screen is as it has always been: the count and the head of the
    // chain in the title, a row of headings, and an event to a row, a row that is too long ending in
    // "..". On a narrower display each event is a few short lines, whole.
    let across = (cols as usize).saturating_sub(4).max(1);
    let full_title = format!("THE KERNEL'S REGISTER: {} events, chain intact, head {}", events.len(), head);
    let table = "seq  time                  actor     kind                 what";
    let wide = table.len() <= across;
    let mut lines: Vec<Line> = Vec::new();
    // The table's title is the one it has always had, ending in ".." if the display cannot hold the whole
    // head of the chain. On a narrower display the title keeps the count and that the chain is intact,
    // and the whole head is the first lines, above the first event.
    let title = if wide { cut(&full_title, across) } else {
        lines.extend(wrapped(&format!("head {}", head), across).into_iter().map(|part| alloc::vec![(2, part, SOFT)]));
        let short = [format!("THE KERNEL'S REGISTER: {} events, chain intact", events.len()), format!("REGISTER: {} events, intact", events.len())];
        short.iter().find(|t| t.len() <= across).cloned().unwrap_or_else(|| cut(&short[1], across))
    };
    for e in events.iter() {
        let field = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let body = e.get("body");
        let what = ["label", "event", "name", "id", "outcome", "note", "to"].iter().filter_map(|k| body.and_then(|b| b.get(k)).and_then(Value::as_str).map(|v| format!("{} {}", k, v))).collect::<Vec<_>>().join("; ");
        let seq = e.get("seq").and_then(Value::as_int).unwrap_or(0);
        if wide {
            let line = format!("{:<4} {:<21} {:<9} {:<20} {}", seq, field("ts"), field("actor"), field("kind"), what);
            // An event to a row, as the table has always been and as its readers expect: a row too long
            // for the display ends in "..", so that it is seen to be cut.
            lines.push(alloc::vec![(2, cut(&line, across), TEXT)]);
        } else {
            lines.extend(lines_of(2, &format!("{} {}", seq, field("kind")), TEXT));
            lines.extend(lines_of(4, &format!("{} {}", field("ts"), field("actor")), SOFT));
            if !what.is_empty() {
                lines.extend(lines_of(4, &what, TEXT));
            }
        }
    }
    text_screen(&title, ui::TEAL, Some(table), &lines, "Up/Down scroll    Esc back", false, true);
}

/// Run one program as a task in the register. Interrupts must be off; returns with them off.
/// Returns the run summary, the resulting world and any reason the program could not start.
fn run_entry(rec: &mut Record, entry: &Entry, world: World, tasks: &mut u32, start: &Start, failed: &mut u32) -> (String, World, Option<&'static str>) {
    *tasks += 1;
    let id = format!("T{}", *tasks);
    let mut recorded = logged("TASK_STARTED", rec.submit(&now(), "kernel", "HOST", "TASK_STARTED",
        obj(&[("label", text(&entry.name.chars().take(31).collect::<String>())), ("worker", text("PROGRAM")), ("review", Value::Null), ("quiet_limit", Value::Int(0)), ("budget", Value::Int(0)),
              ("attempt", text(&format!("program {} of SHA-256 {}", entry.name, entry.digest))), ("observed", text("PROCESS_CREATED"))]))).is_ok();
    let mut args: Vec<&str> = alloc::vec![entry.name.as_str()];
    args.extend(entry.arguments.split_whitespace());
    say!("starting {} ({} bytes, SHA-256 {}){}", entry.name, entry.image.len(), entry.digest, if entry.arguments.is_empty() { String::new() } else { format!(" with: {}", entry.arguments) });
    let none = option(start.boot.command_line.as_str(), "grant") == Some("none");
    let grants = if none { Grants { console: true, ..Grants::default() } } else { Grants { console: true, display: start.display.is_some(), keys: start.keyboard, files: true, store: true, sound: start.sound,
        #[cfg(feature = "guest-ethernet")] ethernet: ethernet_grant(start.boot.exact_command_line(), &entry.digest),
        #[cfg(feature = "game-link")] game_link: option(&start.boot.command_line, "link_image") == Some(entry.digest.as_str()) && arch::game_link_init(), } };
    if none {
        say!("{} is started with the console and no other grant (grant=none)", entry.name);
    }
    let (free0, shows0, t0) = (user::frames_free(), dev::display_shows(), arch::ticks());
    let (_, sound0, dry0) = dev::sound_state();
    // The companion of this program, if the loader's line (taken whole) names this program by its
    // digest as the one a worker is for, names the worker by its digest, and a loaded file is that
    // worker. It is given the console and its end of the channel, and nothing else.
    #[cfg(feature = "companion")]
    let companion: Option<(String, &'static [u8])> = start.boot.exact_command_line().and_then(|line| {
        let (wanted, worker) = (option(line, "worker_for")?, option(line, "worker_image")?);
        if wanted != entry.digest || worker == entry.digest {
            return None;
        }
        world.files.iter().find(|f| sha256_hex(f.1) == worker).map(|f| (f.0.clone(), f.1))
    });
    #[cfg(feature = "companion")]
    let run = || match &companion {
        Some((name, image)) => {
            say!("{} is started with its companion {} ({} bytes), which is given the console and the channel only", entry.name, name, image.len());
            let worker_args = [name.as_str()];
            let (result, beside, world) = user::run_pair(&Program { name: &entry.name, image: entry.image, args: &args, grants, budget: u64::MAX },
                &Program { name, image, args: &worker_args, grants: Grants { console: true, ..Grants::default() }, budget: u64::MAX }, world);
            if let Some(p) = beside {
                let how = match p.end {
                    Some(user::End::Exited(code)) => format!("it exited with {}", code),
                    Some(user::End::Stopped) => String::from("the kernel ended it when the program it was beside ended"),
                    Some(user::End::Refused { vector, .. }) => format!("the processor refused something it did (vector {})", vector),
                    Some(user::End::OverBudget) => String::from("it ran over its budget"),
                    None => String::from("it had not ended"),
                };
                say!("companion {}: {}; {} bytes written to the console", name, how, p.output.len());
                for line in core::str::from_utf8(&p.output).unwrap_or("").lines().take(64) {
                    say!("companion {} wrote: {}", name, line);
                }
            }
            (result, world)
        }
        None => user::run_program(&Program { name: &entry.name, image: entry.image, args: &args, grants, budget: u64::MAX }, world),
    };
    #[cfg(not(feature = "companion"))]
    let run = || user::run_program(&Program { name: &entry.name, image: entry.image, args: &args, grants, budget: u64::MAX }, world);
    #[cfg(feature = "guest-ethernet")]
    let (result, world) = if grants.ethernet {
        let (outcome, status) = dev::virtio_tx::guest::during(run);
        if let Err(reason) = status { say!("guest Ethernet: {}", reason); *failed += 1; }
        outcome
    } else { run() };
    #[cfg(not(feature = "guest-ethernet"))]
    let (result, world) = run();
    let (ticks, shows) = (arch::ticks() - t0, dev::display_shows() - shows0);
    let (_, sound1, dry1) = dev::sound_state();
    let (sound_ms, dry) = ((sound1 - sound0) / (dev::SOUND_RATE as u64 * 4 / 1000), dry1 - dry0);
    let (_, fm_written, fm_late) = dev::fm_state();
    // The display is the kernel's again, in the kernel's own mode.
    if let Some(desktop) = start.display {
        dev::display_set(desktop.width, desktop.height);
        dev::display_show(0);
    }
    let refusal = result.as_ref().err().copied();
    let message = match result {
        Ok(p) => {
            let digest = sha256_hex(&p.output);
            let (outcome, result, how) = match p.end {
                Some(End::Exited(0)) => {
                    recorded &= logged("ARTIFACT_ADDED", rec.submit(&now(), "kernel", "HOST", "ARTIFACT_ADDED",
                        obj(&[("sha256", text(&digest)), ("size", Value::Int(p.output.len() as i64)), ("origin", text("PRODUCED")), ("stored", Value::Bool(false)), ("task", text(&id))]))).is_ok();
                    ("DONE", text(&digest), "exit 0".to_string())
                }
                Some(End::Exited(code)) => ("FAILED", Value::Null, format!("exit {}", code)),
                Some(End::Refused { vector, error, address, rip }) =>
                    ("FAILED", Value::Null, format!("ended by the kernel: the processor refused an act of the program (exception {}, error {:#x}, address {:#x}, instruction {:#x})", vector, error, address, rip)),
                Some(End::OverBudget) => ("STOPPED", Value::Null, "stopped by the kernel: over its budget".to_string()),
                Some(End::Stopped) => ("STOPPED", Value::Null, "stopped by the person with Ctrl+Alt+Delete".to_string()),
                None => ("LOST", Value::Null, "no end was observed".to_string()),
            };
            let seconds = ticks / arch::TICKS_PER_SECOND;
            let rate = if ticks > 0 { shows * arch::TICKS_PER_SECOND * 10 / ticks } else { 0 };
            let idle = if ticks > 0 { p.idle * 100 / ticks } else { 0 };
            let note = format!("{} after {} s; {} system calls; {} frames shown ({}.{} a second); the processor idled {}% of the time; {} ms of sound played, the device ran out of sound {} time(s); {} music register writes so far, the latest {} ms late; {} pages at the end, {} of them the display's, {} both writable and executable; {} frames given back, {} free before and {} after; the store holds {} file(s), {} bytes",
                               how, seconds, p.calls, shows, rate / 10, rate % 10, idle, sound_ms, dry, fm_written, fm_late, p.census.0, p.census.2, p.census.1, p.released, free0, user::frames_free(), world.store.len(), world.store.iter().map(|f| f.1.len()).sum::<usize>());
            recorded &= logged("TASK_ENDED", rec.submit(&now(), "host", "HOST", "TASK_ENDED",
                obj(&[("id", text(&id)), ("outcome", text(outcome)), ("result", result.clone()), ("read_back", result), ("spent", Value::Int(p.periods.min(i64::MAX as u64) as i64)),
                      ("note", text(&note)), ("observed", text("PROCESS_END"))]))).is_ok();
            say!("{} ended {}: {}", entry.name, outcome, note);
            if !world.unanswered.is_empty() {
                say!("calls {} made that this kernel does not answer (number, times): {:?}", entry.name, world.unanswered);
            }
            if p.census.1 != 0 || p.census.3 || user::frames_free() != free0 {
                say!("  FAIL the program's memory was not what it must be: writable-and-executable pages {}, kernel reachable {}, frames {} -> {}", p.census.1, p.census.3, free0, user::frames_free());
                *failed += 1;
            }
            format!("{} ended {}: {} after {} s.\n{} pictures shown, {}.{} a second; the processor idled {}% of the time.\nAll {} frames of its memory were given back.", entry.name, outcome, how, seconds, shows, rate / 10, rate % 10, idle, p.released)
        }
        Err(why) => {
            recorded &= logged("TASK_ENDED", rec.submit(&now(), "host", "HOST", "TASK_ENDED",
                obj(&[("id", text(&id)), ("outcome", text("FAILED")), ("result", Value::Null), ("read_back", Value::Null), ("spent", Value::Int(0)),
                      ("note", text(&format!("not started: {}", why))), ("observed", text("PROCESS_END"))]))).is_ok();
            say!("{} could not be started: {}", entry.name, why);
            format!("{} could not be started: {}.", entry.name, why)
        }
    };
    if !recorded {
        say!("  FAIL the run of {} was not recorded in the register", entry.name);
        *failed += 1;
    }
    (message, world, refusal)
}
