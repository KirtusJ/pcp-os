//! Arithmetic for verified model projections. Intrinsics have a separate runtime gate.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod avx;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub use avx::Avx2;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod packed;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub use packed::PackedAvx2;

/// ggml block_q8_K. A single prepared activation feeds every weight row.
#[repr(C)]
#[derive(Clone)]
pub struct Q8Block {
    pub scale: f32,
    pub values: [i8; 256],
    pub sums: [i16; 16],
}

impl Q8Block {
    pub const ZERO: Self = Self { scale: 0.0, values: [0; 256], sums: [0; 16] };

    /// Mirrors ggml quantize_row_q8_K_ref. Decline values whose inverse scale
    /// cannot be represented, allowing the caller to retain its floating-point path.
    pub fn pack(&mut self, input: &[f32; 256]) -> bool {
        let mut peak = 0.0_f32;
        for &x in input {
            if !x.is_finite() { return false; }
            if x.abs() > peak.abs() { peak = x; }
        }
        if peak == 0.0 {
            *self = Self::ZERO;
            return true;
        }
        let inverse = -127.0 / peak;
        if !inverse.is_finite() { return false; }
        self.scale = 1.0 / inverse;
        for ((s, values), group) in self.sums.iter_mut()
            .zip(self.values.chunks_exact_mut(16)).zip(input.chunks_exact(16))
        {
            let mut sum = 0;
            for (value, x) in values.iter_mut().zip(group) {
                // The product is in [-127, 127]. The nearest_int bias from ggml
                // rounds to nearest, ties to even, with no host math-library call.
                let bits = (x * inverse + 12_582_912.0).to_bits();
                let rounded = (bits & 0x007f_ffff) as i32 - 0x0040_0000;
                *value = rounded.min(127) as i8;
                sum += i16::from(*value);
            }
            *s = sum;
        }
        true
    }
}
