#![no_std]
#![no_main]
extern crate alloc;
mod heap;
include!("pins.rs");

use pcp_game_guest::{call,log,open,exit};
use pcpos_model_media::{Manifest,VerifiedSlices};
use pcpos_qwen::{Model,forward::{Decoder,Error}};

fn fail(s:&[u8])->!{log(b"B3 failure: ");log(s);log(b"\n");exit(1)}
fn require(ok:bool,s:&[u8]){if !ok{fail(s);}}
fn logits_line(values:&[f32]){
    let mut b=[0u8;151];
    b[..6].copy_from_slice(b"logits");let mut p=6;
    for v in values {
        b[p]=b' ';p+=1;
        for shift in (0..8).rev() {b[p]=b"0123456789abcdef"[((v.to_bits()>>(4*shift))&15) as usize];p+=1;}
    }
    b[p]=b'\n';log(&b[..p+1]);
}

pcp_game_guest::entry!(run);
#[no_mangle] pub extern "C" fn run()->! {
    let fd=open(b"/dev/model\0",0);require(fd>=0,b"open model");let fd=fd as usize;
    let mut desc=[0u8;48];require(call(16,fd,0x6d01,desc.as_mut_ptr() as usize,0,0,0)==0,b"describe");
    let length=u64::from_le_bytes(desc[8..16].try_into().unwrap()) as usize;
    let pin:[u8;32]=desc[16..].try_into().unwrap();
    require((pin==QWEN2_PIN||pin==QWEN3_PIN)&&u64::from_le_bytes(desc[..8].try_into().unwrap())==88,b"trusted manifest pin");
    let mut metadata=[0u8;88];require(call(0,fd,metadata.as_mut_ptr() as usize,metadata.len(),0,0,0)==88,b"read manifest");
    let manifest=Manifest::parse(&metadata,pin).unwrap_or_else(|_|fail(b"manifest"));
    require(manifest.model_bytes()==length as u64,b"manifest length");
    let addr=call(9,0,length,1,2,fd,0);require(addr>0,b"model map");
    let bytes=unsafe{core::slice::from_raw_parts(addr as *const u8,length)};
    let shards=[bytes];
    let view=VerifiedSlices::verify(&manifest,&shards,|_|true).unwrap_or_else(|_|fail(b"verify weights"));
    let data=view.shard(0).unwrap();
    let index=pcpos_gguf::Index::parse(data,length as u64).unwrap_or_else(|_|fail(b"index"));
    let model=Model::admit(&index).unwrap_or_else(|_|fail(b"admission"));
    let budget=Decoder::buffer_bytes(&model,4).unwrap();
    let mut decoder=Decoder::new(&model,data,4,budget,||true).unwrap_or_else(|_|fail(b"decoder"));
    let tokens=[3u32,7,5,2];let mut all=[[0u32;16];4];let mut polls=[0usize;4];
    for (i,token) in tokens.iter().enumerate(){
        let out=decoder.step(*token,||{polls[i]+=1;true}).unwrap_or_else(|_|fail(b"step"));
        require(out.len()==16,b"vocabulary");logits_line(out);
        for (x,to) in out.iter().zip(&mut all[i]){*to=x.to_bits();}
    }
    decoder.reset();
    for (i,token) in tokens.iter().enumerate(){
        let mut tick=0;
        if i==2 {
            let cancelled=decoder.step(*token,||{tick+=1;tick<polls[i]-1});
            require(matches!(cancelled,Err(Error::Cancelled)) && decoder.position()==i,b"cancelled position");
        }
        let retried=decoder.step(*token,||true).unwrap_or_else(|_|fail(b"retry"));
        require(retried.iter().map(|x|x.to_bits()).eq(all[i].iter().copied()),b"retry differs");
    }
    log(b"B3 tiny forward verified mapping and replay after late cancellation\n");
    let talk=open(b"/dev/talk\0",2);require(talk>=0 && call(1,talk as usize,b"Y".as_ptr() as usize,1,0,0,0)==1,b"ack");exit(0)
}
