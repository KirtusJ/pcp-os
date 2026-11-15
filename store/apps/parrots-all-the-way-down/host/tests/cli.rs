use pcpos_core::value::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static N: AtomicUsize = AtomicUsize::new(0);
struct File(PathBuf);
impl File {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "recursion-desk-{}-{}.redb",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn command(db: &Path, cmd: &str, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_parrots-all-the-way-down"));
    c.arg(cmd).arg(db).args(args);
    c
}
fn json(out: &Output) -> Value {
    let bytes = if out.status.success() {
        &out.stdout
    } else {
        &out.stderr
    };
    Value::parse(bytes).unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(bytes)))
}
fn ok(db: &Path, cmd: &str, args: &[&str]) -> Value {
    let out = command(db, cmd, args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    json(&out)
}
fn fail(db: &Path, cmd: &str, args: &[&str]) {
    let out = command(db, cmd, args).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(json(&out).get("error").is_some());
}

#[test]
fn durable_requests_survive_reopen_and_bind_the_entire_command() {
    let file = File::new();
    let db = &file.0;
    fail(db, "status", &[]);
    assert!(!db.exists());
    ok(db, "init", &["2", "4", "8"]);
    ok(db, "init", &["2", "4", "8"]);
    fail(db, "init", &["2", "4", "9"]);
    let label = "child \"one\"\n雪";
    let root = ok(db, "open", &["open-root", "root", "top"]);
    let child = ok(db, "open", &["open-child", "1", label]);
    assert_eq!(root.get("revision").and_then(Value::as_int), Some(1));
    assert_eq!(child.get("node").and_then(Value::as_int), Some(2));
    assert_eq!(
        ok(db, "get", &["2"]).get("label").and_then(Value::as_str),
        Some(label)
    );
    let prior = ok(db, "status", &[]);
    fail(db, "finish", &["finish-root", "1"]);
    fail(db, "open", &["open-child", "1", "changed"]);
    fail(db, "open", &["open-child", "root", label]);
    fail(db, "cancel", &["open-child", "2"]);
    fail(db, "open", &["zero-parent", "0", "bad"]);
    assert_eq!(ok(db, "status", &[]), prior);
    ok(db, "cancel", &["cancel-child", "2"]);
    ok(db, "finish", &["finish-root", "1"]);
    let retry = ok(db, "open", &["open-child", "1", label]);
    assert_eq!(retry.get("replayed"), Some(&Value::Bool(true)));
    assert_eq!(retry.get("revision"), child.get("revision"));
    assert_eq!(
        ok(db, "get", &["2"]).get("state").and_then(Value::as_str),
        Some("Cancelled")
    );
    assert_eq!(
        ok(db, "status", &[])
            .get("revision")
            .and_then(Value::as_int),
        Some(4)
    );
}

#[test]
fn independent_processes_cannot_satisfy_the_same_slot_twice() {
    fn race(db: &Path, requests: &[&str]) -> Vec<Output> {
        use std::process::Stdio;
        let mut children = Vec::new();
        for request in requests {
            children.push(
                command(db, "open", &[request, "root", "job"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
        }
        children
            .into_iter()
            .map(|child| child.wait_with_output().unwrap())
            .collect()
    }
    let file = File::new();
    let db = &file.0;
    ok(db, "init", &["1", "1", "6"]);
    let attempts = race(db, &["a", "b", "c", "d"]);
    assert_eq!(attempts.iter().filter(|r| r.status.success()).count(), 1);
    for lost in attempts.iter().filter(|r| !r.status.success()) {
        assert_eq!(
            json(lost).get("error").and_then(Value::as_str),
            Some("live node limit reached")
        );
    }
    ok(db, "finish", &["end", "1"]);
    let attempts = race(db, &["same", "same", "same", "same"]);
    assert!(attempts.iter().all(|r| r.status.success()));
    assert_eq!(
        attempts
            .iter()
            .map(json)
            .filter(|r| r.get("replayed") == Some(&Value::Bool(false)))
            .count(),
        1
    );
    assert!(attempts
        .iter()
        .map(json)
        .all(|r| r.get("node").and_then(Value::as_int) == Some(2)));
    assert_eq!(
        ok(db, "status", &[])
            .get("revision")
            .and_then(Value::as_int),
        Some(3)
    );
}

#[test]
fn abandoning_a_transaction_between_the_event_and_receipt_leaves_neither() {
    let file = File::new();
    let db = &file.0;
    ok(db, "init", &["2", "4", "8"]);
    {
        // Exercise the durable on-disk boundary where the event exists in the writer's own
        // transaction, but the matching request and the commit have yet to happen.
        let database = redb::Database::open(db).unwrap();
        let write = database.begin_write().unwrap();
        let events = redb::TableDefinition::<u64, (u8, u32, &str, u32)>::new("parrots.events.v1");
        write
            .open_table(events)
            .unwrap()
            .insert(1, (1, 0, "abandoned", 1))
            .unwrap();
    }
    assert_eq!(
        ok(db, "status", &[])
            .get("revision")
            .and_then(Value::as_int),
        Some(0)
    );
    let receipt = ok(db, "open", &["retry", "root", "new"]);
    assert_eq!(receipt.get("replayed"), Some(&Value::Bool(false)));
    assert_eq!(receipt.get("revision").and_then(Value::as_int), Some(1));
    assert_eq!(
        ok(db, "get", &["1"]).get("label").and_then(Value::as_str),
        Some("new")
    );
}
