//! A linked directory below a method tree must not redirect clause lookup.
use pcpos_core::value::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const EXTERNAL: &str = "OUTSIDE_METHOD_MARKER";
const INSIDE: &str = "INSIDE_METHOD_MARKER";

fn fresh_root(label: &str) -> PathBuf {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!(
        "pcpos-linked-ancestor-{}-{now}-{label}", std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn content(marker: &str) -> String {
    format!(
        "# Test\n\n**Component:** `acr.core`, Version 1.0  \n\n## 1. Scope\n\n### 1.1 Read\n\n{marker}\n"
    )
}

fn native(method: &Path) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .arg("clause").arg(method).args(["core", "1.1"])
        .output().unwrap();
    (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap(),
     out.status.code().unwrap_or(-1))
}

fn mcp(root: &Path, method: &Path) -> (String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .arg("mcp").env("PCPOS_ROOT", root).env("PCPOS_METHOD", method)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(
        b"{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"arguments\":{\"module\":\"core\",\"section\":\"1.1\"},\"name\":\"acr_clause\"}}\n"
    ).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let data = String::from_utf8(out.stdout).unwrap();
    let parsed = Value::parse(data.lines().next().unwrap().as_bytes()).unwrap();
    let result = parsed.get("result").unwrap();
    let text = match result.get("content") {
        Some(Value::List(c)) => c[0].get("text").and_then(Value::as_str).unwrap().to_string(),
        _ => panic!("MCP tool returned no text"),
    };
    (text, result.get("isError") == Some(&Value::Bool(true)))
}

#[test]
fn ordinary_module_directory_still_serves_exact_text() {
    let root = fresh_root("ordinary");
    let method = root.join("method");
    let spec = method.join("MODULES/01_Core/SOURCE/Specification.md");
    fs::create_dir_all(spec.parent().unwrap()).unwrap();
    fs::write(spec, content(INSIDE)).unwrap();

    let (reply, err, code) = native(&method);
    assert_eq!(code, 0, "{err}");
    assert!(reply.contains(INSIDE));
    let (reply, is_error) = mcp(&root, &method);
    assert!(!is_error && reply.contains(INSIDE));
    assert!(!root.join(".pcp/register.jsonl").exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn linked_module_ancestors_are_not_followed_by_cli_or_mcp() {
    use std::os::unix::fs::symlink;
    for case in ["modules", "component", "source"] {
        let root = fresh_root(case);
        let method = root.join("method");
        let outside = root.join("outside");
        let outside_core = outside.join("01_Core");
        let outside_source = outside_core.join("SOURCE");
        fs::create_dir_all(&outside_source).unwrap();
        fs::write(outside_source.join("Specification.md"), content(EXTERNAL)).unwrap();

        match case {
            "modules" => {
                fs::create_dir_all(&method).unwrap();
                symlink(&outside, method.join("MODULES")).unwrap();
            }
            "component" => {
                let modules = method.join("MODULES");
                fs::create_dir_all(&modules).unwrap();
                symlink(&outside_core, modules.join("01_Core")).unwrap();
            }
            "source" => {
                let core = method.join("MODULES/01_Core");
                fs::create_dir_all(&core).unwrap();
                symlink(&outside_source, core.join("SOURCE")).unwrap();
            }
            _ => unreachable!(),
        }

        let (reply, err, code) = native(&method);
        assert_eq!(code, 6, "{case}: {err}");
        assert!(!reply.contains(EXTERNAL), "{case}: outside text was served");
        let (reply, is_error) = mcp(&root, &method);
        assert!(is_error, "{case}: MCP accepted a linked ancestor");
        assert!(!reply.contains(EXTERNAL), "{case}: MCP served outside text");
        assert!(!root.join(".pcp/register.jsonl").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
