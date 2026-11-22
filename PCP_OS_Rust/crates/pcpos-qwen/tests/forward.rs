mod common;
use common::Fixture;
use pcpos_gguf::Index;
use pcpos_qwen::{Model,forward::{Decoder,Error}};
use sha2::{Digest,Sha256};

#[test]
fn progress_counts_only_completed_prompt_tokens_and_retry_preserves_context() {
    use pcpos_qwen::forward::PrefillEvent::{Poll, TokenProcessed};

    for arch in ["qwen2", "qwen3"] {
        let bytes = Fixture::full(arch);
        let index = Index::parse(&bytes, bytes.len() as u64).unwrap();
        let model = Model::admit(&index).unwrap();
        let make = || Decoder::new(&model, &bytes, 8, 1 << 20, || true).unwrap();
        let mut reference = make();
        reference.step(1, || true).unwrap();
        let prompt = [7, 5, 2];
        let mut reference_polls = 0;
        let logits = reference.prefill(&prompt, || { reference_polls += 1; true }).unwrap().to_vec();
        let continuation = reference.step(4, || true).unwrap().to_vec();

        for stop_after in 1..=prompt.len() {
            let mut decoder = make();
            decoder.step(1, || true).unwrap();
            let mut seen = Vec::new();
            let result = decoder.prefill_with_progress(&prompt, |event| match event {
                Poll => true,
                TokenProcessed { completed, total } => {
                    seen.push((completed, total));
                    completed != stop_after
                }
            });
            assert_eq!(result, Err(Error::Cancelled));
            assert_eq!(decoder.position(), 1);
            assert_eq!(seen, (1..=stop_after).map(|n| (n, 3)).collect::<Vec<_>>());

            seen.clear();
            let mut polls = 0;
            let retry = decoder.prefill_with_progress(&prompt, |event| {
                match event {
                    Poll => polls += 1,
                    TokenProcessed { completed, total } => seen.push((completed, total)),
                }
                true
            }).unwrap();
            assert_eq!(retry, logits);
            assert_eq!(seen, [(1, 3), (2, 3), (3, 3)]);
            assert_eq!(polls, reference_polls);
            assert_eq!(decoder.step(4, || true).unwrap(), continuation);
        }
        let mut decoder = make();
        for (input, error) in [(&[][..], Error::Token), (&[0, u32::MAX], Error::Token),
                               (&[1; 9], Error::ContextFull)] {
            assert_eq!(decoder.prefill_with_progress(input, |_| panic!("invalid prompt event")), Err(error));
        }
    }
}

#[test]
fn full_forward_matches_pinned_llama_reference() {
    for (arch,golden) in [("qwen2",include_str!("data/qwen2-forward.txt")),("qwen3",include_str!("data/qwen3-forward.txt"))] {
        let b=Fixture::full(arch);let mut lines=golden.lines();
        assert_eq!(format!("{:x}",Sha256::digest(&b)),lines.next().unwrap());
        let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
        let mut d=Decoder::new(&m,&b,32,1<<20,||true).unwrap();
        for (position,line) in lines.enumerate() {
            let mut fields=line.split_whitespace();let token=fields.next().unwrap().parse().unwrap();
            let expected:Vec<f32>=fields.map(|s|f32::from_bits(u32::from_str_radix(s,16).unwrap())).collect();
            let actual=d.step(token,||true).unwrap();assert_eq!(actual.len(),expected.len());
            for (i,(a,b)) in actual.iter().zip(expected).enumerate() {
                let tolerance=2e-5+2e-5*b.abs();
                assert!((*a-b).abs()<=tolerance,"{arch} pos={position} logit={i}: {a} vs {b}");
            }
        }
    }
}

#[test]
fn cancelled_step_retries_without_changing_prior_context() {
    for arch in ["qwen2","qwen3"] {
        let b=Fixture::full(arch);let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
        let mut baseline=Decoder::new(&m,&b,4,1<<20,||true).unwrap();
        baseline.step(3,||true).unwrap();
        let mut calls=0;let expected=baseline.step(7,||{calls+=1;true}).unwrap().to_vec();
        let next=baseline.step(5,||true).unwrap().to_vec();
        for cut in [0,1,10,100,calls/2,calls-1] {
            let mut d=Decoder::new(&m,&b,4,1<<20,||true).unwrap();d.step(3,||true).unwrap();
            let mut tick=0;assert_eq!(d.step(7,||{tick+=1;tick<=cut}),Err(Error::Cancelled));
            assert_eq!(d.position(),1);
            assert_eq!(d.step(7,||true).unwrap(),expected);
            assert_eq!(d.step(5,||true).unwrap(),next);
        }
        baseline.reset();assert_eq!(baseline.position(),0);
        let reset=baseline.step(7,||true).unwrap().to_vec();
        let mut fresh=Decoder::new(&m,&b,4,1<<20,||true).unwrap();
        assert_eq!(fresh.step(7,||true).unwrap(),reset);
    }
}

