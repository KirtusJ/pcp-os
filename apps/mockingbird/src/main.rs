#![no_main]
#![no_std]
mod clipboard;
mod editor;
mod find;
#[path = "../../../PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"]
mod font;

use clipboard::Clipboard;
use editor::{advance, Editor, History};
use find::Find;
use pcp_game_guest::{entry, exit, ink::Frame, log, sleep_us, time_us, Display};

const PAPER: u32 = 0xf6f4ed;
const INK: u32 = 0x222c30;
const MUTED: u32 = 0x465a60;
const SELECT: u32 = 0xb6dbd6;
const ACCENT: u32 = 0x17645c;

fn glyph(frame: &mut Frame, x: usize, y: usize, ch: u8, scale: usize, ink: u32, bg: u32) {
    let Some(g) = ch
        .checked_sub(font::FIRST)
        .and_then(|v| font::GLYPHS.get(v as usize))
    else {
        return;
    };
    for (i, &alpha) in g.iter().enumerate() {
        if alpha == 0 {
            continue;
        }
        let a = u32::from(alpha);
        let mix = |s: u32| (((ink >> s) & 255) * a + ((bg >> s) & 255) * (255 - a)) / 255;
        let c = mix(16) << 16 | mix(8) << 8 | mix(0);
        frame.rect(
            (x + (i % font::CELL_WIDTH) * scale) as i32,
            (y + (i / font::CELL_WIDTH) * scale) as i32,
            scale as i32,
            scale as i32,
            c,
        );
    }
}
const BAND: u32 = 0xe7ede8;
fn visible(b: char) -> u8 {
    match b {
        '\u{2018}' | '\u{2019}' => b'\'',
        '\u{201c}' | '\u{201d}' => b'"',
        '\u{2013}' | '\u{2014}' => b'-',
        '\u{a0}' => b' ',
        c if c.is_ascii() => c as u8,
        _ => b'?',
    }
}
fn label(f: &mut Frame, x: usize, y: usize, s: &str, z: usize, color: u32, bg: u32) {
    for (i, c) in s.chars().enumerate() {
        glyph(f, x + i * font::CELL_WIDTH * z, y, visible(c), z, color, bg);
    }
}

