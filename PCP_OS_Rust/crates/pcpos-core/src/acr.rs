//! ACR, native: the state derived from a register and the guards an event must pass before it is written.
//!
//! This is the method's mechanically checkable part as code of the system itself. Each guard is a
//! necessary condition that can be checked without judgment, and cites the clause it enforces. Passing a
//! guard establishes nothing about whether a finding is right.
//!
//! This implementation is the authority. It publishes its answers to a file of cases
//! (`vectors/guard_cases.jsonl`: accepted or refused, the exit code and clause of a refusal, and the exact
//! body that would be written), and other implementations are tested against that file. The Python
//! implementation is one of them; writing the two side by side found defects in each.
//!
//! Layers, in the order they are applied to an offered event:
//!
//! 1. form: the kind is defined, the register is begun, the actor is known;
//! 2. composition: the system's state and the state of ACR use permit this class of event
//!    ([`crate::machine::permits`]);
//! 3. the guard of the kind, which may fill in computed fields;
//! 4. the installation's standard, which narrows and never widens.
use crate::authority::{Method, ModelFindings, Standard};
use crate::ledger::{Ledger, LedgerError};
use crate::machine::{self, Class, Delivery, DeliveryEvent, Link, LinkEvent, Review, ReviewEvent, System, SystemEvent, Use};
use crate::sha256_hex;
use crate::value::Value;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

type Map = BTreeMap<String, Value>;

/// The method release whose mechanically checkable rules this core implements: the SHA-256 of that release's
/// manifest. A register that records `presence: NATIVE` is recording this identity.
pub const METHOD_FINGERPRINT: &str = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707";
/// The SHA-256 of the same release's Core descriptor.
pub const CORE_DESCRIPTOR: &str = "9f5998dd75e921afe490687783787847916428f7ee7bcea410a88dad04646fb0";

/// Exit code of a refusal for a malformed request.
pub const USAGE: u8 = 2;
/// Exit code of a refusal by a guard.
pub const REFUSED: u8 = 3;
/// Exit code of a review step offered while ACR is not in use.
pub const NOT_IN_USE: u8 = 4;
/// Exit code for a register that does not hold what an event refers to.
pub const INTEGRITY: u8 = 6;
/// The clause given when a field has the wrong type for the lookup or comparison it is used in.
pub const FORM: &str = "FORM";

/// An event that was not accepted, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The exit code: [`USAGE`], [`REFUSED`], [`NOT_IN_USE`] or [`INTEGRITY`].
    pub code: u8,
    /// The clause of the method the guard enforces, where there is one.
    pub clause: Option<&'static str>,
    /// What was wrong, in plain words.
    pub message: String,
}

type R<T> = Result<T, Refusal>;

fn refuse<T>(clause: Option<&'static str>, message: String) -> R<T> {
    Err(Refusal { code: REFUSED, clause, message })
}

fn form<T>() -> R<T> {
    refuse(Some(FORM), "a field of the event has the wrong form".to_string())
}

const DISPOSITIONS: [&str; 7] = ["SUPPORTED_IN_SCOPE", "CONDITIONAL", "REFUTED_IN_SCOPE", "NOT_IDENTIFIED_IN_SCOPE", "DISPUTED", "UNRESOLVED", "INCOMPARABLE"];
const PROCEDURES: [&str; 3] = ["SCREENING", "FOCUSED", "FULL"];
const GROUNDING: [&str; 4] = ["NONE", "RECALL_ONLY", "SUPPLIED_ONLY", "EXTERNAL_INSPECTED"];
const ORIGINS: [&str; 3] = ["SUPPLIED", "PRODUCED", "RETRIEVED"];
const CLAIM_ORIGINS: [&str; 3] = ["COMMISSIONER", "TARGET_TEXT", "REVIEWER_CONSTRUCTED"];
const CLAIM_KINDS: [&str; 4] = ["UNSPECIFIED", "EXTERNAL_FACT", "MECHANICAL_CHECK", "DERIVATION"];
const ACTOR_KINDS: [&str; 3] = ["HUMAN", "AUTOMATED", "UNKNOWN"];
const TERMINAL: [&str; 3] = ["CLOSED_COMPLETE", "CLOSED_INCOMPLETE", "ABORTED"];
const EXTENTS: [&str; 4] = ["NONE", "PARTIAL", "TOTAL", "UNKNOWN"];
const OBJECTION_ENDS: [&str; 3] = ["RESOLVED", "WITHDRAWN", "OPEN_RECORDED"];
const COMPATIBILITY: [&str; 4] = ["SAME_METHOD", "DIFFERENT_METHOD_ASSESSED", "DIFFERENT_METHOD_UNKNOWN", "METHOD_NOT_STATED"];
const TRACE: [&str; 4] = ["FULL", "SUMMARY", "NONE", "UNKNOWN"];
const WORKERS: [&str; 2] = ["PROGRAM", "MODEL"];
const TASK_ENDS: [&str; 4] = ["DONE", "FAILED", "STOPPED", "LOST"];
/// How the record came by a check: the host saw the command run, this program ran it itself, or a participant reported it.
/// A check recorded before this was kept says nothing, and is read as a report.
pub const CHECK_OBSERVED: [&str; 3] = ["HOST_OBSERVED_TOOL", "RAN_BY_THIS_PROGRAM", "REPORTED_BY_PARTICIPANT"];
const STANDARD_METHOD: [&str; 2] = ["NATIVE_OR_PINNED", "NATIVE_ONLY"];
const STANDARD_MODEL_FINDINGS: [&str; 3] = ["AS_METHOD", "HUMAN_CLOSES", "MECHANICAL_ONLY"];
const STANDARD_FLAGS: [&str; 4] = ["human_adoption", "dual_authorization", "static_tasks", "offline_only"];
const RESERVED: [&str; 22] = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
                              "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];

/// Kinds of event that are steps of a review. A halt stops these and no others.
pub const STRUCTURED: [&str; 10] = ["REVIEW_COMMISSIONED", "CLAIM_ADDED", "GATE_COMMITTED", "OBJECTION_RAISED", "OBJECTION_RESOLVED",
                                    "FINDING_RECORDED", "REVIEW_CLOSED", "SESSION_OPENED", "REVIEW_PAUSED", "REVIEW_RESUMED"];
const HOST_KINDS: [&str; 7] = ["WORKSPACE_CREATED", "METHOD_PINNED", "SYSTEM_STATE", "LINK_STATE", "HOST_EVENT", "ENVIRONMENT_RECORDED", "TASK_ENDED"];
const DIRECTION_KINDS: [&str; 3] = ["ADOPTED", "PARTICIPANT_ADDED", "STANDARD_SET"];
const RECORD_KINDS: [&str; 13] = ["DELIVERY_STATE", "HALTED", "ARTIFACT_ADDED", "TREE_PINNED", "ARTIFACT_READ", "INSPECTION_DECLARED", "CHECK_RAN", "CONDITION_REGISTERED",
                                  "SUBMISSION_ADMITTED", "OBLIGATION_OPENED", "OBLIGATION_CLOSED", "RUN_COMPLETED", "NOTE"];
const UNREGISTERED_OK: [&str; 8] = ["DELIVERY_STATE", "WORKSPACE_CREATED", "METHOD_PINNED", "PARTICIPANT_ADDED", "HOST_EVENT", "SYSTEM_STATE", "LINK_STATE", "TASK_ENDED"];

/// True if this implementation defines the kind.
pub fn is_kind(kind: &str) -> bool {
    STRUCTURED.contains(&kind) || HOST_KINDS.contains(&kind) || DIRECTION_KINDS.contains(&kind) || RECORD_KINDS.contains(&kind) || kind == "TASK_STARTED"
}

/// The class of a kind of event, for the composition rule.
pub fn class_of(kind: &str) -> Class {
    if HOST_KINDS.contains(&kind) || kind.starts_with("X_") {
        Class::Host
    } else if kind == "HALTED" {
        Class::Halt
    } else if DIRECTION_KINDS.contains(&kind) {
        Class::Direction
    } else if STRUCTURED.contains(&kind) {
        Class::Review
    } else if kind == "TASK_STARTED" {
        Class::Task
    } else {
        Class::Record
    }
}

// ---- reading values the way the guards need ---------------------------------------------------------

static NULL: Value = Value::Null;

fn get<'a>(m: &'a Map, k: &str) -> &'a Value {
    m.get(k).unwrap_or(&NULL)
}

fn is_null(v: &Value) -> bool {
    matches!(v, Value::Null)
}

/// Whether a value counts as present where a guard reads "this, or else a default".
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Str(s) => !s.is_empty(),
        Value::List(l) => !l.is_empty(),
        Value::Map(m) => !m.is_empty(),
    }
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn is_hex64(v: &Value) -> bool {
    matches!(v, Value::Str(s) if s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
}

fn nat(v: &Value) -> Option<i64> {
    match v {
        Value::Int(n) if *n >= 0 => Some(*n),
        _ => None,
    }
}

fn label_ok(l: &str) -> bool {
    let b = l.as_bytes();
    !b.is_empty() && b.len() <= 64 && b[0].is_ascii_alphanumeric() && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn check_label<'a>(v: &'a Value, what: &str) -> R<&'a str> {
    if let Value::Str(l) = v {
        let first = l.split('.').next().unwrap_or("");
        if label_ok(l) && !RESERVED.iter().any(|r| first.eq_ignore_ascii_case(r)) {
            return Ok(l);
        }
    }
    Err(Refusal { code: USAGE, clause: None, message: format!("the {} is not usable: letters, digits, '.', '_' and '-', at most 64, starting with a letter or digit", what) })
}

/// Text: required unless `required` is false and the value is absent; not blank; no control characters;
/// at most `limit` characters. Returns the text without surrounding space, or null.
fn text(v: &Value, what: &str, limit: usize, required: bool) -> R<Value> {
    if is_null(v) && !required {
        return Ok(Value::Null);
    }
    let t = match v {
        Value::Str(t) if !t.trim().is_empty() => t,
        _ => return refuse(None, format!("{} is required and must be text", what)),
    };
    if t.chars().any(|c| matches!(c as u32, 0..=8 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f)) {
        return refuse(None, format!("{} contains a control character", what));
    }
    if t.chars().count() > limit {
        return refuse(None, format!("{} is longer than the limit of {} characters", what, limit));
    }
    Ok(s(t.trim()))
}

fn one_of<'a>(v: &'a Value, allowed: &[&str], what: &str) -> R<&'a str> {
    match v {
        Value::Str(x) if allowed.contains(&x.as_str()) => Ok(x),
        _ => refuse(if matches!(what, "disposition" | "procedure" | "gate") { Some("Core 15.6") } else { None },
                    format!("the {} is not one of the values ACR defines", what)),
    }
}

fn flag(m: &Map, key: &str) -> R<bool> {
    match get(m, key) {
        Value::Null => Ok(false),
        Value::Bool(b) => Ok(*b),
        _ => refuse(None, format!("{} is true or false", key)),
    }
}

