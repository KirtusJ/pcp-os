//! AVX2 branches of ggml_vec_dot_q4_K_q8_K and ggml_vec_dot_q6_K_q8_K,
//! adapted from llama.cpp b12387. See THIRD_PARTY_NOTICES.
use super::{Avx2, Q8Block};
use core::arch::x86_64::*;

#[derive(Clone, Copy, Debug)]
pub struct PackedAvx2(());

impl PackedAvx2 {
    pub fn detect() -> Option<Self> {
        Avx2::detect()?;
        let required = (1 << 12) | (1 << 29); // FMA and F16C
        ((__cpuid(1).ecx & required) == required).then_some(Self(()))
    }

    /// None means the caller cancelled. A nonfinite result is rejected by the matrix reader.
    pub fn q4<const N: usize>(self, weights: &[[u8; 144]], input: [&[Q8Block]; N], go: &mut impl FnMut() -> bool) -> Option<[f32; N]> {
        assert!(input.iter().all(|v| v.len() == weights.len()));
        // Construction verifies AVX2, OS YMM state, F16C and FMA.
        unsafe { q4(weights, input, go) }
    }

    pub fn q6<const N: usize>(self, weights: &[[u8; 210]], input: [&[Q8Block]; N], go: &mut impl FnMut() -> bool) -> Option<[f32; N]> {
        assert!(input.iter().all(|v| v.len() == weights.len()));
        unsafe { q6(weights, input, go) }
    }
}

#[inline]
#[target_feature(enable="avx2")]
fn sum4(v: __m128) -> f32 {
    let pair = _mm_add_ps(v, _mm_movehl_ps(v, v));
    _mm_cvtss_f32(_mm_add_ss(pair, _mm_movehdup_ps(pair)))
}

#[inline]
#[target_feature(enable="avx2")]
fn sum8(v: __m256) -> f32 {
    sum4(_mm_add_ps(_mm256_castps256_ps128(v), _mm256_extractf128_ps::<1>(v)))
}

#[inline]
#[target_feature(enable="f16c")]
fn half(bytes: &[u8]) -> f32 {
    let bits = u16::from_le_bytes([bytes[0], bytes[1]]);
    _mm_cvtss_f32(_mm_cvtph_ps(_mm_cvtsi32_si128(i32::from(bits))))
}

#[inline]
#[target_feature(enable="avx2")]
fn load32(bytes: &[u8], at: usize) -> __m256i {
    let src: &[u8; 32] = bytes[at..at+32].try_into().unwrap();
    // The array proves the extent. GGUF and Q8 blocks can be unaligned.
    unsafe { _mm256_loadu_si256(src.as_ptr().cast()) }
}

#[inline]
#[target_feature(enable="avx2")]
fn activation(block: &Q8Block, group: usize) -> __m256i {
    let src: &[i8; 32] = block.values[group*32..group*32+32].try_into().unwrap();
    unsafe { _mm256_loadu_si256(src.as_ptr().cast()) }
}

#[inline]
#[target_feature(enable="avx2")]
fn group_scale(scales: __m256i, group: usize) -> __m256i {
    let byte = (group * 2) as i16;
    _mm256_shuffle_epi8(scales, _mm256_set1_epi16(byte | ((byte + 1) << 8)))
}