// A long tail and a pale wing bar distinguish the bird at one framebuffer pixel per dot.
fn bird(f: &mut Frame, x: usize, y: usize, z: usize) {
    const SPRITE: [&[u8]; 16] = [
        b"                 ###",
        b"                ##o##",
        b"               ######++",
        b"              #######",
        b"            ########",
        b"          ###...####",
        b"         ##...#####",
        b"       ###..######",
        b"     #####.######",
        b"    ############",
        b"  ######## ####",
        b" ######    + +",
        b"####       +  +",
        b"##        +++ +++",
        b"",
        b"",
    ];
    for (r, line) in SPRITE.iter().enumerate() {
        for (c, b) in line.iter().enumerate() {
            let col = match b {
                b'#' => ACCENT,
                b'.' => 0xffffff,
                b'o' => 0xf4d7a0,
                b'+' => INK,
                _ => continue,
            };
            f.rect(
                (x + c * z) as i32,
                (y + r * z) as i32,
                z as i32,
                z as i32,
                col,
            );
        }
    }
}
struct View {
    cols: usize,
    first: usize,
}
impl View {
    fn draw(&mut self, d: &mut Display, e: &Editor, msg: &str, find: &Find) {
        d.draw(|surface, w, h, pitch| {
            let z = (w / 820).min(h / 475).clamp(1, 2);
            let cw = font::CELL_WIDTH * z;
            let ch = font::CELL_HEIGHT * z;
            let left = 2 * cw;
            let top = 3 * ch;
            let bottom = if find.open { 3 * ch } else { ch + 12 * z };
            self.cols = (w.saturating_sub(2 * left) / cw).max(1);
            let rows = (h.saturating_sub(top + bottom) / ch).max(1);
            let (r, c) = e.row_col(e.caret, self.cols);
            self.first = self.first.min(r).max((r + 1).saturating_sub(rows));
            let mut f = Frame { px: surface, pitch };
            f.px.fill(PAPER);
            f.rect(0, 0, w as i32, (2 * ch) as i32, BAND);
            bird(&mut f, left, 12 * z, z);
            label(&mut f, left + 32 * z, 10 * z, "Mockingbird", z, INK, BAND);
            if w > 45 * cw {
                label(
                    &mut f,
                    w - left - 12 * cw,
                    10 * z,
                    "Session only",
                    z,
                    MUTED,
                    BAND,
                );
            }
            f.rect(0, (2 * ch) as i32, w as i32, z as i32, 0xcbd6ce);
            let mut at = (0, 0);
            let selection = e.selection();
            for (i, b) in e.text().char_indices() {
                if at.0 >= self.first + rows {
                    break;
                }
                if at.0 >= self.first {
                    let x = left + cw * at.1;
                    let y = top + ch * (at.0 - self.first);
                    let bg = if selection.contains(&i) {
                        SELECT
                    } else {
                        PAPER
                    };
                    if bg != PAPER {
                        f.rect(x as i32, y as i32, cw as i32, ch as i32, bg);
                    }
                    glyph(&mut f, x, y, visible(b), z, INK, bg);
                }
                advance(&mut at, b, self.cols);
            }
            if !find.open {
                f.rect(
                    (left + c * cw) as i32,
                    (top + (r - self.first) * ch) as i32,
                    (2 * z) as i32,
                    ch as i32,
                    ACCENT,
                );
            }
            let band = h.saturating_sub(bottom);
            f.rect(0, band as i32, w as i32, bottom as i32, BAND);
            if find.open {
                label(&mut f, left, band + 3 * z, "Find", z, ACCENT, BAND);
                let x = left + 5 * cw;
                let capacity = (w.saturating_sub(x + left) / cw).saturating_sub(1);
                let query = find.query();
                let skip = query.chars().count().saturating_sub(capacity);
                let offset = query
                    .char_indices()
                    .nth(skip)
                    .map_or(query.len(), |(i, _)| i);
                let shown = &query[offset..];
                let width = shown.chars().count() * cw;
                let bg = if find.select_query { SELECT } else { PAPER };
                f.rect(
                    x as i32,
                    (band + 3 * z) as i32,
                    (w.saturating_sub(left + x)) as i32,
                    ch as i32,
                    PAPER,
                );
                if width > 0 {
                    f.rect(x as i32, (band + 3 * z) as i32, width as i32, ch as i32, bg);
                }
                label(&mut f, x, band + 3 * z, shown, z, INK, bg);
                f.rect(
                    (x + width) as i32,
                    (band + 3 * z) as i32,
                    (2 * z) as i32,
                    ch as i32,
                    ACCENT,
                );
                let hint = if find.message.is_empty() {
                    "Enter next   Shift+Enter previous   Esc close"
                } else {
                    find.message
                };
                label(&mut f, left, h - ch - 4 * z, hint, z, MUTED, BAND);
            } else {
                label(&mut f, left, h - ch - 5 * z, msg, z, MUTED, BAND);
            }
        });
    }
}

