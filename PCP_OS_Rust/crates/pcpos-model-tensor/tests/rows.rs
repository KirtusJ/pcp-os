use pcpos_model_tensor::{Row, Error, TensorType::*};

#[test]
fn selected_k_block_math_preserves_order_and_cancellation() {
    let backend = pcpos_model_tensor::Projection::detect();
    #[cfg(all(target_arch="x86_64", target_feature="sse2"))]
    assert_eq!(matches!(backend, pcpos_model_tensor::Projection::Avx2(_)), std::is_x86_feature_detected!("avx2"));
    let mut seed = 0x396_2026u32;
    let mut next = || { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); seed };
    for (kind, bytes) in [(Q4K,144), (Q6K,210)] {
        // An odd byte offset covers unaligned encoded rows.
        let mut raw = vec![0u8; 1 + 5*bytes];
        for b in &mut raw { *b = (next() >> 24) as u8; }
        for block in raw[1..].chunks_exact_mut(bytes) {
            let at = if kind==Q6K {208} else {0};
            block[at..at+2].copy_from_slice(&0xbc01u16.to_le_bytes());
            if kind==Q4K {block[2..4].copy_from_slice(&0x3421u16.to_le_bytes());}
        }
        let row = Row::new(kind, 1280, &raw[1..]).unwrap();
        for _ in 0..16 {
            let x: Vec<_>=(0..1280).map(|_| f32::from_bits((next() & 0x807f_ffff) | ((65+next()%105)<<23))).collect();
            let mut calls=0;
            assert_eq!(backend.dot(&row,&x,||{calls+=1;true}).unwrap().to_bits(),row.dot(&x,||true).unwrap().to_bits());
            assert_eq!(calls,5);
            for stop in 1..=5 {
                let mut n=0;
                assert_eq!(backend.dot(&row,&x,||{n+=1;n!=stop}),Err(Error::Cancelled));
                assert_eq!(n,stop);
            }
        }
        let mut x=vec![1.0;1280];
        for i in [0,3,4,7,8,15,63,127,255,256,1279] {
            for bad in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY] {
                x[i]=bad;assert_eq!(backend.dot(&row,&x,||true),Err(Error::NonFinite));
            }
            x[i]=1.0;
        }
        assert_eq!(backend.dot(&row,&x[..1279],||true),Err(Error::Length));
    }
}

#[test]
fn upstream_golden_rows_match_every_float_bit() {
    let mut count = 0;
    for line in include_str!("data/ggml-b12387.txt").lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let fields: Vec<_> = line.split_whitespace().collect();
        let kind = match fields[0] { "0" => F32, "1" => F16, "6" => Q5_0, "8" => Q8_0,
            "12" => Q4K, "14" => Q6K, "30" => Bf16, _ => panic!("fixture type") };
        let unhex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2], 16).unwrap()).collect::<Vec<_>>();
        let encoded = unhex(fields[2]);
        let expected = unhex(fields[3]);
        let columns = fields[1].parse().unwrap();
        let row = Row::new(kind, columns, &encoded).unwrap();
        let mut values = vec![f32::NAN; columns];
        row.decode(&mut values).unwrap();
        assert_eq!(values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(), expected, "fixture row {count}");
        let vector: Vec<f32> = (0..columns).map(|i| ((i % 7) as f32 - 3.0) / 4.0).collect();
        let reference = expected.chunks_exact(4).zip(&vector).fold(0.0f64, |sum,(b,&x)| {
            sum + f32::from_le_bytes(b.try_into().unwrap()) as f64 * x as f64
        }) as f32;
        let dot = row.dot(&vector, ||true);
        let selected = pcpos_model_tensor::Projection::detect().dot(&row, &vector, ||true);
        assert_eq!(selected.map(f32::to_bits), dot.map(f32::to_bits), "dispatched fixture row {count}");
        if reference.is_finite() { assert_eq!(dot.unwrap().to_bits(), reference.to_bits()); }
        else { assert_eq!(dot,Err(Error::NonFinite)); }
        count += 1;
    }
    assert_eq!(count, 27);
}

#[test]
fn shapes_and_lengths_refuse_without_writing() {
    assert_eq!(Row::new(Q4K, 255, &[0;144]).unwrap_err(), Error::Shape);
    assert_eq!(Row::new(F32, 0, &[]).unwrap_err(), Error::Shape);
    assert_eq!(Row::new(F32, usize::MAX, &[]).unwrap_err(), Error::Length);
    assert_eq!(Row::new(Q5_0, 32, &[0;21]).unwrap_err(), Error::Length);
    assert_eq!(Row::new(Q6K, 256, &[0;211]).unwrap_err(), Error::Length);
    assert_eq!(Row::new(Q2K, 256, &[0;84]).unwrap_err(), Error::Unsupported);
    let bytes = 1.0f32.to_le_bytes();
    let row = Row::new(F32, 1, &bytes).unwrap();
    let mut output = [123.0,456.0];
    assert_eq!(row.decode(&mut output), Err(Error::Length));
    assert_eq!(output, [123.0,456.0]);
}

