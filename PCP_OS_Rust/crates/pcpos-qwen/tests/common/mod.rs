#![allow(dead_code)]
use pcpos_gguf::Index;
use pcpos_qwen::{Error,Model};
#[derive(Clone)]
pub enum V { U(u32), F(f32), S(String), Tokens, Merges }
pub struct Fixture {
    pub meta: Vec<(String,V)>,
    pub tensors: Vec<(String,Vec<u64>,u32)>,
}
impl Fixture {
    pub fn new(arch: &str) -> Self {
        let mut meta=vec![("general.architecture".into(),V::S(arch.into())),
            ("general.type".into(),V::S("model".into())),
            ("tokenizer.ggml.tokens".into(),V::Tokens)];
        for (k,v) in [("block_count",2),("embedding_length",32),("feed_forward_length",64),
            ("attention.head_count",4),("attention.head_count_kv",2),("context_length",1024)] {
            meta.push((format!("{arch}.{k}"),V::U(v)));
        }
        meta.push((format!("{arch}.rope.freq_base"),V::F(1e6)));
        meta.push((format!("{arch}.attention.layer_norm_rms_epsilon"),V::F(1e-6)));
        let mut tensors=vec![("token_embd.weight".into(),vec![32,16],0),
            ("output.weight".into(),vec![32,16],0),("output_norm.weight".into(),vec![32],0)];
        for i in 0..2 {
            for (k,d) in [("attn_norm.weight",vec![32]),("attn_q.weight",vec![32,32]),
                ("attn_k.weight",vec![32,16]),("attn_v.weight",vec![32,16]),
                ("attn_output.weight",vec![32,32]),("ffn_norm.weight",vec![32]),
                ("ffn_gate.weight",vec![32,64]),("ffn_up.weight",vec![32,64]),("ffn_down.weight",vec![64,32])] {
                tensors.push((format!("blk.{i}.{k}"),d,0));
            }
            let extras=if arch=="qwen2" {vec![("attn_q.bias",32),("attn_k.bias",16),("attn_v.bias",16)]}
                else {vec![("attn_q_norm.weight",8),("attn_k_norm.weight",8)]};
            for (k,n) in extras { tensors.push((format!("blk.{i}.{k}"),vec![n],0)); }
        }
        Self{meta,tensors}
    }
    pub fn set(&mut self,k:&str,v:V) { if let Some(e)=self.meta.iter_mut().find(|e|e.0==k){e.1=v}else{self.meta.push((k.into(),v));} }
    pub fn bytes(&self)->(Vec<u8>,u64) {
        fn text(b:&mut Vec<u8>,s:&str){b.extend((s.len() as u64).to_le_bytes());b.extend(s.as_bytes());}
        let mut b=b"GGUF".to_vec(); b.extend(3u32.to_le_bytes());
        b.extend((self.tensors.len() as u64).to_le_bytes());b.extend((self.meta.len() as u64).to_le_bytes());
        for (key,v) in &self.meta {
            text(&mut b,key);
            match v {
                V::U(n)=>{b.extend(4u32.to_le_bytes());b.extend(n.to_le_bytes());},
                V::F(n)=>{b.extend(6u32.to_le_bytes());b.extend(n.to_le_bytes());},
                V::S(s)=>{b.extend(8u32.to_le_bytes());text(&mut b,s);},
                V::Tokens=>{b.extend(9u32.to_le_bytes());b.extend(8u32.to_le_bytes());b.extend(16u64.to_le_bytes());for i in 0..16 {text(&mut b,&format!("t{i}"));}},
                V::Merges=>{b.extend(9u32.to_le_bytes());b.extend(8u32.to_le_bytes());b.extend(0u64.to_le_bytes());},
            }
        }
        let mut offset=0u64;
        for (name,dims,kind) in &self.tensors {
            text(&mut b,name);b.extend((dims.len() as u32).to_le_bytes());
            for d in dims {b.extend(d.to_le_bytes());}
            b.extend(kind.to_le_bytes());b.extend(offset.to_le_bytes());
            let n:u64=dims.iter().product();
            let size=match kind {0=>n*4,1|30=>n*2,2=>n/32*18,6=>n/32*22,8=>n/32*34,12=>n/256*144,14=>n/256*210,_=>panic!()};
            offset=(offset+size).div_ceil(32)*32;
        }
        b.resize(b.len().div_ceil(32)*32,0);
        let len=b.len() as u64+offset;(b,len)
    }
    pub fn error(&self)->Error {
        let (b,n)=self.bytes();let idx=Index::parse(&b,n).unwrap();Model::admit(&idx).unwrap_err()
    }
    pub fn full(arch:&str)->Vec<u8> {
        Self::full_context(arch,1024)
    }
    pub fn full_context(arch:&str,context:u32)->Vec<u8> {
        let mut f=Self::new(arch);
        f.set(&format!("{arch}.context_length"),V::U(context));
        f.set("tokenizer.ggml.model",V::S("gpt2".into()));
        f.set("tokenizer.ggml.pre",V::S("qwen2".into()));
        f.set("tokenizer.ggml.merges",V::Merges);
        f.set("tokenizer.ggml.bos_token_id",V::U(1));
        f.set("tokenizer.ggml.eos_token_id",V::U(2));
        let(mut b,n)=f.bytes();
        let prefix=b.clone();let idx=Index::parse(&prefix,n).unwrap();
        let tensors=idx.tensors().to_vec();drop(idx);
        b.resize(n as usize,0);
        for t in tensors {
            let seed=t.name.bytes().fold(2166136261u32,|s,v|(s^(v as u32)).wrapping_mul(16777619));
            for j in 0..t.bytes as usize/4 {
                let bits=seed.wrapping_add((j as u32).wrapping_mul(2654435761));
                let centered=((bits>>20) as i32-2048) as f32/2048.0;
                let value=if t.name.contains("norm") {1.0+centered*0.1}
                    else if t.name.ends_with("bias") {centered*0.02} else {centered*0.125};
                let at=t.offset as usize+j*4;b[at..at+4].copy_from_slice(&value.to_le_bytes());
            }
        }
        b
    }
}
