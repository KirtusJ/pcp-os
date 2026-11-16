#[path = "../../../../PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"]
mod font;
use pcp_game_guest::ink::Frame;

pub(super) fn text(frame: &mut Frame, x: usize, y: usize, line: &str, color: u32) {
    for (i, c) in line.bytes().take(frame.pitch.saturating_sub(x) / font::CELL_WIDTH).enumerate() {
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
                let pos = (y + row) * frame.pitch + x + i * font::CELL_WIDTH + col;
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
