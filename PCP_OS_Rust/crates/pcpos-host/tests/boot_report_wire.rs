//! Compare the actual kernel encoder with independent named and positional reference values.
#[path = "../../pcpos-kernel/src/report/wire.rs"]
mod wire;

use pcpos_core::{boot_report::expand_positional, machine::System, r1, value::Value};
use wire::{Format, Message};

fn text(s: &str) -> Value {
    Value::Str(s.into())
}

fn compare(message: Message<'_>, fields: &[(&str, Value)]) {
    let row = Value::Map(fields.iter().map(|(key, value)| ((*key).into(), value.clone())).collect());
    let mut json = row.canonical_json();
    json.push(b'\n');
    // Spell the protocol independently of both the direct writer and the host layout.
    let (tag, keys): (i64, &[&str]) = match row.get("event").unwrap().as_str().unwrap() {
        "boot" => (0, &["image", "match"]),
        "session" => (1, &["pass", "fail", "system", "admit"]),
        "stop" => (2, &["pass", "fail", "replay", "events", "head"]),
        _ => unreachable!(),
    };
    let mut items = vec![Value::Int(2), Value::Int(tag)];
    items.extend(keys.iter().map(|key| row.get(key).unwrap().clone()));
    let positional = r1::encode(&Value::List(items));
    for (format, expected) in [(Format::Json, json), (Format::Cbor, r1::encode(&row)), (Format::Cbor2, positional)] {
        let mut raw = Vec::new();
        message.write(format, |bytes| raw.extend_from_slice(bytes));
        assert_eq!(raw, expected);
        if matches!(format, Format::Cbor2) {
            assert_eq!(expand_positional(r1::decode(&raw).unwrap()).as_ref(), Some(&row));
        }
    }
}

#[test]
fn boot_report_bytes_match_the_generic_encoders() {
    let digest = pcpos_core::sha256_hex(b"PCP OS encoder control");
    for image in [pcpos_core::ZERO, digest.as_str()] {
        for matches_expected in [None, Some(false), Some(true)] {
            compare(
                Message::Boot { image, matches_expected },
                &[
                    ("v", Value::Int(1)),
                    ("event", text("boot")),
                    ("image", text(image)),
                    ("match", matches_expected.map(Value::Bool).unwrap_or(Value::Null)),
                ],
            );
        }
    }
    let counts = [0, 1, 23, 24, 255, 256, 65_535, 65_536, u32::MAX];
    for system in [
        None,
        Some(System::Start),
        Some(System::Verify),
        Some(System::Operational),
        Some(System::Degraded),
        Some(System::Maintenance),
        Some(System::Safe),
        Some(System::Stopping),
        Some(System::Stopped),
    ] {
        for passed in counts {
            for failed in counts {
                compare(
                    Message::Session { passed, failed, system },
                    &[
                        ("v", Value::Int(1)),
                        ("event", text("session")),
                        ("pass", Value::Int(i64::from(passed))),
                        ("fail", Value::Int(i64::from(failed))),
                        ("admit", Value::Bool(failed == 0)),
                        ("system", system.map(|s| text(s.name())).unwrap_or(Value::Null)),
                    ],
                );
            }
        }
    }
    for passed in counts {
        let failed = u32::MAX - passed;
        for replay in [false, true] {
            for events in counts.map(u64::from).into_iter().chain([u64::from(u32::MAX) + 1, i64::MAX as u64])
            {
                compare(
                    Message::Stop { passed, failed, replay, events, head: &digest },
                    &[
                        ("v", Value::Int(1)),
                        ("event", text("stop")),
                        ("pass", Value::Int(i64::from(passed))),
                        ("fail", Value::Int(i64::from(failed))),
                        ("replay", Value::Bool(replay)),
                        ("events", Value::Int(i64::try_from(events).unwrap())),
                        ("head", text(&digest)),
                    ],
                );
            }
        }
    }
}
