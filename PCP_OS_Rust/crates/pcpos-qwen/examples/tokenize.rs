//! Host tokenizer diagnostic: reads only a bounded prefix, never the model weights.
use std::{fs::File,io::Read};
use pcpos_gguf::{Index,MAX_PREFIX_BYTES};
use pcpos_qwen::tokenizer::{Tokenizer,Utf8Stream,MAX_TOKEN_BYTES};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let a:Vec<_>=std::env::args().skip(1).collect();
    if a.len()!=2{return Err("usage: tokenize MODEL.gguf TEXT_FILE (UTF-8, <=16 KiB)".into());}
    let file=File::open(&a[0])?;let whole=file.metadata()?.len();let mut prefix=Vec::new();
    file.take(MAX_PREFIX_BYTES as u64).read_to_end(&mut prefix)?;
    let idx=Index::parse(&prefix,whole).map_err(|e|format!("index: {e:?}"))?;
    let tok=Tokenizer::new(&idx,16<<20,||true).map_err(|e|format!("tokenizer: {e:?}"))?;
    let mut text=String::new();File::open(&a[1])?.take(16*1024+1).read_to_string(&mut text)?;
    let ids=tok.encode(&text,16*1024,4<<20,||true).map_err(|e|format!("encode: {e:?}"))?;
    println!("{}",ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
    let mut decoded=String::new();let mut stream=Utf8Stream::new();let mut piece=[0;MAX_TOKEN_BYTES];
    for id in ids {let n=tok.piece(id,&mut piece).map_err(|e|format!("piece: {e:?}"))?;stream.push(&piece[..n],|s|decoded.push_str(s)).map_err(|e|format!("UTF8: {e:?}"))?;}
    stream.finish().map_err(|e|format!("finish: {e:?}"))?;
    if decoded!=text{return Err("token byte round trip differs".into());}Ok(())
}
