//! Bounded weight-row decoding and matrix projections for verified model bytes.
//! Quantized layouts follow ggml a956a852c9c8bc1fb07206364838ddef299178c0;
//! see THIRD_PARTY_NOTICES. Bytes must come from the caller's verified immutable model.
#![no_std]
#![forbid(unsafe_code)]

pub use pcpos_gguf::TensorType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Unsupported, Shape, Length, NonFinite, Cancelled }

#[derive(Clone, Copy, Debug)]
pub enum Projection {
    Scalar,
    Unrolled,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    Avx2(pcpos_model_simd::Avx2),
}

impl Projection {
    /// Select once per decoder. Unavailable CPU or operating-system state uses scalar math.
    pub fn detect() -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        if let Some(cpu) = pcpos_model_simd::Avx2::detect() { return Self::Avx2(cpu); }
        Self::Scalar
    }

    /// Backend used for the release model's Q4_K and Q6_K projections.
    pub fn name(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Unrolled => "unrolled",
            #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
            Self::Avx2(_) => "AVX2",
        }
    }

    pub fn dot(self, row: &Row<'_>, input: &[f32], keep_going: impl FnMut() -> bool) -> Result<f32, Error> {
        match self {
            Self::Unrolled => row.dot_unrolled(input, keep_going),
            #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
            Self::Avx2(cpu) if matches!(row.kind, TensorType::Q4K | TensorType::Q6K) =>
                row.dot_avx2(cpu, input, keep_going),
            _ => row.dot(input, keep_going),
        }
    }
}

/// One exact encoded row. Construction validates geometry and every floating scale.
/// This does not authenticate bytes or check a model's tensor names/architecture.
#[derive(Debug)]
pub struct Row<'a> {
    kind: TensorType,
    bytes: &'a [u8],
    columns: usize,
    block_values: usize,
    block_bytes: usize,
}

impl<'a> Row<'a> {
    pub fn new(kind: TensorType, columns: usize, bytes: &'a [u8]) -> Result<Self, Error> {
        use TensorType::*;
        if !matches!(kind, F32 | F16 | Bf16 | Q5_0 | Q8_0 | Q4K | Q6K) {
            return Err(Error::Unsupported);
        }
        let (values, size) = kind.block_layout();
        let (values, size) = (values as usize, size as usize);
        if columns == 0 || columns % values != 0 { return Err(Error::Shape); }
        let wanted = (columns / values).checked_mul(size).ok_or(Error::Length)?;
        if bytes.len() != wanted { return Err(Error::Length); }
        // Validate the whole row before decode can modify any caller output. Every other
        // stored field is a bounded integer; finite half scales cannot overflow its decode.
        for block in bytes.chunks_exact(size) {
            let finite = match kind {
                F32 => float(block).is_finite(),
                Bf16 => f32::from_bits((word(block) as u32) << 16).is_finite(),
                Q6K => half(&block[208..]).is_finite(),
                Q4K => half(block).is_finite() && half(&block[2..]).is_finite(),
                _ => half(block).is_finite(),
            };
            if !finite { return Err(Error::NonFinite); }
        }
        Ok(Self { kind, bytes, columns, block_values: values, block_bytes: size })
    }

    pub fn columns(&self) -> usize { self.columns }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn dot_avx2(&self, cpu: pcpos_model_simd::Avx2, input: &[f32], mut keep_going: impl FnMut() -> bool) -> Result<f32, Error> {
        if input.len() != self.columns { return Err(Error::Length); }
        let mut sum = 0.0;
        for (block, vector) in self.bytes.chunks_exact(self.block_bytes).zip(input.chunks_exact(256)) {
            if !keep_going() { return Err(Error::Cancelled); }
            let vector = vector.try_into().map_err(|_| Error::Length)?;
            sum = match self.kind {
                TensorType::Q4K => {
                    let d = half(block);
                    let minimum = half(&block[2..]);
                    let scales = core::array::from_fn(|i| {
                        let (s, m) = scale_min(&block[4..16], i);
                        (d*s, minimum*m)
                    });
                    cpu.q4(block.try_into().map_err(|_| Error::Length)?, vector, &scales, sum)
                }
                TensorType::Q6K => cpu.q6(block.try_into().map_err(|_| Error::Length)?, vector, half(&block[208..]), sum),
                _ => return Err(Error::Unsupported),
            }.ok_or(Error::NonFinite)?;
        }
        let value = sum as f32;
        if value.is_finite() { Ok(value) } else { Err(Error::NonFinite) }
    }