// ---- state --------------------------------------------------------------------------------------------

/// A participant of the record.
#[derive(Clone, Debug)]
pub struct Participant {
    /// HUMAN, AUTOMATED or UNKNOWN.
    pub kind: String,
    /// True for a person directing the work.
    pub principal: bool,
    /// The model, for a model-driven participant.
    pub model: Option<String>,
    /// The lineage, where stated.
    pub lineage: Option<String>,
}

struct Artifact {
    origin: String,
    /// Sequence number of the latest event that recorded these bytes.
    last: i64,
    /// Sequence number of the first event that recorded these bytes.
    first: i64,
    /// True once any event has recorded these bytes as PRODUCED.
    produced: bool,
}

struct Check {
    exit: Option<i64>,
    expect_exit: i64,
    timeout: bool,
    /// True where the record holds the check only as a participant's report: no host saw it run.
    reported: bool,
}

struct Claim {
    kind: String,
    criterion: bool,
}

/// One review as the record has it.
pub struct ReviewState {
    /// Its label.
    pub label: String,
    /// The state of its present session.
    pub lifecycle: Review,
    /// The number of its present session, from 1.
    pub session: i64,
    /// SCREENING, FOCUSED or FULL.
    pub procedure: String,
    /// The gates it requires, in order.
    pub required_gates: Vec<String>,
    /// The gates committed in the present session, in order.
    pub passed_gates: Vec<String>,
    claims: BTreeMap<String, Claim>,
    /// Claim labels in the order they were added.
    pub claim_order: Vec<String>,
    current: BTreeMap<(String, String), String>,
    author: Option<String>,
    /// The Director of the review.
    pub director: String,
    /// Sequence number of the event that commissioned it.
    commissioned: i64,
}

struct Finding {
    review: String,
    session: i64,
    disposition: String,
}

/// One task as the record has it.
pub struct TaskState {
    /// Its label.
    pub label: String,
    /// PROGRAM or MODEL.
    pub worker: String,
    /// The review it belongs to, if any.
    pub review: Option<String>,
    /// Who started it.
    pub actor: String,
    /// How it ended, once it has: DONE, FAILED, STOPPED or LOST.
    pub outcome: Option<String>,
    /// What it had spent when it ended.
    pub spent: i64,
    /// Its budget.
    pub budget: i64,
    /// Sequence number of the event that started it.
    pub started: i64,
    /// Sequence number of the event that asked it to stop (a halt, or a system transition that stops work), if any.
    /// A request to stop is not a stop: until the task's end is recorded, its worker may still be acting.
    pub stop_requested: Option<i64>,
    /// The digest it reported as its result, once it has ended.
    pub result: Option<String>,
    /// The supervisor's identifier for this attempt, where one was recorded.
    pub attempt: Option<String>,
    /// Identity evidence supplied by the host's subagent-start hook.
    pub hook_attempt: Option<HookAttempt>,
}

/// Older subagent hook observations stored only the clipped attempt label.
pub enum HookAttempt {
    Legacy,
    Key(String),
}

#[derive(Clone)]
struct StandardNow {
    std: Standard,
    tasks: Vec<String>,
}

/// Everything the guards need to know, derived from the events so far.
#[derive(Default)]
pub struct State {
    created: bool,
    workspace: String,
    method_presence: Option<Option<String>>,
    /// Participants by label.
    pub participants: BTreeMap<String, Participant>,
    artifacts: BTreeMap<String, Artifact>,
    names: BTreeMap<String, String>,
    trees: BTreeSet<String>,
    adopted: bool,
    checks: BTreeMap<String, Check>,
    /// Reviews by label.
    pub reviews: BTreeMap<String, ReviewState>,
    objections: BTreeMap<String, (String, String)>,
    obligations: BTreeMap<String, bool>,
    submissions: BTreeSet<String>,
    inspections: BTreeSet<String>,
    findings: BTreeMap<String, Finding>,
    system: Option<System>,
    link: Option<Link>,
    standard: Option<StandardNow>,
    /// Whether each participant holds the method text in its present context, as recorded.
    pub delivery: BTreeMap<String, Delivery>,
    /// Tasks by id.
    pub tasks: BTreeMap<String, TaskState>,
    /// How many starts found that the run before had not stopped in order.
    pub unclean_starts: u32,
    consumed: BTreeSet<i64>,
    adoption: Option<(String, String)>,
    seq: i64,
    counters: [u32; 7],
}

const K: usize = 0;
const F: usize = 1;
const O: usize = 2;
const C: usize = 3;
const B: usize = 4;
const RUN: usize = 5;
const T: usize = 6;
const LETTERS: [char; 7] = ['K', 'F', 'O', 'C', 'B', 'R', 'T'];

impl State {
    /// The state before any event.
    pub fn new() -> Self {
        State { seq: -1, ..Default::default() }
    }

    fn next_id(&self, which: usize) -> String {
        format!("{}{}", LETTERS[which], self.counters[which] + 1)
    }

    /// Whether ACR is in use.
    pub fn in_use(&self) -> Use {
        if self.adopted { Use::InUse } else { Use::NotInUse }
    }

    /// The system's state, if the register is kept by a system that records it.
    pub fn system(&self) -> Option<System> {
        self.system
    }

    /// Connectivity as recorded. Offline until recorded otherwise.
    pub fn link(&self) -> Link {
        self.link.unwrap_or(Link::Offline)
    }

    /// The standard in force.
    pub fn standard(&self) -> Standard {
        self.standard.as_ref().map(|x| x.std).unwrap_or(Standard::BASELINE)
    }

    /// Who adopted ACR use and who wrote the event, while ACR is in use: (by, actor). Where the two differ,
    /// the adoption was relayed by another participant and the person did not act as themselves.
    pub fn adoption(&self) -> Option<(&str, &str)> {
        self.adoption.as_ref().map(|(b, a)| (b.as_str(), a.as_str()))
    }

    /// True if a method identity is recorded.
    pub fn method_pinned(&self) -> bool {
        self.method_presence.is_some()
    }

    /// Number of events applied.
    pub fn events(&self) -> i64 {
        self.seq + 1
    }

    /// Open objections of a review, by id.
    pub fn open_objections(&self, review: &str) -> Vec<String> {
        self.objections.iter().filter(|(_, (r, st))| r == review && st == "OPEN").map(|(id, _)| id.clone()).collect()
    }

    /// Claims of a review that have no current finding.
    pub fn claims_without_finding(&self, r: &ReviewState) -> Vec<String> {
        r.claim_order.iter().filter(|c| !r.current.keys().any(|(cl, _)| cl == *c)).cloned().collect()
    }

    /// The id of the check recorded last, if any.
    pub fn last_check(&self) -> Option<String> {
        if self.counters[K] == 0 { None } else { Some(format!("{}{}", LETTERS[K], self.counters[K])) }
    }

    /// Open obligations, by id.
    pub fn open_obligations(&self) -> Vec<String> {
        self.obligations.iter().filter(|(_, open)| **open).map(|(id, _)| id.clone()).collect()
    }

    fn review(&self, label: &Value, active: bool, working: bool) -> R<&ReviewState> {
        let r = match label {
            Value::Str(l) => self.reviews.get(l),
            _ => None,
        };
        let r = match r {
            Some(r) => r,
            None => return refuse(None, "no review has that label".to_string()),
        };
        if active && TERMINAL.contains(&r.lifecycle.name()) {
            return refuse(Some("Core 7.1"), format!("review {} is {}; its assessment is sealed", r.label, r.lifecycle.name()));
        }
        if active && working && r.lifecycle == Review::Paused {
            return refuse(None, format!("review {} is PAUSED; no step is recorded until it is resumed", r.label));
        }
        Ok(r)
    }

    fn model_driven(&self, label: &str) -> bool {
        match self.participants.get(label) {
            Some(p) if p.kind == "HUMAN" => false,
            Some(p) => !(p.kind == "AUTOMATED" && p.model.is_none()),
            None => true,
        }
    }

    fn human_principal(&self, label: &Value) -> bool {
        match label {
            Value::Str(l) => matches!(self.participants.get(l), Some(p) if p.principal && p.kind == "HUMAN"),
            _ => false,
        }
    }

    fn principal(&self, label: &Value) -> bool {
        match label {
            Value::Str(l) => matches!(self.participants.get(l), Some(p) if p.principal),
            _ => false,
        }
    }

    fn standard_now(&self) -> StandardNow {
        self.standard.clone().unwrap_or(StandardNow { std: Standard::BASELINE, tasks: Vec::new() })
    }
}

// ---- references -----------------------------------------------------------------------------------------

fn ref_key(r: &Map) -> R<String> {
    match get(r, "t").as_str() {
        Some("artifact") => get(r, "sha256").as_str().map(|x| x.to_string()).ok_or(()).or_else(|_| form()),
        Some("member") => match (get(r, "tree").as_str(), get(r, "path").as_str()) {
            (Some(t), Some(p)) => Ok(format!("tree:{}/{}", t, p)),
            _ => form(),
        },
        _ => get(r, "id").as_str().map(|x| x.to_string()).ok_or(()).or_else(|_| form()),
    }
}

struct Item {
    r: Map,
    extent: Value,
}

impl Item {
    fn value(&self) -> Value {
        let mut m = Map::new();
        m.insert("ref".to_string(), Value::Map(self.r.clone()));
        if truthy(&self.extent) {
            m.insert("extent".to_string(), self.extent.clone());
        }
        Value::Map(m)
    }

    fn t(&self) -> &str {
        get(&self.r, "t").as_str().unwrap_or("")
    }
}

fn items_value(items: &[Item]) -> Value {
    Value::List(items.iter().map(Item::value).collect())
}

/// Each reference exists, and each inspected item says how much of it was inspected (Core 4.3, 8.2).
fn check_basis(state: &State, items: &Value, what: &str) -> R<Vec<Item>> {
    let list: &[Value] = match items {
        Value::List(l) => l,
        v if !truthy(v) => &[],
        _ => return refuse(None, format!("{} must be a list of references", what)),
    };
    let mut out = Vec::new();
    for it in list {
        let (r, extent) = match it {
            Value::Map(m) => match get(m, "ref") {
                Value::Map(r) => (r.clone(), get(m, "extent").clone()),
                _ => return refuse(None, format!("{} holds a malformed reference", what)),
            },
            _ => return refuse(None, format!("{} holds a malformed reference", what)),
        };
        let t = get(&r, "t").as_str().unwrap_or("").to_string();
        let lookup = |k: &str| -> R<Option<String>> {
            match get(&r, k) {
                Value::Str(x) => Ok(Some(x.clone())),
                Value::List(_) | Value::Map(_) => form(),
                _ => Ok(None),
            }
        };
        match t.as_str() {
            "artifact" => {
                if !matches!(lookup("sha256")?, Some(d) if state.artifacts.contains_key(&d)) {
                    return refuse(None, format!("{} refers to an artifact that is not in the record", what));
                }
            }
            "member" => {
                if get(&r, "tree").as_str().is_none() || get(&r, "path").as_str().is_none() {
                    return refuse(None, format!("{} holds a malformed reference", what));
                }
                if !state.trees.contains(get(&r, "tree").as_str().unwrap_or("")) {
                    return refuse(None, format!("{} refers to a tree that is not pinned", what));
                }
            }
            "check" => {
                if !matches!(lookup("id")?, Some(d) if state.checks.contains_key(&d)) {
                    return refuse(Some("Core 15.8"), format!("{} refers to a check that was not run through this record", what));
                }
            }
            "finding" => {
                if !matches!(lookup("id")?, Some(d) if state.findings.contains_key(&d)) {
                    return refuse(None, format!("{} refers to a finding that is not in the record", what));
                }
            }
            _ => return refuse(None, format!("{} holds a reference of unknown type", what)),
        }
        let mut extent = extent;
        if t == "artifact" || t == "member" {
            if !is_null(&extent) {
                extent = text(&extent, "the inspected extent", 500, true)?;
            } else if !state.inspections.contains(&ref_key(&r)?) {
                return refuse(Some("Core 4.3, 8.2"), "no inspection is recorded for an item of the basis; say how much of it was inspected".to_string());
            }
        }
        out.push(Item { r, extent });
    }
    Ok(out)
}