#[test]
fn budgets_tokens_context_and_initialization_cancellation() {
    let b=Fixture::full("qwen3");let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
    let need=Decoder::buffer_bytes(&m,1).unwrap();
    let mut calls=0;
    assert!(matches!(Decoder::new(&m,&b,1,need-1,||{calls+=1;true}),Err(Error::Budget)));assert_eq!(calls,0);
    assert!(matches!(Decoder::new(&m,&b[..b.len()-1],1,need,||true),Err(Error::Weights)));
    assert!(matches!(Decoder::new(&m,&b,1,need,||false),Err(Error::Cancelled)));
    let mut d=Decoder::new(&m,&b,1,need,||true).unwrap();
    assert_eq!(d.step(16,||true),Err(Error::Token));assert_eq!(d.position(),0);
    d.step(3,||true).unwrap();assert_eq!(d.step(4,||true),Err(Error::ContextFull));assert_eq!(d.position(),1);
}

#[test]
fn nonfinite_late_weight_does_not_commit_a_position() {
    let mut b=Fixture::full("qwen3");
    let idx=Index::parse(&b,b.len() as u64).unwrap();
    let end=idx.tensors().iter().find(|t|t.name=="output.weight").unwrap();let at=(end.offset+end.bytes-4) as usize;
    drop(idx);b[at..at+4].copy_from_slice(&f32::NAN.to_le_bytes());
    let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
    let mut d=Decoder::new(&m,&b,4,1<<20,||true).unwrap();
    assert_eq!(d.step(3,||true),Err(Error::NonFinite));assert_eq!(d.position(),0);
}

#[test]
fn prefill_matches_step_bits_across_batches_resets_and_full_context() {
    for (arch,unrolled) in [("qwen2",false),("qwen3",false),("qwen2",true),("qwen3",true)] {
        let b=Fixture::full(arch);let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
        let mut slow=Decoder::new(&m,&b,32,1<<20,||true).unwrap();
        let mut fast=if unrolled{Decoder::new_unrolled(&m,&b,32,1<<20,||true)}else{Decoder::new(&m,&b,32,1<<20,||true)}.unwrap();
        let mut seed=739u64;
        for trial in 0..64 {
            slow.reset();fast.reset();let mut remaining=32;
            while remaining>0 {
                seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let count=(1+(seed as usize%7)).min(remaining);
                let ids:Vec<_>=(0..count).map(|_|{seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);(seed>>32) as u32%16}).collect();
                let mut expected=Vec::new();for &id in &ids{expected=slow.step(id,||true).unwrap().to_vec();}
                assert_eq!(fast.prefill(&ids,||true).unwrap(),expected,"{arch} trial {trial}");
                remaining-=count;assert_eq!(fast.position(),32-remaining);
            }
            assert_eq!(fast.prefill(&[1],||true),Err(Error::ContextFull));
        }
    }
}

#[test]
fn prefill_cancellation_edges_and_seeded_positions_roll_back_and_retry() {
    for (arch,unrolled) in [("qwen2",false),("qwen3",false),("qwen2",true),("qwen3",true)] {
        let b=Fixture::full(arch);let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
        let make=||if unrolled{Decoder::new_unrolled(&m,&b,8,1<<20,||true)}else{Decoder::new(&m,&b,8,1<<20,||true)}.unwrap();
        let mut baseline=make();baseline.step(3,||true).unwrap();
        let mut calls=0;let expected=baseline.prefill(&[7,5,2],||{calls+=1;true}).unwrap().to_vec();
        let next=baseline.step(4,||true).unwrap().to_vec();
        let mut d=make();
        let mut cuts=std::collections::BTreeSet::new();
        for cut in 0..128.min(calls){cuts.insert(cut);cuts.insert(calls-1-cut);}
        for i in 0..128{cuts.insert(calls*i/128);}
        let mut seed=917u64;for _ in 0..512{seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);cuts.insert(seed as usize%calls);}
        let count=cuts.len();
        for cut in cuts {
            d.reset();d.step(3,||true).unwrap();let mut tick=0;
            assert_eq!(d.prefill(&[7,5,2],||{tick+=1;tick<=cut}),Err(Error::Cancelled),"{arch} cut {cut}");
            assert_eq!(d.position(),1);assert_eq!(d.prefill(&[7,5,2],||true).unwrap(),expected);
            assert_eq!(d.step(4,||true).unwrap(),next);
        }
        eprintln!("{arch} unrolled={unrolled}: {count} boundary/seeded cuts among {calls} prefill callbacks retried exactly");
    }
}

