use crate::{Error, TensorType};
pub use pcpos_model_simd::Q8Block;

/// Supported CPUs can project several tokens through each weight row together.
#[derive(Clone, Copy)]
pub enum PackedKernel {
    #[cfg(all(target_arch="x86_64", target_feature="sse2"))]
    X86(pcpos_model_simd::PackedAvx2),
}

impl PackedKernel {
    pub fn detect() -> Option<Self> {
        #[cfg(all(target_arch="x86_64", target_feature="sse2"))]
        return pcpos_model_simd::PackedAvx2::detect().map(Self::X86);
        #[cfg(not(all(target_arch="x86_64", target_feature="sse2")))]
        None
    }

    /// Input and output contain N token vectors, stored consecutively.
    /// Ok(false) selects floating-point fallback before any output is written.
    pub fn project<const N: usize>(self, kind: TensorType, weights: &[u8], input: &[f32], output: &mut [f32],
        scratch: &mut [Q8Block], go: &mut impl FnMut() -> bool) -> Result<bool, Error> {
        let block_bytes = match kind {
            TensorType::Q4K => 144usize,
            TensorType::Q6K => 210,
            _ => return Ok(false),
        };
        if N == 0 || input.len() % N != 0 || output.len() % N != 0 { return Err(Error::Shape); }
        let columns = input.len() / N;
        let rows = output.len() / N;
        if columns == 0 || columns % 256 != 0 || rows == 0 { return Err(Error::Shape); }
        let blocks = columns / 256;
        let row_bytes = blocks.checked_mul(block_bytes).ok_or(Error::Length)?;
        if row_bytes.checked_mul(rows) != Some(weights.len()) { return Err(Error::Length); }
        let prepared = scratch.get_mut(..input.len()/256).ok_or(Error::Length)?;
        for (src, dst) in input.as_chunks::<256>().0.iter().zip(prepared.iter_mut()) {
            if !go() { return Err(Error::Cancelled); }
            if !dst.pack(src) { return Ok(false); }
        }
        match self {
            #[cfg(all(target_arch="x86_64", target_feature="sse2"))]
            Self::X86(cpu) => {
                let tokens: [&[Q8Block]; N] = core::array::from_fn(|i| &prepared[i*blocks..(i+1)*blocks]);
                for (index, row) in weights.chunks_exact(row_bytes).enumerate() {
                    let values = if kind == TensorType::Q4K {
                        cpu.q4(row.as_chunks::<144>().0, tokens, go)
                    } else {
                        cpu.q6(row.as_chunks::<210>().0, tokens, go)
                    }.ok_or(Error::Cancelled)?;
                    for (i, v) in values.into_iter().enumerate() {
                        // Every stored half scale participates, including for zero activations.
                        if !v.is_finite() { return Err(Error::NonFinite); }
                        output[i*rows + index] = v;
                    }
                }
                Ok(true)
            }
        }
    }
}
