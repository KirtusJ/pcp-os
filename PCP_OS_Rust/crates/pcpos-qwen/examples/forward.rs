//! Bounded host diagnostic for reference models <=1 GiB; not a native worker.
use std::{fs::{File,OpenOptions},io::{Read,Write},path::PathBuf,time::Instant};
use pcpos_gguf::Index;
use pcpos_qwen::{Model,forward::Decoder};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=std::env::args_os().skip(1).collect();
    run(&args)
}
fn run(args:&[std::ffi::OsString])->Result<(),Box<dyn std::error::Error>>{
    if args.len()<3||args.len()>34{return Err("usage: forward MODEL.gguf NEW_OUTPUT_DIR TOKEN... (1..32)".into());}
    let tokens:Vec<u32>=args[2..].iter().map(|a|a.to_str().ok_or("token UTF8")?.parse::<u32>().map_err(|_|"token integer")).collect::<Result<_,_>>()?;
    let file=File::open(&args[0])?;let n=file.metadata()?.len();
    if n==0||n>1<<30{return Err("host helper limits models to 1 GiB; use mapped verified pages in the native worker".into());}
    let mut bytes=Vec::new();bytes.try_reserve_exact(n as usize)?;
    file.take(n+1).read_to_end(&mut bytes)?;
    if bytes.len() as u64!=n{return Err("model length changed".into());}
    let idx=Index::parse(&bytes,n).map_err(|e|format!("index: {e:?}"))?;
    let model=Model::admit(&idx).map_err(|e|format!("admission: {e:?}"))?;
    let mut decoder=Decoder::new(&model,&bytes,tokens.len(),64<<20,||true).map_err(|e|format!("decoder: {e:?}"))?;
    let output=PathBuf::from(&args[1]);std::fs::create_dir(&output)?;
    println!("scope=host-token-forward-unverified-weights-no-tokenizer-no-native-worker");
    let started=Instant::now();
    for (position,token) in tokens.into_iter().enumerate(){
        let logits=decoder.step(token,||true).map_err(|e|format!("step {position}: {e:?}"))?;
        let top=logits.iter().enumerate().max_by(|a,b|a.1.total_cmp(b.1)).unwrap().0;
        let mut file=std::io::BufWriter::new(OpenOptions::new().write(true).create_new(true).open(output.join(format!("{position}.f32")))?);
        for v in logits {file.write_all(&v.to_le_bytes())?;}file.flush()?;
        println!("position={position} token={token} top={top}");
    }
    println!("seconds={:.6}",started.elapsed().as_secs_f64());Ok(())
}

#[cfg(test)]
#[path="../tests/common/mod.rs"]
mod common;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_training_context_accepts_requested_tokens_and_refuses_excess_before_output() {
        let root=std::env::temp_dir().join(format!("qwen-forward-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let model=root.join("context-8.gguf");
        std::fs::write(&model,common::Fixture::full_context("qwen3",8)).unwrap();
        for count in [1,8,9] {
            let output=root.join(count.to_string());
            let mut args=vec![model.clone().into_os_string(),output.clone().into_os_string()];
            args.extend((0..count).map(|_|std::ffi::OsString::from("3")));
            let result=run(&args);
            if count<=8 {
                result.unwrap();
                assert_eq!(std::fs::read_dir(&output).unwrap().count(),count);
                for position in 0..count {assert_eq!(std::fs::metadata(output.join(format!("{position}.f32"))).unwrap().len(),16*4);}
            } else {
                assert!(result.unwrap_err().to_string().contains("decoder: Budget"));
                assert!(!output.exists());
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