#[target_feature(enable="avx2,fma,f16c")]
fn q4<const N: usize>(weights: &[[u8; 144]], input: [&[Q8Block]; N], go: &mut impl FnMut() -> bool) -> Option<[f32; N]> {
    let mask = _mm256_set1_epi8(15);
    let mut sum = [_mm256_setzero_ps(); N];
    let mut min_sum = [_mm_setzero_ps(); N];
    for (i, w) in weights.iter().enumerate() {
        if !go() { return None; }
        let word = |i| u32::from_le_bytes(w[i..i+4].try_into().unwrap());
        let (s, m, hi) = (word(4), word(8), word(12));
        let unpacked = _mm_set_epi32(
            (((hi >> 4) & 0x0f0f0f0f) | (((m >> 6) & 0x03030303) << 4)) as i32,
            (m & 0x3f3f3f3f) as i32,
            ((hi & 0x0f0f0f0f) | (((s >> 6) & 0x03030303) << 4)) as i32,
            (s & 0x3f3f3f3f) as i32,
        );
        let decoded = _mm256_cvtepu8_epi16(unpacked);
        let scales = _mm256_broadcastsi128_si256(_mm256_castsi256_si128(decoded));
        let d = half(w);
        let dmin = half(&w[2..]);
        let mins = _mm256_extracti128_si256::<1>(decoded);
        for lane in 0..N {
            let a = &input[lane][i];
            // Exactly sixteen i16 subgroup sums.
            let sums = unsafe { _mm256_loadu_si256(a.sums.as_ptr().cast()) };
            let pairs = _mm_hadd_epi16(_mm256_castsi256_si128(sums), _mm256_extracti128_si256::<1>(sums));
            let correction = _mm_madd_epi16(mins, pairs);
            min_sum[lane] = _mm_fmadd_ps(_mm_set1_ps(-a.scale * dmin), _mm_cvtepi32_ps(correction), min_sum[lane]);
        }
        let mut acc = [_mm256_setzero_si256(); N];
        for j in 0..4 {
            let bits = load32(w, 16 + 32*j);
            let lo = _mm256_and_si256(bits, mask);
            let hi = _mm256_and_si256(_mm256_srli_epi16::<4>(bits), mask);
            let scale_lo = group_scale(scales, 2*j);
            let scale_hi = group_scale(scales, 2*j+1);
            for lane in 0..N {
                let a = &input[lane][i];
                let p = _mm256_maddubs_epi16(lo, activation(a, 2*j));
                let q = _mm256_maddubs_epi16(hi, activation(a, 2*j+1));
                let left = _mm256_madd_epi16(p, scale_lo);
                let right = _mm256_madd_epi16(q, scale_hi);
                acc[lane] = _mm256_add_epi32(acc[lane], _mm256_add_epi32(left, right));
            }
        }
        for lane in 0..N {
            let scale = _mm256_set1_ps(input[lane][i].scale * d);
            sum[lane] = _mm256_fmadd_ps(scale, _mm256_cvtepi32_ps(acc[lane]), sum[lane]);
        }
    }
    Some(core::array::from_fn(|lane| sum8(sum[lane]) + sum4(min_sum[lane])))
}

#[target_feature(enable="avx2,fma,f16c")]
fn q6<const N: usize>(weights: &[[u8; 210]], input: [&[Q8Block]; N], go: &mut impl FnMut() -> bool) -> Option<[f32; N]> {
    let mask = _mm256_set1_epi8(15);
    let mut sum = [_mm256_setzero_ps(); N];
    for (i, w) in weights.iter().enumerate() {
        if !go() { return None; }
        // Exactly sixteen scale bytes and sixteen i16 sums.
        let scales = unsafe { _mm_loadu_si128(w[192..208].as_ptr().cast()) };
        let scales16 = _mm256_cvtepi8_epi16(scales);
        let d = half(&w[208..]);
        let mut acc = [_mm256_setzero_si256(); N];
        for j in 0..2 {
            let x = load32(w, 64*j);
            let y = load32(w, 64*j+32);
            let h = load32(w, 128+32*j);
            let groups = [
                _mm256_or_si256(_mm256_and_si256(x, mask), _mm256_slli_epi16::<4>(_mm256_and_si256(h, _mm256_set1_epi8(3)))),
                _mm256_or_si256(_mm256_and_si256(y, mask), _mm256_slli_epi16::<2>(_mm256_and_si256(h, _mm256_set1_epi8(12)))),
                _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi16::<4>(x), mask), _mm256_and_si256(h, _mm256_set1_epi8(48))),
                _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi16::<4>(y), mask), _mm256_srli_epi16::<2>(_mm256_and_si256(h, _mm256_set1_epi8(-64)))),
            ];
            for (k, bits) in groups.into_iter().enumerate() {
                let g = 4*j+k;
                let lo = _mm_set1_epi16(i16::from(w[192+2*g] as i8));
                let hi = _mm_set1_epi16(i16::from(w[193+2*g] as i8));
                let scale = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo), hi);
                for lane in 0..N {
                    let prod = _mm256_maddubs_epi16(bits, activation(&input[lane][i], g));
                    acc[lane] = _mm256_add_epi32(acc[lane], _mm256_madd_epi16(prod, scale));
                }
            }
        }
        for lane in 0..N {
            let a = &input[lane][i];
            let sums = unsafe { _mm256_loadu_si256(a.sums.as_ptr().cast()) };
            let correction = _mm256_slli_epi32::<5>(_mm256_madd_epi16(sums, scales16));
            let corrected = _mm256_sub_epi32(acc[lane], correction);
            sum[lane] = _mm256_fmadd_ps(_mm256_set1_ps(a.scale * d), _mm256_cvtepi32_ps(corrected), sum[lane]);
        }
    }
    Some(core::array::from_fn(|lane| sum8(sum[lane])))
}
