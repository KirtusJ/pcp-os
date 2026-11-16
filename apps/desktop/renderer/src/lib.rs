//! Pure, device-independent PCP OS desktop rasterizer and responsive launcher geometry.
//!
//! This module does not choose a physical display mode and does not access a framebuffer.
//! Firmware and device drivers are responsible for reporting an accurate mode, pitch,
//! pixel layout and mapping. The native kernel shell validates those measurements
//! before writing each packed RGB row to the existing safe display driver.
#![no_std]

/// A bounded raster avoids untrusted giant dimensions and arithmetic overflow.
/// Displays beyond this limit currently fall back to the kernel's flat-colour view.
pub const MAX_DIMENSION: u32 = 8192;

/// Fail closed: the caller must not rasterize a dimension or pitch it cannot map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RasterError {
    InvalidDimensions,
    InvalidPitch,
    Unmapped,
    InvalidRow,
}

/// Pixel dimensions and byte stride of one mapped 32-bit RGB frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub pitch_bytes: u32,
}

impl Surface {
    /// Check that a tightly produced row can safely be sent into a pitched frame.
    ///
    /// The framebuffer adapter, not the rasterizer, owns the final memory writes.
    pub fn new(width: u32, height: u32, pitch_bytes: u32, mapped_bytes: usize) -> Result<Self, RasterError> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(RasterError::InvalidDimensions);
        }
        let row_bytes = width.checked_mul(4).ok_or(RasterError::InvalidPitch)?;
        if pitch_bytes < row_bytes || pitch_bytes % 4 != 0 {
            return Err(RasterError::InvalidPitch);
        }
        let needed = (pitch_bytes as usize)
            .checked_mul(height as usize)
            .ok_or(RasterError::Unmapped)?;
        if needed > mapped_bytes {
            return Err(RasterError::Unmapped);
        }
        Ok(Self { width, height, pitch_bytes })
    }
}

