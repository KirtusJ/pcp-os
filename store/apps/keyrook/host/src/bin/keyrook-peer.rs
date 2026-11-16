//! Work orders contain public pairs only and travel as ordinary UTF-8.
use keyrook_core::{attack_range, Challenge};
use std::{env,error::Error};

const USAGE: &str = "keyrook-peer split|work BITS STRATEGY START END P1:C1 P2:C2 P3:C3";
fn main() {
    if let Err(e)=run(){eprintln!("{e}");std::process::exit(2);}
}
fn run()->Result<(),Box<dyn Error>>{
    let args:Vec<_>=env::args().skip(1).collect();
    if args.is_empty()||args==["--help"]{println!("{USAGE}");return Ok(())}
    let [action,bits,strategy,lo,hi,p,q,r]=args.as_slice() else{return Err(USAGE.into())};
    let bits:u8=bits.parse()?; let strategy:u8=strategy.parse()?;
    let lo:u16=lo.parse()?; let hi:u16=hi.parse()?;
    if !matches!(bits,4|6|8)||strategy>1||lo>=hi||hi>1<<bits{return Err("invalid search partition".into())}
    let parse=|v:&str|->Result<(u16,u16),Box<dyn Error>>{
        let (x,y)=v.split_once(':').ok_or("expected P:C")?;
        if [x,y].iter().any(|s|s.len()!=4||!s.bytes().all(|b|b.is_ascii_hexdigit())){return Err("invalid block".into())}
        Ok((u16::from_str_radix(x,16)?,u16::from_str_radix(y,16)?))
    };
    let task=Challenge{bits,pairs:[parse(p)?,parse(q)?,parse(r)?]};
    match action.as_str(){
        "split" if hi-lo>1=>{
            let mid=lo+(hi-lo)/2;
            for (start,end) in [(lo,mid),(mid,hi)]{println!("work {bits} {strategy} {start} {end} {p} {q} {r}");}
        }
        "work"=>{
            let out=attack_range(&task,lo,hi,0,strategy);
            let key=out.key.map_or_else(||"null".to_string(),|(a,b)|format!("[{a},{b}]"));
            println!("{{\"start\":{lo},\"end\":{hi},\"candidates\":{},\"key\":{key},\"work\":{}}}",out.candidates,out.work);
        }
        _=>return Err("expected work or a divisible interval for split".into())
    }
    Ok(())
}
