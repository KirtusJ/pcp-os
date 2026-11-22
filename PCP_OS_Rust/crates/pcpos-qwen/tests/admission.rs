use pcpos_gguf::Index;
use pcpos_qwen::{Architecture, Error, Model};

mod common;
use common::{Fixture,V};

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
