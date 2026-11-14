//! Source-of-truth for the latest method pin, maintained as events are applied.
//! This flag is historical evidence only: it never authenticates a live file.
use pcpos_core::acr::Record;
use pcpos_core::sha256_hex;
use pcpos_core::value::Value;

const TS: &str = "2026-11-13T00:00:00Z";

fn value(text: &str) -> Value {
    Value::parse(text.as_bytes()).expect("test fixture is JSON")
}

fn pin(verified: &str) -> Value {
    let fingerprint = sha256_hex(b"release");
    let core = sha256_hex(b"descriptor");
    value(&format!(
        r#"{{"presence":"NATIVE","release_fingerprint":"{fingerprint}","core_descriptor_sha256":"{core}","verified":"{verified}"}}"#
    ))
}

#[test]
fn method_verified_flag_follows_latest_pin_and_survives_replay() {
    let mut record = Record::new();
    assert!(!record.state().method_pinned());
    assert!(!record.state().method_verified_at_pin());
    record.submit(TS, "host", "HOST", "WORKSPACE_CREATED", value(r#"{"workspace":"w"}"#))
        .expect("create temporary workspace");
    record.submit(TS, "host", "HOST", "METHOD_PINNED", pin("FILES_VERIFIED_AGAINST_OWN_MANIFEST"))
        .expect("verify initial method pin");
    assert!(record.state().method_pinned());
    assert!(record.state().method_verified_at_pin());

    let copied = record.ledger().bytes().to_vec();
    let (mut replayed, problems) = Record::open(&copied).expect("read existing record");
    assert!(problems.is_empty());
    assert!(replayed.state().method_verified_at_pin());

    replayed.submit(TS, "host", "HOST", "METHOD_PINNED", pin("DECLARED_NOT_VERIFIED"))
        .expect("new pin changes verification status");
    assert!(replayed.state().method_pinned());
    assert!(!replayed.state().method_verified_at_pin());

    let (reopened, problems) = Record::open(replayed.ledger().bytes()).expect("reopen new method pin");
    assert!(problems.is_empty());
    assert!(!reopened.state().method_verified_at_pin());
}
