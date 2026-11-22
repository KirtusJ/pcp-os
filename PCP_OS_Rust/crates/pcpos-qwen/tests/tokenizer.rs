use pcpos_gguf::Index;
use pcpos_qwen::tokenizer::{Tokenizer,Utf8Stream,Error,MAX_TOKEN_BYTES,MAX_TEXT_BYTES};
fn text(b:&mut Vec<u8>,s:&str){b.extend((s.len() as u64).to_le_bytes());b.extend(s.as_bytes());}
fn byte_chars()->Vec<String>{
    let direct:Vec<_>=(33u32..=126).chain(161..=172).chain(174..=255).collect();
    let mut next=256;
    (0..256).map(|b|{let c=if direct.contains(&b){b}else{let c=next;next+=1;c};char::from_u32(c).unwrap().to_string()}).collect()
}
struct Fixture{tokens:Vec<String>,kinds:Vec<i32>,merges:Vec<String>,pre:String}
impl Fixture{
    fn new()->Self{
        let mut tokens=byte_chars();let mut kinds=vec![1;tokens.len()];
        for (s,k) in [("ab",1),("bc",1),("abc",1),("aa",1),("aaaa",1),("12",1),("a1",1),
                      ("<|endoftext|>",3),("<|im_start|>",3),("<|im_end|>",3),("<think>",4),("[PAD]",5)]{tokens.push(s.into());kinds.push(k);}
        Self{tokens,kinds,merges:["b c","a b","a bc","a a","aa aa","1 2","a 1"].map(str::to_string).to_vec(),pre:"qwen2".into()}
    }
    fn bytes(&self)->Vec<u8>{
        let mut b=b"GGUF".to_vec();b.extend(3u32.to_le_bytes());b.extend(0u64.to_le_bytes());b.extend(9u64.to_le_bytes());
        for (k,v) in [("tokenizer.ggml.model","gpt2"),("tokenizer.ggml.pre",self.pre.as_str())]{text(&mut b,k);b.extend(8u32.to_le_bytes());text(&mut b,v);}
        for (k,values) in [("tokenizer.ggml.tokens",&self.tokens),("tokenizer.ggml.merges",&self.merges)]{
            text(&mut b,k);b.extend(9u32.to_le_bytes());b.extend(8u32.to_le_bytes());b.extend((values.len() as u64).to_le_bytes());for s in values{text(&mut b,s);}
        }
        text(&mut b,"tokenizer.ggml.token_type");b.extend(9u32.to_le_bytes());b.extend(5u32.to_le_bytes());b.extend((self.kinds.len() as u64).to_le_bytes());for k in &self.kinds{b.extend(k.to_le_bytes());}
        for (k,v) in [("tokenizer.ggml.bos_token_id",263u32),("tokenizer.ggml.eos_token_id",265),("tokenizer.ggml.padding_token_id",263)]{text(&mut b,k);b.extend(4u32.to_le_bytes());b.extend(v.to_le_bytes());}
        text(&mut b,"tokenizer.ggml.add_bos_token");b.extend(7u32.to_le_bytes());b.push(0);
        b.resize(b.len().div_ceil(32)*32,0);b
    }
}
fn error(f:&Fixture)->Error{let b=f.bytes();let i=Index::parse(&b,b.len() as u64).unwrap();match Tokenizer::new(&i,1<<20,||true){Err(e)=>e,Ok(_)=>panic!("accepted invalid fixture")}}
fn roundtrip(t:&Tokenizer<'_>,ids:&[u32])->String{
    let mut out=String::new();let mut s=Utf8Stream::new();let mut p=[0;MAX_TOKEN_BYTES];
    for &id in ids{let n=t.piece(id,&mut p).unwrap();s.push(&p[..n],|v|out.push_str(v)).unwrap();}s.finish().unwrap();out
}
#[test]
fn rank_order_ties_byte_roundtrip_and_literal_delimiters(){
    let b=Fixture::new().bytes();let i=Index::parse(&b,b.len() as u64).unwrap();let t=Tokenizer::new(&i,1<<20,||true).unwrap();
    let encode=|s|t.encode(s,MAX_TEXT_BYTES,4<<20,||true).unwrap();
    assert_eq!(encode("abc"),[258]); // bc wins before ab; then a+bc, not ab+c
    assert_eq!(encode("aaaaa"),[260,97]); // equal-rank aa chooses the left pair
    assert_eq!(encode("12 a1"),[49,50,32,97,49]); // digit boundaries forbid available merges
    for s in ["", "Hello, Taylor!", "We're WE'LL I'd I'S", " café e\u{301}  中文 हिन्दी العربية", "🛠️ 🐦\u{200d}🔥",
              "a\0b\r\n\t\u{85}\u{a0}\u{3000}z", "  !?!\n\r  ", "<|im_start|>assistant\n<think>"]{
        let ids=encode(s);assert_eq!(roundtrip(&t,&ids),s);assert!(ids.iter().all(|&id|id<263));
    }
    assert_eq!(t.special_id("<think>"),Some(266));assert_eq!(t.special_id("abc"),None);
    let mut p=[0;MAX_TOKEN_BYTES];assert_eq!(t.piece(263,&mut p),Err(Error::Special));assert_eq!(t.piece(267,&mut p),Err(Error::Unused));
    assert_eq!(t.piece(999,&mut p),Err(Error::Token));assert_eq!(t.piece(258,&mut [0;2]),Err(Error::Output));
}
#[test]
fn admission_refuses_ambiguous_or_unsupported_tables(){
    let mut f=Fixture::new();f.pre="qwen35".into();assert_eq!(error(&f),Error::Profile);
    let mut f=Fixture::new();f.tokens[0]="abc".into();assert_eq!(error(&f),Error::Vocabulary);
    let mut f=Fixture::new();f.tokens[0]="☃".into();assert_eq!(error(&f),Error::Vocabulary);
    let mut f=Fixture::new();f.kinds[0]=2;assert_eq!(error(&f),Error::Vocabulary);
    let mut f=Fixture::new();f.kinds[0]=5;assert_eq!(error(&f),Error::Vocabulary);
    let mut f=Fixture::new();f.kinds.pop();assert_eq!(error(&f),Error::Metadata);
    let mut f=Fixture::new();f.merges.push("b c".into());assert_eq!(error(&f),Error::Merge);
    let mut f=Fixture::new();f.merges[0]="bc x".into();assert_eq!(error(&f),Error::Merge);
    let mut f=Fixture::new();f.merges[0]=" b".into();assert_eq!(error(&f),Error::Merge);
    let mut f=Fixture::new();f.kinds[263]=1;assert_eq!(error(&f),Error::Metadata);
}
#[test]
fn limits_budget_and_cancellation_do_not_return_partial_tokens(){
    let b=Fixture::new().bytes();let i=Index::parse(&b,b.len() as u64).unwrap();let need=Tokenizer::table_bytes(&i).unwrap();
    let mut calls=0;assert!(matches!(Tokenizer::new(&i,need-1,||{calls+=1;true}),Err(Error::Budget)));assert_eq!(calls,0);
    let t=Tokenizer::new(&i,need,||{calls+=1;true}).unwrap();
    for cut in [0,1,255,1000,calls/2,calls-1]{let mut count=0;assert!(matches!(Tokenizer::new(&i,need,||{count+=1;count<=cut}),Err(Error::Cancelled)));}
    let s="aaaaabc\n  中文🛠️ 123";let n=Tokenizer::encode_bytes(s.len(),100).unwrap();
    calls=0;let baseline=t.encode(s,100,n,||{calls+=1;true}).unwrap();
    for cut in 0..calls {let mut tick=0;assert_eq!(t.encode(s,100,n,||{tick+=1;tick<=cut}),Err(Error::Cancelled));}
    assert_eq!(t.encode(s,100,n-1,||true),Err(Error::Budget));
    assert_eq!(t.encode(s,baseline.len()-1,4<<20,||true),Err(Error::Output));
    assert_eq!(t.encode(s,100,n,||true).unwrap(),baseline);
    assert_eq!(t.encode(&"a".repeat(MAX_TEXT_BYTES+1),MAX_TEXT_BYTES,4<<20,||true),Err(Error::Limit));
    let longest="a".repeat(MAX_TEXT_BYTES);let ids=t.encode(&longest,MAX_TEXT_BYTES,4<<20,||true).unwrap();assert_eq!(roundtrip(&t,&ids),longest);
}
#[test]
fn utf8_splits_and_invalid_sequences_are_explicit(){
    let source="Aé中🛠️Z";
    for cut in 0..=source.len(){let mut s=Utf8Stream::new();let mut out=String::new();s.push(&source.as_bytes()[..cut],|v|out.push_str(v)).unwrap();s.push(&source.as_bytes()[cut..],|v|out.push_str(v)).unwrap();s.finish().unwrap();assert_eq!(out,source);}
    let mut s=Utf8Stream::new();let mut out=String::new();for b in source.bytes(){s.push(&[b],|v|out.push_str(v)).unwrap();}s.finish().unwrap();assert_eq!(out,source);
    for invalid in [&[0xff][..],&[0xc0,0x80],&[0xed,0xa0,0x80],&[0xf4,0x90,0x80,0x80]]{assert_eq!(Utf8Stream::new().push(invalid,|_|panic!()),Err(Error::Utf8));}
    s.push(&[0xe4,0xb8],|_|panic!()).unwrap();assert_eq!(s.finish(),Err(Error::Utf8));s.clear();assert_eq!(s.finish(),Ok(()));
}

