//! Read one completed COM2 capture. The result describes the supplied report.
use pcpos_core::{machine::System, r1, value::Value};
use std::io::{self, Read, Write};

pub enum Format {
    Json,
    Cbor,
}

// Three v1 records with bounded counters, two digests and short enum fields.
const MAX_CAPTURE: u64 = 4096;
type Check<T> = Result<T, &'static str>;

impl Format {
    fn next<'a>(&self, input: &mut &'a [u8]) -> Check<Value> {
        if input.is_empty() {
            return Err("missing record");
        }
        let (value, rest) = match self {
            Self::Json => {
                let end = input.iter().position(|&b| b == b'\n').ok_or("unfinished JSON line")?;
                let value = Value::parse(&input[..end]).map_err(|_| "invalid JSON record")?;
                (value, &input[end + 1..])
            }
            Self::Cbor => r1::decode_prefix(input).map_err(|_| "invalid or unfinished CBOR record")?,
        };
        *input = rest;
        Ok(value)
    }

    fn record(&self, input: &mut &[u8], event: &str, fields: usize) -> Check<Value> {
        let row = self.next(input)?;
        if row.get("v").and_then(Value::as_int) != Some(1) {
            return Err("missing or unsupported report version");
        }
        if row.get("event").and_then(Value::as_str) != Some(event) {
            return Err("unexpected record order");
        }
        if !matches!(&row, Value::Map(m) if m.len() == fields) {
            return Err("unexpected fields");
        }
        Ok(row)
    }
}

fn boolean(row: &Value, key: &str) -> Check<bool> {
    match row.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("missing or invalid boolean"),
    }
}

fn count(row: &Value, key: &str) -> Check<u32> {
    row.get(key).and_then(Value::as_int).and_then(|n| u32::try_from(n).ok())
        .ok_or("missing or invalid check count")
}

fn digest(row: &Value, key: &str) -> Check<()> {
    let value = row.get(key).and_then(Value::as_str).ok_or("missing digest")?;
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err("invalid SHA-256 digest");
    }
    Ok(())
}

fn verdict(format: Format, mut input: &[u8]) -> Check<(bool, Value)> {
    let boot = format.record(&mut input, "boot", 4)?;
    let session = format.record(&mut input, "session", 6)?;
    let stop = format.record(&mut input, "stop", 7)?;
    if !input.is_empty() {
        return Err("trailing data after stop");
    }
    digest(&boot, "image")?;
    digest(&stop, "head")?;
    let image = match boot.get("match") {
        Some(Value::Null) => "unchecked",
        Some(Value::Bool(true)) => "matched",
        Some(Value::Bool(false)) => "mismatch",
        _ => return Err("invalid image comparison"),
    };
    let system = match session.get("system") {
        Some(Value::Null) => None,
        Some(Value::Str(s)) => Some(System::from_name(s).ok_or("unknown system state")?),
        _ => return Err("invalid system state"),
    };
    let admitted = boolean(&session, "admit")?;
    let session_fail = count(&session, "fail")?;
    let session_pass = count(&session, "pass")?;
    let failed = count(&stop, "fail")?;
    let passed = count(&stop, "pass")?;
    let replay = boolean(&stop, "replay")?;
    let events = stop.get("events").and_then(Value::as_int).ok_or("invalid event count")?;
    if events < 0 || (replay && events == 0) {
        return Err("inconsistent event count");
    }
    // Each successful replay contributes one final check. An unsuccessful
    // replay adds a failure; shutdown and the session can add others.
    if admitted != (session_fail == 0)
        || (admitted && (system != Some(System::Operational) || image == "mismatch"))
        || passed < session_pass || (replay && passed == session_pass)
        || failed < session_fail || (!replay && failed == session_fail)
    {
        return Err("inconsistent boot results");
    }
    let healthy = failed == 0 && replay && admitted;
    let mut fields = vec![
        ("boot", super::s(if healthy { "ok" } else { "failed" })),
        ("image", super::s(image)),
    ];
    if !healthy {
        fields.extend([
            ("admit", Value::Bool(admitted)), ("fail", Value::Int(i64::from(failed))),
            ("replay", Value::Bool(replay)), ("system", system.map(|s| super::s(s.name())).unwrap_or(Value::Null)),
        ]);
    }
    Ok((healthy, super::obj(fields)))
}

pub fn run(format: Format, input: impl Read) -> i32 {
    let mut capture = Vec::new();
    let unknown = || super::obj(vec![("boot", super::s("unknown"))]);
    let (code, summary) = match input.take(MAX_CAPTURE + 1).read_to_end(&mut capture) {
        Err(error) => {
            eprintln!("pcpos boot-report: input read failed: {error}");
            (2, unknown())
        }
        Ok(_) => {
            let result = if capture.len() as u64 > MAX_CAPTURE {
                Err("capture exceeds 4096 bytes")
            } else {
                verdict(format, &capture)
            };
            match result {
                Ok((true, summary)) => (0, summary),
                Ok((false, summary)) => (1, summary),
                Err(reason) => {
                    eprintln!("pcpos boot-report: {reason}");
                    (6, unknown())
                }
            }
        }
    };
    let mut message = summary.canonical_json();
    message.push(b'\n');
    if let Err(error) = io::stdout().lock().write_all(&message) {
        eprintln!("pcpos boot-report: output write failed: {error}");
        return 2;
    }
    code
}
