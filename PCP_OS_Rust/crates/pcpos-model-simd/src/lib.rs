//! Narrow x86 intrinsic boundary. GGUF parsing and row construction remain in safe Rust.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg(all(target_arch = "x86_64", target_feature = "sse2"))]

use core::arch::x86_64::*;

/// A capability constructed only when the processor and the OS enable YMM registers.
#[derive(Clone, Copy, Debug)]
pub struct Avx2(());

impl Avx2 {
    pub fn detect() -> Option<Self> {
        if __cpuid(0).eax < 7 { return None; }
        let ecx = __cpuid(1).ecx;
        let required = (1 << 26) | (1 << 27) | (1 << 28); // XSAVE, OSXSAVE, AVX
        if ecx & required != required || __cpuid_count(7, 0).ebx & (1 << 5) == 0 {
            return None;
        }
        // OSXSAVE guarantees XGETBV is usable. Both XMM and YMM state must be enabled.
        (unsafe { _xgetbv(0) } & 6 == 6).then_some(Self(()))
    }

    /// Accumulate one Q4_K block. Subgroup scales/minima have already been decoded.
    pub fn q4(self, block: &[u8; 144], input: &[f32; 256], scales: &[(f32,f32); 8], sum: f64) -> Option<f64> {
        // The private capability guards every entry into AVX2 code.
        unsafe { q4(block, input, scales, sum) }
    }

    /// Accumulate one Q6_K block. The final half scale has already been decoded.
    pub fn q6(self, block: &[u8; 210], input: &[f32; 256], scale: f32, sum: f64) -> Option<f64> {
        unsafe { q6(block, input, scale, sum) }
    }
}

#[target_feature(enable = "avx2")]
fn bytes8(bytes: &[u8], offset: usize) -> __m256i {
    let word = i64::from_le_bytes(bytes[offset..offset+8].try_into().unwrap());
    _mm256_cvtepu8_epi32(_mm_cvtsi64_si128(word))
}

#[target_feature(enable = "avx2")]
fn accumulate(sum: f64, weights: __m256, input: &[f32; 8]) -> Option<f64> {
    // Exactly eight readable f32 values. Unaligned model/input addresses are supported.
    let values = unsafe { _mm256_loadu_ps(input.as_ptr()) };
    let absolute = _mm256_andnot_ps(_mm256_set1_ps(-0.0), values);
    let finite = _mm256_cmp_ps::<_CMP_LE_OQ>(absolute, _mm256_set1_ps(f32::MAX));
    if _mm256_movemask_ps(finite) != 255 { return None; }

    // Retain the scalar reference's f64 products and addition order. AVX2 fits four
    // doubles per register; the two halves correspond to the eight decoded weights.
    let low = _mm256_mul_pd(_mm256_cvtps_pd(_mm256_castps256_ps128(weights)),
                            _mm256_cvtps_pd(_mm256_castps256_ps128(values)));
    let high = _mm256_mul_pd(_mm256_cvtps_pd(_mm256_extractf128_ps::<1>(weights)),
                             _mm256_cvtps_pd(_mm256_extractf128_ps::<1>(values)));
    let mut products = [0.0; 8];
    // The two stores cover disjoint groups of four within the local array.
    unsafe {
        _mm256_storeu_pd(products.as_mut_ptr(), low);
        _mm256_storeu_pd(products[4..].as_mut_ptr(), high);
    }
    Some(products.into_iter().fold(sum, |acc, product| acc + product))
}

#[target_feature(enable = "avx2")]
fn q4(block: &[u8; 144], input: &[f32; 256], scales: &[(f32,f32); 8], mut sum: f64) -> Option<f64> {
    let mask = _mm256_set1_epi32(15);
    for (group, &(scale, minimum)) in scales.iter().enumerate() {
        for j in (0..32).step_by(8) {
            let packed = bytes8(block, 16 + group / 2 * 32 + j);
            let q = if group % 2 == 0 { _mm256_and_si256(packed, mask) } else { _mm256_srli_epi32::<4>(packed) };
            let weights = _mm256_sub_ps(_mm256_mul_ps(_mm256_set1_ps(scale), _mm256_cvtepi32_ps(q)),
                                        _mm256_set1_ps(minimum));
            let start = group * 32 + j;
            sum = accumulate(sum, weights, input[start..start+8].try_into().unwrap())?;
        }
    }
    Some(sum)
}

#[target_feature(enable = "avx2")]
fn q6(block: &[u8; 210], input: &[f32; 256], scale: f32, mut sum: f64) -> Option<f64> {
    let low_mask = _mm256_set1_epi32(15);
    let high_mask = _mm256_set1_epi32(3);
    for half in 0..2 {
        for group in 0..4 {
            let shift = _mm_cvtsi32_si128((2 * group) as i32);
            for j in (0..32).step_by(8) {
                let packed = bytes8(block, half * 64 + (group % 2) * 32 + j);
                let low = if group < 2 { _mm256_and_si256(packed, low_mask) } else { _mm256_srli_epi32::<4>(packed) };
                let packed_high = bytes8(block, 128 + half * 32 + j);
                let high = _mm256_and_si256(_mm256_srl_epi32(packed_high, shift), high_mask);
                let q = _mm256_or_si256(low, _mm256_slli_epi32::<4>(high));
                let signed = _mm256_sub_epi32(q, _mm256_set1_epi32(32));
                let subgroup_scale = scale * f32::from(block[192 + half*8 + group*2 + j/16] as i8);
                let weights = _mm256_mul_ps(_mm256_set1_ps(subgroup_scale), _mm256_cvtepi32_ps(signed));
                let start = half * 128 + group * 32 + j;
                sum = accumulate(sum, weights, input[start..start+8].try_into().unwrap())?;
            }
        }
    }
    Some(sum)
}
