//! D11 synthetic contract audit: exact Rust core::ledger::verify, no guest.
use std::fs;
use std::path::Path;

#[test]
fn source_pinned_synthetic_reader_grammar_controls() {
    let root = std::env::var("D11_VECTOR_DIR").expect("need bounded fixture directory");
    let index = fs::read_to_string(Path::new(&root).join("index.tsv")).unwrap();
    let mut total = 0usize;
    let mut excluded_empty = 0usize;
    for entry in index.lines() {
        let fields: Vec<_> = entry.split('\t').collect();
        assert_eq!(fields.len(), 3, "malformed source-pinned fixture index");
        let (name, expected_accepted, expected_head) = (fields[0], fields[1] == "1", fields[2]);
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
        let raw = fs::read(Path::new(&root).join(format!("{name}.bin"))).unwrap();
        let got = pcpos_core::ledger::verify(&raw);
        assert_eq!(
            got.is_ok(), expected_accepted,
            "Rust ledger verify unexpectedly differed for {name}: {got:?}"
        );
        if expected_accepted {
            let (_, head) = got.unwrap();
            assert_eq!(head, expected_head, "Rust head mismatch for {name}");
        }
        if name == "empty_chain_scope_difference" {
            // Intentional: Rust ledger accepts [], D10 snapshot reader requires
            // a nonempty newline-terminated register. Not a false accept.
            assert!(raw.is_empty());
            assert!(expected_accepted);
            excluded_empty += 1;
        }
        total += 1;
    }
    assert!(total >= 22, "missing expected adversarial corpus");
    assert_eq!(excluded_empty, 1, "expected explicit empty-chain exception");
    println!("D11 bounded Rust original-source verifier passed {total} vectors");
}