/// The strongest grounding for which the record holds an inspected item: an upper bound, set by observation.
fn observed_grounding(state: &State, basis: &[Item]) -> &'static str {
    let mut best = 0;
    for it in basis {
        match it.t() {
            "artifact" => {
                let retrieved = get(&it.r, "sha256").as_str().and_then(|d| state.artifacts.get(d)).map(|a| a.origin == "RETRIEVED").unwrap_or(false);
                best = best.max(if retrieved { 3 } else { 2 });
            }
            "member" | "check" => best = best.max(2),
            _ => {}
        }
    }
    GROUNDING[best]
}

/// What the record can say about the relation between the party assessing and the party that authored
/// the subject. It reports dimensions and never the word "independent" (Core 4.2, 6.2).
fn relation(state: &State, review: &ReviewState, actor: &str) -> Value {
    let (label, lineage) = match &review.author {
        None => ("AUTHOR_NOT_RECORDED", "UNKNOWN"),
        Some(a) if a == actor => ("SELF_CHECK", "SAME"),
        Some(a) => {
            let la = state.participants.get(actor).and_then(|p| p.lineage.clone()).filter(|x| !x.is_empty());
            let lb = state.participants.get(a).and_then(|p| p.lineage.clone()).filter(|x| !x.is_empty());
            match (la, lb) {
                (Some(x), Some(y)) if x == y => ("SEPARATE_ACTOR_SAME_LINEAGE", "SAME"),
                (Some(_), Some(_)) => ("SEPARATE_ACTOR_DIFFERENT_LINEAGE", "DIFFERENT"),
                _ => ("SEPARATE_ACTOR_LINEAGE_UNKNOWN", "UNKNOWN"),
            }
        }
    };
    let mut m = Map::new();
    m.insert("label".to_string(), s(label));
    m.insert("author".to_string(), review.author.as_deref().map(s).unwrap_or(Value::Null));
    m.insert("lineage".to_string(), s(lineage));
    m.insert("exposure".to_string(), s("NOT_OBSERVED"));
    Value::Map(m)
}

// ---- guards -------------------------------------------------------------------------------------------

/// Offer an event to the guards. Returns the body to write, with computed fields filled in, or the
/// refusal. Applying the guards to their own output gives the same output.
pub fn guard(state: &State, kind: &str, body: &Value, actor: &str) -> R<Value> {
    if !is_kind(kind) && !kind.starts_with("X_") {
        return refuse(None, format!("unknown event kind {}", kind));
    }
    if kind != "WORKSPACE_CREATED" && !state.created {
        return refuse(None, "the register does not begin with WORKSPACE_CREATED".to_string());
    }
    if matches!(kind, "SYSTEM_STATE" | "LINK_STATE" | "TASK_ENDED") && actor != "host" {
        return refuse(None, format!("{} is written by the system itself", kind));
    }
    if !UNREGISTERED_OK.contains(&kind) && !state.participants.contains_key(actor) {
        return refuse(None, format!("the acting participant {} is not registered", actor));
    }
    let b = match body {
        Value::Map(m) => m.clone(),
        _ => return form(),
    };
    let boot = kind == "SYSTEM_STATE" && get(&b, "event").as_str() == Some("BOOT");
    if !boot {
        if let Some(sys) = state.system {
            // Whether ACR is in use is answered below with its own exit code, so it is not asked here.
            if machine::permits(sys, Use::InUse, class_of(kind)).is_err() {
                return refuse(None, format!("the system is in state {}, in which no event of this class is accepted", sys.name()));
            }
        }
    }
    if STRUCTURED.contains(&kind) && !state.adopted {
        return Err(Refusal { code: NOT_IN_USE, clause: Some("Core 2.1, 15.1, 15.2"), message: "ACR is not in use in this workspace, so no review step is recorded".to_string() });
    }
    let b = match kind {
        "WORKSPACE_CREATED" => g_created(state, b)?,
        "METHOD_PINNED" => g_method(b)?,
        "PARTICIPANT_ADDED" => g_participant(state, b)?,
        "ADOPTED" => g_direct(state, b, "Core 15.1", true)?,
        "HALTED" => g_direct(state, b, "Core 15.2", false)?,
        "ARTIFACT_ADDED" => g_artifact(state, b)?,
        "TREE_PINNED" => g_tree(state, b)?,
        "ARTIFACT_READ" => g_read(state, b)?,
        "INSPECTION_DECLARED" => g_inspect(state, b)?,
        "CHECK_RAN" => g_check(state, b)?,
        "REVIEW_COMMISSIONED" => g_commission(state, b)?,
        "CLAIM_ADDED" => g_claim_added(state, b)?,
        "GATE_COMMITTED" => g_gate(state, b)?,
        "OBJECTION_RAISED" => g_objection(state, b)?,
        "OBJECTION_RESOLVED" => g_objection_end(state, b)?,
        "FINDING_RECORDED" => g_finding(state, b, actor)?,
        "REVIEW_CLOSED" => g_close(state, b)?,
        "SESSION_OPENED" => g_session(state, b)?,
        "REVIEW_PAUSED" => g_pause(state, b)?,
        "REVIEW_RESUMED" => g_resume(state, b)?,
        "CONDITION_REGISTERED" => g_condition(state, b)?,
        "SUBMISSION_ADMITTED" => g_submission(state, b)?,
        "OBLIGATION_OPENED" => g_obligation(state, b)?,
        "OBLIGATION_CLOSED" => g_obligation_end(state, b)?,
        "RUN_COMPLETED" => g_run(state, b)?,
        "SYSTEM_STATE" => g_system(state, b)?,
        "LINK_STATE" => g_link(state, b)?,
        "TASK_STARTED" => g_task_start(state, b)?,
        "TASK_ENDED" => g_task_end(state, b)?,
        "STANDARD_SET" => g_standard(state, b, actor)?,
        "DELIVERY_STATE" => g_delivery(state, b, actor)?,
        _ => b,
    };
    standard_check(state, kind, &b, actor)?;
    Ok(Value::Map(b))
}

fn seq_id(state: &State, b: &mut Map, which: usize, what: &str) -> R<()> {
    let want = state.next_id(which);
    match get(b, "id") {
        Value::Null => {}
        Value::Str(x) if *x == want => {}
        _ => return refuse(None, format!("{} id out of sequence", what)),
    }
    b.insert("id".to_string(), Value::Str(want));
    Ok(())
}

fn own_id(state: &State, b: &mut Map, what: &str) -> R<()> {
    match get(b, "id") {
        Value::Null => {
            // No random source is needed in the trusted base: the workspace and the position in the register name it.
            b.insert("id".to_string(), Value::Str(format!("acr:{}:{}:{}", what, state.workspace, state.seq + 1)));
            Ok(())
        }
        Value::Str(x) if !x.is_empty() => Ok(()),
        _ => refuse(None, format!("a {} id, when given, is text", what)),
    }
}

fn g_created(state: &State, b: Map) -> R<Map> {
    if state.created {
        return refuse(None, "the workspace is already created".to_string());
    }
    Ok(b)
}

fn g_method(b: Map) -> R<Map> {
    for k in ["release_fingerprint", "core_descriptor_sha256"] {
        if !is_hex64(get(&b, k)) {
            return refuse(Some("Core 14.1, 14.2"), format!("the method identity needs {} as 64 hexadecimal characters", k));
        }
    }
    one_of(get(&b, "verified"), &["FILES_VERIFIED_AGAINST_OWN_MANIFEST", "DECLARED_NOT_VERIFIED"], "verification")?;
    if !is_null(get(&b, "presence")) {
        one_of(get(&b, "presence"), &["NATIVE", "PINNED"], "presence")?;
    }
    Ok(b)
}

fn g_participant(state: &State, mut b: Map) -> R<Map> {
    let label = check_label(get(&b, "label"), "participant label")?.to_string();
    if state.participants.contains_key(&label) {
        return refuse(None, format!("participant {} is already registered", label));
    }
    one_of(get(&b, "kind"), &ACTOR_KINDS, "actor kind")?;
    own_id(state, &mut b, "participant")?;
    let p = flag(&b, "principal")?;
    b.insert("principal".to_string(), Value::Bool(p));
    for k in ["name", "model", "provider", "lineage"] {
        if !is_null(get(&b, k)) {
            let t = text(get(&b, k), k, 200, true)?;
            b.insert(k.to_string(), t);
        }
    }
    Ok(b)
}

fn g_direct(state: &State, mut b: Map, clause: &'static str, adopt: bool) -> R<Map> {
    let by = get(&b, "by");
    if matches!(by, Value::List(_) | Value::Map(_)) {
        return form();
    }
    if by.as_str() != Some("host") && !state.principal(by) {
        return refuse(Some(clause), "this is decided by the person directing the work or by the host".to_string());
    }
    if adopt {
        let basis = text(get(&b, "basis"), "the basis (the instruction in its original words)", 2000, true)?;
        b.insert("basis".to_string(), basis);
    } else {
        let closed: Vec<Value> = state.reviews.values().filter(|r| !TERMINAL.contains(&r.lifecycle.name())).map(|r| s(&r.label)).collect();
        b.insert("closed".to_string(), Value::List(closed));
    }
    Ok(b)
}

fn g_artifact(state: &State, mut b: Map) -> R<Map> {
    if !is_hex64(get(&b, "sha256")) {
        return refuse(None, "an artifact needs its SHA-256".to_string());
    }
    one_of(get(&b, "origin"), &ORIGINS, "origin")?;
    if nat(get(&b, "size")).is_none() {
        return refuse(None, "an artifact needs its size in bytes".to_string());
    }
    if !is_null(get(&b, "name")) {
        let name = check_label(get(&b, "name"), "artifact name")?;
        if matches!(state.names.get(name), Some(d) if Some(d.as_str()) != get(&b, "sha256").as_str()) {
            return refuse(None, format!("the name {} already belongs to another artifact", name));
        }
    }
    if get(&b, "origin").as_str() == Some("RETRIEVED") && !matches!(get(&b, "source"), Value::Map(_)) {
        return refuse(None, "a retrieved artifact records where and when it was retrieved".to_string());
    }
    let st = flag(&b, "stored")?;
    b.insert("stored".to_string(), Value::Bool(st));
    Ok(b)
}

