//! Exact provision retrieval at the 60,000-character output boundary.
//! The CLI and MCP must not describe a partial preview as an exact provision.
use pcpos_core::{sha256_hex, value::Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(chars: usize) -> (PathBuf, PathBuf, String) {
    const PREFIX: &str = "### 1.1 Payload\n\n";
    assert!(chars >= PREFIX.chars().count());
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!(
        "pcpos-exact-clause-{}-{stamp}", std::process::id()
    ));
    let method = root.join("method");
    let source = method.join("MODULES/01_Core/SOURCE/Specification.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    let body = format!("{PREFIX}{}", "X".repeat(chars - PREFIX.chars().count()));
    assert_eq!(body.chars().count(), chars);
    let spec = format!(
        "# T\n\n**Component:** `acr.core`, Version 1.0  \n\n## 1. Boundary\n\n{body}\n\n## 2. End\n"
    );
    fs::write(&source, spec).unwrap();
    (root, method, body)
}

fn cli(method: &Path) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .arg("clause").arg(method).args(["core", "1.1"])
        .output().unwrap();
    (String::from_utf8(output.stdout).unwrap(),
     String::from_utf8(output.stderr).unwrap(),
     output.status.code().unwrap_or(-1))
}

fn mcp(root: &Path, method: &Path) -> (String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcpos"))
        .arg("mcp").env("PCPOS_ROOT", root).env("PCPOS_METHOD", method)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    let query = b"{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"arguments\":{\"module\":\"core\",\"section\":\"1.1\"},\"name\":\"acr_clause\"}}\n";
    child.stdin.take().unwrap().write_all(query).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let response = String::from_utf8(output.stdout).unwrap();
    let parsed = Value::parse(response.lines().next().unwrap().as_bytes()).unwrap();
    let result = parsed.get("result").expect("MCP tool returned no result");
    let text = match result.get("content") {
        Some(Value::List(c)) => c[0].get("text").and_then(Value::as_str).unwrap().to_owned(),
        _ => panic!("MCP tool returned no text"),
    };
    (text, result.get("isError") == Some(&Value::Bool(true)))
}

#[test]
fn exact_clause_at_display_limit_has_its_actual_text_and_digest() {
    let (root, method, body) = fixture(60_000);
    let digest = sha256_hex(format!("{body}\n").as_bytes());
    let (text, err, code) = cli(&method);
    assert_eq!(code, 0, "{err}");
    assert!(text.contains("PROVISION 1.1"));
    assert!(text.contains(&format!("SHA-256 of this text {digest}")));
    assert!(text.ends_with(&format!("\n\n{body}\n")));
    assert!(!text.contains("[cut at"), "exact lookup was clipped");

    let (mcp_text, is_error) = mcp(&root, &method);
    assert!(!is_error, "MCP unexpectedly rejected an exact-size clause");
    assert!(mcp_text.contains(&format!("SHA-256 of this text {digest}")));
    assert!(mcp_text.ends_with(&format!("\n\n{body}\n")));
    assert!(!root.join(".pcp/register.jsonl").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn oversized_clause_is_refused_instead_of_being_labelled_exact() {
    let (root, method, body) = fixture(60_001);
    let digest = sha256_hex(format!("{body}\n").as_bytes());
    let (text, err, code) = cli(&method);
    assert_eq!(code, 6, "unexpected exit, stderr: {}", err.chars().take(200).collect::<String>());
    assert!(err.contains("beyond the exact clause output limit"), "{err}");
    assert!(err.contains("no provision text was served"), "{err}");
    assert!(!text.contains("PROVISION 1.1"));
    assert!(!text.contains(&digest));
    assert!(!text.contains("[cut at"));

    let (mcp_text, is_error) = mcp(&root, &method);
    assert!(is_error, "MCP treated a partial clause as an exact result");
    assert!(mcp_text.contains("beyond the exact clause output limit"));
    assert!(mcp_text.contains("no provision text was served"));
    assert!(!mcp_text.contains("PROVISION 1.1"));
    assert!(!root.join(".pcp/register.jsonl").exists());
    fs::remove_dir_all(root).unwrap();
}
