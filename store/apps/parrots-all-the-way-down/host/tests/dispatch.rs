use parrots_all_the_way_down::{get, init, status, State};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const DISPATCH: &str = env!("CARGO_BIN_EXE_pcp-delegate");

struct Arena(PathBuf);
impl Drop for Arena {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn arena(test: &str, depth: u16, active: u16, total: u32) -> (Arena, PathBuf) {
    let base = std::env::temp_dir().join(format!("parrots-{test}-{}", std::process::id()));
    fs::create_dir(&base).unwrap();
    let file = base.join("desk.redb");
    init(&file, depth, active, total).unwrap();
    (Arena(base), file)
}
fn command(file: &Path, request: &str) -> Command {
    let mut c = dispatch();
    c.arg(file).args([request, "synthetic worker", "--"]);
    c
}
fn dispatch() -> Command {
    let mut c = Command::new(DISPATCH);
    for key in [
        "PCP_RECURSION_DESK",
        "PCP_RECURSION_PARENT",
        "PCP_DELEGATE_REGISTER",
        "PCP_DELEGATE_TASK",
        "PCP_DELEGATE_ACTOR",
        "PARROTS_TEST_SENTINEL",
    ] {
        c.env_remove(key);
    }
    c
}
fn ok(out: Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn nested_process_inherits_parent_and_duplicate_launch_is_refused() {
    let (_a, db) = arena("nested", 1, 2, 3);
    ok(command(&db, "root")
        .arg(DISPATCH)
        .args(["-", "child", "leaf", "--", DISPATCH, "--help"])
        .output()
        .unwrap());
    let nodes = status(&db).unwrap().nodes;
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[1].parent, Some(nodes[0].id));
    assert!(nodes.iter().all(|n| n.state == State::Finished));
    assert!(!command(&db, "root")
        .arg(DISPATCH)
        .arg("--help")
        .status()
        .unwrap()
        .success());
    assert_eq!(status(&db).unwrap().nodes.len(), 2);
}

#[test]
fn limits_and_spawn_failure_release_the_live_slot() {
    let (a, db) = arena("limits", 0, 1, 3);
    let refused = command(&db, "root")
        .arg(DISPATCH)
        .args(["-", "child", "leaf", "--", DISPATCH, "--help"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(status(&db).unwrap().nodes.len(), 1);
    assert_eq!(get(&db, 1).unwrap().state, State::Cancelled);
    assert!(!command(&db, "missing")
        .arg(a.0.join("missing-worker"))
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(get(&db, 2).unwrap().state, State::Cancelled);
    ok(command(&db, "last")
        .arg(DISPATCH)
        .arg("--help")
        .output()
        .unwrap());
    assert!(!command(&db, "over-total")
        .arg(DISPATCH)
        .arg("--help")
        .output()
        .unwrap()
        .status
        .success());
}

// This test binary doubles as a controllable external worker.
#[test]
fn slow_child() {
    if let Some(marker) = std::env::var_os("PARROTS_TEST_SENTINEL") {
        let marker = PathBuf::from(marker);
        let lease = fs::File::create(marker.with_extension("lock")).unwrap();
        lease.lock().unwrap();
        fs::write(&marker, b"started").unwrap();
        let start = Instant::now();
        while !marker.with_extension("leave").exists() && start.elapsed() < Duration::from_secs(20)
        {
            thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn cancellation_reaps_the_direct_worker_and_replay_does_not_spawn() {
    let (a, db) = arena("cancel", 1, 3, 4);
    let ready = a.0.join("ready");
    let exe = std::env::current_exe().unwrap();
    let mut running = command(&db, "active")
        .arg(exe)
        .args(["--exact", "slow_child"])
        .env("PARROTS_TEST_SENTINEL", &ready)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() && start.elapsed() < Duration::from_secs(10) {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(ready.exists());
    assert!(!command(&db, "active")
        .arg(DISPATCH)
        .arg("--help")
        .output()
        .unwrap()
        .status
        .success());
    parrots_all_the_way_down::cancel(&db, "operator", 1).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while running.try_wait().unwrap().is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(running.try_wait().unwrap().is_some_and(|s| !s.success()));
    assert_eq!(status(&db).unwrap().nodes.len(), 1);
    assert!(fs::File::open(ready.with_extension("lock"))
        .unwrap()
        .try_lock()
        .is_ok());
}

#[test]
fn cascading_cancellation_lets_a_nested_adapter_reap_its_own_worker() {
    let (a, db) = arena("cascade", 2, 4, 5);
    let ready = a.0.join("nested-ready");
    let mut running = command(&db, "outer")
        .arg(DISPATCH)
        .args(["-", "inner", "leaf", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "slow_child"])
        .env("PARROTS_TEST_SENTINEL", &ready)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() && start.elapsed() < Duration::from_secs(10) {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(ready.exists());
    parrots_all_the_way_down::cancel(&db, "operator", 1).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while running.try_wait().unwrap().is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    let exited = running.try_wait().unwrap().is_some_and(|s| !s.success());
    let stopped = fs::File::open(ready.with_extension("lock"))
        .unwrap()
        .try_lock()
        .is_ok();
    fs::write(ready.with_extension("leave"), b"").unwrap();
    assert!(
        exited && stopped,
        "dispatcher exited={exited}, leaf process released its lock={stopped}"
    );
    assert!(status(&db)
        .unwrap()
        .nodes
        .iter()
        .all(|n| n.state == State::Cancelled));
}

#[test]
#[ignore = "requires a separately built compatible pcpos; set PCPOS_BIN and run with --include-ignored"]
fn register_admission_precedes_launch_and_releases_the_exact_task() {
    use pcpos_core::{ledger, value::Value};
    let host = std::env::var_os("PCPOS_BIN").expect("set PCPOS_BIN to the built pcpos executable");
    let (a, _) = arena("register", 1, 2, 5);
    let reg = a.0.join("review.jsonl");
    let actor = "director";
    ok(Command::new(&host)
        .arg("init")
        .arg(&reg)
        .args(["--principal", actor])
        .output()
        .unwrap());
    let invoke = |label: &str| {
        let mut c = dispatch();
        c.env("PCPOS_BIN", &host)
            .arg("--register")
            .arg(&reg)
            .args([actor, label, "--"]);
        c
    };
    let sentinel = a.0.join("register-worker");
    let before = fs::read(&reg).unwrap();
    let denied = invoke("refused")
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "slow_child"])
        .env("PARROTS_TEST_SENTINEL", &sentinel)
        .output()
        .unwrap();
    assert!(
        !denied.status.success()
            && String::from_utf8_lossy(&denied.stderr).contains("set task limits")
    );
    assert!(!sentinel.exists());
    assert_eq!(fs::read(&reg).unwrap(), before);
    ok(Command::new(&host)
        .arg("task-limits")
        .arg(&reg)
        .args([
            "--by",
            actor,
            "--max-depth",
            "1",
            "--max-live",
            "2",
            "--max-total",
            "3",
        ])
        .output()
        .unwrap());
    ok(invoke("outer")
        .arg(DISPATCH)
        .args(["--register", "-", actor, "inner", "--", DISPATCH, "--help"])
        .output()
        .unwrap());
    let missing = invoke("missing")
        .arg(a.0.join("no-such-worker"))
        .output()
        .unwrap();
    assert!(
        !missing.status.success()
            && String::from_utf8_lossy(&missing.stderr).contains("could not start worker")
    );
    let denied = invoke("over").arg(DISPATCH).arg("--help").output().unwrap();
    assert!(
        !denied.status.success()
            && String::from_utf8_lossy(&denied.stderr).contains("total node limit")
    );
    let events = ledger::verify(&fs::read(&reg).unwrap()).unwrap().0;
    let mut transitions = Vec::new();
    for event in &events {
        let kind = event.get("kind").and_then(Value::as_str).unwrap();
        if matches!(kind, "TASK_ADMITTED" | "TASK_RELEASED") {
            let body = event.get("body").unwrap();
            transitions.push((
                kind,
                body.get("id").and_then(Value::as_str).unwrap(),
                body.get("parent").and_then(Value::as_str),
            ));
        }
    }
    assert_eq!(
        transitions,
        [
            ("TASK_ADMITTED", "T1", None),
            ("TASK_ADMITTED", "T2", Some("T1")),
            ("TASK_RELEASED", "T2", None),
            ("TASK_RELEASED", "T1", None),
            ("TASK_ADMITTED", "T3", None),
            ("TASK_RELEASED", "T3", None)
        ]
    );
    let verified = Command::new(host).arg("verify").arg(reg).output().unwrap();
    assert!(String::from_utf8_lossy(&verified.stdout).contains("guards replayed: 0 problem(s)"));
    ok(verified);
}
