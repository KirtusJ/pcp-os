mod common;
use common::Fixture;
use pcpos_gguf::Index;
use pcpos_qwen::{Model,forward::{Decoder,Error}};
use sha2::{Digest,Sha256};

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
#[ignore = "QA input export: set QWEN_FIXTURE_DIR to an existing output directory"]
fn export_reference_models() {
    let p=std::path::PathBuf::from(std::env::var_os("QWEN_FIXTURE_DIR").expect("QWEN_FIXTURE_DIR"));
    assert!(p.is_dir());
    for arch in ["qwen2","qwen3"] {
        let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(p.join(format!("{arch}.gguf"))).unwrap();
        std::io::Write::write_all(&mut f,&Fixture::full(arch)).unwrap();
    }
}
