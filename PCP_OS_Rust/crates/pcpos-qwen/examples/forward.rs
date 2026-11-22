//! Bounded host diagnostic for reference models <=1 GiB; not a native worker.
use std::{fs::{File,OpenOptions},io::{Read,Write},path::PathBuf,time::Instant};
use pcpos_gguf::Index;
use pcpos_qwen::{Model,forward::Decoder};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=std::env::args_os().skip(1).collect();
    if args.len()<3||args.len()>34{return Err("usage: forward MODEL.gguf NEW_OUTPUT_DIR TOKEN... (1..32)".into());}
    let tokens:Vec<u32>=args[2..].iter().map(|a|a.to_str().ok_or("token UTF8")?.parse::<u32>().map_err(|_|"token integer")).collect::<Result<_,_>>()?;
    let file=File::open(&args[0])?;let n=file.metadata()?.len();
    if n==0||n>1<<30{return Err("host helper limits models to 1 GiB; use mapped verified pages in the native worker".into());}
    let mut bytes=Vec::new();bytes.try_reserve_exact(n as usize)?;
    file.take(n+1).read_to_end(&mut bytes)?;
    if bytes.len() as u64!=n{return Err("model length changed".into());}
    let idx=Index::parse(&bytes,n).map_err(|e|format!("index: {e:?}"))?;
    let model=Model::admit(&idx).map_err(|e|format!("admission: {e:?}"))?;
    let mut decoder=Decoder::new(&model,&bytes,32,64<<20,||true).map_err(|e|format!("decoder: {e:?}"))?;
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