fn g_tree(state: &State, b: Map) -> R<Map> {
    let name = check_label(get(&b, "name"), "tree name")?;
    if state.trees.contains(name) {
        return refuse(Some("Core 7.4"), format!("a tree named {} is already pinned", name));
    }
    if !is_hex64(get(&b, "manifest")) {
        return refuse(None, "a pinned tree needs the digest of its manifest".to_string());
    }
    one_of(get(&b, "origin"), &["SUPPLIED", "PRODUCED"], "origin")?;
    Ok(b)
}

fn one_item(r: &Value, extent: Value) -> Value {
    let mut m = Map::new();
    m.insert("ref".to_string(), r.clone());
    m.insert("extent".to_string(), extent);
    Value::List(alloc::vec![Value::Map(m)])
}

fn g_read(state: &State, b: Map) -> R<Map> {
    let r = b.get("ref").cloned().unwrap_or(Value::Map(Map::new()));
    check_basis(state, &one_item(&r, s("x")), "the item read")?;
    Ok(b)
}

fn g_inspect(state: &State, mut b: Map) -> R<Map> {
    let extent = text(get(&b, "extent"), "the inspected extent", 500, true)?;
    b.insert("extent".to_string(), extent.clone());
    let r = b.get("ref").cloned().unwrap_or(Value::Map(Map::new()));
    let it = check_basis(state, &one_item(&r, extent), "the inspected item")?;
    b.insert("ref".to_string(), Value::Map(it[0].r.clone()));
    Ok(b)
}

fn g_check(state: &State, mut b: Map) -> R<Map> {
    seq_id(state, &mut b, K, "check")?;
    if !matches!(get(&b, "argv"), Value::List(l) if !l.is_empty() && l.iter().all(|a| matches!(a, Value::Str(_)))) {
        return refuse(None, "a check records the exact command that was run".to_string());
    }
    for k in ["stdout", "stderr"] {
        if !is_hex64(get(&b, k)) {
            return refuse(None, format!("a check records the digest of its {}", k));
        }
    }
    if !matches!(get(&b, "exit"), Value::Int(_) | Value::Null) {
        return refuse(None, "a check records its exit status".to_string());
    }
    if !matches!(get(&b, "expect_exit"), Value::Int(_)) || !matches!(get(&b, "timeout"), Value::Bool(_)) {
        return refuse(None, "a check records the exit status it expected and whether it timed out".to_string());
    }
    if !is_null(get(&b, "observed")) {
        one_of(get(&b, "observed"), &CHECK_OBSERVED, "how the check was observed")?;
    }
    Ok(b)
}

fn claim(mut c: Map, existing: &dyn Fn(&str) -> bool) -> R<Map> {
    let label = check_label(get(&c, "label"), "claim label")?.to_string();
    if existing(&label) {
        return refuse(None, format!("claim {} already exists in this review", label));
    }
    let t = text(get(&c, "text"), "the claim, in its original words", 8000, true)?;
    c.insert("text_sha256".to_string(), Value::Str(sha256_hex(t.as_str().unwrap_or("").as_bytes())));
    c.insert("text".to_string(), t);
    one_of(get(&c, "origin"), &CLAIM_ORIGINS, "claim origin")?;
    let kind = if truthy(get(&c, "kind")) { one_of(get(&c, "kind"), &CLAIM_KINDS, "claim kind")?.to_string() } else { "UNSPECIFIED".to_string() };
    c.insert("kind".to_string(), Value::Str(kind));
    let crit = text(get(&c, "criterion"), "the criterion", 2000, false)?;
    c.insert("criterion".to_string(), crit);
    match get(&c, "substitute_of") {
        Value::Null => {}
        Value::Str(x) if existing(x) => {}
        Value::List(_) | Value::Map(_) => return form(),
        _ => return refuse(Some("Core 5.2"), "a substitute claim names the commissioned claim it stands beside".to_string()),
    }
    Ok(c)
}

fn g_commission(state: &State, mut b: Map) -> R<Map> {
    if state.method_presence.is_none() {
        return refuse(Some("Core 14.1, 14.2"), "no method identity is recorded".to_string());
    }
    let label = check_label(get(&b, "label"), "review label")?.to_string();
    if state.reviews.contains_key(&label) {
        return refuse(Some("Core 2.4"), format!("a review labelled {} already exists", label));
    }
    own_id(state, &mut b, "review")?;
    let title = text(get(&b, "title"), "title", 300, false)?;
    b.insert("title".to_string(), title);
    let tgt = match get(&b, "target") {
        Value::Map(m) => m.clone(),
        v if !truthy(v) => Map::new(),
        _ => return form(),
    };
    let key = |k: &str| -> R<Option<String>> {
        match get(&tgt, k) {
            Value::Str(x) => Ok(Some(x.clone())),
            Value::List(_) | Value::Map(_) => form(),
            _ => Ok(None),
        }
    };
    match get(&tgt, "t").as_str() {
        Some("tree") => {
            if !matches!(key("name")?, Some(n) if state.trees.contains(&n)) {
                return refuse(Some("Core 2.1, 2.2"), "the target tree is not pinned".to_string());
            }
        }
        Some("artifact") => {
            if !matches!(key("sha256")?, Some(d) if state.artifacts.contains_key(&d)) {
                return refuse(Some("Core 2.1, 2.2"), "the target artifact is not in the record".to_string());
            }
        }
        _ => return refuse(Some("Core 2.1"), "a review binds exactly one target: a pinned tree or an artifact".to_string()),
    }
    one_of(get(&b, "extent"), &["TOTAL", "PARTIAL"], "intended extent")?;
    let arrangement = if truthy(get(&b, "arrangement")) { one_of(get(&b, "arrangement"), &["UNARY", "COMPARATIVE"], "arrangement")?.to_string() } else { "UNARY".to_string() };
    b.insert("arrangement".to_string(), Value::Str(arrangement));
    let procedure = one_of(get(&b, "procedure"), &PROCEDURES, "procedure")?.to_string();
    let gates: Vec<String> = if truthy(get(&b, "required_gates")) {
        let l = match get(&b, "required_gates") {
            Value::List(l) => l,
            _ => return refuse(Some("Core 6.1"), "required gates are a unique ordered subset of G1 to G6".to_string()),
        };
        let mut out = Vec::new();
        let mut last = -1i32;
        for g in l {
            let i = g.as_str().and_then(|x| machine::GATES.iter().position(|y| *y == x)).map(|i| i as i32).unwrap_or(-1);
            if i <= last {
                return refuse(Some("Core 6.1"), "required gates are a unique ordered subset of G1 to G6".to_string());
            }
            last = i;
            out.push(machine::GATES[i as usize].to_string());
        }
        out
    } else if procedure == "FULL" {
        machine::GATES.iter().map(|g| g.to_string()).collect()
    } else {
        alloc::vec!["G1".to_string(), "G5".to_string(), "G6".to_string()]
    };
    let has = |g: &str| gates.iter().any(|x| x == g);
    if !(has("G1") && has("G5") && has("G6")) || (procedure == "FULL" && gates.len() != 6) {
        return refuse(Some("Core 1.2, 6.1"), format!("{} does not have the gates it requires", procedure));
    }
    b.insert("required_gates".to_string(), Value::List(gates.iter().map(|g| s(g)).collect()));
    let known = |v: &Value, set: &dyn Fn(&str) -> bool| -> R<bool> {
        match v {
            Value::Str(x) => Ok(set(x)),
            Value::List(_) | Value::Map(_) => form(),
            _ => Ok(false),
        }
    };
    if !known(get(&b, "director"), &|x| state.participants.contains_key(x))? {
        return refuse(Some("Core 3.2"), "the Director is a registered participant".to_string());
    }
    if !is_null(get(&b, "author")) && !known(get(&b, "author"), &|x| state.participants.contains_key(x))? {
        return refuse(None, "the author of the subject, when stated, is a registered participant".to_string());
    }
    if !is_null(get(&b, "parent")) {
        if !known(get(&b, "parent"), &|x| state.reviews.contains_key(x))? {
            return refuse(Some("Core 7.2"), "the parent review does not exist".to_string());
        }
        let parent = &state.reviews[get(&b, "parent").as_str().unwrap_or("")];
        let mut pb = Map::new();
        pb.insert("seq".to_string(), Value::Int(state.seq));
        pb.insert("session".to_string(), Value::Int(parent.session));
        b.insert("parent_basis".to_string(), Value::Map(pb));
    }
    let mods = match get(&b, "modules") {
        Value::List(l) => l.clone(),
        v if !truthy(v) => Vec::new(),
        _ => return refuse(Some("Core 14.1"), "each selected module is recorded by identifier, version and digest".to_string()),
    };
    if mods.iter().any(|m| !matches!(m, Value::Map(x) if is_hex64(get(x, "sha256")))) {
        return refuse(Some("Core 14.1"), "each selected module is recorded by identifier, version and digest".to_string());
    }
    b.insert("modules".to_string(), Value::List(mods));
    let list = match get(&b, "claims") {
        Value::List(l) if !l.is_empty() => l.clone(),
        _ => return refuse(Some("Core 2.1, 6.1"), "a review needs at least one claim, in its original words, with its origin".to_string()),
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut claims = Vec::new();
    for c in &list {
        let c = match c {
            Value::Map(m) => m.clone(),
            _ => return refuse(Some("Core 2.1, 6.1"), "each claim is an object with its label, text and origin".to_string()),
        };
        let c = claim(c, &|l| seen.contains(l))?;
        seen.insert(get(&c, "label").as_str().unwrap_or("").to_string());
        claims.push(Value::Map(c));
    }
    b.insert("claims".to_string(), Value::List(claims));
    let exp = if truthy(get(&b, "expectation")) { get(&b, "expectation").clone() } else { s("NO_DIRECTIONAL_EXPECTATION") };
    let exp = text(&exp, "the opening expectation", 1000, true)?;
    b.insert("expectation".to_string(), exp);
    Ok(b)
}

fn g_claim_added(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), true, true)?;
    let c = match get(&b, "claim") {
        Value::Map(m) => m.clone(),
        _ => return refuse(None, "the claim is an object with its label, text and origin".to_string()),
    };
    let c = claim(c, &|l| r.claims.contains_key(l))?;
    if r.passed_gates.iter().any(|g| g == "G1") && get(&c, "origin").as_str() != Some("REVIEWER_CONSTRUCTED") {
        return refuse(Some("Core 6.1"), "G1 is committed: a further commissioned claim is an amendment of the review, not an addition".to_string());
    }
    b.insert("claim".to_string(), Value::Map(c));
    Ok(b)
}

