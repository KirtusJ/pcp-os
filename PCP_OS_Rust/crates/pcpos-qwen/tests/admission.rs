use pcpos_gguf::Index;
use pcpos_qwen::{Architecture, Error, Model};

#[derive(Clone)]
enum V { U(u32), F(f32), S(String), Tokens }
struct Fixture {
    meta: Vec<(String,V)>,
    tensors: Vec<(String,Vec<u64>,u32)>,
}
impl Fixture {
    fn new(arch: &str) -> Self {
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
    fn set(&mut self,k:&str,v:V) { if let Some(e)=self.meta.iter_mut().find(|e|e.0==k){e.1=v}else{self.meta.push((k.into(),v));} }
    fn bytes(&self)->(Vec<u8>,u64) {
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
    fn error(&self)->Error {
        let (b,n)=self.bytes();let idx=Index::parse(&b,n).unwrap();Model::admit(&idx).unwrap_err()
    }
}

#[test]
fn both_architectures_tied_output_and_exact_kv_budget() {
    for arch in ["qwen2","qwen3"] {
        let mut f=Fixture::new(arch);
        for tied in [false,true] {
            if tied {f.tensors.retain(|t|t.0!="output.weight");}
            // Directory order must not supply layer ordering.
            f.tensors.reverse();
            let(b,n)=f.bytes();let idx=Index::parse(&b,n).unwrap();let m=Model::admit(&idx).unwrap();
            assert_eq!(m.config().architecture,if arch=="qwen2"{Architecture::Qwen2}else{Architecture::Qwen3});
            assert_eq!(m.kv_bytes(1024,262144),Ok(262144));
            assert_eq!(m.kv_bytes(1024,262143),Err(Error::Budget));
            assert_eq!(m.kv_bytes(0,u64::MAX),Err(Error::Context));
            assert_eq!(m.kv_bytes(1025,u64::MAX),Err(Error::Context));
            assert_eq!(core::ptr::eq(m.output(),m.embedding()),tied);
            assert!(m.layer(2).is_none()); assert!(m.layer(usize::MAX).is_none());
            assert_eq!(m.layer(1).unwrap().query.name,"blk.1.attn_q.weight");
        }
    }
}

#[test]
fn unknown_semantics_and_metadata_do_not_become_plain_qwen() {
    for (key,value,error) in [
        ("general.architecture",V::S("qwen3moe".into()),Error::Architecture),
        ("general.type",V::S("adapter".into()),Error::Architecture),
        ("qwen3.rope.scaling.factor",V::F(2.0),Error::UnsupportedMetadata),
        ("qwen3.attention.sliding_window",V::U(256),Error::UnsupportedMetadata),
        ("qwen3.expert_count",V::U(8),Error::UnsupportedMetadata),
        ("qwen2.embedding_length",V::U(32),Error::UnsupportedMetadata),
        ("split.count",V::U(2),Error::UnsupportedMetadata),
        ("qwen3.block_count",V::U(65),Error::Metadata),
        ("qwen3.block_count",V::U(u32::MAX),Error::Metadata),
        ("qwen3.block_count",V::U(0),Error::Metadata),
        ("qwen3.embedding_length",V::S("32".into()),Error::Metadata),
        ("qwen3.rope.freq_base",V::F(f32::NAN),Error::Metadata),
        ("qwen3.attention.layer_norm_rms_epsilon",V::F(0.0),Error::Metadata),
        ("qwen3.attention.head_count_kv",V::U(3),Error::Geometry),
        ("qwen3.attention.key_length",V::U(3),Error::Geometry),
        ("qwen3.attention.value_length",V::U(4),Error::Geometry),
        ("qwen3.rope.dimension_count",V::U(4),Error::Geometry),
    ] {
        let mut f=Fixture::new("qwen3");f.set(key,value);assert_eq!(f.error(),error,"{key}");
    }
}

#[test]
fn missing_extra_transposed_wrong_rank_and_unsupported_tensors_refuse() {
    let mut f=Fixture::new("qwen3");f.tensors.retain(|t|t.0!="blk.1.attn_k_norm.weight");assert_eq!(f.error(),Error::TensorSet);
    let mut f=Fixture::new("qwen3");f.tensors.push(("vision.weight".into(),vec![32,32],0));assert_eq!(f.error(),Error::TensorSet);
    let mut f=Fixture::new("qwen3");f.tensors.iter_mut().find(|t|t.0=="blk.0.ffn_down.weight").unwrap().1.reverse();assert_eq!(f.error(),Error::TensorShape);
    let mut f=Fixture::new("qwen3");f.tensors.iter_mut().find(|t|t.0=="output_norm.weight").unwrap().1.push(1);assert_eq!(f.error(),Error::TensorShape);
    let mut f=Fixture::new("qwen3");f.tensors[0].2=2;f.set("general.quantization_version",V::U(2));assert_eq!(f.error(),Error::TensorEncoding);
    let mut f=Fixture::new("qwen3");f.tensors[2].2=8;f.set("general.quantization_version",V::U(2));assert_eq!(f.error(),Error::TensorEncoding);
    let mut f=Fixture::new("qwen2");f.tensors.retain(|t|t.0!="blk.1.attn_q.bias");assert_eq!(f.error(),Error::TensorSet);
}

#[test]
fn qwen3_explicit_head_width_can_differ_from_embedding_per_head() {
    let mut f=Fixture::new("qwen3");f.set("qwen3.attention.key_length",V::U(16));
    f.set("qwen3.attention.value_length",V::U(16));f.set("qwen3.rope.dimension_count",V::U(16));
    for (name,dims,_) in &mut f.tensors {
        if name.ends_with("attn_q.weight") || name.ends_with("attn_k.weight") || name.ends_with("attn_v.weight") {dims[1]*=2;}
        if name.ends_with("attn_output.weight") || name.ends_with("attn_q_norm.weight") || name.ends_with("attn_k_norm.weight") {dims[0]*=2;}
    }
    let(b,n)=f.bytes();let idx=Index::parse(&b,n).unwrap();let m=Model::admit(&idx).unwrap();
    assert_eq!(m.config().head_width,16);assert_eq!(m.kv_bytes(1024,524288),Ok(524288));
    let mut f=Fixture::new("qwen2");f.set("qwen2.attention.key_length",V::U(16));assert_eq!(f.error(),Error::Geometry);
}

#[test]
fn quantized_layout_version_is_required_and_exact_for_every_supported_kind() {
    for arch in ["qwen2","qwen3"] {
        for kind in [6,8,12,14] {
            for version in [None,Some(V::S("2".into())),Some(V::U(99)),Some(V::U(2))] {
                let valid=matches!(version,Some(V::U(2)));
                let mut f=Fixture::new(arch);
                f.set(&format!("{arch}.embedding_length"),V::U(256));
                f.set(&format!("{arch}.feed_forward_length"),V::U(512));
                for (name,dims,_) in &mut f.tensors {
                    dims[0]*=8;
                    if dims.len()==2 && name!="token_embd.weight" && name!="output.weight" {dims[1]*=8;}
                }
                f.tensors[0].2=kind;
                if let Some(v)=version {f.set("general.quantization_version",v);}
                let(b,n)=f.bytes();let idx=Index::parse(&b,n).unwrap();let result=Model::admit(&idx);
                if valid {assert!(result.is_ok(),"{arch} kind={kind}");}
                else {assert_eq!(result.unwrap_err(),Error::QuantizationVersion,"{arch} kind={kind}");}
            }
        }
        let f=Fixture::new(arch);let(b,n)=f.bytes();let idx=Index::parse(&b,n).unwrap();
        assert!(Model::admit(&idx).is_ok()); // All-floating models need no quantization version.
    }
}
