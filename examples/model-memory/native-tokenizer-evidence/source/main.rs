#![no_std]
#![no_main]
extern crate alloc;

mod heap;
include!("pins.rs");

use pcp_game_guest::{call, exit, log, open};
use pcpos_gguf::Index;
use pcpos_model_media::{Manifest, VerifiedSlices};
use pcpos_qwen::tokenizer::{Error, Tokenizer, Utf8Stream, MAX_TEXT_BYTES, MAX_TOKEN_BYTES};
use sha2::{Digest, Sha256};

fn fail(message: &[u8]) -> ! {
    log(b"B3 native tokenizer failure: ");
    log(message);
    log(b"\n");
    exit(1)
}

fn require(condition: bool, message: &[u8]) {
    if !condition { fail(message); }
}

fn decimal(mut value: u32, output: &mut [u8; 10]) -> &[u8] {
    let mut at = output.len();
    loop {
        at -= 1;
        output[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 { return &output[at..]; }
    }
}

fn reference_cases(tokenizer: &Tokenizer<'_>) {
    let mut cases = &include_bytes!("cases.bin")[..];
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut total = 0;
    let mut piece = [0; MAX_TOKEN_BYTES];
    const HEX: &[u8; 16] = b"0123456789abcdef";

    while !cases.is_empty() {
        let n = u16::from_le_bytes(cases[..2].try_into().unwrap()) as usize;
        let source = &cases[2..n + 2];
        cases = &cases[n + 2..];
        let text = core::str::from_utf8(source).unwrap();
        let budget = Tokenizer::encode_bytes(n, MAX_TEXT_BYTES).unwrap();
        let ids = tokenizer.encode(text, MAX_TEXT_BYTES, budget, || true)
            .unwrap_or_else(|_| fail(b"encode reference input"));
        for &byte in source {
            hash.update([HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]]);
        }
        hash.update(b"\t");
        let mut at = 0;
        let mut stream = Utf8Stream::new();
        for (i, &id) in ids.iter().enumerate() {
            if i != 0 { hash.update(b","); }
            hash.update(decimal(id, &mut [0; 10]));
            let n = tokenizer.piece(id, &mut piece).unwrap_or_else(|_| fail(b"reference piece"));
            stream.push(&piece[..n], |s| {
                require(source.get(at..at+s.len()) == Some(s.as_bytes()), b"utf8 roundtrip");
                at += s.len();
            }).unwrap_or_else(|_| fail(b"UTF-8 stream"));
        }
        require(stream.finish().is_ok() && at == source.len(), b"incomplete stream");
        hash.update(b"\n");
        count += 1;
        total += ids.len();
    }
    require(count == 541 && total == 50223, b"reference corpus totals");
    require(<[u8; 32]>::from(hash.finalize()) == VECTORS, b"reference token vector hash");
    log(b"B3 541 cases, 50223 ids, reference vector digest, all UTF-8 roundtrips PASS\n");
    let mut raw = Sha256::new();
    for id in 0u32..151643 {
        let n = tokenizer.piece(id, &mut piece).unwrap_or_else(|_| fail(b"normal piece"));
        raw.update(id.to_le_bytes());
        raw.update((n as u32).to_le_bytes());
        raw.update(&piece[..n]);
    }
    require(<[u8; 32]>::from(raw.finalize()) == PIECES, b"normal piece digest");
    log(b"B3 151643 normal pieces, reference id-length-bytes digest PASS\n");
}

