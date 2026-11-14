//! Exercise the installed CLI with all boot report formats, including partial captures.
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

// Encode the documented positions independently of the receiver and kernel.
fn positional(row: &Value) -> Value {
    let (tag, keys): (_, &[&str]) = match row.get("event").and_then(Value::as_str).unwrap() {
        "boot" => (0, &["image", "match"]),
        "session" => (1, &["pass", "fail", "system", "admit"]),
        "stop" => (2, &["pass", "fail", "replay", "events", "head"]),
        event => panic!("unknown fixture event {event}"),
    };
    let mut fields = vec![Value::Int(2), Value::Int(tag)];
    fields.extend(keys.iter().map(|key| row.get(key).unwrap().clone()));
    Value::List(fields)
}

fn encode(format: &str, rows: &[Value]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in rows {
        match format {
            "cbor2" => raw.extend(r1::encode(&positional(row))),
            "cbor" => raw.extend(r1::encode(row)),
            "json" => {
                raw.extend(row.canonical_json());
                raw.push(b'\n');
            }
            _ => panic!("unknown fixture format {format}"),
        }
    }
    raw
}

fn call(format: &str, raw: &[u8]) -> (i32, String) {
    summary("--health", format, raw)
}

fn summary(view: &str, format: &str, raw: &[u8]) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .args(["boot-report", view, format])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(raw).unwrap();
    let result = child.wait_with_output().unwrap();
    (result.status.code().unwrap(), String::from_utf8(result.stdout).unwrap())
}