    /// Decode into an exactly sized output. A wrong length leaves output untouched.
    pub fn decode(&self, output: &mut [f32]) -> Result<(), Error> {
        if output.len() != self.columns { return Err(Error::Length); }
        for (block, out) in self.bytes.chunks_exact(self.block_bytes)
            .zip(output.chunks_exact_mut(self.block_values)) {
            decode(self.kind, block, out);
        }
        Ok(())
    }

    /// Scalar reference dot product, with f64 accumulation and a finite f32 result.
    /// Checks cancellation between blocks (at most 256 weights). The callback must
    /// itself be bounded; this makes no kernel scheduling or Stop latency guarantee.
    /// No decoded model-sized copy is retained: the scratch row is 1 KiB on the stack.
    pub fn dot(&self, input: &[f32], mut keep_going: impl FnMut() -> bool) -> Result<f32, Error> {
        if input.len() != self.columns { return Err(Error::Length); }
        let mut scratch = [0.0f32; 256];
        let mut sum = 0.0f64;
        for (block, vector) in self.bytes.chunks_exact(self.block_bytes)
            .zip(input.chunks_exact(self.block_values)) {
            if !keep_going() { return Err(Error::Cancelled); }
            decode(self.kind, block, &mut scratch[..self.block_values]);
            for (&weight, &value) in scratch.iter().zip(vector) {
                if !value.is_finite() { return Err(Error::NonFinite); }
                sum += weight as f64 * value as f64;
            }
        }
        let result = sum as f32;
        if result.is_finite() { Ok(result) } else { Err(Error::NonFinite) }
    }

    /// Four independent f64 accumulators expose parallel arithmetic to the
    /// compiler. Same validated rows, bounded scratch and cancellation cadence
    /// as `dot`; summation order differs. Ill-conditioned cancellation can produce
    /// larger differences, so model-level numerical qualification is required.
    /// `dot` remains the sequential numerical reference.
    pub fn dot_unrolled(&self, input: &[f32], mut keep_going: impl FnMut() -> bool) -> Result<f32, Error> {
        if input.len() != self.columns { return Err(Error::Length); }
        let mut scratch=[0.0f32;256];let mut sums=[0.0f64;4];
        for (block, vector) in self.bytes.chunks_exact(self.block_bytes).zip(input.chunks_exact(self.block_values)) {
            if !keep_going(){return Err(Error::Cancelled);}
            decode(self.kind,block,&mut scratch[..self.block_values]);
            let mut at=0;
            while at+4<=vector.len() {
                let v=&vector[at..at+4];let w=&scratch[at..at+4];
                if !(v[0].is_finite() & v[1].is_finite() & v[2].is_finite() & v[3].is_finite()){return Err(Error::NonFinite);}
                sums[0]+=w[0] as f64*v[0] as f64;sums[1]+=w[1] as f64*v[1] as f64;
                sums[2]+=w[2] as f64*v[2] as f64;sums[3]+=w[3] as f64*v[3] as f64;at+=4;
            }
            for (&w,&v) in scratch[at..self.block_values].iter().zip(&vector[at..]) {
                if !v.is_finite(){return Err(Error::NonFinite);}sums[0]+=w as f64*v as f64;
            }
        }
        let result=((sums[0]+sums[1])+(sums[2]+sums[3])) as f32;
        if result.is_finite(){Ok(result)}else{Err(Error::NonFinite)}
    }
}