fn g_gate(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), true, true)?;
    let gate = one_of(get(&b, "gate"), &machine::GATES, "gate")?.to_string();
    let bits = |v: &[String]| v.iter().fold(0u8, |a, g| a | (1 << machine::GATES.iter().position(|x| x == g).unwrap_or(0)));
    let index = machine::GATES.iter().position(|x| *x == gate).unwrap_or(0) as u8;
    match machine::gate_step(bits(&r.required_gates), bits(&r.passed_gates), index) {
        Ok(_) => {}
        Err(machine::GateRefusal::AlreadyCommitted) => return refuse(None, format!("{} is already committed in this session", gate)),
        Err(_) => return refuse(Some("Core 6.1"), format!("{} cannot be committed now; completed gates form an ordered prefix of the required gates", gate)),
    }
    let basis = check_basis(state, get(&b, "basis"), "the gate's evidence")?;
    let note = text(get(&b, "note"), "note", 2000, false)?;
    if gate != "G1" && basis.is_empty() && !truthy(&note) {
        return refuse(Some("Core 6.1, 7.1"), format!("{} is committed with its evidence: a reference or a note of what was done", gate));
    }
    if gate == "G5" && !state.claims_without_finding(r).is_empty() {
        return refuse(Some("Core 6.1"), "G5 needs a finding for each claim".to_string());
    }
    b.insert("basis".to_string(), items_value(&basis));
    b.insert("note".to_string(), note);
    Ok(b)
}

fn g_objection(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), true, true)?;
    seq_id(state, &mut b, O, "objection")?;
    match get(&b, "claim") {
        Value::Null => {}
        Value::Str(c) if r.claims.contains_key(c) => {}
        Value::List(_) | Value::Map(_) => return form(),
        _ => return refuse(None, "the claim is not in this review".to_string()),
    }
    let t = text(get(&b, "text"), "the objection", 8000, true)?;
    b.insert("text".to_string(), t);
    let basis = check_basis(state, get(&b, "basis"), "the objection's basis")?;
    b.insert("basis".to_string(), items_value(&basis));
    Ok(b)
}

fn g_objection_end(state: &State, mut b: Map) -> R<Map> {
    let (review, status) = match get(&b, "id") {
        Value::Str(id) => match state.objections.get(id) {
            Some(o) => o.clone(),
            None => return refuse(None, "no objection has that id".to_string()),
        },
        _ => return refuse(None, "no objection has that id".to_string()),
    };
    if status != "OPEN" {
        return refuse(None, format!("the objection is already {}", status));
    }
    state.review(&Value::Str(review), true, true)?;
    one_of(get(&b, "resolution"), &OBJECTION_ENDS, "resolution")?;
    let how = text(get(&b, "how"), "how it was resolved, or why it stays open", 2000, true)?;
    b.insert("how".to_string(), how);
    let basis = check_basis(state, get(&b, "basis"), "the resolution's basis")?;
    b.insert("basis".to_string(), items_value(&basis));
    Ok(b)
}

fn g_finding(state: &State, mut b: Map, actor: &str) -> R<Map> {
    let r = state.review(get(&b, "review"), true, true)?;
    if !r.passed_gates.iter().any(|g| g == "G1") {
        return refuse(Some("Core 6.1"), "no finding before G1: the subject, the claims and their origin are committed first".to_string());
    }
    let (claim_label, cl) = match get(&b, "claim") {
        Value::Str(c) => match r.claims.get(c) {
            Some(cl) => (c.clone(), cl),
            None => return refuse(Some("Core 5.2"), "a finding answers a claim as commissioned, and this claim is not in the review".to_string()),
        },
        Value::List(_) | Value::Map(_) => return form(),
        _ => return refuse(Some("Core 5.2"), "a finding answers a claim as commissioned, and this claim is not in the review".to_string()),
    };
    let disp = one_of(get(&b, "disposition"), &DISPOSITIONS, "disposition")?.to_string();
    seq_id(state, &mut b, F, "finding")?;
    let basis = check_basis(state, get(&b, "basis"), "the basis")?;
    let contrary = check_basis(state, get(&b, "contrary"), "the contrary evidence")?;
    let conds: Vec<Value> = match get(&b, "conditions") {
        Value::List(l) => l.clone(),
        v if !truthy(v) => Vec::new(),
        _ => return refuse(None, "conditions are a list of statements".to_string()),
    };
    let mut conditions = Vec::new();
    for c in &conds {
        conditions.push(text(c, "a condition", 2000, true)?);
    }
    let estimate = text(get(&b, "estimate"), "the estimate", 2000, false)?;
    let left_open = text(get(&b, "left_open"), "what is left open", 4000, false)?;
    let mut noncritical: Vec<String> = Vec::new();
    match get(&b, "noncritical") {
        Value::List(l) => {
            for x in l {
                match x {
                    Value::Str(x) => noncritical.push(x.clone()),
                    _ => return refuse(None, "noncritical is a list of finding ids".to_string()),
                }
            }
        }
        v if !truthy(v) => {}
        _ => return refuse(None, "noncritical is a list of finding ids".to_string()),
    }
    let every = || basis.iter().chain(contrary.iter());
    let checks: Vec<&Check> = every().filter(|i| i.t() == "check").filter_map(|i| get(&i.r, "id").as_str().and_then(|id| state.checks.get(id))).collect();
    let observed = {
        let a = observed_grounding(state, &basis);
        let c = observed_grounding(state, &contrary);
        if GROUNDING.iter().position(|g| *g == a) >= GROUNDING.iter().position(|g| *g == c) { a } else { c }
    };
    let rank = |g: &str| GROUNDING.iter().position(|x| *x == g).unwrap_or(0);
    let grounding = if !is_null(get(&b, "grounding_declared")) {
        let declared = one_of(get(&b, "grounding_declared"), &GROUNDING, "grounding")?;
        if rank(declared) > rank(observed) && !(declared == "RECALL_ONLY" && observed == "NONE") {
            return refuse(Some("Core 15.8"), format!("grounding {} is declared but the record holds inspected material only up to {}", declared, observed));
        }
        declared.to_string()
    } else if observed == "NONE" {
        return refuse(Some("Core 15.8"), "this finding cites nothing inspected or run; say what it rests on".to_string());
    } else {
        observed.to_string()
    };
    let none_cited = basis.is_empty() && contrary.is_empty();
    if matches!(disp.as_str(), "SUPPORTED_IN_SCOPE" | "REFUTED_IN_SCOPE" | "NOT_IDENTIFIED_IN_SCOPE") && none_cited {
        return refuse(Some("Core 5.2"), format!("{} needs something this review inspected or ran", disp));
    }
    if disp == "REFUTED_IN_SCOPE" && contrary.is_empty() {
        return refuse(Some("Core 5.2"), "REFUTED_IN_SCOPE needs checked contrary evidence".to_string());
    }
    if matches!(disp.as_str(), "SUPPORTED_IN_SCOPE" | "REFUTED_IN_SCOPE") && !cl.criterion {
        return refuse(Some("Core 5.1, 5.2"), format!("{} is assessed against a stated criterion, and the claim has none", disp));
    }
    if disp == "CONDITIONAL" && conditions.is_empty() {
        return refuse(Some("Core 5.2"), "CONDITIONAL names the essential assumptions it depends on".to_string());
    }
    if disp == "SUPPORTED_IN_SCOPE" {
        if cl.kind == "EXTERNAL_FACT" && matches!(grounding.as_str(), "RECALL_ONLY" | "NONE") {
            return refuse(Some("Core 15.8"), "a claim about an external fact that rests on recall cannot be SUPPORTED_IN_SCOPE".to_string());
        }
        if cl.kind == "MECHANICAL_CHECK" && !checks.iter().any(|k| k.exit == Some(k.expect_exit) && !k.timeout) {
            return refuse(Some("Core 5.1, 15.8"), "a mechanically checkable claim is supported by an executed check that met its expectation, and none is cited".to_string());
        }
        let weak = basis.iter().filter(|i| i.t() == "finding").filter_map(|i| get(&i.r, "id").as_str()).any(|id| {
            state.findings.get(id).map(|f| f.disposition != "SUPPORTED_IN_SCOPE").unwrap_or(false) && !noncritical.iter().any(|n| n == id)
        });
        if weak {
            return refuse(Some("Core 5.2"), "the basis includes a finding that is not SUPPORTED_IN_SCOPE".to_string());
        }
    }
    // What a review writes out for itself is its reasoning. Recording that text does not make it evidence the review checked
    // (Core 4.3: a further record supplies no further observation). Material that was in the record before the review was
    // commissioned, a check, a pinned tree and an earlier finding are not the review's own product.
    let own = |i: &Item| i.t() == "artifact" && get(&i.r, "sha256").as_str().and_then(|d| state.artifacts.get(d)).map(|a| a.produced && a.first > r.commissioned).unwrap_or(false);
    if disp == "REFUTED_IN_SCOPE" && contrary.iter().all(own) {
        return refuse(Some("Core 4.3, 5.2"), "REFUTED_IN_SCOPE needs checked contrary evidence, and all that is cited against the claim is text this review itself produced. Cite the check that was run or the material that was inspected; where there is neither, the claim is UNRESOLVED (Core 5.1)".to_string());
    }
    if disp == "SUPPORTED_IN_SCOPE" && every().all(own) {
        return refuse(Some("Core 4.3, 5.2"), "SUPPORTED_IN_SCOPE needs something this review inspected or ran, and all that is cited is text this review itself produced".to_string());
    }
    if cl.kind == "MECHANICAL_CHECK" && disp == "REFUTED_IN_SCOPE" && !checks.iter().any(|k| k.exit.is_some() && !k.timeout) {
        return refuse(Some("Core 5.1"), "a mechanically checkable claim is refuted by an executed check that ran to its end, and none is cited".to_string());
    }
    noncritical.sort();
    b.insert("basis".to_string(), items_value(&basis));
    b.insert("contrary".to_string(), items_value(&contrary));
    b.insert("conditions".to_string(), Value::List(conditions));
    b.insert("estimate".to_string(), estimate);
    b.insert("left_open".to_string(), left_open);
    // EXECUTED says the record itself holds that a check ran. Where every cited check is a participant's report that no
    // host saw, the record does not know that, and says so (Core 15.8).
    b.insert("execution".to_string(), s(if checks.is_empty() { "REASONED" } else if checks.iter().all(|k| k.reported) { "UNKNOWN" } else { "EXECUTED" }));
    b.insert("grounding_observed".to_string(), s(observed));
    b.insert("grounding".to_string(), Value::Str(grounding));
    b.insert("noncritical".to_string(), Value::List(noncritical.iter().map(|x| s(x)).collect()));
    b.insert("relation".to_string(), relation(state, r, actor));
    b.insert("supersedes".to_string(), r.current.get(&(claim_label, actor.to_string())).map(|x| s(x)).unwrap_or(Value::Null));
    b.insert("session".to_string(), Value::Int(r.session));
    Ok(b)
}

