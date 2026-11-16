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
    let mut world = World { files: files.iter().map(|f| (f.0.clone(), f.1)).collect(), store: Vec::new(),
                            epoch_at_start: epoch(y, mo, d, h, mi, s).saturating_sub(arch::ticks() / arch::TICKS_PER_SECOND), unanswered: Vec::new() };
    let line = start.boot.command_line.as_str();
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
    let mut chosen = 0usize;
    arch::interrupts_on();
    dev::keys_forget();
    loop {
        draw_menu(start, &programs, &files, rec, chosen, &message, world.store.len());
        let key = wait_key();
        match key {
            (0x48, _) if chosen > 0 => chosen -= 1,
            (0x50, _) if chosen + 1 < programs.len() => chosen += 1,
            (0x1C, _) if !programs.is_empty() && safe => message = String::from("Not started: the kernel did not verify itself, and starts no program."),
            (0x1C, _) if !programs.is_empty() => {
                arch::interrupts_off();
                let (m, w) = run_entry(rec, &programs[chosen], world, &mut tasks, start, failed);
                world = w;
                message = m;
                arch::interrupts_on();
                dev::keys_forget();
            }
            (0x21, false) => files_view(&files),
            (0x13, false) => register_view(rec),
            (0x19, false) | (0x01, false) => break,
            _ => {}
        }
    }
    arch::interrupts_off();
    ui::clear(ui::GROUND);
    ui::at(2, 2, "PCP OS is stopping. Its register is being closed and verified.", ui::TEXT, ui::GROUND);
    tasks
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

/// Clip a text field to the selected mode's character grid.
/// The text renderer already clips framebuffer writes; this also bounds work.
fn desktop_at(cols: u32, rows: u32, x: u32, y: u32, s: &str, fg: u32, bg: u32) {
    if x >= cols || y >= rows {
        return;
    }
    let clipped: String = s.chars().take((cols - x) as usize).collect();
    ui::at(x, y, &clipped, fg, bg);
}

