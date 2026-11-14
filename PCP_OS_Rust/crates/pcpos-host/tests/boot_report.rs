//! Exercise the installed CLI with both on-wire forms, including partial captures.
use pcpos_core::{r1, value::Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn sample() -> [Value; 3] {
    [
        r#"{"event":"boot","image":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","match":null,"v":1}"#,
        r#"{"admit":true,"event":"session","fail":0,"pass":70,"system":"OPERATIONAL","v":1}"#,
        r#"{"event":"stop","events":43,"fail":0,"head":"456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123","pass":71,"replay":true,"v":1}"#,
    ].map(|s| Value::parse(s.as_bytes()).unwrap())
}

fn set(row: &mut Value, key: &str, value: Value) {
    let Value::Map(fields) = row else { panic!("expected record") };
    fields.insert(key.to_owned(), value);
}

fn encode(format: &str, rows: &[Value]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in rows {
        if format == "cbor" {
            raw.extend(r1::encode(row));
        } else {
            raw.extend(row.canonical_json());
            raw.push(b'\n');
        }
    }
    raw
}

fn call(format: &str, raw: &[u8]) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .args(["boot-report", format])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(raw).unwrap();
    let result = child.wait_with_output().unwrap();
    (result.status.code().unwrap(), String::from_utf8(result.stdout).unwrap())
}

#[test]
fn complete_capture_preserves_comparison_and_failure_stage() {
    for format in ["json", "cbor"] {
        let mut rows = sample();
        assert_eq!(call(format, &encode(format, &rows)), (0, "{\"boot\":\"ok\",\"image\":\"unchecked\"}\n".into()));
        set(&mut rows[0], "match", Value::Bool(true));
        assert_eq!(call(format, &encode(format, &rows)), (0, "{\"boot\":\"ok\",\"image\":\"matched\"}\n".into()));
        // v1 counts are cumulative; additional successful checks can run during the session.
        set(&mut rows[2], "pass", Value::Int(72));
        assert_eq!(call(format, &encode(format, &rows)).0, 0);
        set(&mut rows[2], "fail", Value::Int(1));
        let (code, summary) = call(format, &encode(format, &rows));
        assert_eq!(code, 1);
        assert!(summary.contains("\"admit\":true") && summary.contains("\"replay\":true"));
        set(&mut rows[0], "match", Value::Bool(false));
        set(&mut rows[1], "system", Value::Str("SAFE".into()));
        set(&mut rows[1], "fail", Value::Int(1));
        set(&mut rows[1], "admit", Value::Bool(false));
        let (code, summary) = call(format, &encode(format, &rows));
        assert_eq!(code, 1);
        assert!(summary.contains("\"image\":\"mismatch\"") && summary.contains("\"system\":\"SAFE\""));
        set(&mut rows[2], "replay", Value::Bool(false));
        set(&mut rows[2], "fail", Value::Int(2));
        set(&mut rows[2], "pass", Value::Int(70));
        let (code, summary) = call(format, &encode(format, &rows));
        assert_eq!(code, 1);
        assert!(summary.contains("\"replay\":false"));
    }
}

#[test]
fn incomplete_or_contradictory_capture_has_no_success_verdict() {
    let unknown = (6, "{\"boot\":\"unknown\"}\n".to_owned());
    for format in ["json", "cbor"] {
        let rows = sample();
        let capture = encode(format, &rows);
        let mut extra = capture.clone();
        extra.extend(&capture);
        for raw in [vec![], encode(format, &rows[..2]), encode(format, &rows[1..]),
                    capture[..capture.len()-1].to_vec(), extra, vec![b' '; 4097]] {
            assert_eq!(call(format, &raw), unknown);
        }
        for (n, k, value) in [
            (0, "v", Value::Int(2)), (0, "image", Value::Str("short".into())),
            (0, "match", Value::Bool(false)), (1, "fail", Value::Bool(false)),
            (1, "system", Value::Str("SAFE".into())), (2, "pass", Value::Int(70)),
            (2, "events", Value::Int(0)), (2, "replay", Value::Bool(false)),
        ] {
            let mut changed = rows.clone();
            set(&mut changed[n], k, value);
            assert_eq!(call(format, &encode(format, &changed)), unknown, "{format} {k}");
        }
    }
}