fn word(b: &[u8]) -> u16 { u16::from_le_bytes([b[0], b[1]]) }
fn float(b: &[u8]) -> f32 { f32::from_le_bytes([b[0], b[1], b[2], b[3]]) }
fn half(b: &[u8]) -> f32 {
    let bits = word(b) as u32;
    let sign = (bits & 0x8000) << 16;
    let exp = (bits >> 10) & 31;
    let fraction = bits & 1023;
    let magnitude = match exp {
        0 if fraction == 0 => 0,
        0 => {
            let shift = fraction.leading_zeros() - 21;
            ((113 - shift) << 23) | (((fraction << shift) & 1023) << 13)
        }
        31 => 0x7f80_0000 | (fraction << 13),
        _ => ((exp + 112) << 23) | (fraction << 13),
    };
    f32::from_bits(sign | magnitude)
}

fn scale_min(scales: &[u8], j: usize) -> (f32, f32) {
    let (scale, min) = if j < 4 {
        (scales[j] & 63, scales[j + 4] & 63)
    } else {
        ((scales[j + 4] & 15) | ((scales[j - 4] >> 6) << 4),
         (scales[j + 4] >> 4) | ((scales[j] >> 6) << 4))
    };
    (scale as f32, min as f32)
}

fn decode(kind: TensorType, b: &[u8], y: &mut [f32]) {
    use TensorType::*;
    match kind {
        F32 => y[0] = float(b),
        F16 => y[0] = half(b),
        Bf16 => y[0] = f32::from_bits((word(b) as u32) << 16),
        Q5_0 => {
            let d = half(b);
            let high = u32::from_le_bytes(b[2..6].try_into().unwrap());
            for j in 0..16 {
                let lo = (b[6 + j] & 15) as i32 | (((high >> j) & 1) << 4) as i32;
                let hi = (b[6 + j] >> 4) as i32 | (((high >> (j + 16)) & 1) << 4) as i32;
                y[j] = (lo - 16) as f32 * d;
                y[j + 16] = (hi - 16) as f32 * d;
            }
        }
        Q8_0 => {
            let d = half(b);
            for j in 0..32 { y[j] = b[2 + j] as i8 as f32 * d; }
        }
        Q4K => {
            let (d, min) = (half(b), half(&b[2..]));
            for group in 0..4 {
                let (s1, m1) = scale_min(&b[4..16], group * 2);
                let (s2, m2) = scale_min(&b[4..16], group * 2 + 1);
                let (d1, d2, m1, m2) = (d * s1, d * s2, min * m1, min * m2);
                for j in 0..32 {
                    let q = b[16 + group * 32 + j];
                    y[group * 64 + j] = d1 * (q & 15) as f32 - m1;
                    y[group * 64 + j + 32] = d2 * (q >> 4) as f32 - m2;
                }
            }
        }
        Q6K => {
            let d = half(&b[208..]);
            for group in 0..2 {
                for j in 0..32 {
                    let lo1 = b[group * 64 + j];
                    let lo2 = b[group * 64 + j + 32];
                    let high = b[128 + group * 32 + j];
                    let quants = [
                        (lo1 & 15) | ((high & 3) << 4),
                        (lo2 & 15) | (((high >> 2) & 3) << 4),
                        (lo1 >> 4) | (((high >> 4) & 3) << 4),
                        (lo2 >> 4) | (((high >> 6) & 3) << 4),
                    ];
                    for k in 0..4 {
                        let sc = b[192 + group * 8 + j / 16 + k * 2] as i8;
                        y[group * 128 + j + k * 32] = d * sc as f32 * (quants[k] as i32 - 32) as f32;
                    }
                }
            }
        }
        _ => unreachable!(), // Row construction rejected all other kinds.
    }
}
