#![no_main]
#![no_std]
mod chrome;
mod clipboard;
mod editor;
mod find;
#[path = "../../../PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"]
mod font;
mod theme;

use clipboard::Clipboard;
use editor::{cell_width, Editor, History};
use find::Find;
use pcp_game_guest::{entry, exit, ink::Frame, log, sleep_us, time_us, Display};
use theme::{Palette, Theme};

const F6: usize = 64;
const DOUBLE: &[u8; 95 * 20 * 40] =
    include_bytes!("../../../PCP_OS_Rust/crates/pcpos-kernel/src/font-2x.bin");

fn blend(ink: u32, bg: u32, alpha: u8) -> u32 {
    let a = u32::from(alpha);
    let mix = |s: u32| (((ink >> s) & 255) * a + ((bg >> s) & 255) * (255 - a)) / 255;
    mix(16) << 16 | mix(8) << 8 | mix(0)
}

fn mask(frame: &mut Frame, x: usize, y: usize, coverage: &[u8], width: usize, ink: u32, bg: u32) {
    for (row, mask_row) in frame
        .px
        .chunks_exact_mut(frame.pitch)
        .skip(y)
        .zip(coverage.chunks_exact(width))
    {
        let Some(tail) = row.get_mut(x..) else { break };
        for (pixel, &alpha) in tail.iter_mut().zip(mask_row) {
            if alpha != 0 {
                *pixel = blend(ink, bg, alpha);
            }
        }
    }
}

fn glyph(frame: &mut Frame, x: usize, y: usize, ch: u8, scale: usize, ink: u32, bg: u32) {
    if !(font::FIRST..=b'~').contains(&ch) {
        return;
    }
    let index = usize::from(ch - font::FIRST);
    let width = font::CELL_WIDTH * scale;
    let coverage: &[u8] = if scale == 2 {
        let length = width * font::CELL_HEIGHT * scale;
        &DOUBLE[index * length..(index + 1) * length]
    } else {
        &font::GLYPHS[index]
    };
    mask(frame, x, y, coverage, width, ink, bg);
}
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