#[test]
#[ignore="QA only: QWEN_TOKENIZER_MODEL and QWEN_TOKENIZER_VECTORS identify actual model and pinned-reference vectors"]
fn actual_vocabulary_matches_reference_vectors_and_all_normal_pieces(){
    use std::io::Read;
    use sha2::{Digest,Sha256};
    let path=std::env::var_os("QWEN_TOKENIZER_MODEL").unwrap();
    let file=std::fs::File::open(path).unwrap();let size=file.metadata().unwrap().len();let mut b=Vec::new();file.take(16<<20).read_to_end(&mut b).unwrap();
    let prefix=format!("{:x}",Sha256::digest(&b));
    let want=match size{491400064=>"675fc8527c3ec57696d129f70aaa8424907911e7007eafd2f6b8cf9dbc1552c8",5027783488=>"ce8ee0a29cb34c0580ebee996e9d67b2c72858389640a77b42ecb1583c4149b1",_=>panic!("unnominated reference model")};
    assert_eq!(prefix,want);println!("prefix_sha256={prefix}");
    let i=Index::parse(&b,size).unwrap();let t=Tokenizer::new(&i,16<<20,||true).unwrap();
    let source=std::fs::read_to_string(std::env::var_os("QWEN_TOKENIZER_VECTORS").unwrap()).unwrap();
    assert_eq!(format!("{:x}",Sha256::digest(source.as_bytes())),"55e2295e6a6dae9d8ac0da219f6f3fe25ff7ef0e76226566bced4d4cf5ba4feb");
    let mut count=0;let mut total=0;
    for line in source.lines(){
        let (hex,ids)=line.split_once('\t').unwrap();
        let bytes:Vec<_>=(0..hex.len()).step_by(2).map(|i|u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect();
        let text=std::str::from_utf8(&bytes).unwrap();
        let expected:Vec<u32>=if ids.is_empty(){vec![]}else{ids.split(',').map(|v|v.parse().unwrap()).collect()};
        let got=t.encode(text,MAX_TEXT_BYTES,4<<20,||true).unwrap();
        assert_eq!(got,expected,"case {count}: {text:?}");assert_eq!(roundtrip(&t,&got),text);count+=1;total+=got.len();
    }
    let mut hash=Sha256::new();let mut p=[0;MAX_TOKEN_BYTES];
    let reference=std::fs::read(std::path::Path::new(&std::env::var_os("QWEN_TOKENIZER_VECTORS").unwrap()).with_file_name("pieces.bin")).unwrap();let mut at=0;
    assert_eq!(format!("{:x}",Sha256::digest(&reference)),"2fcefb56f07ebf27d6d4d89db448b4750abc6ef4ef9b236789f2cda8b4e65a30");
    for id in 0..151643u32{
        let n=t.piece(id,&mut p).unwrap();hash.update(id.to_le_bytes());hash.update((n as u32).to_le_bytes());hash.update(&p[..n]);
        assert_eq!(u32::from_le_bytes(reference[at..at+4].try_into().unwrap()),id);
        let len=u32::from_le_bytes(reference[at+4..at+8].try_into().unwrap()) as usize;at+=8;
        assert_eq!(&p[..n],&reference[at..at+len],"piece {id}");at+=len;
    }
    assert_eq!(at,reference.len());
    println!("cases={count} token_ids={total} table_bytes={} normal_pieces_sha256={:x}",Tokenizer::table_bytes(&i).unwrap(),hash.finalize());
}