fn strs(v: &[String]) -> Value {
    Value::List(v.iter().map(|x| s(x)).collect())
}

fn g_close(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), true, false)?;
    let lifecycle = one_of(get(&b, "lifecycle"), &TERMINAL, "closing lifecycle")?.to_string();
    let event = match lifecycle.as_str() {
        "CLOSED_COMPLETE" => ReviewEvent::CloseComplete,
        "CLOSED_INCOMPLETE" => ReviewEvent::CloseIncomplete,
        _ => ReviewEvent::Abort,
    };
    if let Err(e) = r.lifecycle.step(event) {
        return refuse(None, format!("{}", e));
    }
    let without = state.claims_without_finding(r);
    if lifecycle == "CLOSED_COMPLETE" {
        if r.passed_gates != r.required_gates {
            return refuse(Some("Core 7.1"), "CLOSED_COMPLETE needs every required gate".to_string());
        }
        if !without.is_empty() {
            return refuse(Some("Core 5.2, 7.1"), "CLOSED_COMPLETE needs a finding for each commissioned claim".to_string());
        }
    }
    let mut cov = Map::new();
    cov.insert("passed_gates".to_string(), strs(&r.passed_gates));
    cov.insert("required_gates".to_string(), strs(&r.required_gates));
    cov.insert("claims_without_finding".to_string(), strs(&without));
    cov.insert("open_objections".to_string(), strs(&state.open_objections(&r.label)));
    cov.insert("findings".to_string(), Value::Int(state.findings.values().filter(|f| f.review == r.label && f.session == r.session).count() as i64));
    b.insert("coverage".to_string(), Value::Map(cov));
    let ext = if truthy(get(&b, "examined_extent")) { one_of(get(&b, "examined_extent"), &EXTENTS, "examined extent")?.to_string() } else { "UNKNOWN".to_string() };
    b.insert("examined_extent".to_string(), Value::Str(ext));
    let note = text(get(&b, "note"), "note", 2000, false)?;
    b.insert("note".to_string(), note);
    Ok(b)
}

fn g_session(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), false, false)?;
    if r.lifecycle.step(ReviewEvent::OpenSession).is_err() {
        return refuse(Some("Core 7.1"), format!("review {} has an open session", r.label));
    }
    b.insert("session".to_string(), Value::Int(r.session + 1));
    let reason = text(get(&b, "reason"), "the reason for a successor session", 2000, true)?;
    b.insert("reason".to_string(), reason);
    Ok(b)
}

fn g_pause(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), true, false)?;
    if let Err(e) = r.lifecycle.step(ReviewEvent::Pause) {
        return refuse(None, format!("{}", e));
    }
    let reason = text(get(&b, "reason"), "the reason for the pause", 2000, true)?;
    b.insert("reason".to_string(), reason);
    let brief = check_basis(state, get(&b, "brief"), "the state brief")?;
    b.insert("brief".to_string(), items_value(&brief));
    Ok(b)
}

fn g_resume(state: &State, mut b: Map) -> R<Map> {
    let r = state.review(get(&b, "review"), true, false)?;
    if let Err(e) = r.lifecycle.step(ReviewEvent::Resume) {
        return refuse(None, format!("{}", e));
    }
    let note = text(get(&b, "note"), "note", 2000, false)?;
    b.insert("note".to_string(), note);
    Ok(b)
}

fn g_condition(state: &State, mut b: Map) -> R<Map> {
    seq_id(state, &mut b, C, "condition")?;
    let t = text(get(&b, "text"), "the condition, in its original words", 4000, true)?;
    b.insert("text".to_string(), t);
    let o = text(get(&b, "origin"), "where the condition comes from", 500, true)?;
    b.insert("origin".to_string(), o);
    if !is_null(get(&b, "review")) {
        state.review(get(&b, "review"), false, false)?;
    }
    Ok(b)
}

fn g_submission(state: &State, mut b: Map) -> R<Map> {
    let label = check_label(get(&b, "label"), "submission label")?;
    if state.submissions.contains(label) {
        return refuse(None, format!("a submission labelled {} is already admitted", label));
    }
    if one_of(get(&b, "compatibility"), &COMPATIBILITY, "compatibility")? == "DIFFERENT_METHOD_ASSESSED" {
        let note = text(get(&b, "note"), "the compatibility assessment", 2000, true)?;
        b.insert("note".to_string(), note);
    }
    Ok(b)
}

fn g_obligation(state: &State, mut b: Map) -> R<Map> {
    seq_id(state, &mut b, B, "obligation")?;
    let t = text(get(&b, "text"), "the obligation", 4000, true)?;
    b.insert("text".to_string(), t);
    Ok(b)
}

fn g_obligation_end(state: &State, mut b: Map) -> R<Map> {
    if !matches!(get(&b, "id"), Value::Str(id) if state.obligations.get(id) == Some(&true)) {
        return refuse(None, "no open obligation has that id".to_string());
    }
    let how = text(get(&b, "how"), "how it was discharged", 2000, true)?;
    b.insert("how".to_string(), how);
    Ok(b)
}

fn g_run(state: &State, mut b: Map) -> R<Map> {
    seq_id(state, &mut b, RUN, "run")?;
    if truthy(get(&b, "trace")) {
        one_of(get(&b, "trace"), &TRACE, "trace")?;
    }
    Ok(b)
}

fn g_system(state: &State, mut b: Map) -> R<Map> {
    let from = state.system.map(|x| s(x.name())).unwrap_or(Value::Null);
    if get(&b, "event").as_str() == Some("BOOT") {
        b.insert("from".to_string(), from);
        b.insert("to".to_string(), s("START"));
        // The run before this one did not stop in order.
        b.insert("unclean".to_string(), Value::Bool(!matches!(state.system, None | Some(System::Stopped))));
    } else {
        let now = match state.system {
            Some(x) => x,
            None => return refuse(None, "no start of the system is recorded; the first SYSTEM_STATE event is BOOT".to_string()),
        };
        let event = match get(&b, "event").as_str().and_then(SystemEvent::from_name) {
            Some(e) => e,
            None => return refuse(None, "that is not an event of the system".to_string()),
        };
        let next = match now.step(event) {
            Ok(n) => n,
            Err(e) => return refuse(None, format!("{}", e)),
        };
        b.insert("from".to_string(), from);
        b.insert("to".to_string(), s(next.name()));
    }
    let reason = text(get(&b, "reason"), "reason", 2000, false)?;
    b.insert("reason".to_string(), reason);
    match get(&b, "authorized_by") {
        Value::Null => {}
        Value::List(l) if l.iter().all(|x| matches!(x, Value::Str(_))) => {}
        _ => return refuse(None, "authorized_by is a list of participant labels".to_string()),
    }
    Ok(b)
}

fn g_link(state: &State, mut b: Map) -> R<Map> {
    let event = match get(&b, "event").as_str() {
        Some("UP") => LinkEvent::Up,
        Some("DOWN") => LinkEvent::Down,
        _ => return refuse(None, "a connectivity event is UP or DOWN".to_string()),
    };
    let next = state.link().step(event).map_err(|e| Refusal { code: REFUSED, clause: None, message: format!("{}", e) })?;
    b.insert("to".to_string(), s(next.name()));
    Ok(b)
}

/// Delivery of the method text to a participant, and what undoes it. Written by the system, or by a
/// participant about what happened in its own session.
fn g_delivery(state: &State, mut b: Map, actor: &str) -> R<Map> {
    if actor != "host" && !state.participants.contains_key(actor) {
        return refuse(None, format!("the acting participant {} is not registered", actor));
    }
    let agent = match get(&b, "agent") {
        Value::Str(a) if state.participants.contains_key(a) => a.clone(),
        _ => return refuse(None, "delivery is recorded for a registered participant".to_string()),
    };
    let event = match get(&b, "event").as_str().and_then(DeliveryEvent::from_name) {
        Some(e) => e,
        None => return refuse(None, "a delivery event is DELIVER, CONTEXT_RESET or RELEASE_CHANGED".to_string()),
    };
    if event == DeliveryEvent::Deliver {
        if !is_hex64(get(&b, "text")) {
            return refuse(None, "a delivery names what was delivered by its SHA-256".to_string());
        }
    } else if !is_null(get(&b, "text")) {
        return refuse(None, "only a delivery names a text".to_string());
    }
    let now = state.delivery.get(&agent).copied().unwrap_or(Delivery::NotDelivered);
    let next = now.step(event).map_err(|e| Refusal { code: REFUSED, clause: None, message: format!("{}", e) })?;
    b.insert("to".to_string(), s(next.name()));
    Ok(b)
}

fn g_task_start(state: &State, mut b: Map) -> R<Map> {
    seq_id(state, &mut b, T, "task")?;
    check_label(get(&b, "label"), "task label")?;
    one_of(get(&b, "worker"), &WORKERS, "worker")?;
    if !is_null(get(&b, "review")) {
        state.review(get(&b, "review"), true, true)?;
    }
    for k in ["quiet_limit", "budget"] {
        if nat(get(&b, k)).is_none() {
            return refuse(None, format!("a task records its {} as a whole number, zero or more", k));
        }
    }
    Ok(b)
}

fn g_task_end(state: &State, mut b: Map) -> R<Map> {
    let t = match get(&b, "id") {
        Value::Str(id) => state.tasks.get(id),
        _ => None,
    };
    let t = match t {
        Some(t) => t,
        None => return refuse(None, "no task with that id was started in this record".to_string()),
    };
    if t.outcome.is_some() {
        return refuse(None, "the task has already ended".to_string());
    }
    let outcome = one_of(get(&b, "outcome"), &TASK_ENDS, "outcome")?;
    for k in ["result", "read_back"] {
        if !is_null(get(&b, k)) && !is_hex64(get(&b, k)) {
            return refuse(None, "a result, and what was read back, are each named by SHA-256".to_string());
        }
    }
    if outcome == "DONE" {
        // Completion is a fact the supervisor observed about this attempt, not the worker's report and not
        // the presence of some artifact: the digest read back after the exit equals the digest reported, the
        // bytes were produced in this record after the task started, and that observation settles one task.
        if t.stop_requested.is_some() {
            return refuse(Some("Core 7.3"), "a stop was requested for this task: it ends STOPPED, FAILED or LOST, and what it produced afterwards is not its result".to_string());
        }
        let a = match get(&b, "result").as_str().and_then(|d| state.artifacts.get(d)) {
            Some(a) => a,
            None => return refuse(Some("Core 7.3"), "a task is DONE only when its result is in the record; none is named, or the one named is not there".to_string()),
        };
        if get(&b, "read_back") != get(&b, "result") {
            return refuse(Some("Core 7.3"), "a task is DONE only when the digest read back after it exited equals the digest it reported".to_string());
        }
        if a.origin != "PRODUCED" {
            return refuse(Some("Core 7.3"), "a task's result is something produced in this record, not supplied or retrieved material".to_string());
        }
        if a.last <= t.started {
            return refuse(Some("Core 7.3"), "the result was recorded before the task started; record what was read back after it exited".to_string());
        }
        if state.consumed.contains(&a.last) {
            return refuse(Some("Core 7.3"), "that record of the result already settled another task; record this task's own read-back".to_string());
        }
    }
    if nat(get(&b, "spent")).is_none() {
        return refuse(None, "a task records what it spent as a whole number, zero or more".to_string());
    }
    let note = text(get(&b, "note"), "note", 2000, false)?;
    b.insert("note".to_string(), note);
    Ok(b)
}