#[test]
fn all_floating_fields_are_checked_including_later_blocks() {
    for (kind, size, scales) in [(F16,2,vec![0]), (Bf16,2,vec![0]), (Q5_0,22,vec![0]),
        (Q8_0,34,vec![0]), (Q4K,144,vec![0,2]), (Q6K,210,vec![208])] {
        for scale in scales {
            for bits in [0x7c00u16,0x7e00,0xfc00] {
                let mut bytes = vec![0u8;size*2];
                let bits = if kind == Bf16 { match bits {0x7c00=>0x7f80,0x7e00=>0x7fc0,_=>0xff80} } else {bits};
                bytes[size+scale..size+scale+2].copy_from_slice(&bits.to_le_bytes());
                assert_eq!(Row::new(kind, kind.block_layout().0 as usize * 2, &bytes).unwrap_err(), Error::NonFinite);
            }
        }
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(Row::new(F32,1,&value.to_le_bytes()).unwrap_err(), Error::NonFinite);
    }
}

#[test]
fn dot_uses_bounded_scratch_and_can_cancel_between_quant_blocks() {
    let mut bytes = [0u8;68];
    for start in [0,34] { bytes[start..start+2].copy_from_slice(&0x3c00u16.to_le_bytes()); bytes[start+2..start+34].fill(2); }
    let row = Row::new(Q8_0,64,&bytes).unwrap();
    assert_eq!(row.columns(),64);
    assert_eq!(row.dot(&[3.0;64],||true),Ok(384.0));
    let mut calls=0;
    assert_eq!(row.dot(&[3.0;64],||{calls+=1;calls<2}),Err(Error::Cancelled));
    assert_eq!(calls,2);
    assert_eq!(row.dot(&[3.0;63],||true),Err(Error::Length));
    assert_eq!(row.dot(&[f32::NAN;64],||true),Err(Error::NonFinite));
    let large=f32::MAX.to_le_bytes();
    assert_eq!(Row::new(F32,1,&large).unwrap().dot(&[2.0],||true),Err(Error::NonFinite));
}

#[test]
fn unrolled_dot_matches_reference_with_bounded_rounding_error_and_cancellation() {
    let mut rows=0;let mut seed=491u64;
    for line in include_str!("data/ggml-b12387.txt").lines().filter(|l|!l.starts_with('#')&&!l.is_empty()) {
        let f:Vec<_>=line.split_whitespace().collect();let kind=match f[0]{"0"=>F32,"1"=>F16,"6"=>Q5_0,"8"=>Q8_0,"12"=>Q4K,"14"=>Q6K,"30"=>Bf16,_=>panic!()};
        let raw:Vec<_>=(0..f[2].len()).step_by(2).map(|i|u8::from_str_radix(&f[2][i..i+2],16).unwrap()).collect();
        let width:usize=f[1].parse().unwrap();
        for copies in [1,2,17,257] {
            let bytes=raw.repeat(copies);let row=Row::new(kind,width*copies,&bytes).unwrap();
            let input:Vec<_>=(0..row.columns()).map(|_|{seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);((seed>>40)as i32-(1<<23))as f32/(1<<20)as f32}).collect();
            let scalar=row.dot(&input,||true);let fast=row.dot_unrolled(&input,||true);
            match (scalar,fast){(Ok(a),Ok(b))=>assert!((a-b).abs()<=2e-5+2e-6*a.abs(),"{kind:?} {copies}: {a} {b}"),(Err(a),Err(b))=>assert_eq!(a,b),other=>panic!("{other:?}")}
            let mut count=0;let _=row.dot_unrolled(&input,||{count+=1;true});
            // Every boundary on small rows; distributed, first and final on large rows.
            let mut cuts=std::collections::BTreeSet::from([0,count/2,count-1]);
            if count<=1024{cuts.extend(0..count);}else{cuts.extend((0..64).map(|i|count*i/64));}
            for cut in cuts {let mut n=0;assert_eq!(row.dot_unrolled(&input,||{n+=1;n<=cut}),Err(Error::Cancelled));assert_eq!(row.dot_unrolled(&input,||true),fast);}
            let mut bad=input.clone();*bad.last_mut().unwrap()=f32::NAN;assert_eq!(row.dot_unrolled(&bad,||true),Err(Error::NonFinite));
            assert_eq!(row.dot_unrolled(&input[..input.len()-1],||true),Err(Error::Length));rows+=1;
        }
    }
    assert_eq!(rows,108);
    let bytes=f32::MAX.to_le_bytes();assert_eq!(Row::new(F32,1,&bytes).unwrap().dot_unrolled(&[2.0],||true),Err(Error::NonFinite));
}

#[test]
fn reordered_accumulation_is_explicit_for_ill_conditioned_inputs() {
    let mut bytes=[1u8;34];bytes[..2].copy_from_slice(&0x3c00u16.to_le_bytes());
    let row=Row::new(Q8_0,32,&bytes).unwrap();let mut input=[0.0;32];input[..4].copy_from_slice(&[1e30,1.0,-1e30,1.0]);
    // Both are finite floating-point reductions, not exact-real arithmetic.
    // Keep the sequential API and expose this difference instead of asserting
    // universal bit parity from ordinary model-vector measurements.
    assert_eq!(row.dot(&input,||true),Ok(1.0));
    assert_eq!(row.dot_unrolled(&input,||true),Ok(0.0));
}
