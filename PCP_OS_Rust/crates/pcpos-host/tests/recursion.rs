use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};

fn cli(reg: &Path, cmd: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pcpos")).arg(cmd).arg(reg).args(args).output().unwrap()
}

fn assert_ok(o: Output) -> String {
    assert!(o.status.success(), "stdout: {} stderr: {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn competing_processes_cannot_both_reserve_the_last_slot() {
    let name = format!("pcpos-admission-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
    let dir: PathBuf = env::temp_dir().join(name);
    fs::create_dir(&dir).unwrap();
    let reg = dir.join("register.jsonl");
    assert_ok(cli(&reg, "init", &["--principal","owner"]));
    assert_ok(cli(&reg, "task-limits", &["--by","owner","--max-depth","1","--max-live","2","--max-total","3"]));
    let first = assert_ok(cli(&reg, "task-admit", &["--by","owner","--label","root"]));
    assert_eq!(first.lines().next(), Some("ADMITTED T1"));

    let bound = |own: &str, cmd: &str, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_pcpos")).env("PCP_DELEGATE_REGISTER",&reg)
            .env("PCP_DELEGATE_ACTOR","owner").env("PCP_DELEGATE_TASK",own)
            .arg(cmd).arg(&reg).args(args).output().unwrap()
    };
    let old = fs::read(&reg).unwrap();
    assert!(!bound("T1","task-limits",&["--by","owner","--max-depth","10","--max-live","100","--max-total","1000"]).status.success());
    assert!(!bound("T1","task-release",&["--by","owner","--id","T1"]).status.success());
    assert!(!bound("T1","task-admit",&["--by","owner","--parent","T99","--label","graft"]).status.success());
    assert!(!cli(&reg, "task-admit", &["--by","owner","--label","typo","--parnt","T1"]).status.success());
    assert_eq!(fs::read(&reg).unwrap(), old);
    let start = |label| Command::new(env!("CARGO_BIN_EXE_pcpos")).arg("task-admit").arg(&reg)
        .args(["--by","owner","--parent","T1","--label",label])
        .stdout(std::process::Stdio::piped()).spawn().unwrap();
    let a = start("first");
    let b = start("second");
    let outputs = [a.wait_with_output().unwrap(), b.wait_with_output().unwrap()];
    assert_eq!(outputs.iter().filter(|o| o.status.success()).count(), 1);
    assert!(outputs.iter().any(|o| o.status.code() == Some(3)));
    assert!(!cli(&reg, "task-release", &["--by","owner","--id","T1"]).status.success());
    assert!(!bound("T2","task-admit",&["--by","owner","--label","flatten"]).status.success());
    assert_ok(bound("T1","task-release",&["--by","owner","--id","T2"]));
    // Omitting --parent must inherit T1 even in the low-level CLI.
    assert_ok(bound("T1","task-admit",&["--by","owner","--label","last"]));
    assert!(fs::read_to_string(&reg).unwrap().lines().last().unwrap().contains(r#""parent":"T1""#));
    assert_ok(cli(&reg, "task-release", &["--by","owner","--id","T3"]));
    assert_ok(cli(&reg, "task-release", &["--by","owner","--id","T1"]));
    let old = fs::read(&reg).unwrap();
    assert_eq!(cli(&reg, "task-admit", &["--by","owner","--label","extra"]).status.code(), Some(3));
    assert_eq!(fs::read(&reg).unwrap(), old);
    assert_ok(cli(&reg, "verify", &[]));
    assert!(assert_ok(cli(&reg, "status", &[])).contains("live 0/2, lifetime 3/3"));
    fs::remove_file(reg).unwrap();
    fs::remove_dir(dir).unwrap();
}

#[test]
fn inherited_mcp_offers_and_raw_submissions_use_the_actual_parent() {
    use std::{io::Write, process::Stdio};
    let name = format!("pcpos-bound-mcp-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
    let dir = env::temp_dir().join(name);
    fs::create_dir(&dir).unwrap();
    let reg = dir.join("register.jsonl");
    assert_ok(cli(&reg, "init", &["--principal","owner"]));
    assert_ok(cli(&reg, "task-limits", &["--by","owner","--max-depth","0","--max-live","3","--max-total","6"]));

    let call = |tool: &str, args: &str, bound: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pcpos"));
        cmd.arg("mcp").current_dir(&dir).env("PCPOS_ROOT",&dir).env("PCPOS_REGISTER","register.jsonl")
            .env("PCPOS_ACTOR","agent").env("PCPOS_PRINCIPAL","owner")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if bound {
            cmd.env("PCP_DELEGATE_REGISTER",&reg).env("PCP_DELEGATE_TASK","T1").env("PCP_DELEGATE_ACTOR","agent");
        }
        let mut proc = cmd.spawn().unwrap();
        writeln!(proc.stdin.take().unwrap(),r#"{{"id":1,"jsonrpc":"2.0","method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#).unwrap();
        let output = assert_ok(proc.wait_with_output().unwrap());
        let reply = pcpos_core::value::Value::parse(output.trim().as_bytes()).unwrap();
        let content = reply.get("result").unwrap().get("content").unwrap();
        let pcpos_core::value::Value::List(items) = content else {panic!("expected MCP content")};
        items[0].get("text").unwrap().as_str().unwrap().to_string()
    };
    call("acr_begin","{}", false);
    assert!(call("acr_task_admit",r#"{"label":"root"}"#,false).starts_with("ADMITTED T1"));
    let bytes = fs::read(&reg).unwrap();
    for tool in ["acr_offer", "acr_submit"] {
        let reply = call(tool,r#"{"kind":"TASK_ADMITTED","body":{"label":"newroot","parent":null}}"#,true);
        assert!(reply.contains("depth limit reached"),"{reply}");
    }
    assert!(call("acr_task_release",r#"{"id":"T1"}"#,true).contains("direct children's reservations"));
    assert_eq!(fs::read(&reg).unwrap(), bytes);
    assert!(call("acr_task_release",r#"{"id":"T1"}"#,false).starts_with("RELEASED T1"));
    fs::remove_file(reg).unwrap();
    fs::remove_dir(dir).unwrap();
}
