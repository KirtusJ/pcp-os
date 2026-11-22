//! Host diagnostic: bounded prefix only. Does not verify weights or run inference.
use std::{fs::File, io::Read};
use pcpos_gguf::{Index, MAX_PREFIX_BYTES};
use pcpos_qwen::Model;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 { return Err("usage: admit FILE WHOLE_FILE_BYTES CONTEXT KV_BUDGET_BYTES".into()); }
    let number = |i: usize| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(args[i].to_str().ok_or("non-UTF8 number")?.parse()?)
    };
    let file = File::open(&args[0])?;
    let bytes = number(1)?;
    if file.metadata()?.len() > bytes { return Err("input longer than declared model".into()); }
    let mut prefix = Vec::new();
    file.take(MAX_PREFIX_BYTES as u64).read_to_end(&mut prefix)?;
    let index = Index::parse(&prefix, bytes).map_err(|e| format!("index: {e:?}"))?;
    let model = Model::admit(&index).map_err(|e| format!("model: {e:?}"))?;
    let context = usize::try_from(number(2)?)?;
    let kv = model.kv_bytes(context, number(3)?).map_err(|e| format!("KV: {e:?}"))?;
    println!("scope=index-geometry-only-unverified-weights-no-inference");
    println!("{:?}", model.config());
    println!("tensors={} context={context} kv_f32_bytes={kv} tied_output={}",
        index.tensors().len(), core::ptr::eq(model.embedding(), model.output()));
    Ok(())
}
