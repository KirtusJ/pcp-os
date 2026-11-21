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
                let (m, w) = run_entry(rec, entry, world, &mut tasks, start, failed);
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
    let mut surface = Surface::Home;
    let mut shortcuts: Vec<usize> = Vec::new();
    let mut widgets = [false; WIDGETS.len()];
    let display = start.display.unwrap();
    ui::set_scale(if option(line, "ui_scale") == Some("2") && display.width >= 1280 && display.height >= 960 { 2 } else { 1 });
    say!("desktop: {}x{} at {}x UI scale", display.width, display.height, ui::scale());
    arch::interrupts_on();
    dev::keys_forget();
    loop {
        #[cfg(feature = "vm-store")]
        let mounted = world.volume.is_some();
        #[cfg(not(feature = "vm-store"))]
        let mounted = false;
        place = place.min(shortcuts.len().saturating_sub(1));
        match surface {
            Surface::Home => draw_home(start, &programs, rec, &shortcuts, place, &widgets, &world.store, mounted, tasks - start.tasks, &message),
            Surface::Launcher => draw_desktop(start, &programs, rec, chosen, &message, &shortcuts),
            Surface::Details => draw_menu(start, &programs, &files, rec, chosen, &message, world.store.len(),
                                          #[cfg(feature = "vm-store")] mounted),
        }
        let key = wait_key();
        // Which program Enter starts, if any: the chosen shortcut on the desktop, the chosen row elsewhere.
        let to_start = match surface {
            Surface::Home => shortcuts.get(place).copied(),
            _ => if programs.is_empty() { None } else { Some(chosen) },
        };
        match (surface, key) {
            (_, (0x19, false)) => break,
            (_, (0x13, false)) => register_view(rec),
            (Surface::Home, (0x4B, _)) | (Surface::Home, (0x48, _)) if place > 0 => place -= 1,
            (Surface::Home, (0x4D, _)) | (Surface::Home, (0x50, _)) if place + 1 < shortcuts.len() => place += 1,
            (Surface::Home, (0x39, false)) => surface = Surface::Launcher,
            (Surface::Home, (0x1C, _)) if to_start.is_none() => surface = Surface::Launcher,
            (Surface::Home, (0x21, false)) => files_view(&world.store, mounted),
            (Surface::Home, (0x11, false)) => widgets_view(&mut widgets),
            (Surface::Home, (0x53, _)) | (Surface::Home, (0x0E, false)) if !shortcuts.is_empty() => { shortcuts.remove(place); }
            (Surface::Launcher, (0x48, _)) | (Surface::Details, (0x48, _)) if chosen > 0 => chosen -= 1,
            (Surface::Launcher, (0x50, _)) | (Surface::Details, (0x50, _)) if chosen + 1 < programs.len() => chosen += 1,
            (_, (0x1C, _)) if to_start.is_some() && safe => message = String::from("Not started: the kernel did not verify itself, and starts no program."),
            (_, (0x1C, _)) if to_start.is_some() => {
                arch::interrupts_off();
                let (m, w) = run_entry(rec, &programs[to_start.unwrap()], world, &mut tasks, start, failed);
                world = w;
                message = m;
                arch::interrupts_on();
                dev::keys_forget();
            }
            (Surface::Launcher, (0x1F, false)) if !programs.is_empty() => match shortcuts.iter().position(|p| *p == chosen) {
                Some(at) => { shortcuts.remove(at); }
                None => shortcuts.push(chosen),
            },
            (Surface::Launcher, (0x20, false)) => surface = Surface::Details,
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
const OUTLINE: u32 = 0x2F8C88;
const PLATE: u32 = 0x12304A;
const PLATE_CHOSEN: u32 = 0x1B4A63;
const PLATE_EDGE: u32 = 0x24405C;

/// The status bar of every desktop surface: what this is, whether it verified itself, and the state of its
/// register. Its text is on the character grid (multiples of 10 across, 20 down) over flat ground, and the
/// sentence about verification is the one the details screen uses, so that a check reading a picture of the
/// screen finds the same words on every surface.
fn status_bar(start: &Start, rec: &Record, w: u32) {
    use ui::{blend, label, label_width, BAD, GOOD, SOFT, TEXT};
    let safe = start.failed != 0;
    let on_bar = ui::bar(0, 60, w, 0x17334F, 0x11283F, OUTLINE);
    let mut x = label(30, 20, "PCP OS", TEXT, false, &on_bar);
    x = label(x + 10, 20, &format!("{} | x86-64", start.version), SOFT, false, &on_bar);
    let verified = if safe { format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed) }
                   else { format!("Verified at start: all {} self-tests passed.", start.passed) };
    let middle = on_bar(0, 30);
    let pill = |x: u32, s: &str, colour: u32, tint: u32| {
        let width = label_width(s, false) + 20;
        let flat = move |_: u32, _: u32| blend(middle, tint, 46);
        let _ = ui::panel(x - 10, 14, width, 32, &flat, tint, 0, if colour == BAD { BAD } else { OUTLINE }, false);
        label(x, 20, s, colour, false, &flat);
    };
    pill(x + 30, &verified, if safe { BAD } else { GOOD }, if safe { BAD } else { 0x187C78 });
    let head = rec.ledger().head();
    let chain = "chain intact";
    let events = format!("Register: {} events, head {}...", rec.ledger().len(), &head[..head.len().min(16)]);
    let right = (w - 30 - label_width(chain, false)) / 10 * 10;
    pill(right, chain, TEXT, 0x187C78);
    label((right.saturating_sub(30 + label_width(&events, false))) / 10 * 10, 20, &events, SOFT, false, &on_bar);
}

/// The bar of keys at the foot of a desktop surface. Returns the row its text is on.
fn key_bar(w: u32, h: u32, keys: &[(&str, &str)], note: &str) {
    use ui::{label, label_width, SOFT, TEXT};
    let top = h / 20 * 20 - 60;
    let on_keys = ui::bar(top, h - top, w, 0x142C45, 0x0F2439, 0x0F2439);
    let middle = on_keys(0, top + 30);
    let mut x = 30;
    for (key, does) in keys {
        let width = label_width(key, false) + 20;
        let cap = move |_: u32, _: u32| ui::blend(middle, TEXT, 20);
        let _ = ui::panel(x - 10, top + 14, width, 32, &cap, TEXT, 0, 0x3B5672, true);
        label(x, top + 20, key, TEXT, false, &cap);
        x = label(x + width, top + 20, does, SOFT, false, &on_keys) + 30;
    }
    if x + label_width(note, false) + 30 <= w {
        label((w - 30 - label_width(note, false)) / 10 * 10, top + 20, note, SOFT, false, &on_keys);
    }
}

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
    let size = h * 28 / 100;
    let (px, py) = (field.cx - size / 2, field.cy - size / 2);
    let on_plate = ui::panel(px, py, size, size, &over_field, 0x10283F, 235, OUTLINE, true);
    let bird = size * 70 / 100;
    ui::emblem_over(px + (size - bird * 121 / 132) / 2, py + (size - bird) / 2, bird, &on_plate);
    let motto = "EVIDENCE BEFORE AGREEMENT";
    ui::label(field.cx - ui::label_width(motto, false) / 2, py + size + 40, motto, ui::SOFT, false, &over_field);
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
fn draw_home(start: &Start, programs: &[Entry], rec: &Record, shortcuts: &[usize], place: usize, widgets: &[bool; 3], store: &crate::regular::Files, mounted: bool, run: u32, message: &str) {
    use ui::{label, label_width, AMBER, BAD, GOOD, SOFT, TEAL, TEXT};
    let safe = start.failed != 0;
    let recent = if widgets[0] { recent_tasks(rec, programs, 3) } else { Vec::new() };
    let (pool, free, _, _) = crate::mem::census();
    let files_line = if mounted { "VM files | saved when an app syncs" } else { "Session files | cleared at power-off" };
    let keys: &[(&str, &str)] = if shortcuts.is_empty() { &[("Enter", "programs"), ("W", "widgets"), ("F", "files"), ("R", "register"), ("P", "power off")] }
                                else { &[("Left/Right", "move"), ("Enter", "start"), ("Space", "programs"), ("Del", "remove"), ("W", "widgets"), ("F", "files"), ("R", "register"), ("P", "power off")] };
    let display = match dev::display() {
        Some(d) if d.width >= 1280 && d.height >= 720 && ui::scale() == 1 => d,
        _ => {
            use ui::{at, GROUND};
            ui::clear(GROUND);
            let (_, rows) = ui::grid();
            ui::emblem(20, 16, 36, GROUND);
            at(7, 1, "PCP OS | DESKTOP", TEXT, GROUND);
            at(2, 3, &if safe { format!("NOT VERIFIED: {} of {} self-tests failed.", start.failed, start.passed + start.failed) }
                      else { format!("Verified at start: all {} self-tests passed.", start.passed) }, if safe { BAD } else { GOOD }, GROUND);
            let mut row = 5;
            for (i, p) in shortcuts.iter().enumerate() {
                let ground = if i == place { ui::PANEL } else { GROUND };
                ui::fill(2 * ui::CELL_W, row * ui::CELL_H, 44 * ui::CELL_W, ui::CELL_H, ground);
                at(3, row, &format!("SHORTCUT {}", programs[*p].name), TEXT, ground);
                row += 1;
            }
            for (time, name, outcome, _) in recent.iter() {
                at(2, row + 1, &format!("{} {} {}", time, outcome, name), SOFT, GROUND);
                row += 1;
            }
            if widgets[1] { at(2, row + 2, &format!("memory {} MiB, {} MiB free", pool * 4 / 1024, free * 4 / 1024), SOFT, GROUND); row += 1; }
            if widgets[2] { at(2, row + 3, files_line, SOFT, GROUND); }
            at(2, rows - 2, &keys.iter().map(|(k, d)| format!("{} {}", k, d)).collect::<Vec<_>>().join("   "), TEXT, GROUND);
            return;
        }
    };
    let (w, h) = (display.width, display.height);
    let field = ui::Field { width: w, height: h, cx: w * 74 / 100, cy: h * 48 / 100 };
    field.paint();
    let over_field = |x: u32, y: u32| field.colour(x, y);
    status_bar(start, rec, w);
    let left = w / 200 * 10;
    let mut y = 100;

    // Shortcuts the person placed, as tiles.
    if !shortcuts.is_empty() {
        for (i, p) in shortcuts.iter().enumerate().take(((w * 47 / 100) / 220) as usize) {
            let x = left + i as u32 * 220;
            let chosen = i == place;
            let on_tile = plate(x, y, 200, 120, if chosen { PLATE_CHOSEN } else { PLATE }, if chosen { OUTLINE } else { PLATE_EDGE }, chosen);
            if chosen {
                dev::display_fill(x + 20, y + 116, 160, 3, AMBER);
            }
            for (bx, by, bw, bh, colour) in [(0u32, 8u32, 60u32, 8u32, TEAL), (0, 26, 60, 8, GOOD), (0, 44, 40, 8, AMBER)] {
                dev::display_fill(x + 70 + bx, y + 6 + by, bw, bh, colour);
            }
            let name: String = programs[*p].name.chars().take(18).collect();
            label(x + (200 - label_width(&name, false)) / 20 * 10, y + 80, &name, TEXT, false, &on_tile);
        }
        y += 140;
    }

    // Widgets the person placed.
    let wide = 860;
    if widgets[0] {
        let on_card = plate(left, y, wide, 140, PLATE, PLATE_EDGE, false);
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
            label(x + 20, line, &note.chars().take(room).collect::<String>(), SOFT, false, &on_card);
        }
        y += 160;
    }
    let mut x = left;
    if widgets[1] {
        let on_machine = plate(x, y, 420, 140, PLATE, PLATE_EDGE, false);
        label(x + 20, y + 20, "THIS MACHINE", TEAL, false, &on_machine);
        label(x + 20, y + 60, &format!("memory {} MiB, {} MiB free", pool * 4 / 1024, free * 4 / 1024), TEXT, false, &on_machine);
        label(x + 20, y + 80, &format!("display {} x {}", w, h), TEXT, false, &on_machine);
        label(x + 20, y + 100, &format!("keyboard PS/2, sound {}", if start.sound { "present" } else { "absent" }), TEXT, false, &on_machine);
        x += 440;
    }
    if widgets[2] {
        let on_session = plate(x, y, 420, 140, PLATE, PLATE_EDGE, false);
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
    for (i, line) in message.lines().take(3).enumerate() {
        label(left, y + i as u32 * 20, line, AMBER, false, &over_field);
    }

    emblem_plate(&field, h);
    key_bar(w, h, keys, "Ctrl+Alt+Delete stops a running program.");
}

/// Choose which widgets are on the desktop. Up and Down move, Enter puts one on or takes it off, Esc goes back.
fn widgets_view(widgets: &mut [bool; 3]) {
    use ui::{at, GROUND, PANEL, SOFT, TEXT};
    let (_, rows) = ui::grid();
    let mut at_row = 0usize;
    loop {
        ui::clear(GROUND);
        at(2, 1, "WIDGETS ON THE DESKTOP", ui::TEAL, GROUND);
        at(2, 2, "They last for this session.", SOFT, GROUND);
        for (i, name) in WIDGETS.iter().enumerate() {
            let ground = if i == at_row { PANEL } else { GROUND };
            ui::fill(ui::CELL_W, (4 + i as u32) * ui::CELL_H, 46 * ui::CELL_W, ui::CELL_H, ground);
            at(2, 4 + i as u32, if widgets[i] { "[on]" } else { "[  ]" }, if widgets[i] { ui::GOOD } else { SOFT }, ground);
            at(8, 4 + i as u32, name, TEXT, ground);
        }
        at(2, rows - 2, "Up/Down choose    Enter on or off    Esc back", SOFT, GROUND);
        match wait_key() {
            (0x48, _) if at_row > 0 => at_row -= 1,
            (0x50, _) if at_row + 1 < WIDGETS.len() => at_row += 1,
            (0x1C, _) | (0x39, false) => widgets[at_row] = !widgets[at_row],
            (0x01, _) => return,
            _ => {}
        }
    }
}

/// The files programs have written, by name and size. Esc or Enter goes back.
fn files_view(store: &crate::regular::Files, mounted: bool) {
    use ui::{at, GROUND, SOFT, TEXT};
    let (cols, rows) = ui::grid();
    ui::clear(GROUND);
    let total: usize = store.iter().map(|f| f.1.len()).sum();
    at(2, 1, &format!("FILES: {} file(s), {}", store.len(), size_text(total)), ui::TEAL, GROUND);
    at(2, 2, if mounted { "VM files | saved when an app syncs" } else { "Session files | cleared at power-off" }, SOFT, GROUND);
    if store.len() == 0 {
        at(2, 4, "No program has written a file.", SOFT, GROUND);
    }
    for (n, (name, bytes)) in store.iter().take(rows.saturating_sub(8) as usize).enumerate() {
        at(2, 4 + n as u32, &name.chars().take(cols as usize - 20).collect::<String>(), TEXT, GROUND);
        at(cols.saturating_sub(14), 4 + n as u32, &size_text(bytes.len()), SOFT, GROUND);
    }
    at(2, rows - 2, "Esc back", SOFT, GROUND);
    loop {
        match wait_key() {
            (0x01, _) | (0x1C, _) => return,
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
    let field = ui::Field { width: w, height: h, cx: w * 74 / 100, cy: h * 48 / 100 };
    field.paint();
    let over_field = |x: u32, y: u32| field.colour(x, y);
    status_bar(start, rec, w);

    let left = w / 200 * 10;
    let list_width = (w * 47 / 100).min(1100) / 10 * 10;
    label(left, 100, "Programs", TEXT, true, &over_field);
    let on_note = plate(left, 150, list_width, 40, PLATE, PLATE_EDGE, false);
    label(left + 20, 160, if safe { "SAFE: verification failed. Programs are disabled." } else if programs.is_empty() { "The loader handed over no program." }
                               else { "Choose one to start. Each runs with only what it is granted." }, if safe { BAD } else { SOFT }, false, &on_note);
    let none = option(&start.boot.command_line, "grant") == Some("none");
    let grants = if none { "console only" } else if start.sound { "display, keyboard, sound, loader files, store" } else { "display, keyboard, loader files, store" };
    let foot = h / 20 * 20 - 60;
    let mut y = 210;
    let room = ((foot.saturating_sub(y + 120)) / 80).max(1) as usize;
    let first = (chosen + 1).saturating_sub(room);
    for (i, p) in programs.iter().enumerate().skip(first).take(room) {
        let on_row = plate(left, y, list_width, 60, if i == chosen { PLATE_CHOSEN } else { PLATE }, if i == chosen { OUTLINE } else { PLATE_EDGE }, i == chosen);
        if i == chosen {
            dev::display_fill(left, y + 8, 4, 44, AMBER);
        }
        // The row says, on the character grid: which is chosen, its name, its size and digest, and what it may use.
        label(left + 20, y + 10, if i == chosen { ">" } else { " " }, AMBER, false, &on_row);
        label(left + 40, y + 10, &p.name.chars().take(28).collect::<String>(), TEXT, false, &on_row);
        let size = size_text(p.image.len());
        let digest = format!("SHA-256 {}...", &p.digest[..16]);
        label((left + list_width - 20 - label_width(&digest, false)) / 10 * 10, y + 10, &digest, SOFT, false, &on_row);
        label((left + list_width - 60 - label_width(&digest, false) - label_width(&size, false)) / 10 * 10, y + 10, &size, SOFT, false, &on_row);
        #[allow(unused_mut)]
        let mut access = String::from(grants);
        #[cfg(feature = "guest-ethernet")]
        if !none && ethernet_grant(start.boot.exact_command_line(), &p.digest) { access.push_str(", raw Ethernet frames"); }
        #[cfg(feature = "game-link")]
        if !none && option(&start.boot.command_line, "link_image") == Some(p.digest.as_str()) { access.push_str(", second serial port"); }
        label(left + 40, y + 30, &format!("may use: {}", access), SOFT, false, &on_row);
        if shortcuts.contains(&i) {
            let mark = "on the desktop";
            label((left + list_width - 20 - label_width(mark, false)) / 10 * 10, y + 30, mark, ui::GOOD, false, &on_row);
        }
        y += 80;
    }
    if !message.is_empty() {
        let lines = message.lines().take(3).count() as u32;
        let on_message = plate(left, y + 10, list_width, 20 + lines * 20, PLATE, PLATE_EDGE, false);
        for (i, line) in message.lines().take(3).enumerate() {
            label(left + 20, y + 20 + i as u32 * 20, &line.chars().take((list_width / 10 - 4) as usize).collect::<String>(), AMBER, false, &on_message);
        }
    }

    emblem_plate(&field, h);
    let keys: &[(&str, &str)] = if programs.is_empty() || safe { &[("Esc", "desktop"), ("D", "details"), ("R", "register"), ("P", "power off")] }
                                else { &[("Up/Down", "choose"), ("Enter", "start"), ("S", "shortcut"), ("Esc", "desktop"), ("D", "details"), ("R", "register"), ("P", "power off")] };
    key_bar(w, h, keys, "Ctrl+Alt+Delete stops a running program.");
}

fn draw_desktop_plain(start: &Start, programs: &[Entry], chosen: usize, message: &str) {
    use ui::{at, GROUND, PANEL, SOFT, TEXT};
    ui::clear(GROUND);
    let (cols, rows) = ui::grid();
    ui::emblem(20, 16, 36, GROUND);
    at(7, 1, "PCP OS", TEXT, GROUND);
    if start.failed != 0 {
        at(2, 3, "SAFE: verification failed. Programs are disabled.", ui::BAD, GROUND);
    }
    let visible = rows.saturating_sub(12).max(1) as usize;
    let first = (chosen + 1).saturating_sub(visible);
    if programs.is_empty() {
        at(2, 5, "No programs loaded", SOFT, GROUND);
    }
    for (i, p) in programs.iter().enumerate().skip(first).take(visible) {
        let row = 5 + (i - first) as u32;
        let width = cols.saturating_sub(4).min(44);
        let bg = if i == chosen { PANEL } else { GROUND };
        ui::fill(2 * ui::CELL_W, row * ui::CELL_H, width * ui::CELL_W, ui::CELL_H, bg);
        if i == chosen { ui::fill(2 * ui::CELL_W, row * ui::CELL_H, 3, ui::CELL_H, ui::TEAL); }
        at(3, row, &p.name.chars().take(width.saturating_sub(2) as usize).collect::<String>(), TEXT, bg);
    }
    if let Some(_entry) = programs.get(chosen).filter(|_| start.failed == 0) {
        let none = option(&start.boot.command_line, "grant") == Some("none");
        let access = if none { "Access: console only" } else if start.sound {
            "Access: console, display, keyboard, files, store, sound"
        } else { "Access: console, display, keyboard, files and store" };
        at(2, rows - 7, access, SOFT, GROUND);
        #[cfg(feature = "guest-ethernet")]
        if !none && ethernet_grant(start.boot.exact_command_line(), &_entry.digest) {
            at(2, rows - 6, "Access: raw Ethernet frames", ui::AMBER, GROUND);
        }
        #[cfg(feature = "game-link")]
        if !none && option(&start.boot.command_line, "link_image") == Some(_entry.digest.as_str()) {
            at(2, rows - 5, "Access: second serial port", ui::AMBER, GROUND);
        }
    }
    if !message.is_empty() {
        at(2, rows - 4, message.lines().next().unwrap_or(""), ui::AMBER, GROUND);
    }
    let keys = if programs.is_empty() || start.failed != 0 {
        "D details   R register   P off"
    } else { "Up/Down choose   Enter open   D details   R register   P off" };
    at(2, rows - 2, keys, TEXT, GROUND);
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
    let per = (rows - 6) as usize;
    let mut top = events.len().saturating_sub(per);
    loop {
        ui::clear(GROUND);
        at(2, 1, &format!("THE KERNEL'S REGISTER: {} events, chain intact, head {}", events.len(), head), ui::TEAL, GROUND);
        at(2, 2, "seq  time                  actor     kind                 what", SOFT, GROUND);
        for (n, e) in events.iter().skip(top).take(per).enumerate() {
            let field = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let body = e.get("body");
            let what = ["label", "event", "name", "id", "outcome", "note", "to"].iter().filter_map(|k| body.and_then(|b| b.get(k)).and_then(Value::as_str).map(|v| format!("{} {}", k, v))).collect::<Vec<_>>().join("; ");
            let line = format!("{:<4} {:<21} {:<9} {:<20} {}", e.get("seq").and_then(Value::as_int).unwrap_or(0), field("ts"), field("actor"), field("kind"), what);
            at(2, 3 + n as u32, &line.chars().take(cols as usize - 4).collect::<String>(), TEXT, GROUND);
        }
        at(2, rows - 2, "Up/Down scroll    Esc back", SOFT, GROUND);
        match wait_key() {
            (0x48, _) => top = top.saturating_sub(per / 2),
            (0x50, _) => top = (top + per / 2).min(events.len().saturating_sub(per)),
            (0x01, _) | (0x1C, _) => return,
            _ => {}
        }
    }
}

/// Run one program as a task in the register. Interrupts must be off; returns with them off.
/// Returns what to tell the person, and the world as the program left it.
fn run_entry(rec: &mut Record, entry: &Entry, world: World, tasks: &mut u32, start: &Start, failed: &mut u32) -> (String, World) {
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
    (message, world)
}
