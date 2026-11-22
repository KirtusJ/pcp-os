//! Completion, stopping and adoption as the record establishes them.
//!
//! A task is DONE on what the supervisor observed about that attempt, never on its own report and never on
//! the mere presence of an artifact. A request to stop is not a stop. Under a standard that has a person
//! adopt ACR use, the person acts as themselves.
use pcpos_core::acr::{Record, SubmitError};
use pcpos_core::sha256_hex;
use pcpos_core::value::Value;

const TS: &str = "2026-11-12T00:00:00Z";

fn v(json: &str) -> Value {
    Value::parse(json.as_bytes()).unwrap_or_else(|e| panic!("{json}: {e:?}"))
}

fn h(t: &str) -> String {
    sha256_hex(t.as_bytes())
}

fn put(r: &mut Record, actor: &str, kind: &str, body: &str) -> Result<(), String> {
    match r.submit(TS, actor, "DECLARED", kind, v(body)) {
        Ok(_) => Ok(()),
        Err(SubmitError::Refused(x)) => Err(x.message),
        Err(e) => panic!("{e:?}"),
    }
}

fn ok(r: &mut Record, actor: &str, kind: &str, body: &str) {
    put(r, actor, kind, body).unwrap_or_else(|m| panic!("{kind} was refused: {m}"));
}

