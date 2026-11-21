//! Resolution-aware first-launch desktop background.
//!
//! Procedural, deterministic software rasterization: every selected display
//! pixel receives its own colour. No fixed bitmap, model, network or disk.
//! The active video mode, channel layout and framebuffer mapping are owned
//! by the driver/loader, not this painter. This file intentionally contains
//! no platform calls and can be unit-tested on an ordinary Rust host.

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub stride_bytes: u32,
}

impl Geometry {
    /// Accept only a mapped 32-bit packed framebuffer with enough row stride.
    /// Driver/firmware must also authenticate its pixel channel layout.
    pub fn checked(width: u32, height: u32, stride_bytes: u32, mapped_bytes: usize) -> Option<Self> {
        // These bounds also keep the pixel loop and one-row allocation bounded.
        if !(320..=16384).contains(&width) || !(200..=16384).contains(&height) {
            return None;
        }
        let minimum_stride = width.checked_mul(4)?;
        if stride_bytes < minimum_stride || stride_bytes % 4 != 0 {
            return None;
        }
        let visible_extent = (height as u64).checked_mul(stride_bytes as u64)?;
        if visible_extent > mapped_bytes as u64 {
            return None;
        }
        Some(Self { width, height, stride_bytes })
    }
}

fn mix(from: u32, to: u32, weight: u32) -> u32 {
    let weight = weight.min(255);
    let inv = 255 - weight;
    let channel = |shift: u32| {
        let a = (from >> shift) & 255;
        let b = (to >> shift) & 255;
        (a * inv + b * weight + 127) / 255
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn fraction(position: u32, extent: u32) -> u32 {
    if extent < 2 { 0 } else { ((position as u64 * 65535) / (extent - 1) as u64) as u32 }
}

/// Colour of pixel (x,y) at the active display's native resolution.
/// Integer-only normalized geometry stays independent of the aspect ratio.
pub fn pixel(g: Geometry, x: u32, y: u32) -> u32 {
    let u = fraction(x, g.width) as i64;
    let v = fraction(y, g.height) as i64;
    // Deep-navy vertical wash with a subdued horizontal light gradient.
    let mut colour = mix(0x0B1D30, 0x163F55, (v * 175 / 65535 + u * 35 / 65535) as u32);

    // A wide, softly rasterized teal diagonal, not a stretched texture.
    let ridge = 51000 - (u * 26000 / 65535);
    let separation = (v - ridge).abs();
    if separation < 18000 {
        let opacity = (18000 - separation) * 92 / 18000;
        colour = mix(colour, 0x1E7380, opacity as u32);
    }

    // Low-contrast light pool to balance the left-side launcher panel.
    let dx = (u - 50000) / 256;
    let dy = (v - 24000) / 256;
    let distance = dx * dx + dy * dy;
    const LIMIT: i64 = 190 * 190;
    if distance < LIMIT {
        let opacity = (LIMIT - distance) * 62 / LIMIT;
        colour = mix(colour, 0x39A3A8, opacity as u32);
    }

    // Restrained warm horizon for depth. All arithmetic is bounded.
    if v > 57500 {
        let opacity = (v - 57500) * 20 / (65535 - 57500);
        colour = mix(colour, 0x5B4752, opacity as u32);
    }
    colour
}

/// Emits precisely one packed RGB row of width pixels for every y in 0..height.
/// Only a single row is allocated, and framebuffer stride is handled by the
/// caller's device writer. A failure to allocate returns false before writing.
pub fn rasterize(g: Geometry, mut emit: impl FnMut(u32, &[u32])) -> bool {
    let mut row = Vec::<u32>::new();
    if row.try_reserve_exact(g.width as usize).is_err() {
        return false;
    }
    row.resize(g.width as usize, 0);
    for y in 0..g.height {
        for x in 0..g.width {
            row[x as usize] = pixel(g, x, y);
        }
        emit(y, &row);
    }
    true
}