#[test]
fn complete_capture_preserves_comparison_and_failure_stage() {
    for format in ["json", "cbor", "cbor2"] {
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
    for format in ["json", "cbor", "cbor2"] {
        let rows = sample();
        let capture = encode(format, &rows);
        let mut extra = capture.clone();
        extra.extend(&capture);
        for raw in [vec![], encode(format, &rows[..2]), encode(format, &rows[1..]),
                    capture[..capture.len()-1].to_vec(), extra, vec![b' '; 4097]] {
            assert_eq!(call(format, &raw), unknown);
        }
        for (n, k, value) in [
            (0, "image", Value::Str("short".into())),
            (0, "match", Value::Bool(false)), (1, "fail", Value::Bool(false)),
            (1, "system", Value::Str("SAFE".into())), (2, "pass", Value::Int(70)),
            (2, "events", Value::Int(0)), (2, "replay", Value::Bool(false)),
        ] {
            let mut changed = rows.clone();
            set(&mut changed[n], k, value);
            assert_eq!(call(format, &encode(format, &changed)), unknown, "{format} {k}");
        }
        if format != "cbor2" {
            let mut changed = rows.clone();
            set(&mut changed[0], "v", Value::Int(2));
            assert_eq!(call(format, &encode(format, &changed)), unknown);
        }
    }
}

#[test]
fn admission_only_describes_the_first_two_records() {
    let unknown = (6, "{\"admit\":null}\n".to_owned());
    for format in ["json", "cbor", "cbor2"] {
        let mut rows = sample();
        let raw = encode(format, &rows[..2]);
        let admitted = (0, "{\"admit\":true,\"image\":\"unchecked\"}\n".to_owned());
        assert_eq!(summary("--admission", format, &raw[..raw.len() - 1]), unknown);
        assert_eq!(summary("--admission", format, &raw), admitted);
        // A later program error changes final health, independently of admission.
        set(&mut rows[2], "fail", Value::Int(1));
        assert_eq!(summary("--admission", format, &encode(format, &rows)), admitted);
        assert_eq!(call(format, &encode(format, &rows)).0, 1);
        set(&mut rows[0], "match", Value::Bool(false));
        assert_eq!(summary("--admission", format, &encode(format, &rows)), unknown);
        set(&mut rows[1], "system", Value::Str("SAFE".into()));
        set(&mut rows[1], "admit", Value::Bool(false));
        set(&mut rows[1], "fail", Value::Int(1));
        assert_eq!(summary("--admission", format, &encode(format, &rows[..2])),
                   (1, "{\"admit\":false,\"fail\":1,\"image\":\"mismatch\",\"system\":\"SAFE\"}\n".into()));
    }
}

#[test]
fn summary_budget_is_inclusive_and_independent_of_following_records() {
    let rows = sample();
    let all = encode("json", &rows);
    let prefix = encode("json", &rows[..2]);
    for (view, boundary) in [("--admission", prefix.len()), ("--health", all.len())] {
        let mut input = vec![b' '; 4096 - boundary];
        input.extend_from_slice(&all);
        assert_eq!(summary(view, "json", &input).0, 0);
        input.insert(0, b' ');
        assert_eq!(summary(view, "json", &input).0, 6);
    }
}

#[test]
fn widest_report_counters_fit_all_encodings() {
    let mut rows = sample();
    let maximum = i64::from(u32::MAX);
    set(&mut rows[1], "pass", Value::Int(maximum - 1));
    set(&mut rows[1], "admit", Value::Bool(false));
    set(&mut rows[1], "fail", Value::Int(maximum));
    set(&mut rows[2], "pass", Value::Int(maximum));
    set(&mut rows[2], "fail", Value::Int(maximum));
    set(&mut rows[2], "events", Value::Int(i64::MAX));
    for format in ["json", "cbor", "cbor2"] {
        assert_eq!(call(format, &encode(format, &rows)).0, 1);
        assert_eq!(summary("--admission", format, &encode(format, &rows[..2])).0, 1);
    }
}

#[test]
fn binary_translation_selects_one_layout_and_only_writes_complete_observations() {
    let rows = sample();
    let root = std::env::temp_dir().join(format!("pcpos-translated-boot-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    // A single argument remains a literal filename, even if it spells a format.
    let capture = root.join("cbor2");
    let translate = |raw: &[u8], code: i32, expected: &[Value]| {
        std::fs::write(&capture, raw).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_pcpos")).current_dir(&root)
            .args(["boot-report", "cbor2"]).output().unwrap();
        assert_eq!(result.status.code(), Some(code));
        assert_eq!(result.stdout, encode("json", expected));
    };
    for (format, other) in [("cbor", "cbor2"), ("cbor2", "cbor")] {
        translate(&encode(format, &rows), 0, &rows);
        assert_eq!(call(other, &encode(format, &rows)).0, 6);
        assert_eq!(summary("--admission", other, &encode(format, &rows[..2])).0, 6);
        for boundary in [1, 2] {
            let mut mixed = encode(format, &rows[..boundary]);
            mixed.extend(encode(other, &rows[boundary..]));
            translate(&mixed, 6, &rows[..boundary]);
            assert_eq!(call(format, &mixed).0, 6);
            assert_eq!(summary("--admission", format, &mixed).0, if boundary == 2 { 0 } else { 6 });
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn positional_records_require_exact_version_tag_arity_and_types() {
    let rows = sample();
    for (record, position, value) in [
        (0,0,Value::Int(1)), (0,0,Value::Int(3)), (0,1,Value::Bool(false)),
        (1,1,Value::Int(2)), (2,1,Value::Int(3)), (0,2,Value::Null),
        (0,3,Value::Int(0)), (1,2,Value::Int(i64::from(u32::MAX) + 1)),
        (1,3,Value::Int(-1)), (1,4,Value::Bool(true)), (1,5,Value::Int(1)),
        (2,4,Value::Null), (2,5,Value::Int(-1)), (2,6,Value::Bool(true)),
    ] {
        let mut positional_rows = rows.clone().map(|v| positional(&v));
        let Value::List(fields) = &mut positional_rows[record] else { unreachable!() };
        fields[position] = value;
        let wire: Vec<u8> = positional_rows.iter().flat_map(r1::encode).collect();
        assert_eq!(call("cbor2", &wire).0, 6, "record {record}, field {position}");
    }
    for record in 0..3 {
        for extra in [true, false] {
            let mut positional_rows = rows.clone().map(|v| positional(&v));
            let Value::List(fields) = &mut positional_rows[record] else { unreachable!() };
            if extra { fields.push(Value::Null); } else { fields.pop(); }
            let wire: Vec<u8> = positional_rows.iter().flat_map(r1::encode).collect();
            assert_eq!(call("cbor2", &wire).0, 6);
        }
    }
    let mut failure = rows;
    set(&mut failure[1], "system", Value::Null);
    set(&mut failure[1], "admit", Value::Bool(false));
    set(&mut failure[1], "fail", Value::Int(1));
    let wire = encode("cbor2", &failure[..2]);
    assert_eq!(summary("--admission", "cbor2", &wire),
               (1, "{\"admit\":false,\"fail\":1,\"image\":\"unchecked\",\"system\":null}\n".into()));
}

#[test]
fn positional_admission_finishes_with_stdin_open_after_fragmented_writes() {
    use std::time::{Duration, Instant};

    let prefix = encode("cbor2", &sample()[..2]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .args(["boot-report", "--admission", "cbor2"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    for &byte in &prefix {
        input.write_all(&[byte]).unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("admission waited beyond the second complete observation for EOF");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(result.stdout, b"{\"admit\":true,\"image\":\"unchecked\"}\n");
    drop(input);
}