/// A register with a person `p`, a model-driven participant `z`, ACR in use and one active review `R`.
fn base(system: bool) -> Record {
    let mut r = Record::new();
    ok(&mut r, "host", "WORKSPACE_CREATED", r#"{"workspace":"w"}"#);
    ok(&mut r, "host", "PARTICIPANT_ADDED", r#"{"kind":"HUMAN","label":"p","principal":true}"#);
    ok(&mut r, "host", "PARTICIPANT_ADDED", r#"{"kind":"AUTOMATED","label":"z","model":"m"}"#);
    ok(&mut r, "host", "METHOD_PINNED", &format!(r#"{{"core_descriptor_sha256":"{}","presence":"NATIVE","release_fingerprint":"{}","verified":"DECLARED_NOT_VERIFIED"}}"#, h("d"), h("f")));
    if system {
        for e in ["BOOT", "BEGIN_VERIFY", "VERIFIED_OK"] {
            ok(&mut r, "host", "SYSTEM_STATE", &format!(r#"{{"event":"{e}"}}"#));
        }
    }
    ok(&mut r, "p", "ARTIFACT_ADDED", &format!(r#"{{"origin":"SUPPLIED","sha256":"{}","size":1}}"#, h("target")));
    ok(&mut r, "p", "ADOPTED", r#"{"basis":"Use ACR on the target.","by":"p"}"#);
    ok(&mut r, "z", "REVIEW_COMMISSIONED", &format!(
        r#"{{"claims":[{{"label":"A","origin":"COMMISSIONER","text":"the target holds"}}],"director":"p","extent":"TOTAL","label":"R","procedure":"FOCUSED","target":{{"sha256":"{}","t":"artifact"}}}}"#, h("target")));
    r
}

fn start(r: &mut Record, attempt: &str) {
    ok(r, "z", "TASK_STARTED", &format!(r#"{{"attempt":"{attempt}","budget":0,"label":"work","quiet_limit":0,"review":"R","worker":"MODEL"}}"#));
}

fn produced(r: &mut Record, what: &str) {
    ok(r, "z", "ARTIFACT_ADDED", &format!(r#"{{"origin":"PRODUCED","sha256":"{}","size":1}}"#, h(what)));
}

fn end(r: &mut Record, id: &str, outcome: &str, result: Option<&str>, read_back: Option<&str>) -> Result<(), String> {
    let q = |x: Option<&str>| x.map(|d| format!("\"{d}\"")).unwrap_or("null".to_string());
    put(r, "host", "TASK_ENDED", &format!(r#"{{"id":"{id}","outcome":"{outcome}","read_back":{},"result":{},"spent":0}}"#, q(read_back), q(result)))
}

#[test]
fn done_needs_the_supervisors_own_read_back_of_bytes_produced_after_the_task_started() {
    let mut r = base(false);
    start(&mut r, "a1");
    let (t, res) = (h("target"), h("result"));
    assert!(end(&mut r, "T1", "DONE", None, None).is_err(), "no result named");
    assert!(end(&mut r, "T1", "DONE", Some(&res), Some(&res)).is_err(), "a digest that is not in the record");
    assert!(end(&mut r, "T1", "DONE", Some(&t), Some(&t)).unwrap_err().contains("produced"), "supplied material is not a task's result");
    produced(&mut r, "result");
    assert!(end(&mut r, "T1", "DONE", Some(&res), None).unwrap_err().contains("read back"), "nothing read back");
    assert!(end(&mut r, "T1", "DONE", Some(&res), Some(&t)).unwrap_err().contains("read back"), "read back something else");
    assert_eq!(r.state().tasks["T1"].outcome, None, "a refused end changes nothing");
    end(&mut r, "T1", "DONE", Some(&res), Some(&res)).unwrap();
    assert_eq!(r.state().tasks["T1"].outcome.as_deref(), Some("DONE"));
    assert_eq!(r.state().tasks["T1"].result.as_deref(), Some(res.as_str()));
    assert!(end(&mut r, "T1", "FAILED", None, None).is_err(), "an ended task does not end again");
}

#[test]
fn one_observation_settles_one_attempt() {
    // An artifact recorded before a task started is not that task's result, even with the right digest.
    let mut r = base(false);
    start(&mut r, "a1");
    produced(&mut r, "same");
    let d = h("same");
    end(&mut r, "T1", "DONE", Some(&d), Some(&d)).unwrap();
    start(&mut r, "a2");
    assert!(end(&mut r, "T2", "DONE", Some(&d), Some(&d)).unwrap_err().contains("before the task started"));
    // A second attempt that produces the same bytes is settled by its own read-back, recorded after it started.
    produced(&mut r, "same");
    end(&mut r, "T2", "DONE", Some(&d), Some(&d)).unwrap();
    // Two attempts running together: one record of the bytes settles one of them.
    start(&mut r, "a3");
    start(&mut r, "a4");
    produced(&mut r, "shared");
    let s = h("shared");
    end(&mut r, "T3", "DONE", Some(&s), Some(&s)).unwrap();
    assert!(end(&mut r, "T4", "DONE", Some(&s), Some(&s)).unwrap_err().contains("already settled"));
    end(&mut r, "T4", "FAILED", Some(&s), None).unwrap();
    assert_eq!(r.state().tasks["T4"].outcome.as_deref(), Some("FAILED"));
}

#[test]
fn a_halt_asks_a_reviews_tasks_to_stop_and_a_late_completion_is_not_done() {
    let mut r = base(false);
    start(&mut r, "a1");
    ok(&mut r, "z", "TASK_STARTED", r#"{"budget":0,"label":"unrelated","quiet_limit":0,"worker":"PROGRAM"}"#);
    ok(&mut r, "p", "HALTED", r#"{"by":"p","words":"Stop."}"#);
    let n = r.state().events() - 1;
    assert_eq!(r.state().tasks["T1"].stop_requested, Some(n), "the task of the closed review was asked to stop");
    assert_eq!(r.state().tasks["T1"].outcome, None, "asking is not stopping: no end is recorded");
    assert_eq!(r.state().tasks["T2"].stop_requested, None, "a task of no review is not ACR's to stop");
    // The worker finishes anyway. What it produced after the stop was requested is not its result.
    produced(&mut r, "late");
    let d = h("late");
    assert!(end(&mut r, "T1", "DONE", Some(&d), Some(&d)).unwrap_err().contains("stop was requested"));
    end(&mut r, "T1", "STOPPED", Some(&d), Some(&d)).unwrap();
    assert_eq!(r.state().tasks["T1"].outcome.as_deref(), Some("STOPPED"));
}

#[test]
fn a_fault_a_shutdown_or_maintenance_asks_every_task_to_stop() {
    for event in ["INTEGRITY_FAULT", "SHUTDOWN", "MAINTENANCE_BEGIN"] {
        let mut r = base(true);
        start(&mut r, "a1");
        ok(&mut r, "z", "TASK_STARTED", r#"{"budget":0,"label":"unrelated","quiet_limit":0,"worker":"PROGRAM"}"#);
        ok(&mut r, "host", "SYSTEM_STATE", &format!(r#"{{"event":"{event}"}}"#));
        assert!(r.state().tasks.values().all(|t| t.stop_requested.is_some() && t.outcome.is_none()), "{event}");
    }
    let mut r = base(true);
    start(&mut r, "a1");
    ok(&mut r, "host", "SYSTEM_STATE", r#"{"event":"SERVICE_LOST"}"#);
    assert_eq!(r.state().tasks["T1"].stop_requested, None, "a lost service lowers what may be claimed and stops nothing");
}

#[test]
fn where_a_person_must_adopt_the_person_acts_as_themselves() {
    let mut r = base(false);
    assert_eq!(r.state().adoption(), Some(("p", "p")));
    ok(&mut r, "p", "HALTED", r#"{"by":"p"}"#);
    assert_eq!(r.state().adoption(), None);
    // Without the standard, a participant may relay the person's instruction; the record shows who wrote it.
    ok(&mut r, "z", "ADOPTED", r#"{"basis":"Use ACR again.","by":"p"}"#);
    assert_eq!(r.state().adoption(), Some(("p", "z")));
    ok(&mut r, "p", "HALTED", r#"{"by":"p"}"#);
    ok(&mut r, "p", "STANDARD_SET", r#"{"by":"p","dual_authorization":false,"human_adoption":true,"method":"NATIVE_OR_PINNED","model_findings":"AS_METHOD","offline_only":false,"static_tasks":false,"tasks":[]}"#);
    assert!(put(&mut r, "z", "ADOPTED", r#"{"basis":"Use ACR again.","by":"p"}"#).unwrap_err().contains("acting as themselves"));
    assert!(put(&mut r, "z", "ADOPTED", r#"{"basis":"Use ACR again.","by":"host"}"#).is_err());
    ok(&mut r, "p", "ADOPTED", r#"{"basis":"Use ACR again.","by":"p"}"#);
}

#[test]
fn recording_an_earlier_product_again_as_supplied_material_does_not_make_it_this_task_s_result() {
    let mut r = base(false);
    produced(&mut r, "old");                 // made before the task existed
    start(&mut r, "a1");
    // The same bytes, recorded again after the task started, as supplied material.
    ok(&mut r, "z", "ARTIFACT_ADDED", &format!(r#"{{"origin":"SUPPLIED","sha256":"{}","size":1}}"#, h("old")));
    let refused = end(&mut r, "T1", "DONE", Some(&h("old")), Some(&h("old")));
    assert!(refused.unwrap_err().contains("before the task started"), "the product's position is where it was last recorded as produced");
    // Recorded as produced after the start, it is this attempt's result.
    produced(&mut r, "old");
    end(&mut r, "T1", "DONE", Some(&h("old")), Some(&h("old"))).unwrap();
}

#[test]
fn hook_identity_agrees_after_submit_extend_and_reopen() {
    use pcpos_core::acr::HookAttempt;
    let base = base(false);
    let raw = base.ledger().bytes();
    let key = h("session and agent");
    for (auth, observed, keyed, expected) in [
        ("HOST", "SUBAGENT_START", true, Some(key.as_str())),
        ("HOST", "SUBAGENT_START", false, Some("legacy")),
        ("DECLARED", "SUBAGENT_START", true, None),
        ("HOST", "OTHER", true, None),
    ] {
        let mut live = Record::open(raw).unwrap().0;
        let mut continued = Record::open(raw).unwrap().0;
        let field = if keyed { format!(r#","hook_attempt_key":"{key}""#) } else { String::new() };
        let body = v(&format!(r#"{{"label":"work","worker":"MODEL","attempt":"agent","quiet_limit":0,"budget":0,"observed":"{observed}"{field}}}"#));
        live.submit(TS, "z", auth, "TASK_STARTED", body).unwrap();
        let problems = continued.extend(&live.ledger().bytes()[raw.len()..]).unwrap();
        assert!(problems.is_empty());
        let (reopened, problems) = Record::open(live.ledger().bytes()).unwrap();
        assert!(problems.is_empty());
        for record in [&reopened, &continued, &live] {
            let identity = match record.state().tasks["T1"].hook_attempt.as_ref() {
                Some(HookAttempt::Key(k)) => Some(k.as_str()),
                Some(HookAttempt::Legacy) => Some("legacy"),
                None => None,
            };
            assert_eq!(identity, expected, "{auth} {observed} keyed={keyed}");
        }
    }
}
