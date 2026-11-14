//! Compare the actual kernel encoder with independent named and positional reference values.
#[path = "../../pcpos-kernel/src/report/wire.rs"]
mod wire;

use pcpos_core::{boot_report::{expand_positional, BootId}, machine::System, r1, value::Value};
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
    let id = BootId::parse("00112233445566778899aabbccddeeff").unwrap();
    let mut bound = vec![0x83, 3, 0x50, 0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
                         0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
    bound.extend_from_slice(&positional);
    for (format, expected) in [
        (Format::Json, json), (Format::Cbor, r1::encode(&row)),
        (Format::Cbor2, positional), (Format::Cbor3(id), bound),
    ] {
        let mut raw = Vec::new();
        message.write(format, |bytes| raw.extend_from_slice(bytes));
        assert_eq!(raw, expected);
        if matches!(format, Format::Cbor2) {
            assert_eq!(expand_positional(r1::decode(&raw).unwrap()).as_ref(), Some(&row));
        }
    }
}

#[test]
fn boot_id_requires_exact_lowercase_hex() {
    let zero = BootId::parse("00000000000000000000000000000000").unwrap();
    assert_eq!(&zero.prefix()[3..], &[0; 16]);
    for invalid in [
        "", "0000000000000000000000000000000", "000000000000000000000000000000000",
        "0000000000000000000000000000000A", "+0000000000000000000000000000000",
        "000000000000000000000000000000é",
    ] {
        assert!(BootId::parse(invalid).is_none(), "{invalid:?}");
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

#[test]
fn streamed_r1_scalars_match_the_generic_writer() {
    for len in [0, 1, 23, 24, 63, 64, 65, 255, 256, 65_535, 65_536] {
        for ch in ['a', 'z', 'é'] {
            let s = ch.to_string().repeat(len);
            let mut bytes = Vec::new();
            r1::emit_text(&s, |part| bytes.extend_from_slice(part));
            assert_eq!(bytes, r1::encode(&Value::Str(s)));
        }
    }
    let mut bytes = Vec::new();
    r1::emit_uint(u64::MAX, |part| bytes.extend_from_slice(part));
    assert_eq!(bytes, [0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
}
