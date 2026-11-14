//! pcpos-verify: check a register and report its head, and measure the draft compact form against it.
//!
//!     pcpos-verify <register.jsonl> [--r1 <output file>]
//!
//! Exit status: 0 the register is intact; 6 it is not; 2 usage. A pass is a fact about these bytes.
#![forbid(unsafe_code)]
use pcpos_core::{ledger, r1, sha256_hex};
use std::{env, fs, process::exit};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0].starts_with('-') {
        eprintln!("usage: pcpos-verify <register.jsonl> [--r1 <output file>]");
        exit(2);
    }
    let raw = match fs::read(&args[0]) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pcpos-verify: cannot read {}: {}", args[0], e);
            exit(2);
        }
    };
    let (events, head) = match ledger::verify(&raw) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("pcpos-verify: the register is not intact: {:?}", e);
            exit(6);
        }
    };
    println!("register: {} events, chain intact, head {}", events.len(), head);
    let export = args.len() == 3 && args[1] == "--r1";
    let mut packed = Vec::new();
    let mut first_unrepresentable = None;
    let (mut generic, mut positional) = (0usize, 0usize);
    for (index, ev) in events.iter().enumerate() {
        let g = r1::encode(ev);
        assert_eq!(&r1::decode(&g).expect("R1 decodes"), ev, "R1 must decode to the value encoded");
        generic += g.len();
        if first_unrepresentable.is_none() {
            match r1::encode_event(ev) {
                Some(p) => {
                    positional += p.len();
                    if export { packed.extend_from_slice(&p); }
                }
                None => first_unrepresentable = Some(index),
            }
        }
    }
    let json = raw.len();
    let pct = |n: usize| if json == 0 { 0 } else { n * 100 / json };
    if let Some(index) = first_unrepresentable {
        println!("size: JSON form {json} bytes; draft R1, map form {generic} bytes; positional form unavailable at event {index}");
        if export {
            eprintln!("pcpos-verify: positional export requires a representable UTC calendar timestamp at every event");
            exit(2);
        }
        return;
    }
    println!("size: JSON form {} bytes; draft R1, map form {} bytes ({}%); draft R1, event form {} bytes ({}%)", json, generic, pct(generic), positional, pct(positional));
    if export {
        fs::write(&args[2], &packed).expect("write");
        println!("wrote {} ({} bytes, sha256 {})", args[2], packed.len(), sha256_hex(&packed));
    }
}
