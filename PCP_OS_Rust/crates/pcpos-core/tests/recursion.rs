use pcpos_core::acr::{HookAttempt, Record, SubmitError};
use pcpos_core::value::Value;

const WHEN: &str = "2026-11-14T00:00:00Z";

fn submit<'a>(r: &'a mut Record, who: &str, kind: &str, json: &str) -> Result<&'a str, SubmitError> {
    r.submit(WHEN, who, "HOST", kind, Value::parse(json.as_bytes()).unwrap())
}

fn refuse(r: &mut Record, who: &str, kind: &str, body: &str) {
    let old = r.ledger().bytes().to_vec();
    assert!(matches!(submit(r, who, kind, body), Err(SubmitError::Refused(_))), "{kind} {body}");
    assert_eq!(r.ledger().bytes(), old);
}

fn register() -> Record {
    let mut r = Record::new();
    submit(&mut r, "host", "WORKSPACE_CREATED", "{}").unwrap();
    for (name, kind, directing) in [("owner","HUMAN",true), ("agent","AUTOMATED",false), ("other","AUTOMATED",false)] {
        let body = format!(r#"{{"label":"{name}","kind":"{kind}","principal":{directing}}}"#);
        submit(&mut r, "host", "PARTICIPANT_ADDED", &body).unwrap();
    }
    r
}

#[test]
fn admission_is_atomic_parented_and_scoped_to_the_register() {
    let mut r = register();
    let root = r#"{"label":"root"}"#;
    let limits = r#"{"max_depth":1,"max_live":3,"max_total":3}"#;
    refuse(&mut r, "agent", "TASK_ADMITTED", root);
    refuse(&mut r, "agent", "TASK_LIMITS_SET", limits);
    refuse(&mut r, "owner", "TASK_LIMITS_SET", r#"{"max_depth":1,"max_live":0,"max_total":1}"#);
    submit(&mut r, "owner", "TASK_LIMITS_SET", limits).unwrap();
    submit(&mut r, "agent", "TASK_ADMITTED", root).unwrap();
    refuse(&mut r, "other", "TASK_ADMITTED", r#"{"label":"child","parent":"T1"}"#);
    refuse(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"child","parant":"T1"}"#);
    submit(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"child","parent":"T1"}"#).unwrap();
    refuse(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"deep","parent":"T2","depth":0}"#);
    refuse(&mut r, "agent", "TASK_RELEASED", r#"{"id":"T1"}"#);
    refuse(&mut r, "other", "TASK_RELEASED", r#"{"id":"T2"}"#);
    refuse(&mut r, "host", "TASK_ENDED", r#"{"id":"T2","outcome":"STOPPED","spent":0}"#);
    submit(&mut r, "agent", "TASK_RELEASED", r#"{"id":"T2"}"#).unwrap();
    refuse(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"closed-parent","parent":"T2"}"#);
    submit(&mut r, "owner", "TASK_ADMITTED", r#"{"label":"third","parent":"T1"}"#).unwrap();
    // Repeating a policy does not reset the cumulative count.
    submit(&mut r, "owner", "TASK_LIMITS_SET", limits).unwrap();
    refuse(&mut r, "agent", "TASK_ADMITTED", root);

    let (opened, problems) = Record::open(r.ledger().bytes()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!((opened.state().recursion_live, opened.state().recursion_total), (2,3));
    assert_eq!(opened.state().tasks["T3"].admission.as_ref().unwrap().depth, 1);
    assert_eq!(opened.state().tasks["T2"].outcome.as_deref(), Some("RELEASED"));
}

#[test]
fn observing_a_launched_worker_remains_possible_at_capacity() {
    let mut r = register();
    let observation = r#"{"label":"observed","worker":"MODEL","review":null,"quiet_limit":0,"budget":0,"attempt":"ABC","observed":"SUBAGENT_START"}"#;
    // Before a policy existed, the observation schema permitted opaque metadata of the same names.
    let historical = observation.replace("{", r#"{"depth":7,"parent":"upstream","admission":"vendor","#);
    submit(&mut r, "agent", "TASK_STARTED", &historical).unwrap();
    submit(&mut r, "owner", "TASK_LIMITS_SET", r#"{"max_depth":2,"max_live":1,"max_total":5}"#).unwrap();
    refuse(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"not-a-reservation","parent":"T1"}"#);
    submit(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"reserved"}"#).unwrap();
    refuse(&mut r, "agent", "TASK_ADMITTED", r#"{"label":"second"}"#);
    let fake = observation.replace("{", r#"{"parent":"T2","#);
    refuse(&mut r, "agent", "TASK_STARTED", &fake);
    submit(&mut r, "agent", "TASK_STARTED", observation).unwrap();
    refuse(&mut r, "agent", "TASK_RELEASED", r#"{"id":"T3"}"#);
    let (opened, problems) = Record::open(r.ledger().bytes()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!((opened.state().recursion_live, opened.state().recursion_total), (1,1));
    for id in ["T1","T3"] {
        assert!(opened.state().tasks[id].admission.is_none());
        assert!(matches!(opened.state().tasks[id].hook_attempt, Some(HookAttempt::Legacy)));
    }
}
