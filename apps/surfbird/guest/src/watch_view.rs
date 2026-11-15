//! Local views of the authenticated watch document, using the kernel's printable ASCII font.
#[path = "../../../../PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"]
mod font;
use super::{H, PIXELS, W};
use pcp_game_guest::{ink::Frame, log, sleep_us, Display};
use surfbird_cable::watch_document::Document;

const BG: u32 = 0x10252c;
fn text(frame: &mut Frame, x: usize, y: usize, line: &str, color: u32) {
    for (i, c) in line.bytes().enumerate() {
        let Some(glyph) = c
            .checked_sub(font::FIRST)
            .and_then(|k| font::GLYPHS.get(k as usize))
        else {
            continue;
        };
        for row in 0..font::CELL_HEIGHT {
            for col in 0..font::CELL_WIDTH {
                let a = glyph[row * font::CELL_WIDTH + col] as u32;
                if a == 0 {
                    continue;
                }
                let pos = (y + row) * W + x + i * font::CELL_WIDTH + col;
                let Some(pixel) = frame.px.get_mut(pos) else {
                    continue;
                };
                let shade = |shift: u32| {
                    (((color >> shift) & 255) * a + ((*pixel >> shift) & 255) * (255 - a)) / 255
                };
                *pixel = shade(16) << 16 | shade(8) << 8 | shade(0);
            }
        }
    }
}
pub fn show(doc: &Document) {
    let mut display = Display::new(W, H);
    let mut details = false;
    super::record("document-title", doc.title());
    super::record("document-canonical", doc.canonical());
    loop {
        let pixels = unsafe { &mut *(&raw mut PIXELS) };
        pixels.fill(BG);
        let mut f = Frame {
            px: pixels,
            pitch: W,
        };
        text(&mut f, 24, 20, "Surfbird", 0x85ddd1);
        f.rect(24, 52, 592, 1, 0x48636a);
        if details {
            text(&mut f, 24, 74, "Document address", 0xf4f6f7);
            text(&mut f, 24, 122, doc.canonical(), 0xb9e4e2);
            text(
                &mut f,
                24,
                176,
                "Canonical address from the HTML head.",
                0xd0dada,
            );
            text(
                &mut f,
                24,
                208,
                "Server identity verified by TLS.",
                0xd0dada,
            );
            text(
                &mut f,
                24,
                310,
                "B  Back to title       Esc  Exit",
                0xa8d2ce,
            );
        } else {
            text(&mut f, 24, 72, "YouTube / Document title", 0x91c7c3);
            let mut line = doc.title();
            let mut y = 119;
            while !line.is_empty() {
                let cut = if line.len() <= 58 {
                    line.len()
                } else {
                    line[..59].rfind(' ').filter(|&n| n > 0).unwrap_or(58)
                };
                text(&mut f, 24, y, &line[..cut], 0xf4f6f7);
                line = line[cut..].trim_start();
                y += 28;
            }
            text(
                &mut f,
                24,
                256,
                "Video playback is unavailable in this reader.",
                0xc2d0d1,
            );
            text(
                &mut f,
                24,
                310,
                "1  Document address       Esc  Exit",
                0xa8d2ce,
            );
        }
        display.show(pixels);
        log(if details {
            b"SURFBIRD view address\n"
        } else {
            b"SURFBIRD view title\n"
        });
        loop {
            display.poll();
            if display.pressed[1] {
                return;
            }
            if !details && display.pressed[2] {
                details = true;
                break;
            }
            if details && display.pressed[48] {
                details = false;
                break;
            }
            sleep_us(3000);
        }
    }
}
