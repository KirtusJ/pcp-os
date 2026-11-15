use parrots_all_the_way_down::{admit, cancel, finish, get, init, status, Node, Receipt};
use pcpos_core::value::Value;
use std::{env, path::Path, str::FromStr};

const HELP: &str = "Parrots all the way Down
  parrots-all-the-way-down init FILE MAX_DEPTH MAX_LIVE MAX_TOTAL
  parrots-all-the-way-down open FILE REQUEST root|PARENT LABEL
  parrots-all-the-way-down finish FILE REQUEST NODE
  parrots-all-the-way-down cancel FILE REQUEST NODE
  parrots-all-the-way-down get FILE NODE
  parrots-all-the-way-down status FILE

Roots are at depth zero. Node IDs start at one. A mutation needs a request ID.
Retry with exactly the same ID and arguments to retrieve its original receipt.
Finish children before their parent. Cancel closes the active subtree.
Status reports the last committed revision. Each successful mutation produces one revision.";

fn number<T: FromStr>(arg: &str, field: &str) -> Result<T, String> {
    arg.parse().map_err(|_| format!("invalid {field}"))
}
fn quote(s: &str) -> String {
    String::from_utf8(Value::Str(s.into()).canonical_json()).expect("canonical JSON is UTF-8")
}
fn node(n: &Node) -> String {
    let parent = n
        .parent
        .map_or_else(|| "null".to_owned(), |p| p.to_string());
    let label = quote(&n.label);
    format!(
        r#"{{"id":{},"parent":{parent},"label":{label},"state":"{:?}"}}"#,
        n.id, n.state
    )
}
fn receipt(r: Receipt) -> String {
    format!(
        r#"{{"node":{},"revision":{},"replayed":{}}}"#,
        r.node, r.revision, r.replayed
    )
}
fn run(args: &[String]) -> Result<String, String> {
    match args {
        [] => Ok(HELP.into()),
        [cmd] if cmd == "--help" || cmd == "help" => Ok(HELP.into()),
        [cmd, path, d, l, t] if cmd == "init" => {
            init(
                Path::new(path),
                number(d, "depth")?,
                number(l, "live limit")?,
                number(t, "total limit")?,
            )?;
            Ok(r#"{"initialized":true}"#.into())
        }
        [cmd, path, req, parent, label] if cmd == "open" => {
            let parent = if parent == "root" {
                None
            } else {
                Some(number(parent, "parent ID")?)
            };
            admit(Path::new(path), req, parent, label).map(receipt)
        }
        [cmd, path, req, id] if cmd == "finish" => {
            finish(Path::new(path), req, number(id, "node ID")?).map(receipt)
        }
        [cmd, path, req, id] if cmd == "cancel" => {
            cancel(Path::new(path), req, number(id, "node ID")?).map(receipt)
        }
        [cmd, path, id] if cmd == "get" => {
            get(Path::new(path), number(id, "node ID")?).map(|n| node(&n))
        }
        [cmd, path] if cmd == "status" => {
            let s = status(Path::new(path))?;
            let nodes = s.nodes.iter().map(node).collect::<Vec<_>>().join(",");
            Ok(format!(
                r#"{{"max_depth":{},"max_live":{},"max_total":{},"revision":{},"nodes":[{nodes}]}}"#,
                s.max_depth, s.max_live, s.max_total, s.revision
            ))
        }
        _ => Err("unrecognized command or arguments; use --help".into()),
    }
}
fn main() {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!(r#"{{"error":{}}}"#, quote(&error));
            std::process::exit(2);
        }
    }
}
