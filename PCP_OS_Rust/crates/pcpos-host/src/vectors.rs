//! pcpos-vectors: write the cross-implementation vectors from the native implementation.
//!
//!     pcpos-vectors <vectors directory>
//!
//! The native implementation is the authority. From `scenario_inputs.jsonl` (events as offered, before any
//! guard has filled in a field) this program writes:
//!
//!   register_scenario.jsonl      the register the native record builds from those inputs
//!   guard_cases.jsonl            many (prefix of that register, offered event) pairs with the native
//!                                answer: ACCEPT and the body written, or REFUSE with code and clause
//!   machines.tsv                 every state machine's whole table
//!
//! Other implementations are tested against these files. The cases are every event of the scenario offered
//! at every prefix, every single-field mutation of every event at its own position, and each event offered
//! by other actors.
#![forbid(unsafe_code)]
use pcpos_core::acr::{self, Record, State};
use pcpos_core::value::Value;
use pcpos_core::{ledger, machine};
use std::collections::{BTreeMap, BTreeSet};
use std::{env, fs, process::exit};

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn subs() -> Vec<Value> {
    let m = |k: &str, v: &str| Value::Map([(k.to_string(), s(v))].into_iter().collect());
    vec![Value::Null, Value::Bool(true), Value::Bool(false), Value::Int(0), Value::Int(-1), Value::Int(7), s(""), s("x"), s(" G1 "), s("bad\u{7}"),
         Value::List(vec![]), Value::Map(BTreeMap::new()), Value::List(vec![s("x")]), m("t", "x"), s("@LONG")]
}

fn with(m: &BTreeMap<String, Value>, k: &str, v: Value) -> Value {
    let mut n = m.clone();
    n.insert(k.to_string(), v);
    Value::Map(n)
}

/// Every value obtained by dropping or replacing one field, at any depth.
fn mutations(v: &Value, skip_id: bool, out: &mut Vec<Value>) {
    match v {
        Value::Map(m) => {
            for (k, x) in m {
                if skip_id && k == "id" {
                    continue;
                }
                let mut rest = m.clone();
                rest.remove(k);
                out.push(Value::Map(rest));
                for sub in subs() {
                    if sub != *x {
                        out.push(with(m, k, sub));
                    }
                }
                let mut inner = Vec::new();
                mutations(x, false, &mut inner);
                out.extend(inner.into_iter().map(|i| with(m, k, i)));
            }
            out.push(with(m, "unexpected", s("x")));
        }
        Value::List(l) => {
            for (i, item) in l.iter().enumerate() {
                let mut a = l.clone();
                a.remove(i);
                out.push(Value::List(a));
                let mut b = l.clone();
                b.push(item.clone());
                out.push(Value::List(b));
                let mut inner = Vec::new();
                mutations(item, false, &mut inner);
                for sub in inner {
                    let mut c = l.clone();
                    c[i] = sub;
                    out.push(Value::List(c));
                }
            }
        }
        _ => {}
    }
}

/// In the case file the 9000-character text is the token "@LONG"; it is expanded before use.
fn expand(v: &Value) -> Value {
    match v {
        Value::Str(x) if x == "@LONG" => Value::Str("A".repeat(9000)),
        Value::List(l) => Value::List(l.iter().map(expand).collect()),
        Value::Map(m) => Value::Map(m.iter().map(|(k, v)| (k.clone(), expand(v))).collect()),
        other => other.clone(),
    }
}

fn shrink(v: &Value) -> Value {
    match v {
        Value::Str(x) if x.len() == 9000 && x.bytes().all(|b| b == b'A') => s("@LONG"),
        Value::List(l) => Value::List(l.iter().map(shrink).collect()),
        Value::Map(m) => Value::Map(m.iter().map(|(k, v)| (k.clone(), shrink(v))).collect()),
        other => other.clone(),
    }
}