/// Integer-only RGB interpolation; the per-pixel raster is sized by the real
/// selected mode, rather than scaling a fixed bitmap.
fn mix(a: u32, b: u32, amount: u32, full: u32) -> u32 {
    let n = amount.min(full);
    let channel = |shift| {
        let av = (a >> shift) & 255;
        let bv = (b >> shift) & 255;
        (av * (full - n) + bv * n + full / 2) / full
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

fn pixel(surface: Surface, x: u32, y: u32) -> u32 {
    let last_y = surface.height.saturating_sub(1).max(1);
    let base = mix(0x10283F, 0x091C31, y.saturating_mul(255) / last_y, 255);
    let horizon = surface.height * 62 / 100;
    let mut colour = if y > horizon {
        let below = surface.height.saturating_sub(horizon).max(1);
        mix(base, 0x17344B, (y - horizon) * 155 / below, 255)
    } else {
        base
    };
    // Subtle upper-right glow. All coordinates are normalized, so every
    // raster dimension is sampled independently, including odd widths.
    let nx = (x as i64 * 1000 / surface.width as i64) - 750;
    let ny = (y as i64 * 1000 / surface.height as i64) - 265;
    let distance2 = nx * nx + ny * ny;
    if distance2 < 80_000 {
        colour = mix(colour, 0x41BAD7, (80_000 - distance2) as u32, 480_000);
    }
    colour
}

/// Rasterize one logical row. Padding belongs to the device stride and is
/// deliberately excluded from the slice. Never writes beyond `width`.
pub fn raster_row(surface: Surface, y: u32, pixels: &mut [u32]) -> Result<(), RasterError> {
    if y >= surface.height || pixels.len() != surface.width as usize {
        return Err(RasterError::InvalidRow);
    }
    for (x, colour) in pixels.iter_mut().enumerate() {
        *colour = pixel(surface, x as u32, y);
    }
    Ok(())
}

/// Character-grid geometry for the kernel's built-in first-launch desktop.
/// The foreground always has a selected-item highlight and keyboard help.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub pane_left: u32,
    pub pane_cols: u32,
    pub top: u32,
    pub list_row: u32,
    pub slots: usize,
    pub info_row: u32,
    pub footer_row: u32,
}

impl Layout {
    pub fn new(cols: u32, rows: u32) -> Option<Self> {
        if cols < 32 || rows < 12 {
            return None;
        }
        let pane_cols = cols.saturating_sub(4).min(104);
        let pane_left = (cols - pane_cols) / 2;
        let top = if rows >= 40 { 2 } else { 0 };
        let list_row = top + 5;
        let footer_row = rows - 2;
        let info_row = footer_row - 3;
        let slots = info_row.saturating_sub(list_row).max(1) as usize;
        Some(Self { pane_left, pane_cols, top, list_row, slots, info_row, footer_row })
    }

    /// Index of the first application in the visible scroll window.
    pub fn first_visible(self, selected: usize, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        selected.min(count - 1).saturating_add(1).saturating_sub(self.slots)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    #[test]
    fn raster_fits_different_reported_resolutions() {
        for (w, h, stride) in [(640, 480, 640), (1024, 768, 1024), (1366, 768, 1408), (1920, 1080, 1920), (7680, 4320, 7680)] {
            let s = Surface::new(w, h, stride * 4, (stride as usize) * (h as usize) * 4).unwrap();
            let mut pixels = vec![0xDEADBEEF; w as usize];
            raster_row(s, 0, &mut pixels).unwrap();
            let top = pixels[0];
            raster_row(s, h - 1, &mut pixels).unwrap();
            assert_ne!(top, pixels[0], "{w}x{h} must paint different scanlines");
            assert!(pixels.iter().all(|x| x & 0xFF00_0000 == 0));
        }
    }

    #[test]
    fn refuses_inconsistent_or_unmapped_video() {
        assert_eq!(Surface::new(0, 480, 1920, 1_000_000), Err(RasterError::InvalidDimensions));
        assert_eq!(Surface::new(9000, 600, 36_000, 99_000_000), Err(RasterError::InvalidDimensions));
        assert_eq!(Surface::new(640, 480, 2556, 99_000_000), Err(RasterError::InvalidPitch));
        assert_eq!(Surface::new(640, 480, 2561, 99_000_000), Err(RasterError::InvalidPitch));
        assert_eq!(Surface::new(640, 480, 2560, 480 * 2560 - 1), Err(RasterError::Unmapped));
    }

    #[test]
    fn rejects_bad_scanline_without_modification() {
        let s = Surface::new(800, 600, 3200, 1_920_000).unwrap();
        let mut too_short = [0xACAC_ACAC; 3];
        assert_eq!(raster_row(s, 0, &mut too_short), Err(RasterError::InvalidRow));
        assert_eq!(too_short, [0xACAC_ACAC; 3]);
        let mut exact = vec![0; 800];
        assert_eq!(raster_row(s, 600, &mut exact), Err(RasterError::InvalidRow));
    }

    #[test]
    fn launcher_fits_small_medium_and_large_grid() {
        assert!(Layout::new(31, 30).is_none());
        assert!(Layout::new(80, 11).is_none());
        for (c, r) in [(40, 12), (80, 30), (128, 48), (240, 68)] {
            let a = Layout::new(c, r).unwrap();
            assert!(a.pane_left + a.pane_cols <= c);
            assert!(a.list_row + a.slots as u32 <= a.info_row);
            assert!(a.info_row + 3 <= a.footer_row);
            assert!(a.footer_row + 1 < r);
        }
    }

    #[test]
    fn long_application_list_keeps_keyboard_selection_visible() {
        let a = Layout::new(80, 30).unwrap();
        assert_eq!(a.first_visible(0, 150), 0);
        assert!(a.first_visible(149, 150) + a.slots >= 150);
        assert_eq!(a.first_visible(30, 0), 0);
        assert_eq!(a.first_visible(100, 3), 0);
    }
}