fn bird(f: &mut Frame, x: usize, y: usize, z: usize, palette: &Palette) {
    let coverage: &[u8] = if z == 2 {
        include_bytes!("../brand/header-64.alpha")
    } else {
        include_bytes!("../brand/header-32.alpha")
    };
    mask(f, x, y, coverage, 32 * z, palette.title, palette.band);
}
struct View {
    cols: usize,
    first: usize,
    theme: Theme,
}
impl View {
    fn draw(&mut self, d: &mut Display, e: &Editor, msg: &str, find: &Find) {
        let palette = self.theme.palette();
        d.draw(|surface, w, h, pitch| {
            let z = (w / 820).min(h / 475).clamp(1, 2);
            let cw = font::CELL_WIDTH * z;
            let ch = font::CELL_HEIGHT * z;
            let left = 2 * cw;
            let top = 3 * ch;
            let bottom = if find.open { 3 * ch } else { ch + 12 * z };
            self.cols = (w.saturating_sub(2 * left) / cw).max(1);
            let rows = (h.saturating_sub(top + bottom) / ch).max(1);
            let caret = e.caret_cell(self.cols);
            let (r, c) = (caret.row, caret.col);
            self.first = self.first.min(r).max((r + 1).saturating_sub(rows));
            let mut f = Frame { px: surface, pitch };
            f.px.fill(palette.sheet);
            f.rect(0, 0, w as i32, (2 * ch) as i32, palette.band);
            bird(&mut f, left, 4 * z, z, palette);
            let ui = chrome::Lettering::new(z);
            let title_x = left + 38 * z;
            ui.wordmark(&mut f, title_x, 8 * z, palette.title, palette.band);
            let title_end = title_x + 120 * z;
            let switch_width = ui
                .width(Theme::Light.switch_hint())
                .max(ui.width(Theme::Dark.switch_hint()));
            let switch_x = w.saturating_sub(left + switch_width);
            let session_x = switch_x.saturating_sub(24 * z + ui.width("Session only"));
            if session_x >= title_end + 16 * z {
                ui.label(
                    &mut f,
                    session_x,
                    10 * z,
                    "Session only",
                    palette.hint,
                    palette.band,
                );
            }
            if switch_x >= title_end + 16 * z {
                ui.label(
                    &mut f,
                    switch_x,
                    10 * z,
                    self.theme.switch_hint(),
                    palette.hint,
                    palette.band,
                );
            }
            f.rect(0, (2 * ch) as i32, w as i32, z as i32, palette.rule);
            let selection = e.selection();
            for cell in e
                .cells(self.cols)
                .skip_while(|cell| cell.row < self.first)
                .take_while(|cell| cell.row < self.first + rows)
            {
                if let Some(b) = cell.ch {
                    let x = left + cw * cell.col;
                    let y = top + ch * (cell.row - self.first);
                    if selection.contains(&cell.byte) {
                        // A selected hard break can occupy the right inset at a full line.
                        let cells = cell_width(b, cell.col).min((self.cols - cell.col).max(1));
                        f.rect(
                            x as i32,
                            y as i32,
                            (cw * cells) as i32,
                            ch as i32,
                            palette.selection,
                        );
                        glyph(
                            &mut f,
                            x,
                            y,
                            visible(b),
                            z,
                            palette.selected_text,
                            palette.selection,
                        );
                    } else {
                        glyph(&mut f, x, y, visible(b), z, palette.text, palette.sheet);
                    }
                }
            }
            if !find.open {
                f.rect(
                    (left + c * cw) as i32,
                    (top + (r - self.first) * ch) as i32,
                    (2 * z) as i32,
                    ch as i32,
                    if selection.contains(&e.caret) {
                        palette.selected_text
                    } else {
                        palette.text
                    },
                );
            }
            let band = h.saturating_sub(bottom);
            f.rect(0, band as i32, w as i32, bottom as i32, palette.band);
            f.rect(0, band as i32, w as i32, z as i32, palette.rule);
            if find.open {
                let y = band + 6 * z;
                ui.label(&mut f, left, y, "Find", palette.title, palette.band);
                let field = left + 5 * cw;
                let inset = 4 * z;
                let x = field + inset;
                let field_width = w.saturating_sub(left + field);
                let capacity = field_width.saturating_sub(2 * inset + 2 * z) / cw;
                let query = find.query();
                let skip = query.chars().count().saturating_sub(capacity);
                let offset = query
                    .char_indices()
                    .nth(skip)
                    .map_or(query.len(), |(i, _)| i);
                let shown = &query[offset..];
                let width = shown.chars().count() * cw;
                let (fg, bg) = if find.select_query {
                    (palette.selected_text, palette.selection)
                } else {
                    (palette.text, palette.sheet)
                };
                f.rect(
                    field as i32,
                    (y - 2 * z) as i32,
                    field_width as i32,
                    (ch + 4 * z) as i32,
                    palette.sheet,
                );
                if width > 0 {
                    f.rect(x as i32, y as i32, width as i32, ch as i32, bg);
                }
                label(&mut f, x, y, shown, z, fg, bg);
                f.rect(
                    field as i32,
                    (y + ch + 2 * z) as i32,
                    field_width as i32,
                    z as i32,
                    palette.field_edge,
                );
                f.rect(
                    (x + width) as i32,
                    y as i32,
                    (2 * z) as i32,
                    ch as i32,
                    palette.text,
                );
                let hint = if find.message.is_empty() {
                    "Enter next   Shift+Enter previous   Esc close"
                } else {
                    find.message
                };
                ui.hints(
                    &mut f,
                    left,
                    h - ch - 4 * z,
                    hint,
                    palette.hint,
                    palette.band,
                );
            } else {
                ui.hints(
                    &mut f,
                    left,
                    h - ch - 5 * z,
                    msg,
                    palette.hint,
                    palette.band,
                );
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
    let mut view = View {
        cols: 80,
        first: 0,
        theme: Theme::default(),
    };
    let mut caps = false;
    let mut leaving = false;
    let mut status = help;
    view.draw(&mut d, e, status, &find);
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
        if k == F6 && !ctrl && !shift && !d.held[56] {
            view.theme = view.theme.other();
            view.draw(&mut d, e, status, &find);
            continue;
        }
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
                    msg = "Discard text and exit? Enter discards. Esc returns.";
                }
                58 => {
                    caps = !caps;
                    continue;
                }
                75 | 77 => e.horizontal(k == 75, shift),
                72 | 80 => e.move_row(k == 80, view.cols, shift),
                71 | 79 => e.line_edge(k == 79, view.cols, shift),
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
        status = msg;
        view.draw(&mut d, e, status, &find);
    }
}