#[test]
fn prefill_refuses_empty_bad_tokens_capacity_and_late_corruption_atomically() {
    for (arch,unrolled) in [("qwen2",false),("qwen3",false),("qwen2",true),("qwen3",true)] {
        let mut b=Fixture::full(arch);
        {let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
            let mut d=if unrolled{Decoder::new_unrolled(&m,&b,4,1<<20,||true)}else{Decoder::new(&m,&b,4,1<<20,||true)}.unwrap();d.step(3,||true).unwrap();
            for (input,error) in [(vec![],Error::Token),(vec![1,16],Error::Token),(vec![1,2,3,4],Error::ContextFull)] {
                let mut calls=0;assert_eq!(d.prefill(&input,||{calls+=1;true}),Err(error));assert_eq!(calls,0);assert_eq!(d.position(),1);
            }
        }
        let idx=Index::parse(&b,b.len() as u64).unwrap();let t=idx.tensors().iter().find(|t|t.name=="output.weight").unwrap();let at=(t.offset+t.bytes-4) as usize;
        drop(idx);b[at..at+4].copy_from_slice(&f32::INFINITY.to_le_bytes());
        let idx=Index::parse(&b,b.len() as u64).unwrap();let m=Model::admit(&idx).unwrap();
        let mut d=if unrolled{Decoder::new_unrolled(&m,&b,4,1<<20,||true)}else{Decoder::new(&m,&b,4,1<<20,||true)}.unwrap();
        for _ in 0..8 {
            let mut processed = Vec::new();
            let result = d.prefill_with_progress(&[1, 2, 3], |event| {
                if let pcpos_qwen::forward::PrefillEvent::TokenProcessed { completed, total } = event {
                    processed.push((completed, total));
                }
                true
            });
            assert_eq!(result, Err(Error::NonFinite));
            assert_eq!(processed, [(1, 3), (2, 3)]);
            assert_eq!(d.position(), 0);
        }
    }
}

#[test]
#[ignore = "QA input export: set QWEN_FIXTURE_DIR to an existing output directory"]
fn export_reference_models() {
    let p=std::path::PathBuf::from(std::env::var_os("QWEN_FIXTURE_DIR").expect("QWEN_FIXTURE_DIR"));
    assert!(p.is_dir());
    for arch in ["qwen2","qwen3"] {
        let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(p.join(format!("{arch}.gguf"))).unwrap();
        std::io::Write::write_all(&mut f,&Fixture::full(arch)).unwrap();
    }
}

#[test]
fn inference_tiles_preserve_context_progress_and_retries_for_every_tail() {
    use pcpos_qwen::forward::PrefillEvent;
    for arch in ["qwen2", "qwen3"] {
        let bytes=Fixture::full(arch);
        let index=Index::parse(&bytes,bytes.len() as u64).unwrap();
        let model=Model::admit(&index).unwrap();
        let minimum = Decoder::buffer_bytes(&model,16).unwrap();
        assert!(matches!(Decoder::for_inference(&model,&bytes,16,minimum-1,||true), Err(Error::Budget)));
        let mut limited = Decoder::for_inference(&model,&bytes,16,minimum,||true).unwrap();
        let mut ordered = Decoder::new(&model,&bytes,16,minimum,||true).unwrap();
        assert_eq!(limited.prefill(&[3,2,1],||true).unwrap(), ordered.prefill(&[3,2,1],||true).unwrap());
        for len in 1..=14 {
            let prompt: Vec<_> = (0..len).map(|i| [7,5,2,1][i%4]).collect();
            let mut reference=Decoder::new(&model,&bytes,16,1<<20,||true).unwrap();
            reference.step(3,||true).unwrap();
            let expected=reference.prefill(&prompt,||true).unwrap().to_vec();
            let next=reference.step(4,||true).unwrap().to_vec();
            let mut decoder=Decoder::for_inference(&model,&bytes,16,1<<20,||true).unwrap();
            decoder.step(3,||true).unwrap();
            for stop in 1..=len {
                let mut seen=0;
                let result=decoder.prefill_with_progress(&prompt,|event| {
                    if let PrefillEvent::TokenProcessed {completed,total}=event {
                        seen+=1;
                        assert_eq!((completed,total),(seen,len));
                        return completed!=stop;
                    }
                    true
                });
                assert_eq!(result,Err(Error::Cancelled));
                assert_eq!(seen,stop);
                assert_eq!(decoder.position(),1);
            }
            assert_eq!(decoder.prefill(&prompt,||true).unwrap(),expected);
            assert_eq!(decoder.step(4,||true).unwrap(),next);
        }
    }
}
