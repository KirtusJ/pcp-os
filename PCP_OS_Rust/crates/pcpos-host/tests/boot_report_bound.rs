//! Exercise the bound receiver through its CLI with independently assembled wire bytes.
use pcpos_core::{r1, value::Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};

const ID_A: [u8; 16] = [0xab; 16];
const ID_B: [u8; 16] = [0xab, 0xab, 0xab, 0xab, 0xab, 0xab, 0xab, 0xab,
                         0xab, 0xab, 0xab, 0xab, 0xab, 0xab, 0xab, 0xac];
const HEX_A: &str = "abababababababababababababababab";
const HEX_B: &str = "abababababababababababababababac";
const OK: &[u8] = b"{\"boot\":\"ok\",\"image\":\"matched\"}\n";
const UNKNOWN: &[u8] = b"{\"boot\":\"unknown\"}\n";
const ADMIT: &[u8] = b"{\"admit\":true,\"image\":\"matched\"}\n";
const NO_ADMISSION: &[u8] = b"{\"admit\":null}\n";

fn positional(stop_fail: u8) -> [Vec<u8>; 3] {
    // v2 [version, tag, ...fields]. These samples use one-byte unsigned counts.
    let mut boot = vec![0x84, 2, 0, 0x58, 32];
    boot.extend([0x42; 32]);
    boot.push(0xf5); // image matched
    let mut session = vec![0x86, 2, 1, 3, 0, 0x6b];
    session.extend(b"OPERATIONAL");
    session.push(0xf5); // admitted at three passes, zero failures
    let mut stop = vec![0x87, 2, 2, 4, stop_fail, 0xf5, 7, 0x58, 32];
    stop.extend([0x17; 32]);
    [boot, session, stop]
}

fn envelope(id: [u8; 16], row: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x83, 3, 0x50]; // array(3), version, byte string(16)
    bytes.extend(id);
    bytes.extend(row);
    bytes
}

fn capture(ids: [[u8; 16]; 3], stop_fail: u8) -> [Vec<u8>; 3] {
    let rows = positional(stop_fail);
    std::array::from_fn(|i| envelope(ids[i], &rows[i]))
}

