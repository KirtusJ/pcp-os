//! pcpos-verify: check a canonical register or transfer its positional R1 frames.
//!
//!     pcpos-verify <register.jsonl> [--r1 <output file> [--after <base head>]]
//!     pcpos-verify --from-r1 <input.r1> <expected head> [--base <base.jsonl>] > restored.jsonl
//!
//! Exit status: 0 intact; 6 integrity failure; 2 usage, representation or I/O error.
#![forbid(unsafe_code)]
use pcpos_core::{ledger, r1, sha256_hex};
use std::io::{self, Read, Write};
use std::{env, fs, process::exit};

type Error = (i32, String);
const USAGE: &str = "usage: pcpos-verify <register.jsonl> [--r1 <output file> [--after <base head>]] | --from-r1 <input.r1> <expected head> [--base <base.jsonl>]";

fn check_head(head: &str, which: &str) -> Result<(), Error> {
    if head.len() == 64 && head.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        Ok(())
    } else {
        Err((2, format!("{which} must be 64 lower-case hex digits")))
    }
}

/// The optional prefix is verified in its canonical form. Only the suffix travels as R1.
fn receive(input: &str, expected: &str, base: Option<&str>) -> Result<(), Error> {
    check_head(expected, "expected register head")?;
    let mut register = match base {
        None => ledger::Ledger::new(),
        Some(path) => {
            let bytes = fs::read(path).map_err(|e| (2, format!("cannot read {path}: {e}")))?;
            ledger::Ledger::from_owned_bytes(bytes).map_err(|e| (6, format!("base register is not intact: {e:?}")))?
        }
    };
    let wire = fs::read(input).map_err(|e| (2, format!("cannot read {input}: {e}")))?;
    let mut rest = wire.as_slice();
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
    io::stdout().lock().write_all(register.bytes()).map_err(|e| (2, format!("cannot write register: {e}")))
}

/// Compare the opened objects before truncating an existing output, including aliases via links.
fn write_frames(source: &fs::File, output: &str, packed: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new().write(true).create(true).truncate(false).open(output)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (a, b) = (source.metadata()?, file.metadata()?);
        if (a.dev(), a.ino()) == (b.dev(), b.ino()) {
            return Err(io::Error::other("output aliases the input register"));
        }
    }
    // Windows prevents this write-open on aliases via the source handle's share mode.
    #[cfg(not(unix))]
    let _ = source;
    file.set_len(0)?;
    file.write_all(packed)
}

fn inspect(input: &str, output: Option<&str>, after: Option<&str>) -> Result<(), Error> {
    if let Some(base) = after {
        check_head(base, "base head")?;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    if output.is_some() {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1 | 4); // FILE_SHARE_READ | FILE_SHARE_DELETE, excluding write opens to this snapshot
    }
    let mut source = options.open(input).map_err(|e| (2, format!("cannot read {input}: {e}")))?;
    let mut raw = Vec::new();
    source.read_to_end(&mut raw).map_err(|e| (2, format!("cannot read {input}: {e}")))?;
    let (events, head) = ledger::verify(&raw).map_err(|e| (6, format!("the register is not intact: {e:?}")))?;
    println!("register: {} events, chain intact, head {}", events.len(), head);
    let mut packed = Vec::new();
    if let Some(base) = after {
        // Each verified event already carries the hash of the preceding canonical line.
        let start = if base == head {
            events.len()
        } else {
            events.iter().position(|ev| ev.get("prev").and_then(|v| v.as_str()) == Some(base))
                .ok_or_else(|| (6, format!("base head {base} is absent from the verified register")))?
        };
        for (index, ev) in events.iter().enumerate().skip(start) {
            let frame = r1::encode_event(ev).ok_or_else(|| (2, format!("positional form unavailable at event {index}")))?;
            packed.extend_from_slice(&frame);
        }
        println!("after {base}: {} events, {} positional bytes", events.len() - start, packed.len());
    } else {
        let (mut generic, mut positional) = (0usize, 0usize);
        let mut first_unrepresentable = None;
        for (index, ev) in events.iter().enumerate() {
            let g = r1::encode(ev);
            assert_eq!(&r1::decode(&g).expect("R1 decodes"), ev, "R1 must decode to the value encoded");
            generic += g.len();
            if first_unrepresentable.is_none() {
                match r1::encode_event(ev) {
                    Some(p) => {
                        positional += p.len();
                        if output.is_some() { packed.extend_from_slice(&p); }
                    }
                    None => first_unrepresentable = Some(index),
                }
            }
        }
        let json = raw.len();
        let pct = |n: usize| if json == 0 { 0 } else { n * 100 / json };
        if let Some(index) = first_unrepresentable {
            println!("size: JSON form {json} bytes; draft R1, map form {generic} bytes; positional form unavailable at event {index}");
            if output.is_some() {
                return Err((2, "positional export requires a representable UTC calendar timestamp at every event".into()));
            }
            return Ok(());
        }
        println!("size: JSON form {} bytes; draft R1, map form {} bytes ({}%); draft R1, event form {} bytes ({}%)", json, generic, pct(generic), positional, pct(positional));
    }
    if let Some(path) = output {
        write_frames(&source, path, &packed).map_err(|e| (2, format!("cannot write {path}: {e}")))?;
        println!("wrote {} ({} bytes, sha256 {})", path, packed.len(), sha256_hex(&packed));
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["--from-r1", input, expected] => receive(input, expected, None),
        ["--from-r1", input, expected, "--base", base] => receive(input, expected, Some(base)),
        [input] if !input.starts_with('-') => inspect(input, None, None),
        [input, "--r1", output] if !input.starts_with('-') => inspect(input, Some(output), None),
        [input, "--r1", output, "--after", base] if !input.starts_with('-') => inspect(input, Some(output), Some(base)),
        _ => Err((2, USAGE.into())),
    };
    if let Err((code, message)) = result {
        eprintln!("pcpos-verify: {message}");
        exit(code);
    }
}