fn read_standard(b: &Map) -> R<StandardNow> {
    let method = match one_of(get(b, "method"), &STANDARD_METHOD, "method")? {
        "NATIVE_ONLY" => Method::NativeOnly,
        _ => Method::NativeOrPinned,
    };
    let model_findings = match one_of(get(b, "model_findings"), &STANDARD_MODEL_FINDINGS, "model_findings")? {
        "MECHANICAL_ONLY" => ModelFindings::MechanicalOnly,
        "HUMAN_CLOSES" => ModelFindings::HumanCloses,
        _ => ModelFindings::AsMethod,
    };
    let mut flags = [false; 4];
    for (i, k) in STANDARD_FLAGS.iter().enumerate() {
        match get(b, k) {
            Value::Bool(x) => flags[i] = *x,
            _ => return refuse(None, format!("{} is true or false", k)),
        }
    }
    let mut tasks: Vec<String> = Vec::new();
    match get(b, "tasks") {
        Value::List(l) => {
            for t in l {
                match t {
                    Value::Str(t) if label_ok(t) && !tasks.contains(t) => tasks.push(t.clone()),
                    _ => return refuse(None, "tasks is a list of distinct task labels".to_string()),
                }
            }
        }
        _ => return refuse(None, "tasks is a list of distinct task labels".to_string()),
    }
    if !tasks.is_empty() && !flags[2] {
        return refuse(None, "tasks are listed only when the set of tasks is fixed".to_string());
    }
    Ok(StandardNow { std: Standard { method, model_findings, human_adoption: flags[0], dual_authorization: flags[1], static_tasks: flags[2], offline_only: flags[3] }, tasks })
}

fn g_standard(state: &State, mut b: Map, actor: &str) -> R<Map> {
    let new = read_standard(&b)?;
    if !state.human_principal(get(&b, "by")) || get(&b, "by").as_str() != Some(actor) {
        return refuse(None, "a standard is set by a person directing the work, acting as themselves".to_string());
    }
    let old = state.standard_now();
    let relaxes = !new.std.at_least(&old.std) || (old.std.static_tasks && !new.tasks.iter().all(|t| old.tasks.contains(t)));
    b.insert("relaxes".to_string(), Value::Bool(relaxes));
    let second = get(&b, "second");
    if !is_null(second) && (!state.human_principal(second) || second == get(&b, "by")) {
        return refuse(None, "the second authorization is that of a different person directing the work".to_string());
    }
    if relaxes && old.std.dual_authorization && is_null(second) {
        return refuse(None, "the standard in force requires two people to relax it".to_string());
    }
    Ok(b)
}

/// What the installation's standard adds to the method's rules. A standard narrows; it never widens.
fn standard_check(state: &State, kind: &str, b: &Map, actor: &str) -> R<()> {
    let st = state.standard_now();
    if st.std.method == Method::NativeOnly {
        if kind == "METHOD_PINNED" && get(b, "presence").as_str() != Some("NATIVE") {
            return refuse(None, "the standard in force admits only the method built into the system release".to_string());
        }
        if kind == "REVIEW_COMMISSIONED" && state.method_presence.as_ref().and_then(|p| p.as_deref()) != Some("NATIVE") {
            return refuse(None, "the standard in force admits only the method built into the system release, and another is pinned".to_string());
        }
    }
    if st.std.human_adoption && kind == "ADOPTED" && (!state.human_principal(get(b, "by")) || get(b, "by").as_str() != Some(actor)) {
        return refuse(Some("Core 15.1"), "the standard in force lets only a person adopt ACR use, acting as themselves".to_string());
    }
    if st.std.offline_only {
        if kind == "ARTIFACT_ADDED" && get(b, "origin").as_str() == Some("RETRIEVED") {
            return refuse(None, "the standard in force keeps this system offline; nothing is retrieved".to_string());
        }
        if kind == "LINK_STATE" && get(b, "event").as_str() == Some("UP") {
            return refuse(None, "the standard in force keeps this system offline".to_string());
        }
    }
    if st.std.static_tasks && kind == "TASK_STARTED" && !matches!(get(b, "label").as_str(), Some(l) if st.tasks.iter().any(|t| t == l)) {
        return refuse(None, "the standard in force fixes the set of tasks, and this is not one of them".to_string());
    }
    if st.std.dual_authorization && kind == "SYSTEM_STATE" && get(b, "event").as_str() == Some("MAINTENANCE_BEGIN") {
        let ok = matches!(get(b, "authorized_by"), Value::List(l) if l.len() == 2 && l[0] != l[1] && l.iter().all(|x| state.human_principal(x)));
        if !ok {
            return refuse(None, "the standard in force requires two different people to authorize maintenance".to_string());
        }
    }
    if st.std.model_findings >= ModelFindings::HumanCloses && kind == "REVIEW_CLOSED" && get(b, "lifecycle").as_str() == Some("CLOSED_COMPLETE") {
        let human = matches!(state.participants.get(actor), Some(p) if p.kind == "HUMAN");
        if !human {
            if let Some(r) = get(b, "review").as_str().and_then(|l| state.reviews.get(l)) {
                if r.current.keys().any(|(_, a)| state.model_driven(a)) {
                    return refuse(None, "the standard in force has a person close, as complete, a review that holds a model's finding".to_string());
                }
            }
        }
    }
    if st.std.model_findings == ModelFindings::MechanicalOnly && kind == "FINDING_RECORDED" && state.model_driven(actor) {
        let mechanical = get(b, "review").as_str().and_then(|l| state.reviews.get(l)).and_then(|r| get(b, "claim").as_str().and_then(|c| r.claims.get(c)))
            .map(|c| c.kind == "MECHANICAL_CHECK").unwrap_or(false);
        if matches!(get(b, "disposition").as_str(), Some("SUPPORTED_IN_SCOPE" | "REFUTED_IN_SCOPE")) && !mechanical {
            return refuse(None, "the standard in force lets a model record this disposition only on a claim that is a mechanical check".to_string());
        }
    }
    Ok(())
}

// ---- applying events ------------------------------------------------------------------------------------

fn integrity<T>(seq: i64, kind: &str) -> R<T> {
    Err(Refusal { code: INTEGRITY, clause: None, message: format!("event {} ({}) refers to something the record does not hold, or lacks a field", seq, kind) })
}

