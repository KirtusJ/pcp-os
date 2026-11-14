//! Positional CBOR transport for the kernel's COM2 boot observations. Its three
//! records carry the same field values as the named v1 JSON and CBOR maps.

use crate::value::Value;
use alloc::collections::BTreeMap;

const VERSION: u8 = 2;

/// A supervisor-supplied identifier echoed by all three v3 records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootId([u8; 16]);

impl BootId {
    /// Parse exactly 32 lowercase hexadecimal digits.
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != 32 {
            return None;
        }
        let nibble = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        let mut bytes = [0; 16];
        for (byte, digits) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            *byte = (nibble(digits[0])? << 4) | nibble(digits[1])?;
        }
        Some(Self(bytes))
    }

    /// Canonical CBOR prefix of [3, bytes(16), v2_positional_record].
    pub fn prefix(self) -> [u8; 19] {
        let mut prefix = [0; 19];
        prefix[..3].copy_from_slice(&[0x83, 0x03, 0x50]);
        prefix[3..].copy_from_slice(&self.0);
        prefix
    }
}

/// A COM2 event with a fixed v2 tag and ordered body fields.
pub struct Layout {
    tag: u8,
    name: &'static str,
    keys: &'static [&'static str],
}

/// Image digest and optional image comparison.
pub const BOOT: Layout = Layout { tag: 0, name: "boot", keys: &["image", "match"] };
/// Cumulative counters, system state and eligibility at session entry.
pub const SESSION: Layout = Layout { tag: 1, name: "session", keys: &["pass", "fail", "system", "admit"] };
/// Final counters, replay result, register event count and head.
pub const STOP: Layout = Layout { tag: 2, name: "stop", keys: &["pass", "fail", "replay", "events", "head"] };

impl Layout {
    /// Definite array header followed by its version and event tag.
    pub const fn prefix(&self) -> [u8; 3] {
        // All three items in this fixed prefix fit in the shortest single-byte form.
        assert!(self.keys.len() + 2 < 24 && VERSION < 24 && self.tag < 24);
        [0x80 | (self.keys.len() as u8 + 2), VERSION, self.tag]
    }
}

/// Expand a decoded v2 array to the named v1 observation. The receiver validates
/// field values, types and relationships using its usual v1 checks.
pub fn expand_positional(row: Value) -> Option<Value> {
    let Value::List(row) = row else { return None };
    let mut values = row.into_iter();
    if values.next() != Some(Value::Int(i64::from(VERSION))) {
        return None;
    }
    let Some(Value::Int(tag)) = values.next() else { return None };
    let layout = [&BOOT, &SESSION, &STOP].into_iter().find(|layout| i64::from(layout.tag) == tag)?;
    if values.len() != layout.keys.len() {
        return None;
    }
    let mut fields = BTreeMap::new();
    fields.insert("v".into(), Value::Int(1));
    fields.insert("event".into(), Value::Str(layout.name.into()));
    fields.extend(layout.keys.iter().zip(values).map(|(&key, value)| (key.into(), value)));
    Some(Value::Map(fields))
}
