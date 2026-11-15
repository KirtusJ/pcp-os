mod session;
mod sqlite;
use keyrook_core::{attack, fixture, Challenge, Memory};
use std::{env, fs, path::Path};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const HELP:&str = "Keyrook | recursive cipher arena\n  keyrook train DIR BITS SEED DEPTH\n  keyrook stats DIR BITS\n  keyrook new DIR\n  keyrook export-sqlite DIR NEW.sqlite\n  keyrook challenge BITS SEED\n  keyrook solve BITS DEPTH STRATEGY P1:C1 P2:C2 P3:C3\nBITS 4/6/8, DEPTH 0..3, STRATEGY 0 exhaustive / 1 meet-in-the-middle. Blocks are four hex digits.";

fn bits(s: &str) -> Result<u8> {
    match s.parse()? {
        b @ (4 | 6 | 8) => Ok(b),
        _ => Err("bits must be 4, 6 or 8".into()),
    }
}
fn depth(s: &str) -> Result<u8> {
    let d: u8 = s.parse()?;
    if d > 3 {
        return Err("depth must be 0..3".into());
    }
    Ok(d)
}
fn pair(s: &str) -> Result<(u16, u16)> {
    let (p, c) = s.split_once(':').ok_or("expected P:C")?;
    if [p, c]
        .iter()
        .any(|x| x.len() != 4 || !x.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("use four hexadecimal digits per block".into());
    }
    Ok((u16::from_str_radix(p, 16)?, u16::from_str_radix(c, 16)?))
}

fn run(args: &[String]) -> Result<()> {
    match args {
        [] => println!("{HELP}"),
        [cmd] if cmd == "--help" || cmd == "help" => println!("{HELP}"),
        [cmd, b, s] if cmd == "challenge" => {
            let b = bits(b)?;
            let (c, _) = fixture(b, s.parse()?).unwrap();
            print!("{b}");
            for (p, v) in c.pairs {
                print!(" {p:04x}:{v:04x}");
            }
            println!();
        }
        [cmd, b, d, p, a, e, f] if cmd == "solve" => {
            let p: u8 = p.parse()?;
            if p > 1 {
                return Err("invalid strategy".into());
            }
            let c = Challenge {
                bits: bits(b)?,
                pairs: [pair(a)?, pair(e)?, pair(f)?],
            };
            let out = attack(&c, depth(d)?, p);
            let k = out
                .key
                .map_or_else(|| "null".into(), |(a, b)| format!("[{a},{b}]"));
            println!(
                "{{\"candidates\":{},\"key\":{k},\"work\":{},\"nodes\":{}}}",
                out.candidates, out.work, out.branches
            );
        }
        [cmd, dir, b] if cmd == "stats" => {
            let b = bits(b)?;
            let path = Path::new(dir);
            let m = if path.try_exists()? {
                session::stats(&session::open(path)?, b)?
            } else {
                Memory::default()
            };
            println!(
                "{{\"runs\":{:?},\"wins\":{:?},\"work\":{:?},\"next\":{}}}",
                m.runs,
                m.wins,
                m.work,
                m.choose()
            );
        }
        [cmd, dir, b, s, d] if cmd == "train" => {
            let (b, seed, d) = (bits(b)?, s.parse()?, depth(d)?);
            let out = session::train(&session::open(Path::new(dir))?, b, seed, d)?;
            let p = out.strategy;
            println!("{{\"seed\":{seed},\"strategy\":{p},\"verified\":{},\"candidates\":{},\"work\":{},\"nodes\":{}}}",out.verified,out.candidates,out.work,out.branches);
        }
        [cmd, dir] if cmd == "new" => {
            let path = Path::new(dir);
            fs::create_dir(path)?;
            session::open(path)?;
            println!("Created {}", path.join(session::FILE_NAME).display());
        }
        [cmd, dir, file] if cmd == "export-sqlite" => {
            let path = Path::new(dir);
            if !path.is_dir() {
                return Err("session directory does not exist".into());
            }
            sqlite::export(&session::open(path)?, Path::new(file))?;
            println!("Exported {file}");
        }
        _ => return Err(HELP.into()),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run(&env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{e}");
        std::process::exit(2);
    }
}