fn invoke(arguments: &[&str], bytes: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .arg("boot-report").args(arguments)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

fn check(arguments: &[&str], bytes: &[u8], code: i32, stdout: &[u8]) {
    let output = invoke(arguments, bytes);
    assert_eq!(output.status.code(), Some(code), "{arguments:?}, {} input bytes: {output:?}", bytes.len());
    assert_eq!(output.stdout, stdout, "{arguments:?}, {} input bytes", bytes.len());
}

#[test]
fn expected_id_is_checked_on_each_consumed_record() {
    for (id, hex) in [(ID_A, HEX_A), (ID_B, HEX_B)] {
        let rows = capture([id; 3], 0);
        check(&["--health", "cbor3", hex], &rows.concat(), 0, OK);
        check(&["--admission", "cbor3", hex], &rows[..2].concat(), 0, ADMIT);
    }
    for i in 0..3 {
        let mut ids = [ID_A; 3];
        ids[i] = ID_B;
        let mixed = capture(ids, 0).concat();
        check(&["--health", "cbor3", HEX_A], &mixed, 6, UNKNOWN);
        if i < 2 {
            check(&["--admission", "cbor3", HEX_A], &mixed, 6, NO_ADMISSION);
        } else {
            check(&["--admission", "cbor3", HEX_A], &mixed, 0, ADMIT);
        }
    }
    let saved = capture([ID_B; 3], 0).concat();
    check(&["--health", "cbor3", HEX_A], &saved, 6, UNKNOWN);
    // The caller controls freshness. Reusing the original expected ID permits replay.
    check(&["--health", "cbor3", HEX_B], &saved, 0, OK);

    let failed = capture([ID_A; 3], 1).concat();
    let summary = b"{\"admit\":true,\"boot\":\"failed\",\"fail\":1,\"image\":\"matched\",\"replay\":true,\"system\":\"OPERATIONAL\"}\n";
    check(&["--health", "cbor3", HEX_A], &failed, 1, summary);
    check(&["--admission", "cbor3", HEX_A], &failed, 0, ADMIT);
}

#[test]
fn interrupted_wrappers_and_payloads_only_report_completed_views() {
    let rows = capture([ID_A; 3], 0);
    let admission_len = rows[0].len() + rows[1].len();
    let full = rows.concat();
    for end in 0..full.len() {
        check(&["--health", "cbor3", HEX_A], &full[..end], 6, UNKNOWN);
        if end < admission_len {
            check(&["--admission", "cbor3", HEX_A], &full[..end], 6, NO_ADMISSION);
        }
    }
    check(&["--admission", "cbor3", HEX_A], &full[..admission_len], 0, ADMIT);
    let mut trailing = full;
    trailing.push(0);
    check(&["--health", "cbor3", HEX_A], &trailing, 6, UNKNOWN);
}

#[test]
fn only_the_canonical_wrapper_and_v2_payload_are_recognized() {
    let raw = positional(0);
    let rows = capture([ID_A; 3], 0);
    let health = ["--health", "cbor3", HEX_A];
    for (prefix, suffix) in [
        (&[0x9f, 3, 0x50][..], &[0xff][..]), // indefinite array
        (&[0x83, 0x18, 3, 0x50], &[]), // non-shortest version
        (&[0x83, 3, 0x58, 16], &[]), // non-shortest byte string length
        (&[0x83, 2, 0x50], &[]), // wrong version
    ] {
        let altered = [prefix, &ID_A, &raw[2], suffix].concat();
        check(&health, &[&rows[0][..], &rows[1], &altered].concat(), 6, UNKNOWN);
    }
    let text_id = [&[0x83, 3, 0x78, 32][..], HEX_A.as_bytes(), &raw[2]].concat();
    check(&health, &[&rows[0][..], &rows[1], &text_id].concat(), 6, UNKNOWN);

    // Each legacy fixture is accepted under its proper mode before the downgrade check.
    let mut named = Vec::new();
    for fields in [
        r#"{"event":"boot","image":"4242424242424242424242424242424242424242424242424242424242424242","match":true,"v":1}"#,
        r#"{"admit":true,"event":"session","fail":0,"pass":3,"system":"OPERATIONAL","v":1}"#,
        r#"{"event":"stop","events":7,"fail":0,"head":"1717171717171717171717171717171717171717171717171717171717171717","pass":4,"replay":true,"v":1}"#,
    ] {
        named.push(r1::encode(&Value::parse(fields.as_bytes()).unwrap()));
    }
    for (format, legacy) in [("cbor", named.concat()), ("cbor2", raw.concat())] {
        check(&["--health", format], &legacy, 0, OK);
        check(&health, &legacy, 6, UNKNOWN);
        check(&["--health", format], &rows.concat(), 6, UNKNOWN);
    }
    for i in 0..3 {
        let mut mixed = rows.clone();
        mixed[i] = raw[i].clone(); // omit the entire envelope on just this row
        check(&health, &mixed.concat(), 6, UNKNOWN);
    }
    check(&health, &named.iter().flat_map(|row| envelope(ID_A, row)).collect::<Vec<_>>(), 6, UNKNOWN);
}

#[test]
fn only_cbor3_accepts_and_requires_a_well_formed_expected_id() {
    for args in [
        vec!["--health", "cbor3"],
        vec!["--admission", "cbor3", ""],
        vec!["--health", "cbor3", "abababababababababababababababa"],
        vec!["--health", "cbor3", "abababababababababababababababag"],
        vec!["--health", "cbor3", "ABABABABABABABABABABABABABABABAB"],
        vec!["--health", "cbor3", HEX_A, HEX_B],
        vec!["--health", "cbor2", HEX_A],
        vec!["--admission", "cbor", HEX_A],
        vec!["--health", "json", HEX_A],
    ] {
        check(&args, &[], 2, b"");
    }
}