/// Keyboard-accessible first-launch shell drawn atop a per-resolution raster.
/// It is not a multiwindow desktop and cannot choose a monitor's native mode.
fn draw_menu(start: &Start, programs: &[Entry], files: &[(String, &'static [u8], String)], rec: &Record, chosen: usize, message: &str, stored: usize) {
    use ui::{GROUND, PANEL, SOFT, TEXT};
    if !ui::desktop_background() {
        // Unsupported video dimensions/pitch keep the original flat background.
        ui::clear(GROUND);
    }
    let (cols, rows) = ui::grid();
    let layout = match pcpos_desktop::Layout::new(cols, rows) {
        Some(a) => a,
        None => {
            desktop_at(cols, rows, 1, 1, "PCP OS: display too small for desktop", TEXT, GROUND);
            desktop_at(cols, rows, 1, 2, "Up/Down select, Enter run, R register, F files, P power off", SOFT, GROUND);
            if let Some(p) = programs.get(chosen) {
                desktop_at(cols, rows, 1, 4, &p.name, TEXT, GROUND);
            }
            return;
        }
    };
    let l = layout.pane_left;
    let inner = l + 2;
    let bg = PANEL;
    let focus = 0x27526A;
    ui::fill(l * ui::CELL_W, layout.top * ui::CELL_H,
             layout.pane_cols * ui::CELL_W, (rows - layout.top) * ui::CELL_H, bg);
    desktop_at(cols, rows, inner, layout.top, "PCP OS | FIRST-LAUNCH DESKTOP", ui::TEAL, bg);
    if layout.pane_cols >= 82 {
        ui::emblem((l + layout.pane_cols - 8) * ui::CELL_W,
                   (layout.top + 1) * ui::CELL_H, 42, bg);
    }
    let status = if start.failed == 0 {
        format!("Kernel checks: {}/{} passed", start.passed, start.passed)
    } else {
        format!("SAFE: {}/{} checks failed; no programs may start", start.failed, start.passed + start.failed)
    };
    desktop_at(cols, rows, inner, layout.top + 1, &status,
               if start.failed == 0 { ui::GOOD } else { ui::BAD }, bg);
    if let Some(d) = dev::display() {
        let origin = if d.kind == dev::DisplayKind::Bochs { "Bochs" } else { "loader" };
        desktop_at(cols, rows, inner, layout.top + 2,
                   &format!("Display: {}x{}  stride {} bytes  source {}", d.width, d.height, d.pitch, origin),
                   SOFT, bg);
    }
    let head = rec.ledger().head();
    desktop_at(cols, rows, inner, layout.top + 3,
               &format!("Register: {} events  head {}...", rec.ledger().len(), &head[..head.len().min(16)]),
               SOFT, bg);
    desktop_at(cols, rows, inner, layout.list_row - 1,
               &format!("APPLICATIONS ({} supplied by loader)", programs.len()), ui::TEAL, bg);

    let first = layout.first_visible(chosen, programs.len());
    for (n, p) in programs.iter().enumerate().skip(first).take(layout.slots) {
        let row = layout.list_row + (n - first) as u32;
        let highlighted = n == chosen;
        let line_bg = if highlighted { focus } else { bg };
        if highlighted {
            ui::fill((l + 1) * ui::CELL_W, row * ui::CELL_H,
                     (layout.pane_cols - 2) * ui::CELL_W, ui::CELL_H, line_bg);
        }
        let line = format!("{} {:>3}/{}  {}  ({})", if highlighted { ">" } else { " " },
                           n + 1, programs.len(), p.name, size_text(p.image.len()));
        desktop_at(cols, rows, inner, row, &line,
                   if highlighted { TEXT } else { SOFT }, line_bg);
    }
    if programs.is_empty() {
        desktop_at(cols, rows, inner, layout.list_row,
                   "No applications installed. Manual kernel operation remains available.", SOFT, bg);
    }
    if let Some(p) = programs.get(chosen) {
        desktop_at(cols, rows, inner, layout.info_row,
                   &format!("Selected SHA-256: {}", p.digest), SOFT, bg);
    } else {
        desktop_at(cols, rows, inner, layout.info_row, "Select a loader-supplied application to run.", SOFT, bg);
    }
    let (pool, free, _, _) = crate::mem::census();
    desktop_at(cols, rows, inner, layout.info_row + 1,
               &format!("Loader files: {}  RAM: {} MiB / {} MiB free  session store: {}",
                        files.len(), pool * 4096 >> 20, free * 4096 >> 20, stored),
               SOFT, bg);
    desktop_at(cols, rows, inner, layout.info_row + 2,
               if message.is_empty() { "No Qwen or Tailorbird needed to use this launcher." } else { message },
               if message.is_empty() { SOFT } else { ui::AMBER }, bg);

    ui::fill(l * ui::CELL_W, layout.footer_row * ui::CELL_H,
             layout.pane_cols * ui::CELL_W, 2 * ui::CELL_H, GROUND);
    desktop_at(cols, rows, inner, layout.footer_row,
               "Up/Down select  Enter launch  F files  R register  P power off", TEXT, GROUND);
    desktop_at(cols, rows, inner, layout.footer_row + 1,
               "Esc quit  Ctrl+Alt+Delete: kernel stops active guest", SOFT, GROUND);
}

/// View complete loader artifact identities separately from the compact launcher.
/// This is still the kernel's supplied-file view, not a persistent file explorer.
fn files_view(files: &[(String, &'static [u8], String)]) {
    let (cols, rows) = ui::grid();
    let page = rows.saturating_sub(5).max(1) as usize;
    let mut top = 0usize;
    loop {
        ui::clear(ui::GROUND);
        desktop_at(cols, rows, 1, 1, &format!("LOADER FILES: {}  (not persistent storage)", files.len()), ui::TEAL, ui::GROUND);
        for (n, entry) in files.iter().skip(top).take(page).enumerate() {
            let line = format!("{}  {}  SHA-256 {}", entry.0, size_text(entry.1.len()), entry.2);
            desktop_at(cols, rows, 1, n as u32 + 3, &line, ui::TEXT, ui::GROUND);
        }
        desktop_at(cols, rows, 1, rows.saturating_sub(2), "Up/Down scroll  Esc return", ui::SOFT, ui::GROUND);
        match wait_key() {
            (0x48, _) => top = top.saturating_sub(1),
            (0x50, _) => top = (top + 1).min(files.len().saturating_sub(page)),
            (0x01, _) | (0x1C, _) => return,
            _ => {}
        }
    }
}

/// The kernel's register, newest events last, a screen at a time.
fn register_view(rec: &Record) {
    use ui::{at, GROUND, SOFT, TEXT};
    let events = match ledger::verify(rec.ledger().bytes()) {
        Ok((e, _)) => e,
        Err(_) => Vec::new(),
    };
    let (cols, rows) = ui::grid();
    let per = (rows - 6) as usize;
    let mut top = events.len().saturating_sub(per);
    loop {
        ui::clear(GROUND);
        at(2, 1, &format!("THE KERNEL'S REGISTER: {} events, chain intact, head {}", events.len(), rec.ledger().head()), ui::TEAL, GROUND);
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
    // Return to the actual startup mode, not a hard-coded 1024x768 desktop.
    if let Some(initial) = start.display {
        if dev::display_set(initial.width, initial.height).is_none() {
            say!("desktop display restore refused: {}x{}", initial.width, initial.height);
        }
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
