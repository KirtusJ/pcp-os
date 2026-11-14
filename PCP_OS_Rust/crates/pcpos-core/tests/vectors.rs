//! Cross-implementation tests: the Rust core must agree, byte for byte, with the Python implementation
//! that wrote the files under `vectors/`.
use pcpos_core::cap::{CapError, CapTable, Rights};
use pcpos_core::ledger::{self, Ledger, LedgerError};
use pcpos_core::value::{ParseError, Value};
use pcpos_core::{r1, sha256_hex};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn vectors(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors").join(name);
    fs::read(&p).unwrap_or_else(|e| panic!("{}: {}", p.display(), e))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

#[test]
fn canonical_form_equals_the_python_implementation_byte_for_byte() {
    let all = Value::parse(&vectors("canonical.json")).unwrap();
    let Value::List(cases) = all else { panic!() };
    assert!(cases.len() >= 9);
    for c in cases {
        let input = c.get("input").unwrap().as_str().unwrap();
        let v = Value::parse(input.as_bytes()).unwrap();
        let canon = v.canonical_json();
        assert_eq!(hex(&canon), c.get("canonical_hex").unwrap().as_str().unwrap(), "{}", input);
        assert_eq!(sha256_hex(&canon), c.get("sha256").unwrap().as_str().unwrap());
        assert_eq!(Value::parse(&canon).unwrap(), v, "canonical form parses back to the same value");
    }
}

#[test]
fn registers_written_by_the_python_implementation_verify_to_the_same_head() {
    for name in ["register_reference", "register_fphpm_exercise"] {
        let raw = vectors(&format!("{name}.jsonl"));
        let want = Value::parse(&vectors(&format!("{name}.expected.json"))).unwrap();
        let (events, head) = ledger::verify(&raw).unwrap();
        assert_eq!(events.len() as i64, want.get("events").unwrap().as_int().unwrap(), "{name}");
        assert_eq!(head, want.get("head").unwrap().as_str().unwrap(), "{name}");
    }
}

#[test]
fn a_register_rebuilt_event_by_event_is_identical() {
    let raw = vectors("register_reference.jsonl");
    let (events, head) = ledger::verify(&raw).unwrap();
    let mut l = Ledger::new();
    for ev in &events {
        let s = |k: &str| ev.get(k).unwrap().as_str().unwrap();
        l.append(s("ts"), s("actor"), s("auth"), s("kind"), ev.get("body").unwrap().clone()).unwrap();
    }
    assert_eq!(l.bytes(), &raw[..]);
    assert_eq!(l.head(), head);
    assert_eq!(Ledger::from_bytes(&raw).unwrap().len(), events.len() as u64);
}

#[test]
fn every_edit_of_history_is_refused() {
    let raw = vectors("register_reference.jsonl");
    let lines: Vec<&[u8]> = raw[..raw.len() - 1].split(|&b| b == b'\n').collect();
    let join = |ls: &[&[u8]]| -> Vec<u8> { let mut v = ls.join(&b'\n'); v.push(b'\n'); v };
    let mut changed = raw.clone();
    let at = changed.windows(6).position(|w| w == b"second").unwrap();
    changed[at] = b'S';
    assert!(matches!(ledger::verify(&changed), Err(LedgerError::BrokenChain(4))));
    assert!(matches!(ledger::verify(&join(&[lines[0], lines[2], lines[3]])), Err(LedgerError::BrokenChain(2))));
    assert!(matches!(ledger::verify(&join(&[lines[0], lines[1], lines[3], lines[2]])), Err(LedgerError::BrokenChain(3))));
    assert!(matches!(ledger::verify(&raw[..raw.len() - 1]), Err(LedgerError::IncompleteFinalLine)));
    let spaced = String::from_utf8(raw.clone()).unwrap().replacen("{\"actor\"", "{ \"actor\"", 1);
    assert!(matches!(ledger::verify(spaced.as_bytes()), Err(LedgerError::Malformed(1, _))));
    assert!(ledger::verify(b"").unwrap().0.is_empty());
}

#[test]
fn the_parser_refuses_what_a_lenient_parser_accepts() {
    use ParseError::*;
    for (text, want) in [
        (r#"{"a":1,"a":2}"#, "dup"), ("1.5", "float"), ("1e3", "float"), ("01", "int"), ("9223372036854775808", "int"),
        (r#""\ud800""#, "esc"), (r#""\udc00\ud800""#, "esc"), ("[1,]", "byte"), ("{\"a\":1} x", "trail"), ("NaN", "byte"),
        ("\"a\u{1}b\"", "text"), ("", "end"), ("[1", "end"),
    ] {
        let got = Value::parse(text.as_bytes()).unwrap_err();
        let ok = match (want, got) {
            ("dup", DuplicateKey(_)) | ("float", NotAnInteger(_)) | ("int", BadInteger(_)) | ("esc", BadEscape(_))
            | ("byte", UnexpectedByte(_)) | ("trail", TrailingBytes(_)) | ("text", BadText(_)) | ("end", UnexpectedEnd) => true,
            _ => false,
        };
        assert!(ok, "{text:?} gave {got:?}");
    }
    let deep = "[".repeat(40) + &"]".repeat(40);
    assert!(matches!(Value::parse(deep.as_bytes()), Err(TooDeep(_))));
    assert_eq!(Value::parse(b"-9223372036854775808").unwrap(), Value::Int(i64::MIN));
}

#[test]
fn an_appended_event_is_checked_before_it_is_written() {
    let mut l = Ledger::new();
    let body = || Value::Map(BTreeMap::new());
    assert!(l.append("2026-11-12 00:00:00", "a", "DECLARED", "NOTE", body()).is_err());
    assert!(l.append("2026-11-12T00:00:00Z", "a", "TRUSTED", "NOTE", body()).is_err());
    assert!(l.append("2026-11-12T00:00:00Z", "a", "DECLARED", "NOTE", Value::Int(1)).is_err());
    assert!(l.is_empty() && l.bytes().is_empty());
    l.append("2026-11-12T00:00:00Z", "a", "DECLARED", "NOTE", body()).unwrap();
    assert_eq!(ledger::verify(l.bytes()).unwrap().1, l.head());
}

#[test]
fn capabilities_narrow_and_never_widen() {
    let mut t = CapTable::new(8);
    let root = t.install(7, Rights::READ.with(Rights::APPEND).with(Rights::GRANT)).unwrap();
    let reader = t.derive(root, Rights::READ).unwrap();
    assert_eq!(t.check(reader, Rights::READ), Ok(7));
    assert_eq!(t.check(reader, Rights::APPEND), Err(CapError::Denied));
    assert_eq!(t.derive(reader, Rights::READ), Err(CapError::Denied)); // no GRANT, so nothing further is derived
    assert_eq!(t.derive(root, Rights::READ.with(Rights::DIRECT)), Err(CapError::WouldWiden));
    let mid = t.derive(root, Rights::READ.with(Rights::GRANT)).unwrap();
    let leaf = t.derive(mid, Rights::READ).unwrap();
    assert_eq!(t.revoke(mid), Ok(2)); // the capability and what was derived from it
    assert_eq!(t.check(leaf, Rights::READ), Err(CapError::Invalid));
    assert_eq!(t.check(reader, Rights::READ), Ok(7)); // a sibling is untouched
    let reused = t.derive(root, Rights::READ).unwrap();
    assert_eq!(t.check(mid, Rights::READ), Err(CapError::Invalid)); // a stale handle does not reach a reused slot
    assert_eq!(t.check(reused, Rights::READ), Ok(7));
    let mut small = CapTable::new(1);
    small.install(1, Rights::READ).unwrap();
    assert_eq!(small.install(2, Rights::READ), Err(CapError::Full));
}

#[test]
fn r1_round_trips_and_has_one_form_per_value() {
    for name in ["register_reference", "register_fphpm_exercise"] {
        let (events, _) = ledger::verify(&vectors(&format!("{name}.jsonl"))).unwrap();
        for ev in events {
            let enc = r1::encode(&ev);
            assert_eq!(r1::decode(&enc).unwrap(), ev);
            assert_eq!(r1::encode(&r1::decode(&enc).unwrap()), enc);
            assert!(r1::encode_event(&ev).unwrap().len() < enc.len());
        }
    }
    // not in profile: indefinite length, non-shortest head, a float, unsorted keys, a digest sent as text
    for bad in [&[0x9f, 0xff][..], &[0x18, 0x05], &[0xf9, 0x3c, 0x00], &[0xa2, 0x01, 0x00, 0x00, 0x00], &[0x1b, 0, 0, 0, 0, 0, 0, 0, 1]] {
        assert!(r1::decode(bad).is_err(), "{bad:?}");
    }
    let digest_as_text = { let mut v = vec![0x78, 64]; v.extend_from_slice(&[b'a'; 64]); v };
    assert!(r1::decode(&digest_as_text).is_err());
    assert_eq!(r1::ts_to_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(r1::ts_to_seconds("2026-11-12T00:00:00Z"), Some(1794441600));
    assert_eq!(r1::ts_to_seconds("2024-02-29T12:00:00Z"), Some(1709208000));
}

#[test]
fn continuing_a_checked_record_gives_what_checking_the_whole_gives_at_every_split() {
    use pcpos_core::acr::Record;
    let raw = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors/register_scenario.jsonl")).unwrap();
    let (whole, whole_problems) = Record::open(&raw).unwrap();
    let ends: Vec<usize> = raw.iter().enumerate().filter(|(_, b)| **b == b'\n').map(|(i, _)| i + 1).collect();
    let picture = |r: &Record| {
        let s = r.state();
        format!("{} {:?} {:?} {:?} {:?} {:?}", s.events(), s.in_use(), s.system(),
                s.reviews.values().map(|v| (v.label.clone(), v.lifecycle, v.session, v.passed_gates.clone())).collect::<Vec<_>>(),
                s.tasks.iter().map(|(k, t)| (k.clone(), t.outcome.clone(), t.stop_requested)).collect::<Vec<_>>(),
                s.delivery.iter().map(|(k, d)| (k.clone(), *d)).collect::<Vec<_>>())
    };
    for cut in std::iter::once(0).chain(ends.iter().copied()) {
        let (mut part, mut problems) = Record::open(&raw[..cut]).unwrap();
        problems.extend(part.extend(&raw[cut..]).unwrap());
        assert_eq!(part.ledger().head(), whole.ledger().head(), "split at byte {cut}");
        assert_eq!(part.ledger().bytes(), whole.ledger().bytes());
        assert_eq!(problems.len(), whole_problems.len());
        assert_eq!(picture(&part), picture(&whole), "split at byte {cut}");
    }
    // A tail that does not continue the chain is refused, and so is one that starts mid-line.
    let (mut part, _) = Record::open(&raw[..ends[10]]).unwrap();
    assert!(part.extend(&raw[ends[11]..]).is_err(), "a missing event breaks the chain");
    let (mut part, _) = Record::open(&raw[..ends[10]]).unwrap();
    assert!(part.extend(&raw[ends[10]..ends[11] - 1]).is_err(), "an unfinished line is refused");
}
