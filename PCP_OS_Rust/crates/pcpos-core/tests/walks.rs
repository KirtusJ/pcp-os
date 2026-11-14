//! Randomized walks through the native record. Deterministic: the generator is seeded and has no dependency.
//!
//! Each walk goes through the scenario's inputs in order, sometimes skipping one, sometimes offering a
//! damaged copy or an event from elsewhere in the scenario first. Whatever is accepted or refused, these
//! must hold: nothing panics; the guards applied to their own output return it unchanged; a refused event
//! leaves the register as it was; the finished register verifies, replays through every guard without a
//! problem, and replaying it gives the same state the record arrived at step by step.
use pcpos_core::acr::{self, Record, State};
use pcpos_core::ledger;
use pcpos_core::value::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn damage(body: &Value, rng: &mut Rng) -> Value {
    let subs = [Value::Null, Value::Bool(true), Value::Int(0), Value::Int(-1), s(""), s("x"), s("R1"), s("G5"), s("kirtus"), s("CLOSED_COMPLETE"),
                Value::List(vec![]), Value::Map(BTreeMap::new()), Value::List(vec![s("G1"), s("G5"), s("G6")])];
    match body {
        Value::Map(m) if !m.is_empty() => {
            let mut m = m.clone();
            let key = m.keys().nth(rng.below(m.len())).unwrap().clone();
            match rng.below(4) {
                0 => {
                    m.remove(&key);
                }
                1 => {
                    let inner = damage(&m[&key], rng);
                    m.insert(key, inner);
                }
                _ => {
                    m.insert(key, subs[rng.below(subs.len())].clone());
                }
            }
            Value::Map(m)
        }
        Value::List(l) if !l.is_empty() => {
            let mut l = l.clone();
            let i = rng.below(l.len());
            l[i] = damage(&l[i], rng);
            Value::List(l)
        }
        _ => subs[rng.below(subs.len())].clone(),
    }
}

/// What can be seen of a state from outside, as text, for comparing two states.
fn picture(st: &State) -> String {
    let mut p = format!("{:?} {:?} {:?} {:?} events {} unclean {}", st.system(), st.in_use(), st.link(), st.standard(), st.events(), st.unclean_starts);
    for r in st.reviews.values() {
        p += &format!(" | {} {:?} s{} {:?} nf{:?} oo{:?}", r.label, r.lifecycle, r.session, r.passed_gates, st.claims_without_finding(r), st.open_objections(&r.label));
    }
    for (id, t) in &st.tasks {
        p += &format!(" | {} {} {:?} {}", id, t.label, t.outcome, t.spent);
    }
    p + &format!(" | ob{:?} parts{}", st.open_obligations(), st.participants.len())
}

#[test]
fn randomized_walks_keep_the_record_consistent() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors/scenario_inputs.jsonl");
    let inputs: Vec<Value> = fs::read(p).unwrap().split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(|l| Value::parse(l).unwrap()).collect();
    let f = |i: &Value, k: &str| i.get(k).unwrap().as_str().unwrap().to_string();
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let (mut offered, mut accepted, mut longest) = (0u32, 0u32, 0u64);
    for _walk in 0..400 {
        let mut rec = Record::new();
        for i in &inputs {
            let mut batch: Vec<(String, String, Value)> = Vec::new();
            match rng.below(10) {
                0 => {} // skip this event
                1 => {
                    let other = &inputs[rng.below(inputs.len())];
                    batch.push((f(other, "kind"), f(other, "actor"), other.get("body").unwrap().clone()));
                    batch.push((f(i, "kind"), f(i, "actor"), i.get("body").unwrap().clone()));
                }
                2 => {
                    batch.push((f(i, "kind"), f(i, "actor"), damage(i.get("body").unwrap(), &mut rng)));
                    batch.push((f(i, "kind"), f(i, "actor"), i.get("body").unwrap().clone()));
                }
                3 => batch.push((f(i, "kind"), ["host", "kirtus", "z", "g", "runner", "nobody"][rng.below(6)].to_string(), i.get("body").unwrap().clone())),
                _ => batch.push((f(i, "kind"), f(i, "actor"), i.get("body").unwrap().clone())),
            }
            for (kind, actor, body) in batch {
                offered += 1;
                let before = (rec.ledger().len(), rec.ledger().head().to_string());
                match acr::guard(rec.state(), &kind, &body, &actor) {
                    Ok(out) => {
                        assert_eq!(acr::guard(rec.state(), &kind, &out, &actor).as_ref(), Ok(&out), "the guards do not return their own output unchanged: {kind}");
                        rec.submit("2026-11-12T12:00:00Z", &actor, "DECLARED", &kind, body).unwrap_or_else(|e| panic!("the guard accepted and submit refused: {kind} {e:?}"));
                        assert_eq!(rec.ledger().len(), before.0 + 1);
                        accepted += 1;
                    }
                    Err(_) => {
                        assert!(rec.submit("2026-11-12T12:00:00Z", &actor, "DECLARED", &kind, body).is_err());
                        assert_eq!((rec.ledger().len(), rec.ledger().head().to_string()), before, "a refused event changed the register");
                    }
                }
            }
        }
        longest = longest.max(rec.ledger().len());
        let raw = rec.ledger().bytes().to_vec();
        let (events, head) = ledger::verify(&raw).expect("the register verifies");
        assert_eq!(head, rec.ledger().head());
        let (replayed, problems) = acr::fold(&events, true).expect("the register replays");
        assert_eq!(problems, vec![], "an event that was accepted does not pass the guards on replay");
        assert_eq!(picture(&replayed), picture(rec.state()), "replaying the register gives a different state");
    }
    println!("offered {offered}, accepted {accepted}, longest register {longest}");
    assert!(accepted > 10_000 && longest >= 70);
}
