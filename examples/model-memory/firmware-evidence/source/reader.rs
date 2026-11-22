#![no_std]
#![no_main]
use pcp_game_guest::{call, exit, log, open};
pcp_game_guest::entry!(run);

fn require(ok:bool, why:&[u8]){
    if !ok { log(why);exit(1); }
}

#[cfg(feature="reader")]
fn zero(bytes:&[u8])->bool{
    let (words,tail)=bytes.split_at(bytes.len()/8*8);
    if !words.is_empty(){
        let matched:u8;
        unsafe{core::arch::asm!("cld","repe scasq","sete {ok}",
            ok=lateout(reg_byte) matched,in("rax") 0usize,
            inout("rdi") words.as_ptr()=>_,inout("rcx") words.len()/8=>_,options(nostack,readonly));}
        if matched!=1{return false;}
    }
    tail.iter().all(|&b|b==0)
}
#[cfg(feature="reader")]
fn inspect(){
    const TOTAL:usize=5_027_783_488;
    const MANIFEST:&[u8;2456]=include_bytes!("model.manifest");
    const PIN:[u8;32]=[81,223,248,105,60,216,35,223,202,193,215,210,108,44,227,218,209,76,211,65,55,214,242,25,107,84,32,204,198,124,174,14];
    let fd=open(b"/dev/model\0",0);
    require(fd>=0,b"reader could not open model\n");let fd=fd as usize;
    let mut desc=[0u8;48];
    require(call(16,fd,0x6d01,desc.as_mut_ptr() as usize,0,0,0)==0,b"describe failed\n");
    require(u64::from_le_bytes(desc[..8].try_into().unwrap())==2456 && u64::from_le_bytes(desc[8..16].try_into().unwrap())==TOTAL as u64 && desc[16..]==PIN,b"unexpected model descriptor\n");
    let mut manifest=[0u8;2456];
    require(call(0,fd,manifest.as_mut_ptr() as usize,manifest.len(),0,0,0)==2456 && &manifest==MANIFEST,b"manifest mismatch\n");
    let at=call(9,0,TOTAL,1,2,fd,0);
    require(at>0,b"model read-only mapping failed\n");
    log(b"B3 native reader mapped 5027783488 model bytes\n");
    let payload=unsafe {core::slice::from_raw_parts(at as *const u8,TOTAL)};
    let mut count=0;
    for (i,chunk) in payload.chunks(64<<20).enumerate(){
        let tag=[b's',b'h',b'a',b'r',b'd',i as u8,(i>>8) as u8];
        require(chunk.starts_with(&tag) && chunk.ends_with(&tag) && zero(&chunk[7..chunk.len()-7]),b"payload mismatch\n");
        count+=1;
    }
    require(count==75,b"incorrect shard count\n");
    log(b"B3 native reader verified the pinned manifest and all 5027783488 model bytes across 75 shards\n");
}

#[no_mangle]
pub extern "C" fn run()->!{
    let talk=open(b"/dev/talk\0",2);require(talk>=0,b"cannot open channel\n");
    #[cfg(feature="reader")]
    {inspect();require(call(1,talk as usize,b"Y".as_ptr() as usize,1,0,0,0)==1,b"result delivery failed\n");}
    #[cfg(not(feature="reader"))]
    {let mut answer=[0u8];require(call(0,talk as usize,answer.as_mut_ptr() as usize,1,0,0,0)==1 && answer[0]==b'Y',b"reader did not confirm\n");require(call(0,talk as usize,answer.as_mut_ptr() as usize,1,0,0,0)==0,b"reader did not close channel\n");log(b"B3 foreground received verification and reader EOF\n");}
    exit(0)
}