fn text_policy(t: &Tokenizer<'_>, qwen3: bool) {
    let mut p = [0; MAX_TOKEN_BYTES];
    for (name, id) in [("<|im_start|>",151644), ("<tool_call>",151657), ("<think>",151667)] {
        if name == "<think>" && !qwen3 { continue; }
        let ids = t.encode(name, 64, 32768, || true).unwrap_or_else(|_| fail(b"literal text"));
        require(ids.iter().all(|&v| v < 151643) && t.special_id(name)==Some(id), b"literal/trusted ID");
    }
    require(t.piece(151644, &mut p) == Err(Error::Special), b"control display");
    let n = t.piece(128247, &mut p).unwrap_or_else(|_| fail(b"ordinary end tag"));
    require(&p[..n] == b"</s>", b"ordinary end tag bytes");
    let s = "café e\u{301} 中文\r\n";
    let budget = Tokenizer::encode_bytes(s.len(), 64).unwrap();
    let mut polls = 0;
    let wanted = t.encode(s, 64, budget, || { polls += 1; true }).unwrap();
    let mut tick = 0;
    require(t.encode(s,64,budget,||{tick+=1;tick<polls}) == Err(Error::Cancelled), b"last encode poll cancellation");
    require(t.encode(s,64,budget-1,||true) == Err(Error::Budget), b"encode budget");
    require(t.encode(s,1,budget,||true) == Err(Error::Output), b"output cap");
    require(t.encode(s,64,budget,||true).unwrap() == wanted, b"retry after cancellation");
    log(b"B3 literal CONTROL/USER_DEFINED, ordinary </s>, budget and late encode cancellation/retry PASS\n");
}

pcp_game_guest::entry!(run);
#[no_mangle]
pub extern "C" fn run() -> ! {
    let fd = open(b"/dev/model\0", 0);
    require(fd >= 0, b"open model");
    let fd = fd as usize;
    let mut desc = [0u8; 48];
    require(call(16,fd,0x6d01,desc.as_mut_ptr() as usize,0,0,0)==0, b"describe");
    let length = u64::from_le_bytes(desc[8..16].try_into().unwrap()) as usize;
    let pin: [u8;32] = desc[16..].try_into().unwrap();
    require(length == 16<<20 && (pin==CODER_PIN || pin==QWEN3_PIN), b"pinned prefix identity");
    require(u64::from_le_bytes(desc[..8].try_into().unwrap())==88,b"manifest length");
    let mut meta = [0u8;88];
    require(call(0,fd,meta.as_mut_ptr() as usize,88,0,0,0)==88,b"read manifest");
    let manifest = Manifest::parse(&meta, pin).unwrap_or_else(|_| fail(b"manifest"));
    require(manifest.model_bytes()==length as u64, b"prefix length");
    let addr = call(9,0,length,1,2,fd,0);
    require(addr>0, b"read-only prefix mapping");
    let bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, length) };
    let shards = [bytes];
    let verified = VerifiedSlices::verify(&manifest,&shards,|_| true).unwrap_or_else(|_| fail(b"prefix digest"));
    let size = if pin==CODER_PIN {491400064} else {5027783488};
    let index = Index::parse(verified.shard(0).unwrap(), size).unwrap_or_else(|_| fail(b"GGUF index"));
    let need = Tokenizer::table_bytes(&index).unwrap_or_else(|_| fail(b"table size"));
    require(need==6676400,b"table budget");
    require(matches!(Tokenizer::new(&index,need-1,||true),Err(Error::Budget)), b"under-budget table");
    let mut tick=0;
    require(matches!(Tokenizer::new(&index,need,||{tick+=1;tick<300000}),Err(Error::Cancelled)),b"table sort cancellation");
    log(b"B3 verified 16 MiB prefix, 6676400-byte table budget, sort cancellation PASS\n");
    let t=Tokenizer::new(&index,need,||true).unwrap_or_else(|_| fail(b"full tokenizer admission"));
    require(t.vocabulary()==151936,b"vocabulary size");
    text_policy(&t, pin==QWEN3_PIN);
    reference_cases(&t);
    let talk=open(b"/dev/talk\0",2);
    require(talk>=0 && call(1,talk as usize,b"Y".as_ptr() as usize,1,0,0,0)==1,b"ack");
    exit(0)
}
