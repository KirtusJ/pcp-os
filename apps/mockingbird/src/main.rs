#![no_main]
#![no_std]
mod clipboard;
mod editor;
#[path = "../../../PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"]
mod font;

use clipboard::Clipboard;
use editor::{advance, Editor};
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
fn label(f: &mut Frame, x: usize, y: usize, s: &str, z: usize, color: u32) {
    for (i, b) in s.bytes().enumerate() {
        glyph(f, x + i * font::CELL_WIDTH * z, y, b, z, color, PAPER);
    }
}

struct View {
    cols: usize,
    first: usize,
}
impl View {
    fn draw(&mut self, d: &mut Display, e: &Editor, msg: &str) {
        d.draw(|surface, w, h, pitch| {
            let z = (w / 820).min(h / 475).clamp(1, 2);
            let cw = font::CELL_WIDTH * z;
            let ch = font::CELL_HEIGHT * z;
            let left = 2 * cw;
            let top = 2 * ch;
            self.cols = (w.saturating_sub(2 * left) / cw).max(1);
            let rows = (h.saturating_sub(top + ch + 10 * z) / ch).max(1);
            let (r, c) = e.row_col(e.caret, self.cols);
            self.first = self.first.min(r).max((r + 1).saturating_sub(rows));
            let mut f = Frame { px: surface, pitch };
            f.px.fill(PAPER);
            label(&mut f, left, 6 * z, "Mockingbird", z, INK);
            f.rect(
                left as i32,
                (top - ch / 3) as i32,
                (w - 2 * left) as i32,
                z as i32,
                0xced1c6,
            );
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
                    let visible = match b {
                        '\u{2018}' | '\u{2019}' => b'\'',
                        '\u{201c}' | '\u{201d}' => b'"',
                        '\u{2013}' | '\u{2014}' => b'-',
                        '\u{a0}' => b' ',
                        c if c.is_ascii() => c as u8,
                        _ => b'?',
                    };
                    glyph(&mut f, x, y, visible, z, INK, bg);
                }
                advance(&mut at, b, self.cols);
            }
            f.rect(
                (left + c * cw) as i32,
                (top + (r - self.first) * ch) as i32,
                (2 * z) as i32,
                ch as i32,
                ACCENT,
            );
            label(&mut f, left, h - ch - 4 * z, msg, z, MUTED);
        });
    }
}

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
    clip.connect();
    let help = if clip.shared() {
        "Shift+arrows select   Ctrl+X cut   Ctrl+V paste   Windows clipboard"
    } else {
        "Shift+arrows select   Ctrl+X cut   Ctrl+V paste   Local clipboard"
    };
    let mut d = Display::current();
    let mut view = View { cols: 80, first: 0 };
    let mut caps = false;
    let mut leaving = false;
    view.draw(&mut d, e, help);
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
        if leaving {
            if k == 28 {
                exit(0);
            }
            leaving = false;
        } else if ctrl {
            match k {
                30 => e.all(),
                71 => e.move_to(0, shift),
                79 => e.move_to(e.len, shift),
                45 | 46 if !e.selection().is_empty() => {
                    if clip.put(&e.bytes[e.selection()]).is_ok() {
                        if k == 45 {
                            e.replace(&[]);
                        }
                    } else {
                        msg = "Clipboard unavailable. Selection kept.";
                    }
                }
                47 => match clip.get() {
                    Ok([]) => msg = "Clipboard is empty.",
                    Ok(bytes) => {
                        if !e.replace(bytes) {
                            msg = "Text limit reached (32 KiB). Selection kept.";
                        }
                    }
                    Err(()) => msg = "Paste unavailable. Use text up to 32 KiB.",
                },
                _ => continue,
            }
        } else {
            match k {
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
                14 | 83 => e.erase(k == 14),
                _ => {
                    let byte = match k {
                        28 => b'\n',
                        15 => b'\t',
                        _ => match editor::printable(k, shift, caps) {
                            Some(b) => b,
                            None => continue,
                        },
                    };
                    if !e.replace(core::slice::from_ref(&byte)) {
                        msg = "Text limit reached (32 KiB).";
                    }
                }
            }
        }
        view.draw(&mut d, e, msg);
    }
}
