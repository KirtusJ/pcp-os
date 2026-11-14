//! Translate one v1 CBOR capture from the kernel reporting UART. The data describes the sender's observations.
use pcpos_core::machine::System;
use pcpos_core::r1::{decode_prefix, R1Error};
use pcpos_core::value::Value;
use std::io::{Read, Write};

const EVENTS: [&str; 3] = ["boot", "session", "stop"];
// Each v1 message contains a fixed set of short keys, scalars and at most one 32-byte digest.
const MAX_FRAME: usize = 1024;

fn validate(item: &Value, event: &str) -> Result<(), String> {
    let Value::Map(fields) = item else { return Err("expected a CBOR map".into()) };
    if fields.get("v") != Some(&Value::Int(1)) {
        return Err("missing or unsupported boot report version".into());
    }
    if item.get("event").and_then(Value::as_str) != Some(event) {
        return Err(format!("expected the {event} message"));
    }
    let count = |key| item.get(key).and_then(Value::as_int).is_some_and(|n| u32::try_from(n).is_ok());
    let digest = |key| item.get(key).and_then(Value::as_str).is_some_and(|s| s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    let valid = match event {
        "boot" => fields.len() == 4 && digest("image") && matches!(item.get("match"), Some(Value::Null | Value::Bool(_))),
        "session" => fields.len() == 6 && count("pass") && count("fail") && matches!(item.get("admit"), Some(Value::Bool(_)))
            && match item.get("system") { Some(Value::Null) => true, Some(Value::Str(s)) => System::from_name(s).is_some(), _ => false },
        "stop" => fields.len() == 7 && count("pass") && count("fail") && matches!(item.get("replay"), Some(Value::Bool(_))) && digest("head")
            && item.get("events").and_then(Value::as_int).is_some_and(|n| n >= 0),
        _ => false,
    };
    if !valid { return Err(format!("invalid fields in {event} message")); }
    Ok(())
}

/// Forward complete messages in order. Only EOF immediately after stop confirms a complete capture.
pub fn translate(mut input: impl Read, mut output: impl Write) -> Result<(), String> {
    let mut buffer = [0u8; MAX_FRAME];
    let mut filled = 0;
    for event in EVENTS {
        loop {
            match decode_prefix(&buffer[..filled]) {
                Ok((item, rest)) => {
                    validate(&item, event)?;
                    let mut line = item.canonical_json();
                    line.push(b'\n');
                    output.write_all(&line).and_then(|_| output.flush()).map_err(|e| format!("cannot write {event}: {e}"))?;
                    let consumed = filled - rest.len();
                    buffer.copy_within(consumed..filled, 0);
                    filled -= consumed;
                    break;
                }
                Err(R1Error::UnexpectedEnd) if filled < MAX_FRAME => {
                    let n = input.read(&mut buffer[filled..]).map_err(|e| format!("cannot read {event}: {e}"))?;
                    if n == 0 { return Err(format!("incomplete capture: expected a complete {event} message")); }
                    filled += n;
                }
                Err(error) => return Err(format!("invalid or oversized {event} message: {error:?}")),
            }
        }
    }
    if filled != 0 || input.read(&mut buffer[..1]).map_err(|e| e.to_string())? != 0 {
        return Err("unexpected bytes after the stop message".into());
    }
    Ok(())
}
