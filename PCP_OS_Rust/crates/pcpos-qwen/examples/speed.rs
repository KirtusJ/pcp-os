//! Host-only paired throughput/numerical diagnostic. No native or release claim.
use std::{fs,io::Read,time::Instant,hint::black_box};
use pcpos_gguf::Index;
use pcpos_model_tensor::Row;
use pcpos_qwen::{Model,forward::Decoder};
use sha2::{Digest,Sha256};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=std::env::args().skip(1).collect();
    if args.len()!=4{return Err("speed MODEL NEW_DIR TOKENS_CSV MODE(step|prefill|unrolled-step|unrolled-prefill|rows)".into());}
    let f=fs::File::open(&args[0])?;let n=f.metadata()?.len();
    if n!=491400064{return Err("host helper accepts only the pinned 0.5B reference".into());}
    let mut b=Vec::new();b.try_reserve_exact(n as usize)?;f.take(n+1).read_to_end(&mut b)?;
    if b.len() as u64!=n||format!("{:x}",Sha256::digest(&b))!="1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32"{return Err("weights hash/length".into());}
    let idx=Index::parse(&b,n).map_err(|e|format!("index {e:?}"))?;let m=Model::admit(&idx).map_err(|e|format!("model {e:?}"))?;
    let ids:Vec<u32>=args[2].split(',').map(str::parse).collect::<Result<_,_>>()?;
    if ids.is_empty()||ids.len()>128{return Err("1..128 prompt tokens".into());}
    let out=std::path::Path::new(&args[1]);fs::create_dir(out)?;
    if args[3]=="rows" {
        let mut report=String::from("name,kind,columns,rows,repeats,seconds,checksum\n");
        for name in ["blk.0.attn_q.weight","blk.0.ffn_gate.weight","blk.0.ffn_down.weight","output.weight"]{
            let t=idx.tensors().iter().find(|t|t.name==name).ok_or("tensor")?;
            let (columns,rows)=(t.dimensions()[0] as usize,t.dimensions()[1] as usize);
            let each=t.bytes as usize/rows;let weights=&b[t.offset as usize..(t.offset+t.bytes) as usize];
            let input:Vec<f32>=(0..columns).map(|i|((i%97)as f32-48.0)/50.0).collect();
            let mut checksum=0f64;let start=Instant::now();
            for _ in 0..4{for bytes in weights.chunks_exact(each){let row=Row::new(t.kind,columns,bytes).map_err(|e|format!("row {e:?}"))?;
                checksum+=row.dot_unrolled(black_box(&input),||true).map_err(|e|format!("dot {e:?}"))? as f64;}}
            report+=&format!("{name},{:?},{columns},{rows},4,{:.9},{checksum:.9}\n",t.kind,start.elapsed().as_secs_f64());
        }
        fs::write(out.join("rows.csv"),&report)?;print!("{report}");return Ok(());
    }
    let mut d=if args[3].starts_with("unrolled-"){Decoder::new_unrolled(&m,&b,ids.len()+9,64<<20,||true)}
        else{Decoder::new(&m,&b,ids.len()+9,64<<20,||true)}.map_err(|e|format!("decoder {e:?}"))?;
    let start=Instant::now();let mut next=Vec::new();let mut calls=0u64;
    match args[3].as_str(){
        "step"|"unrolled-step"=>for &id in &ids{next=d.step(id,||{calls+=1;true}).map_err(|e|format!("step {e:?}"))?.to_vec();},
        "prefill"|"unrolled-prefill"=>next=d.prefill(&ids,||{calls+=1;true}).map_err(|e|format!("prefill {e:?}"))?.to_vec(),
        _=>return Err("invalid mode".into()),
    }
    let prefill=start.elapsed().as_secs_f64();let mut emitted=Vec::new();let mut values=Vec::new();
    for _ in 0..8 {
        values.extend(next.iter().flat_map(|x|x.to_le_bytes()));
        let top=next.iter().enumerate().max_by(|a,b|a.1.total_cmp(b.1)).unwrap().0 as u32;emitted.push(top);
        next=d.step(top,||{calls+=1;true}).map_err(|e|format!("decode {e:?}"))?.to_vec();
    }
    let total=start.elapsed().as_secs_f64();fs::write(out.join("logits.f32"),&values)?;
    let report=format!("scope=host-paired-uncontrolled-no-native\nmode={}\nprompt_tokens={}\nsteps=8\nprefill_seconds={prefill:.9}\ndecode_seconds={:.9}\ntotal_seconds={total:.9}\ncallbacks={calls}\nids={emitted:?}\nlogits_sha256={:x}\n",args[3],ids.len(),total-prefill,Sha256::digest(&values));
    fs::write(out.join("result.txt"),&report)?;print!("{report}");Ok(())
}
