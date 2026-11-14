//! The native guards against the published cases (`vectors/guard_cases.jsonl`, written by `pcpos-vectors`).
//! The same file is what other implementations are tested against; here it guards against unnoticed change.
use pcpos_core::acr::{self, Record, State, FORM};
use pcpos_core::ledger;
use pcpos_core::value::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn vectors(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors").join(name);
    // guard_cases.jsonl is generated and is not kept in the source archive: `cargo run --release --bin pcpos-vectors -- vectors` writes it.
    fs::read(&p).unwrap_or_else(|e| panic!("{}: {}. If this is guard_cases.jsonl, write it first: cargo run --release --bin pcpos-vectors -- vectors", p.display(), e))
}

/// In the case file the 9000-character text is written as the token "@LONG".
fn expand(v: &Value) -> Value {
    match v {
        Value::Str(s) if s == "@LONG" => Value::Str("A".repeat(9000)),
        Value::List(l) => Value::List(l.iter().map(expand).collect()),
        Value::Map(m) => Value::Map(m.iter().map(|(k, v)| (k.clone(), expand(v))).collect()),
        other => other.clone(),
    }
}

#[test]
fn the_scenario_register_replays_without_a_problem() {
    let raw = vectors("register_scenario.jsonl");
    let (events, head) = ledger::verify(&raw).unwrap();
    let want = Value::parse(&vectors("register_scenario.expected.json")).unwrap();
    assert_eq!(head, want.get("head").unwrap().as_str().unwrap());
    let (state, problems) = acr::fold(&events, true).unwrap();
    assert_eq!(problems, vec![], "every recorded event passes the native guards and has exactly the fields they compute");
    assert_eq!(state.events(), 82);
    // The same register rebuilt through the native record is byte-identical.
    let mut rec = Record::new();
    for ev in &events {
        let f = |k: &str| ev.get(k).unwrap().as_str().unwrap();
        rec.submit(f("ts"), f("actor"), f("auth"), f("kind"), ev.get("body").unwrap().clone()).unwrap_or_else(|e| panic!("{:?}", e));
    }
    assert_eq!(rec.ledger().bytes(), &raw[..]);
    let (reopened, problems) = Record::open(&raw).unwrap();
    assert!(problems.is_empty());
    assert_eq!(reopened.ledger().head(), head);
}

#[test]
fn the_exercise_register_of_the_research_baseline_replays_without_a_problem() {
    let (events, _) = ledger::verify(&vectors("register_fphpm_exercise.jsonl")).unwrap();
    let (state, problems) = acr::fold(&events, true).unwrap();
    assert_eq!(problems, vec![]);
    assert_eq!(state.reviews.len(), 1);
}

#[test]
fn the_native_guards_answer_as_the_published_cases_say() {
    let (events, _) = ledger::verify(&vectors("register_scenario.jsonl")).unwrap();
    // The state after each prefix.
    let mut states: Vec<State> = Vec::new();
    for n in 0..=events.len() {
        states.push(acr::fold(&events[..n], false).unwrap().0);
    }
    let raw = vectors("guard_cases.jsonl");
    let (mut total, mut accepted, mut form_cases) = (0, 0, 0);
    let mut differences: Vec<String> = Vec::new();
    let mut by_kind: BTreeMap<String, u32> = BTreeMap::new();
    for line in raw.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let case = Value::parse(line).unwrap();
        let n = case.get("n").unwrap().as_int().unwrap() as usize;
        let (kind, actor, body) = match case.get("e").and_then(Value::as_int) {
            Some(e) => {
                let ev = &events[e as usize];
                (ev.get("kind").unwrap().as_str().unwrap().to_string(), ev.get("actor").unwrap().as_str().unwrap().to_string(), ev.get("body").unwrap().clone())
            }
            None => (case.get("kind").unwrap().as_str().unwrap().to_string(), case.get("actor").unwrap().as_str().unwrap().to_string(), expand(case.get("body").unwrap())),
        };
        total += 1;
        *by_kind.entry(kind.clone()).or_default() += 1;
        let got = acr::guard(&states[n], &kind, &body, &actor);
        let want_accept = case.get("verdict").unwrap().as_str().unwrap() == "ACCEPT";
        let problem = match (&got, want_accept) {
            (Ok(out), true) => {
                accepted += 1;
                let want = case.get("out").map(expand).unwrap_or_else(|| body.clone());
                if *out != want { Some(format!("accepted by both, bodies differ: native {}", String::from_utf8_lossy(&out.canonical_json()))) } else { None }
            }
            (Err(r), false) => {
                let code = case.get("code").unwrap().as_int().unwrap() as u8;
                let clause = case.get("clause").and_then(Value::as_str);
                if clause == Some(FORM) || r.clause == Some(FORM) {
                    form_cases += 1; // a field of the wrong type: the verdict is compared, the clause is not
                    None
                } else if r.code != code || r.clause != clause {
                    Some(format!("refused, differently: now code {} clause {:?} ({}), published code {} clause {:?}", r.code, r.clause, r.message, code, clause))
                } else {
                    None
                }
            }
            (Ok(_), false) => Some("published as refused, now accepted".to_string()),
            (Err(r), true) => Some(format!("published as accepted, now refused: {}", r.message)),
        };
        if let Some(p) = problem {
            if differences.len() < 25 {
                differences.push(format!("n={} {} by {}: {}\n    {}", n, kind, actor, p, String::from_utf8_lossy(line).chars().take(400).collect::<String>()));
            } else {
                differences.push(String::new());
            }
        }
    }
    println!("cases {}, accepted by both {}, compared on verdict only {}", total, accepted, form_cases);
    assert!(total > 16000 && by_kind.len() >= 31);
    assert!(differences.is_empty(), "{} differences; the first:\n{}", differences.len(), differences.iter().filter(|d| !d.is_empty()).cloned().collect::<Vec<_>>().join("\n"));
}
