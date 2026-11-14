//! The register: an append-only sequence of events, each bound to the one before it by SHA-256.
//!
//! The type has no operation that removes or changes an event. Checking a register received from
//! elsewhere applies the same rules as writing one, so a record reads the same to every party.
use crate::value::{write_int, write_str, ParseError, Value};
use crate::{sha256_hex, ZERO};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The fields every event has, and no others.
pub const EVENT_KEYS: [&str; 7] = ["actor", "auth", "body", "kind", "prev", "seq", "ts"];
/// How the actor of an event is known: declared by the caller, or bound by the host.
pub const AUTH: [&str; 2] = ["DECLARED", "HOST"];
/// Largest event, in bytes of its canonical form.
pub const MAX_EVENT_BYTES: usize = 64 * 1024;

/// Why a register is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    /// The bytes end in an incomplete line: an interrupted write.
    IncompleteFinalLine,
    /// A line is not an acceptable value.
    Parse(usize, ParseError),
    /// A line is not an event of the right shape, or is not in canonical form.
    Malformed(usize, &'static str),
    /// A line does not follow from the one before it.
    BrokenChain(usize),
    /// An event to append is too large or badly formed.
    Refused(&'static str),
}

fn is_ts(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b'T',
            13 | 16 => c == b':',
            19 => c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

fn check_event(n: usize, line: &[u8], prev: &str) -> Result<Value, LedgerError> {
    let ev = Value::parse(line).map_err(|e| LedgerError::Parse(n + 1, e))?;
    let m = match &ev {
        Value::Map(m) => m,
        _ => return Err(LedgerError::Malformed(n + 1, "not an object")),
    };
    if m.len() != EVENT_KEYS.len() || !EVENT_KEYS.iter().all(|k| m.contains_key(*k)) {
        return Err(LedgerError::Malformed(n + 1, "does not have exactly the event fields"));
    }
    if ev.canonical_json() != line {
        return Err(LedgerError::Malformed(n + 1, "not in canonical form"));
    }
    let ok = matches!(m.get("ts"), Some(Value::Str(s)) if is_ts(s))
        && matches!(m.get("auth"), Some(Value::Str(s)) if AUTH.contains(&s.as_str()))
        && matches!(m.get("kind"), Some(Value::Str(_)))
        && matches!(m.get("actor"), Some(Value::Str(_)))
        && matches!(m.get("body"), Some(Value::Map(_)));
    if !ok {
        return Err(LedgerError::Malformed(n + 1, "a field has the wrong form"));
    }
    if m.get("seq").and_then(Value::as_int) != Some(n as i64) || m.get("prev").and_then(Value::as_str) != Some(prev) {
        return Err(LedgerError::BrokenChain(n + 1));
    }
    Ok(ev)
}

/// Check a whole register. Returns its events and its head: the digest of the last line, or zeros if empty.
pub fn verify(raw: &[u8]) -> Result<(Vec<Value>, String), LedgerError> {
    verify_tail(raw, 0, ZERO)
}

/// Check the lines that follow a part already checked: `first` events came before, and the last of them
/// has the digest `prev`. The same checks as [`verify`], which is this with nothing before.
pub fn verify_tail(raw: &[u8], first: usize, prev: &str) -> Result<(Vec<Value>, String), LedgerError> {
    if !raw.is_empty() && raw[raw.len() - 1] != b'\n' {
        return Err(LedgerError::IncompleteFinalLine);
    }
    let mut events = Vec::new();
    let mut prev = prev.to_string();
    if raw.is_empty() {
        return Ok((events, prev));
    }
    for (n, line) in raw[..raw.len() - 1].split(|&b| b == b'\n').enumerate() {
        let ev = check_event(first + n, line, &prev)?;
        prev = sha256_hex(line);
        events.push(ev);
    }
    Ok((events, prev))
}

/// An append-only register held in memory. Its bytes are exactly what [`verify`] accepts.
#[derive(Default)]
pub struct Ledger {
    bytes: Vec<u8>,
    count: u64,
    head: Option<String>,
}

impl Ledger {
    /// An empty register.
    pub fn new() -> Self {
        Ledger::default()
    }

    /// Adopt bytes received from elsewhere, after checking them in full.
    pub fn from_bytes(raw: &[u8]) -> Result<Self, LedgerError> {
        let (events, head) = verify(raw)?;
        Ok(Ledger { bytes: raw.to_vec(), count: events.len() as u64, head: Some(head) })
    }

    /// Append one event. The sequence number and the link to the previous event are set here and nowhere else.
    pub fn append(&mut self, ts: &str, actor: &str, auth: &str, kind: &str, body: Value) -> Result<&str, LedgerError> {
        self.append_ref(ts, actor, auth, kind, &body)
    }

    pub(crate) fn append_ref(&mut self, ts: &str, actor: &str, auth: &str, kind: &str, body: &Value) -> Result<&str, LedgerError> {
        let mut line = Vec::new();
        self.encode_event(ts, actor, auth, kind, body, |bytes| line.extend_from_slice(bytes))?;
        self.head = Some(sha256_hex(&line));
        self.bytes.extend_from_slice(&line);
        self.bytes.push(b'\n');
        self.count += 1;
        Ok(self.head())
    }

    /// Emit at most one event's byte budget. A preview can discard the chunks to check the same limit.
    pub(crate) fn encode_event(&self, ts: &str, actor: &str, auth: &str, kind: &str, body: &Value,
                               mut emit: impl FnMut(&[u8])) -> Result<(), LedgerError> {
        if !is_ts(ts) {
            return Err(LedgerError::Refused("the time is not of the form YYYY-MM-DDTHH:MM:SSZ"));
        }
        if !AUTH.contains(&auth) {
            return Err(LedgerError::Refused("auth is DECLARED or HOST"));
        }
        if !matches!(body, Value::Map(_)) {
            return Err(LedgerError::Refused("the body of an event is an object"));
        }
        let mut size = 0usize;
        let mut write = |bytes: &[u8]| {
            size = size.saturating_add(bytes.len());
            if size <= MAX_EVENT_BYTES {
                emit(bytes);
            }
        };
        // The seven keys are in canonical order. Every variable value uses the record encoder.
        write(br#"{"actor":"#);
        write_str(actor, &mut write);
        write(br#","auth":"#);
        write_str(auth, &mut write);
        write(br#","body":"#);
        body.write_json(&mut write);
        write(br#","kind":"#);
        write_str(kind, &mut write);
        write(br#","prev":"#);
        write_str(self.head(), &mut write);
        write(br#","seq":"#);
        write_int(self.count as i64, &mut write);
        write(br#","ts":"#);
        write_str(ts, &mut write);
        write(b"}");
        if size > MAX_EVENT_BYTES {
            return Err(LedgerError::Refused("the event exceeds the size limit"));
        }
        Ok(())
    }

    /// The digest of the last event, or zeros for an empty register.
    pub fn head(&self) -> &str {
        self.head.as_deref().unwrap_or(ZERO)
    }

    /// Take in lines that follow this register's own, after checking that they continue its chain.
    /// Returns the events taken in. Nothing is changed if any line fails.
    pub fn extend(&mut self, tail: &[u8]) -> Result<Vec<Value>, LedgerError> {
        let (events, head) = verify_tail(tail, self.count as usize, self.head())?;
        if !events.is_empty() {
            self.bytes.extend_from_slice(tail);
            self.count += events.len() as u64;
            self.head = Some(head);
        }
        Ok(events)
    }

    /// Number of events.
    pub fn len(&self) -> u64 {
        self.count
    }

    /// True when no event has been appended.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The register's bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    #[test]
    fn oversized_event_never_sends_more_than_its_budget_to_the_buffer() {
        for text in ["x".repeat(MAX_EVENT_BYTES), "😀".repeat(MAX_EVENT_BYTES / 12 + 1)] {
            let mut fields = BTreeMap::new();
            fields.insert("text".to_string(), Value::Str(text));
            let mut emitted = 0;
            let result = Ledger::new().encode_event("2000-02-29T23:59:59Z", "assistant", "HOST",
                "NOTE", &Value::Map(fields), |bytes| emitted += bytes.len());
            assert_eq!(result, Err(LedgerError::Refused("the event exceeds the size limit")));
            assert!(emitted > 0 && emitted <= MAX_EVENT_BYTES);
        }
    }
}
