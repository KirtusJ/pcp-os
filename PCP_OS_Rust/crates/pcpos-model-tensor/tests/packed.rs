use pcpos_model_tensor::{packed::{PackedKernel, Q8Block}, Error, TensorType};

fn bytes(kind: TensorType, count: usize, mut seed: u32) -> Vec<u8> {
    let size = kind.block_layout().1 as usize;
    let mut v = vec![0; count * size];
    for b in &mut v {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (seed >> 24) as u8;
    }
    let scales = [0x3c00_u16, 0xb860, 0x0800, 0x32aa, 0xa311, 0x0010];
    for (i, b) in v.chunks_exact_mut(size).enumerate() {
        let d = scales[i % scales.len()].to_le_bytes();
        if kind == TensorType::Q4K {
            b[..2].copy_from_slice(&d);
            b[2..4].copy_from_slice(&scales[(i+1) % scales.len()].to_le_bytes());
        } else {
            b[208..210].copy_from_slice(&d);
        }
    }
    v
}

fn digest(blocks: &[Q8Block]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for block in blocks {
        let encoded = block.scale.to_le_bytes().into_iter()
            .chain(block.values.iter().map(|&x| x as u8))
            .chain(block.sums.iter().flat_map(|v| v.to_le_bytes()));
        for byte in encoded { hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3); }
    }
    hash
}

#[test]
fn pinned_ggml_activation_bytes_and_integer_dot_products() {
    let kernel = PackedKernel::detect();
    for line in include_str!("data/ggml-packed.txt").lines().filter(|s| !s.starts_with('#')) {
        let f: Vec<_> = line.split_whitespace().collect();
        let kind = match f[0] { "12" => TensorType::Q4K, "14" => TensorType::Q6K, _ => panic!() };
        let n: usize = f[1].parse().unwrap();
        let input: Vec<_> = (0..n*256).map(|j| {
            if j/256 % 4 == 0 {0.0} else {((j*73%257) as f32-128.0)/16.0}
        }).collect();
        let weights = bytes(kind, n, f[2].parse().unwrap());
        let mut scratch = vec![Q8Block::ZERO; n];
        for (b, v) in scratch.iter_mut().zip(input.as_chunks::<256>().0) { assert!(b.pack(v)); }
        assert_eq!(digest(&scratch), u64::from_str_radix(f[3], 16).unwrap());
        if let Some(k) = kernel {
            let mut out = [0.0];
            assert_eq!(k.project::<1>(kind, &weights, &input, &mut out, &mut scratch, &mut || true), Ok(true));
            assert_eq!(out[0].to_bits(), u32::from_str_radix(f[4],16).unwrap(), "{line}");
        }
    }
}

#[test]
fn zero_reuse_corrupt_scales_cancellation_and_fallback() {
    let mut scratch = vec![Q8Block::ZERO; 2];
    assert!(scratch[0].pack(&[2.0; 256]));
    assert!(scratch[0].pack(&[0.0; 256]));
    assert_eq!(scratch[0].sums, [0; 16]);
    let Some(kernel) = PackedKernel::detect() else { return; };
    let mut out = [123.0; 2];
    for kind in [TensorType::Q4K, TensorType::Q6K] {
        let mut w = bytes(kind, 4, 713);
        for cut in 0..6 {
            let mut polls=0;
            assert_eq!(kernel.project::<1>(kind,&w,&[1.0;512],&mut out,&mut scratch,
                &mut || { polls+=1; polls<=cut }),Err(Error::Cancelled));
        }
        assert_eq!(kernel.project::<1>(kind,&w,&[1.0;512],&mut out,&mut scratch,&mut ||true),Ok(true));
        out=[123.0;2];
        for value in [f32::NAN, f32::INFINITY, f32::from_bits(1)] {
            assert_eq!(kernel.project::<1>(kind,&w,&[value;512],&mut out,&mut scratch,&mut ||true),Ok(false));
            assert_eq!(out,[123.0;2]);
        }
        let offset = if kind == TensorType::Q4K {0} else {208};
        w[offset..offset+2].copy_from_slice(&0x7e00_u16.to_le_bytes());
        assert_eq!(kernel.project::<1>(kind,&w,&[0.0;512],&mut out,&mut scratch,&mut ||true),Err(Error::NonFinite));
        assert_eq!(kernel.project::<1>(kind,&w[1..],&[0.0;512],&mut out,&mut scratch,&mut ||true),Err(Error::Length));
    }
}


#[test]
fn tiles_match_single_token_projections_and_retry_at_every_boundary() {
    let Some(kernel) = PackedKernel::detect() else { return; };
    fn verify<const N: usize>(kernel: PackedKernel) {
        const COLS: usize = 768;
        const ROWS: usize = 5;
        let input: Vec<_> = (0..COLS*N).map(|j| ((j*97%449) as f32-224.0)/37.0).collect();
        for kind in [TensorType::Q4K, TensorType::Q6K] {
            let weights = bytes(kind, COLS/256*ROWS, 73901);
            let mut reference = vec![0.0; ROWS*N];
            let mut scratch = vec![Q8Block::ZERO; N*COLS/256];
            for (src, dst) in input.chunks_exact(COLS).zip(reference.chunks_exact_mut(ROWS)) {
                assert!(kernel.project::<1>(kind, &weights, src, dst, &mut scratch, &mut ||true).unwrap());
            }
            let mut output = vec![0.0; ROWS*N];
            let mut polls = 0;
            assert!(kernel.project::<N>(kind, &weights, &input, &mut output, &mut scratch, &mut || { polls+=1; true }).unwrap());
            assert_eq!(output, reference);
            for stop in 1..=polls {
                let mut calls = 0;
                assert_eq!(kernel.project::<N>(kind, &weights, &input, &mut output, &mut scratch, &mut || {
                    calls+=1; calls!=stop
                }), Err(Error::Cancelled));
                assert!(kernel.project::<N>(kind, &weights, &input, &mut output, &mut scratch, &mut || true).unwrap());
                assert_eq!(output, reference);
            }
        }
    }
    verify::<2>(kernel);
    verify::<3>(kernel);
    verify::<4>(kernel);
}
