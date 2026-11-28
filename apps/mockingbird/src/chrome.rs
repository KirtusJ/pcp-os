use super::mask;
use pcp_game_guest::ink::Frame;

pub struct Lettering {
    scale: usize,
    metrics: &'static [[u8; 7]],
    coverage: &'static [u8],
    title: &'static [u8],
}

impl Lettering {
    pub fn new(scale: usize) -> Self {
        let (labels, title): (&[u8], &[u8]) = if scale == 2 {
            (
                include_bytes!("../brand/labels-2x.bin"),
                include_bytes!("../brand/wordmark-2x.alpha"),
            )
        } else {
            (
                include_bytes!("../brand/labels-1x.bin"),
                include_bytes!("../brand/wordmark-1x.alpha"),
            )
        };
        let (metrics, coverage) = labels.split_at(95 * 7);
        Self {
            scale,
            metrics: metrics.as_chunks::<7>().0,
            coverage,
            title,
        }
    }

    fn glyph(&self, ch: u8) -> [u8; 7] {
        let index = usize::from(ch.clamp(b' ', b'~') - b' ');
        self.metrics[index]
    }

    pub fn width(&self, text: &str) -> usize {
        text.bytes().map(|ch| usize::from(self.glyph(ch)[0])).sum()
    }

    pub fn label(&self, frame: &mut Frame, mut x: usize, y: usize, text: &str, ink: u32, bg: u32) {
        for ch in text.bytes() {
            let [advance, left, top, width, height, lo, hi] = self.glyph(ch);
            if width != 0 {
                let start = usize::from(u16::from_le_bytes([lo, hi]));
                let width = usize::from(width);
                let end = start + width * usize::from(height);
                mask(
                    frame,
                    (x + usize::from(left)).saturating_sub(self.scale),
                    y + usize::from(top),
                    &self.coverage[start..end],
                    width,
                    ink,
                    bg,
                );
            }
            x += usize::from(advance);
        }
    }

    pub fn wordmark(&self, frame: &mut Frame, x: usize, y: usize, ink: u32, bg: u32) {
        mask(frame, x, y, self.title, 120 * self.scale, ink, bg);
    }

    pub fn hints(&self, frame: &mut Frame, mut x: usize, y: usize, text: &str, ink: u32, bg: u32) {
        for group in text.split("   ") {
            self.label(frame, x, y, group, ink, bg);
            x += self.width(group) + 20 * self.scale;
        }
    }
}