static mut HISTORY: History = History::new();
static mut DOCUMENT: Editor = Editor::new();
static mut CLIPBOARD: Clipboard = Clipboard::new();
entry!(editor_main);
#[no_mangle]
pub extern "C" fn editor_main() -> ! {
    // This guest has one entry point and one input loop.
    let (e, clip) = unsafe {
        (
            &mut *core::ptr::addr_of_mut!(DOCUMENT),
            &mut *core::ptr::addr_of_mut!(CLIPBOARD),
        )
    };
    let history = unsafe { &mut *core::ptr::addr_of_mut!(HISTORY) };
    let mut find = Find::new();
    clip.connect();
    let help = "Ctrl+X Cut   Ctrl+V Paste   Ctrl+Z Undo   Ctrl+F Find";
    let mut d = Display::current();
    let mut view = View { cols: 80, first: 0 };
    let mut caps = false;
    let mut leaving = false;
    view.draw(&mut d, e, help, &find);
    log(b"Mockingbird ready\n");
    let mut repeat: Option<(usize, u64)> = None;
    loop {
        let k = if let Some(k) = d.next_press() {
            repeat = if !d.held[29]
                && (editor::printable(k, false, false).is_some()
                    || matches!(k, 14 | 72 | 75 | 77 | 80 | 83))
            {
                Some((k, time_us() + 450000))
            } else {
                None
            };
            k
        } else if let Some((k, deadline)) = repeat {
            if !d.held[k] {
                repeat = None;
                continue;
            }
            if time_us() < deadline {
                sleep_us(2000);
                continue;
            }
            repeat = Some((k, time_us() + 45000));
            k
        } else {
            sleep_us(2000);
            continue;
        };
        let shift = d.held[42] || d.held[54];
        let ctrl = d.held[29];
        let mut msg = help;
        if ctrl || matches!(k, 1 | 14 | 15 | 28 | 71 | 72 | 75 | 77 | 79 | 80 | 83) {
            history.break_typing();
        }
        if find.open {
            match (ctrl, k) {
                (_, 1) => {
                    find.open = false;
                    if !find.query().is_empty() {
                        msg = "F3 next match   Shift+F3 previous   Ctrl+F edit search";
                    }
                }
                (_, 28 | 61) => find.next(e, shift),
                (true, 30 | 33) => find.select_query = true,
                (true, 47) => match clip.get() {
                    Ok(bytes) if !bytes.is_empty() => find.insert(e, bytes),
                    Ok(_) => find.message = "Clipboard is empty",
                    Err(_) => find.message = "Clipboard unavailable",
                },
                (false, 14) => find.backspace(e),
                (false, 58) => caps = !caps,
                (false, _) => {
                    if let Some(b) = editor::printable(k, shift, caps) {
                        find.insert(e, &[b]);
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        } else if leaving {
            if k == 28 {
                exit(0);
            }
            leaving = false;
        } else if ctrl {
            match k {
                33 => find.begin(e),
                44 | 21 => {
                    let redo = k == 21 || shift;
                    if !history.step(e, redo) {
                        msg = if redo {
                            "Nothing to redo."
                        } else {
                            "Nothing to undo."
                        };
                    }
                }
                30 => e.all(),
                71 => e.move_to(0, shift),
                79 => e.move_to(e.len, shift),
                45 | 46 if !e.selection().is_empty() => {
                    if clip.put(&e.bytes[e.selection()]).is_ok() {
                        if k == 45 {
                            history.edit(e, e.selection(), &[], None);
                        }
                    } else {
                        msg = "Clipboard unavailable. Selection kept.";
                    }
                }
                47 => match clip.get() {
                    Ok([]) => msg = "Clipboard is empty.",
                    Ok(bytes) => {
                        if !history.edit(e, e.selection(), bytes, None) {
                            msg = "Text limit reached (32 KiB). Selection kept.";
                        }
                    }
                    Err(()) => msg = "Paste unavailable. Use text up to 32 KiB.",
                },
                _ => continue,
            }
        } else {
            match k {
                61 => {
                    history.break_typing();
                    if find.query().is_empty() {
                        find.begin(e);
                    } else {
                        find.repeat(e, shift);
                        if !find.message.is_empty() {
                            msg = find.message;
                        }
                    }
                }
                1 if e.len == 0 => exit(0),
                1 => {
                    leaving = true;
                    msg = "Discard text and exit? Enter confirms. Any other key returns.";
                }
                58 => {
                    caps = !caps;
                    continue;
                }
                75 | 77 => e.horizontal(k == 75, shift),
                72 | 80 | 71 | 79 => {
                    let (row, col) = e.row_col(e.caret, view.cols);
                    let (r, c) = match k {
                        72 => (row.saturating_sub(1), col),
                        80 => (row + 1, col),
                        71 => (row, 0),
                        _ => (row, view.cols - 1),
                    };
                    e.vertical(r, c, view.cols, shift);
                }
                14 | 83 => history.erase(e, k == 14),
                _ => {
                    let byte = match k {
                        28 => b'\n',
                        15 => b'\t',
                        _ => match editor::printable(k, shift, caps) {
                            Some(b) => b,
                            None => continue,
                        },
                    };
                    if !history.edit(
                        e,
                        e.selection(),
                        &[byte],
                        if matches!(k, 28 | 15) {
                            None
                        } else {
                            Some(time_us())
                        },
                    ) {
                        msg = "Text limit reached (32 KiB).";
                    }
                }
            }
        }
        view.draw(&mut d, e, msg, &find);
    }
}