/// Apply one event of a verified register to the state. The event is taken as written: guards are not
/// run here. An event that lacks what its kind must carry is an integrity failure.
pub fn apply(state: &mut State, ev: &Value) -> R<()> {
    let (kind, b, actor, seq) = match (ev.get("kind").and_then(Value::as_str), ev.get("body"), ev.get("actor").and_then(Value::as_str), ev.get("seq").and_then(Value::as_int)) {
        (Some(k), Some(Value::Map(b)), Some(a), Some(n)) => (k, b, a, n),
        _ => return integrity(state.seq + 1, "?"),
    };
    state.seq = seq;
    let st = |k: &str| get(b, k).as_str().map(|x| x.to_string());
    let need = |k: &str| -> R<String> { st(k).ok_or(()).or_else(|_| integrity(seq, kind)) };
    match kind {
        "WORKSPACE_CREATED" => {
            state.created = true;
            state.workspace = st("workspace").unwrap_or_else(|| "local".to_string());
        }
        "METHOD_PINNED" => {
            if state.method_presence.is_some() {
                // A changed release: whatever text was delivered before is no longer the pinned one.
                for d in state.delivery.values_mut() {
                    *d = d.step(DeliveryEvent::ReleaseChanged).unwrap_or(*d);
                }
            }
            state.method_presence = Some(st("presence"));
        }
        "DELIVERY_STATE" => {
            let to = st("to").and_then(|x| Delivery::from_name(&x)).ok_or(()).or_else(|_| integrity(seq, kind))?;
            state.delivery.insert(need("agent")?, to);
        }
        "PARTICIPANT_ADDED" => {
            state.participants.insert(need("label")?, Participant { kind: need("kind")?, principal: get(b, "principal") == &Value::Bool(true),
                                                                     model: st("model"), lineage: st("lineage") });
        }
        "ADOPTED" => {
            state.adopted = true;
            state.adoption = Some((st("by").unwrap_or_default(), actor.to_string()));
        }
        "HALTED" => {
            state.adopted = false;
            state.adoption = None;
            if let Value::List(closed) = get(b, "closed") {
                for l in closed {
                    match l.as_str().and_then(|l| state.reviews.get_mut(l)) {
                        Some(r) => r.lifecycle = Review::ClosedIncomplete,
                        None => return integrity(seq, kind),
                    }
                    // The halt asks the tasks of each review it closes to stop. Asking is all the record can say.
                    for t in state.tasks.values_mut() {
                        if t.outcome.is_none() && t.stop_requested.is_none() && t.review.as_deref() == l.as_str() {
                            t.stop_requested = Some(seq);
                        }
                    }
                }
            }
        }
        "ARTIFACT_ADDED" => {
            let sha = need("sha256")?;
            // `last` is where the bytes were last recorded as PRODUCED (or first recorded at all): recording them again as
            // supplied material does not make an earlier product look like the result of a later task.
            let origin = need("origin")?;
            let produced = origin == "PRODUCED";
            let a = state.artifacts.entry(sha.clone()).or_insert(Artifact { origin, last: seq, first: seq, produced });
            if produced {
                a.last = seq;
                a.produced = true;
            }
            if let Some(n) = st("name").filter(|n| !n.is_empty()) {
                state.names.insert(n, sha);
            }
        }
        "TREE_PINNED" => {
            state.trees.insert(need("name")?);
        }
        "ARTIFACT_READ" | "INSPECTION_DECLARED" => match get(b, "ref") {
            Value::Map(r) => {
                state.inspections.insert(ref_key(r).or_else(|_| integrity(seq, kind))?);
            }
            _ => return integrity(seq, kind),
        },
        "CHECK_RAN" => {
            state.checks.insert(need("id")?, Check { exit: get(b, "exit").as_int(), expect_exit: get(b, "expect_exit").as_int().unwrap_or(i64::MIN),
                                                     timeout: get(b, "timeout") != &Value::Bool(false),
                                                     reported: get(b, "observed").as_str() == Some("REPORTED_BY_PARTICIPANT") });
            state.counters[K] += 1;
        }
        "REVIEW_COMMISSIONED" => {
            let label = need("label")?;
            let mut r = ReviewState { label: label.clone(), lifecycle: Review::Active, session: 1, procedure: need("procedure")?, required_gates: Vec::new(),
                                      passed_gates: Vec::new(), claims: BTreeMap::new(), claim_order: Vec::new(), current: BTreeMap::new(), author: st("author"), director: need("director")?, commissioned: seq };
            match (get(b, "required_gates"), get(b, "claims")) {
                (Value::List(g), Value::List(cs)) => {
                    r.required_gates = g.iter().filter_map(|x| x.as_str().map(|x| x.to_string())).collect();
                    for c in cs {
                        add_claim(&mut r, c).or_else(|_| integrity(seq, kind))?;
                    }
                }
                _ => return integrity(seq, kind),
            }
            state.reviews.insert(label, r);
        }
        "CLAIM_ADDED" => match state.reviews.get_mut(&need("review")?) {
            Some(r) => add_claim(r, get(b, "claim")).or_else(|_| integrity(seq, kind))?,
            None => return integrity(seq, kind),
        },
        "GATE_COMMITTED" => {
            let g = need("gate")?;
            match state.reviews.get_mut(&need("review")?) {
                Some(r) => r.passed_gates.push(g),
                None => return integrity(seq, kind),
            }
        }
        "OBJECTION_RAISED" => {
            state.objections.insert(need("id")?, (need("review")?, "OPEN".to_string()));
            state.counters[O] += 1;
        }
        "OBJECTION_RESOLVED" => match state.objections.get_mut(&need("id")?) {
            Some(o) => o.1 = need("resolution")?,
            None => return integrity(seq, kind),
        },
        "FINDING_RECORDED" => {
            let (id, review, cl) = (need("id")?, need("review")?, need("claim")?);
            let session = get(b, "session").as_int().unwrap_or(0);
            state.findings.insert(id.clone(), Finding { review: review.clone(), session, disposition: need("disposition")? });
            state.counters[F] += 1;
            match state.reviews.get_mut(&review) {
                Some(r) => r.current.insert((cl, actor.to_string()), id),
                None => return integrity(seq, kind),
            };
            for key in ["basis", "contrary"] {
                if let Value::List(items) = get(b, key) {
                    for it in items {
                        if let (Some(Value::Map(r)), Some(e)) = (it.get("ref"), it.get("extent")) {
                            if truthy(e) {
                                if let Ok(k) = ref_key(r) {
                                    state.inspections.insert(k);
                                }
                            }
                        }
                    }
                }
            }
        }
        "REVIEW_CLOSED" | "REVIEW_PAUSED" | "REVIEW_RESUMED" => {
            let to = match kind {
                "REVIEW_PAUSED" => Some(Review::Paused),
                "REVIEW_RESUMED" => Some(Review::Active),
                _ => st("lifecycle").and_then(|l| Review::from_name(&l)),
            };
            match (state.reviews.get_mut(&need("review")?), to) {
                (Some(r), Some(to)) => r.lifecycle = to,
                _ => return integrity(seq, kind),
            }
        }
        "SESSION_OPENED" => match (state.reviews.get_mut(&need("review")?), get(b, "session").as_int()) {
            (Some(r), Some(n)) => {
                r.session = n;
                r.lifecycle = Review::Active;
                r.passed_gates.clear();
                r.current.clear();
            }
            _ => return integrity(seq, kind),
        },
        "CONDITION_REGISTERED" => state.counters[C] += 1,
        "SUBMISSION_ADMITTED" => {
            state.submissions.insert(need("label")?);
        }
        "OBLIGATION_OPENED" => {
            state.obligations.insert(need("id")?, true);
            state.counters[B] += 1;
        }
        "OBLIGATION_CLOSED" => match state.obligations.get_mut(&need("id")?) {
            Some(o) => *o = false,
            None => return integrity(seq, kind),
        },
        "RUN_COMPLETED" => state.counters[RUN] += 1,
        "SYSTEM_STATE" => {
            state.system = Some(st("to").and_then(|x| System::from_name(&x)).ok_or(()).or_else(|_| integrity(seq, kind))?);
            if get(b, "unclean") == &Value::Bool(true) {
                state.unclean_starts += 1;
            }
            // A fault, a failed verification, a shutdown or maintenance asks every task to stop.
            if st("event").and_then(|e| SystemEvent::from_name(&e)).and_then(machine::cascade_of_system).is_some() {
                for t in state.tasks.values_mut() {
                    if t.outcome.is_none() && t.stop_requested.is_none() {
                        t.stop_requested = Some(seq);
                    }
                }
            }
        }
        "LINK_STATE" => state.link = Some(st("to").and_then(|x| Link::from_name(&x)).ok_or(()).or_else(|_| integrity(seq, kind))?),
        "TASK_STARTED" => {
            let hook_attempt = if ev.get("auth").and_then(Value::as_str) == Some("HOST")
                && get(b, "observed").as_str() == Some("SUBAGENT_START") {
                Some(st("hook_attempt_key").map_or(HookAttempt::Legacy, HookAttempt::Key))
            } else {
                None
            };
            state.tasks.insert(need("id")?, TaskState { label: need("label")?, worker: need("worker")?, review: st("review"), actor: actor.to_string(), outcome: None,
                                                        spent: 0, budget: get(b, "budget").as_int().unwrap_or(0), started: seq, stop_requested: None,
                                                        result: None, attempt: st("attempt"),
                                                        hook_attempt });
            state.counters[T] += 1;
        }
        "TASK_ENDED" => {
            let settled = if st("outcome").as_deref() == Some("DONE") { st("result").and_then(|d| state.artifacts.get(&d)).map(|a| a.last) } else { None };
            match state.tasks.get_mut(&need("id")?) {
                Some(t) => {
                    t.outcome = Some(need("outcome")?);
                    t.spent = get(b, "spent").as_int().unwrap_or(0);
                    t.result = st("result");
                }
                None => return integrity(seq, kind),
            }
            if let Some(n) = settled {
                state.consumed.insert(n);
            }
        }
        "STANDARD_SET" => state.standard = Some(read_standard(b).or_else(|_| integrity(seq, kind))?),
        _ => {}
    }
    Ok(())
}

fn add_claim(r: &mut ReviewState, c: &Value) -> Result<(), ()> {
    let label = c.get("label").and_then(Value::as_str).ok_or(())?.to_string();
    let kind = c.get("kind").and_then(Value::as_str).unwrap_or("UNSPECIFIED").to_string();
    let criterion = matches!(c.get("criterion"), Some(Value::Str(x)) if !x.is_empty());
    r.claims.insert(label.clone(), Claim { kind, criterion });
    r.claim_order.push(label);
    Ok(())
}

/// A guard that an event already in a register would not have passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The event's sequence number.
    pub seq: i64,
    /// What is wrong with it.
    pub message: String,
}

/// Derive the state from the events of a verified register. With `check`, every guard is replayed and
/// events that would not have passed, or whose recorded fields differ from what the guards compute, are
/// collected. An event of a kind this implementation does not define is an integrity failure, except a
/// declared extension (`X_...`), which is carried and not interpreted.
pub fn fold(events: &[Value], check: bool) -> R<(State, Vec<Problem>)> {
    let mut state = State::new();
    let mut problems = Vec::new();
    fold_onto(&mut state, &mut problems, events, check)?;
    Ok((state, problems))
}

/// Continue a replay: the same steps as [`fold`], onto a state that already holds what came before.
fn fold_onto(state: &mut State, problems: &mut Vec<Problem>, events: &[Value], check: bool) -> R<()> {
    for ev in events {
        let seq = ev.get("seq").and_then(Value::as_int).unwrap_or(-1);
        let kind = ev.get("kind").and_then(Value::as_str).unwrap_or("");
        if !is_kind(kind) {
            if kind.starts_with("X_") {
                state.seq = seq;
                continue;
            }
            return Err(Refusal { code: INTEGRITY, clause: None, message: format!("event {} has kind {}, which this implementation does not define", seq, kind) });
        }
        if check {
            let actor = ev.get("actor").and_then(Value::as_str).unwrap_or("");
            match guard(state, kind, ev.get("body").unwrap_or(&NULL), actor) {
                Ok(expect) if Some(&expect) == ev.get("body") => {}
                Ok(_) => problems.push(Problem { seq, message: "recorded fields differ from what the guards compute".to_string() }),
                Err(e) => problems.push(Problem { seq, message: e.message }),
            }
        }
        apply(state, ev)?;
    }
    Ok(())
}

// ---- the record: guards, register and state kept together -------------------------------------------------

/// Why a submission to a [`Record`] did not result in an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// A guard refused it.
    Refused(Refusal),
    /// The register refused it.
    Ledger(LedgerError),
}

/// A register together with the state derived from it. The only way to add an event is [`Record::submit`],
/// which runs the guards, appends what they return, and applies it: the three cannot drift apart.
pub struct Record {
    ledger: Ledger,
    state: State,
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

impl Record {
    /// An empty record.
    pub fn new() -> Self {
        Record { ledger: Ledger::new(), state: State::new() }
    }

    /// Adopt a register received from elsewhere: its chain is verified and every guard replayed. Problems
    /// found by the replay are returned beside the record; the caller decides what follows from them.
    pub fn open(raw: &[u8]) -> Result<(Self, Vec<Problem>), SubmitError> {
        let (events, _) = crate::ledger::verify(raw).map_err(SubmitError::Ledger)?;
        let (state, problems) = fold(&events, true).map_err(SubmitError::Refused)?;
        let ledger = Ledger::from_bytes(raw).map_err(SubmitError::Ledger)?;
        Ok((Record { ledger, state }, problems))
    }

    /// Take in events that were appended to the same register elsewhere since this record last saw it:
    /// the chain is continued, not checked again from the start, and every guard is replayed over the new
    /// events only. The result is the record [`Record::open`] would give for the whole. On an error the
    /// record must be discarded: its register and its state may no longer agree.
    pub fn extend(&mut self, tail: &[u8]) -> Result<Vec<Problem>, SubmitError> {
        let events = self.ledger.extend(tail).map_err(SubmitError::Ledger)?;
        let mut problems = Vec::new();
        fold_onto(&mut self.state, &mut problems, &events, true).map_err(SubmitError::Refused)?;
        Ok(problems)
    }

    /// Offer an event. On success the event is in the register and the state reflects it.
    pub fn submit(&mut self, ts: &str, actor: &str, auth: &str, kind: &str, body: Value) -> Result<&str, SubmitError> {
        let out = guard(&self.state, kind, &body, actor).map_err(SubmitError::Refused)?;
        let seq = self.ledger.len() as i64;
        self.ledger.append(ts, actor, auth, kind, out.clone()).map_err(SubmitError::Ledger)?;
        let mut ev = Map::new();
        ev.insert("seq".to_string(), Value::Int(seq));
        ev.insert("kind".to_string(), s(kind));
        ev.insert("actor".to_string(), s(actor));
        ev.insert("body".to_string(), out);
        // The guard has just accepted this body, so applying it cannot fail; if it does, the two have diverged.
        apply(&mut self.state, &Value::Map(ev)).map_err(SubmitError::Refused)?;
        Ok(self.ledger.head())
    }

    /// The state.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The register.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }
}
