//! pcpos-verify: check a register and report its head, and measure the draft compact form against it.
//!
//!     pcpos-verify <register.jsonl> [--r1 <output file>]
//!
//!     pcpos-verify --from-r1 <input.r1> <expected register head> > register.jsonl
//!
//! Exit status: 0 the register is intact; 6 it is not; 2 usage. A pass is a fact about these bytes.
#![forbid(unsafe_code)]
use pcpos_core::{ledger, r1, sha256_hex};
use std::{env, fs, process::exit};

/// Restore only after the complete chain matches the supplied canonical register head.
fn receive(input: &str, expected: &str) -> Result<ledger::Ledger, (i32, String)> {
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err((2, "expected register head must be 64 lower-case hex digits".into()));
    }
    let wire = fs::read(input).map_err(|e| (2, format!("cannot read {input}: {e}")))?;
    let mut rest = wire.as_slice();
    let mut register = ledger::Ledger::new();
    while !rest.is_empty() {
        let (event, tail) = r1::decode_event(rest).map_err(|e| (6, format!("R1 event {}: {e:?}", register.len())))?;
        let mut line = event.canonical_json();
        line.push(b'\n');
        register.extend(&line).map_err(|e| (6, format!("register chain: {e:?}")))?;
        rest = tail;
    }
    if register.head() != expected {
        return Err((6, format!("received register head {} differs from expected {expected}", register.head())));
    }
    Ok(register)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let [flag, input, expected] = args.as_slice() {
        if flag == "--from-r1" {
            use std::io::Write;
            let result = receive(input, expected).and_then(|register| {
                std::io::stdout().lock().write_all(register.bytes()).map_err(|e| (2, format!("cannot write register: {e}")))
            });
            if let Err((code, message)) = result {
                eprintln!("pcpos-verify: {message}");
                exit(code);
            }
            return;
        }
    }
    if args.is_empty() || args[0].starts_with('-') {
        eprintln!("usage: pcpos-verify <register.jsonl> [--r1 <output file>] | --from-r1 <input.r1> <expected register head>");
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