fn main() {
    let dir = match env::args().nth(1) {
        Some(d) => d,
        None => {
            eprintln!("usage: pcpos-vectors <vectors directory>");
            exit(2);
        }
    };
    let read = |n: &str| fs::read(format!("{dir}/{n}")).unwrap_or_else(|e| panic!("{n}: {e}"));
    // 1. The scenario, through the native record.
    let mut rec = Record::new();
    for line in read("scenario_inputs.jsonl").split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let i = Value::parse(line).expect("input line");
        let f = |k: &str| i.get(k).and_then(Value::as_str).expect("input field");
        rec.submit(f("ts"), f("actor"), f("auth"), f("kind"), i.get("body").expect("body").clone()).unwrap_or_else(|e| panic!("{}: {:?}", f("kind"), e));
    }
    let raw = rec.ledger().bytes().to_vec();
    let (events, head) = ledger::verify(&raw).expect("the register just written verifies");
    let (_, problems) = acr::fold(&events, true).expect("fold");
    assert!(problems.is_empty(), "{problems:?}");
    fs::write(format!("{dir}/register_scenario.jsonl"), &raw).expect("write");
    let mut exp = BTreeMap::new();
    exp.insert("events".to_string(), Value::Int(events.len() as i64));
    exp.insert("head".to_string(), s(&head));
    let mut text = Value::Map(exp).canonical_json();
    text.push(b'\n');
    fs::write(format!("{dir}/register_scenario.expected.json"), text).expect("write");
    // 2. The cases.
    let states: Vec<State> = (0..=events.len()).map(|n| acr::fold(&events[..n], false).expect("fold").0).collect();
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut lines: Vec<u8> = Vec::new();
    let (mut total, mut accepted) = (0u32, 0u32);
    let mut offer = |n: usize, kind: &str, body: &Value, actor: &str, e: Option<usize>| {
        let key = Value::List(vec![Value::Int(n as i64), s(kind), body.clone(), s(actor)]).canonical_json();
        if !seen.insert(key) {
            return;
        }
        let mut case = BTreeMap::new();
        case.insert("n".to_string(), Value::Int(n as i64));
        match e {
            Some(e) => {
                case.insert("e".to_string(), Value::Int(e as i64));
            }
            None => {
                case.insert("kind".to_string(), s(kind));
                case.insert("actor".to_string(), s(actor));
                case.insert("body".to_string(), shrink(body));
            }
        }
        match acr::guard(&states[n], kind, &expand(body), actor) {
            Ok(out) => {
                accepted += 1;
                case.insert("verdict".to_string(), s("ACCEPT"));
                if out != expand(body) {
                    case.insert("out".to_string(), shrink(&out));
                }
            }
            Err(r) => {
                case.insert("verdict".to_string(), s("REFUSE"));
                case.insert("code".to_string(), Value::Int(r.code as i64));
                case.insert("clause".to_string(), r.clause.map(s).unwrap_or(Value::Null));
            }
        }
        total += 1;
        lines.extend(Value::Map(case).canonical_json());
        lines.push(b'\n');
    };
    for (i, ev) in events.iter().enumerate() {
        let (kind, actor, body) = (ev.get("kind").unwrap().as_str().unwrap(), ev.get("actor").unwrap().as_str().unwrap(), ev.get("body").unwrap());
        for n in 0..=events.len() {
            offer(n, kind, body, actor, Some(i));
        }
        let mut m = Vec::new();
        mutations(body, matches!(kind, "PARTICIPANT_ADDED" | "REVIEW_COMMISSIONED"), &mut m);
        for b in &m {
            offer(i, kind, b, actor, None);
        }
        for a in ["nobody", "host", "kirtus", "z", "runner"] {
            offer(i, kind, body, a, None);
        }
    }
    // Forms that a loosely anchored pattern, or a truth test, would accept.
    let h = |t: &str| pcpos_core::sha256_hex(t.as_bytes());
    let obj = |pairs: &[(&str, Value)]| Value::Map(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect());
    offer(1, "PARTICIPANT_ADDED", &obj(&[("id", s("acr:participant:n")), ("label", s("newline\n")), ("kind", s("HUMAN"))]), "host", None);
    offer(1, "PARTICIPANT_ADDED", &obj(&[("id", s("acr:participant:n")), ("label", s("p2")), ("kind", s("HUMAN")), ("principal", s("false"))]), "host", None);
    offer(7, "ARTIFACT_ADDED", &obj(&[("sha256", s(&(h("a") + "\n"))), ("size", Value::Int(1)), ("origin", s("SUPPLIED"))]), "kirtus", None);
    offer(7, "ARTIFACT_ADDED", &obj(&[("sha256", s(&h("a"))), ("size", Value::Bool(true)), ("origin", s("SUPPLIED"))]), "kirtus", None);
    offer(7, "TREE_PINNED", &obj(&[("name", s("t")), ("manifest", s(&(h("m") + "\n"))), ("origin", s("SUPPLIED"))]), "kirtus", None);
    offer(3, "UNDEFINED_KIND", &obj(&[]), "host", None);
    offer(3, "X_LOCAL", &obj(&[("a", Value::Int(1))]), "kirtus", None);
    // What a review produced for itself is not evidence it checked (Core 4.3, 5.2); how a check came to be in the record.
    let body_of = |i: usize| match events[i].get("body") { Some(Value::Map(m)) => m.clone(), _ => panic!("event {i} has no body") };
    let digest_of = |i: usize| events[i].get("body").and_then(|b| b.get("sha256")).and_then(Value::as_str).expect("artifact digest").to_string();
    let at = |kind: &str, n: usize| (0..events.len()).filter(|i| events[*i].get("kind").and_then(Value::as_str) == Some(kind)).nth(n).expect("event of that kind");
    let item = |d: &str| obj(&[("ref", obj(&[("t", s("artifact")), ("sha256", s(d))])), ("extent", s("all of it"))]);
    let (paper, product) = (digest_of(at("ARTIFACT_ADDED", 0)), (0..events.len()).find(|i| events[*i].get("body").and_then(|b| b.get("origin")).and_then(Value::as_str) == Some("PRODUCED")).map(digest_of).expect("a produced artifact"));
    let refuted = (0..events.len()).find(|i| events[*i].get("body").and_then(|b| b.get("disposition")).and_then(Value::as_str) == Some("REFUTED_IN_SCOPE")).expect("a refutation");
    let who = events[refuted].get("actor").and_then(Value::as_str).expect("actor").to_string();
    let f = body_of(refuted);
    let finding = |disp: &str, basis: Vec<Value>, contrary: Vec<Value>| with(&match with(&match with(&f, "disposition", s(disp)) { Value::Map(m) => m, _ => unreachable!() }, "basis", Value::List(basis)) { Value::Map(m) => m, _ => unreachable!() }, "contrary", Value::List(contrary));
    offer(refuted, "FINDING_RECORDED", &finding("REFUTED_IN_SCOPE", vec![], vec![item(&product)]), &who, None);
    offer(refuted, "FINDING_RECORDED", &finding("REFUTED_IN_SCOPE", vec![item(&paper)], vec![item(&product)]), &who, None);
    offer(refuted, "FINDING_RECORDED", &finding("REFUTED_IN_SCOPE", vec![], vec![item(&product), item(&paper)]), &who, None);
    offer(refuted, "FINDING_RECORDED", &finding("SUPPORTED_IN_SCOPE", vec![item(&product)], vec![]), &who, None);
    offer(refuted, "FINDING_RECORDED", &finding("SUPPORTED_IN_SCOPE", vec![item(&product), item(&paper)], vec![]), &who, None);
    offer(refuted, "FINDING_RECORDED", &finding("UNRESOLVED", vec![item(&product)], vec![]), &who, None);
    // The same product, cited in a review commissioned after it existed, is material that review was given.
    let later = at("REVIEW_COMMISSIONED", 2);
    let unresolved = (later..events.len()).find(|i| events[*i].get("body").and_then(|b| b.get("disposition")).and_then(Value::as_str) == Some("UNRESOLVED")).expect("a later finding");
    let (f2, who2) = (body_of(unresolved), events[unresolved].get("actor").and_then(Value::as_str).expect("actor").to_string());
    offer(unresolved, "FINDING_RECORDED", &with(&match with(&f2, "disposition", s("REFUTED_IN_SCOPE")) { Value::Map(m) => m, _ => unreachable!() }, "contrary", Value::List(vec![item(&product)])), &who2, None);
    // (The standard in force there lets a model refute only a mechanical claim, so the same finding is also offered by the person.)
    offer(unresolved, "FINDING_RECORDED", &with(&match with(&f2, "disposition", s("REFUTED_IN_SCOPE")) { Value::Map(m) => m, _ => unreachable!() }, "contrary", Value::List(vec![item(&product)])), "kirtus", None);
    let k = at("CHECK_RAN", 0);
    let (kb, kwho) = (body_of(k), events[k].get("actor").and_then(Value::as_str).expect("actor").to_string());
    for o in ["HOST_OBSERVED_TOOL", "RAN_BY_THIS_PROGRAM", "REPORTED_BY_PARTICIPANT", "OBSERVED", ""] {
        offer(k, "CHECK_RAN", &with(&kb, "observed", s(o)), &kwho, None);
    }
    offer(k, "CHECK_RAN", &with(&kb, "observed", Value::Bool(true)), &kwho, None);
    fs::write(format!("{dir}/guard_cases.jsonl"), &lines).expect("write");
    // 3. The machines.
    let mut tsv = String::new();
    machine::export(&mut tsv).expect("export");
    fs::write(format!("{dir}/machines.tsv"), tsv).expect("write");
    println!("scenario: {} events, head {}", events.len(), head);
    println!("cases: {} ({} accepted, {} refused)", total, accepted, total - accepted);
}
