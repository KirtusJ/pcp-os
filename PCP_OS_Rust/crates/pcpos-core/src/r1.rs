//! R1: a DRAFT compact byte form for records. Not adopted; the record profile in force is the JSON form.
//!
//! R1 is a restricted, deterministic subset of CBOR (RFC 8949, section 4.2): definite lengths only, shortest
//! integer heads, no floating point, no tags, map keys in bytewise order of their encodings. Any CBOR decoder
//! reads it. Three substitutions remove most of the weight of the JSON form:
//!
//! * a digest written as 64 hexadecimal characters is carried as 32 bytes;
//! * a field name in the table below is carried as a small integer;
//! * an event is a positional array, and its time is an integer of seconds.
//!
//! Every substitution is a bijection, so a value has exactly one R1 form and decodes to exactly the value
//! that was encoded.
use crate::value::Value;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Field names carried as integers. The position is the number; an entry is never moved or reused.
pub const KEYS: &[&str] = &[
    "id", "label", "name", "review", "claim", "text", "kind", "origin", "sha256", "size", "basis", "ref", "t", "extent",
    "disposition", "grounding", "grounding_observed", "execution", "conditions", "contrary", "estimate", "left_open",
    "noncritical", "relation", "supersedes", "session", "gate", "note", "by", "argv", "exit", "expect_exit", "stdout",
    "stderr", "duration_ms", "timeout", "truncated", "tree", "path", "manifest", "files", "bytes", "stored", "source",
    "criterion", "claims", "target", "procedure", "required_gates", "lifecycle", "coverage", "principal", "lineage",
    "model", "provider", "filename", "profile", "release_fingerprint", "core_descriptor_sha256", "verified", "author",
    "director", "modules", "version", "status", "how", "resolution", "closed", "words", "span", "event",
];

/// Why some bytes are not an acceptable R1 value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R1Error {
    /// The input ended inside a value.
    UnexpectedEnd,
    /// A head byte this profile does not use, or a length not in shortest form.
    NotInProfile(usize),
    /// Map keys out of order or repeated.
    KeyOrder(usize),
    /// Text that is not valid UTF-8, or a field number outside the table.
    BadContent(usize),
    /// Nesting deeper than the limit.
    TooDeep(usize),
    /// Bytes after the end of the value.
    TrailingBytes(usize),
}

fn head(major: u8, n: u64, out: &mut Vec<u8>) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.push(m | 24);
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn hex_digest(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let nib = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        out[i] = (nib(b[2 * i])? << 4) | nib(b[2 * i + 1])?;
    }
    Some(out)
}

/// Encode a value.
pub fn encode(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write(v, &mut out);
    out
}

fn write(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.push(0xf6),
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Int(n) if *n >= 0 => head(0, *n as u64, out),
        Value::Int(n) => head(1, !(*n) as u64, out),
        Value::Str(s) => match hex_digest(s) {
            Some(d) => {
                head(2, 32, out);
                out.extend_from_slice(&d);
            }
            None => {
                head(3, s.len() as u64, out);
                out.extend_from_slice(s.as_bytes());
            }
        },
        Value::List(items) => {
            head(4, items.len() as u64, out);
            for i in items {
                write(i, out);
            }
        }
        Value::Map(m) => {
            let mut pairs: Vec<(Vec<u8>, &Value)> = m
                .iter()
                .map(|(k, v)| {
                    let mut kb = Vec::new();
                    match KEYS.iter().position(|x| x == k) {
                        Some(n) => head(0, n as u64, &mut kb),
                        None => {
                            head(3, k.len() as u64, &mut kb);
                            kb.extend_from_slice(k.as_bytes());
                        }
                    }
                    (kb, v)
                })
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            head(5, pairs.len() as u64, out);
            for (kb, v) in pairs {
                out.extend_from_slice(&kb);
                write(v, out);
            }
        }
    }
}

struct Dec<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Dec<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], R1Error> {
        if self.b.len() - self.i < n {
            return Err(R1Error::UnexpectedEnd);
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }

    fn head(&mut self) -> Result<(u8, u64), R1Error> {
        let at = self.i;
        let b = self.take(1)?[0];
        let (major, info) = (b >> 5, b & 31);
        let n = match info {
            0..=23 => info as u64,
            24 => {
                let v = self.take(1)?[0] as u64;
                if v < 24 {
                    return Err(R1Error::NotInProfile(at));
                }
                v
            }
            25 => {
                let v = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as u64;
                if v <= 0xff {
                    return Err(R1Error::NotInProfile(at));
                }
                v
            }
            26 => {
                let v = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as u64;
                if v <= 0xffff {
                    return Err(R1Error::NotInProfile(at));
                }
                v
            }
            27 => {
                let v = u64::from_be_bytes(self.take(8)?.try_into().unwrap());
                if v <= 0xffff_ffff {
                    return Err(R1Error::NotInProfile(at));
                }
                v
            }
            _ => return Err(R1Error::NotInProfile(at)),
        };
        Ok((major, n))
    }

    fn value(&mut self, depth: usize) -> Result<Value, R1Error> {
        if depth > crate::value::MAX_DEPTH {
            return Err(R1Error::TooDeep(self.i));
        }
        let at = self.i;
        match self.b.get(self.i) {
            None => return Err(R1Error::UnexpectedEnd),
            Some(0xf4) => {
                self.i += 1;
                return Ok(Value::Bool(false));
            }
            Some(0xf5) => {
                self.i += 1;
                return Ok(Value::Bool(true));
            }
            Some(0xf6) => {
                self.i += 1;
                return Ok(Value::Null);
            }
            Some(_) => {}
        }
        let (major, n) = self.head()?;
        Ok(match major {
            0 if n <= i64::MAX as u64 => Value::Int(n as i64),
            1 if n <= i64::MAX as u64 => Value::Int(!(n as i64)),
            2 if n == 32 => {
                let d = self.take(32)?;
                let mut s = String::with_capacity(64);
                for b in d {
                    s.push(crate::HEX[(b >> 4) as usize] as char);
                    s.push(crate::HEX[(b & 15) as usize] as char);
                }
                Value::Str(s)
            }
            3 => {
                let raw = self.take(n as usize)?;
                let s = core::str::from_utf8(raw).map_err(|_| R1Error::BadContent(at))?;
                if hex_digest(s).is_some() {
                    return Err(R1Error::NotInProfile(at)); // a digest has one form: bytes
                }
                Value::Str(s.to_string())
            }
            4 => {
                let mut items = Vec::new();
                for _ in 0..n {
                    items.push(self.value(depth + 1)?);
                }
                Value::List(items)
            }
            5 => {
                let mut m = BTreeMap::new();
                let mut last: Option<&[u8]> = None;
                for _ in 0..n {
                    let kstart = self.i;
                    let (kmajor, kn) = self.head()?;
                    let key = match kmajor {
                        0 => KEYS.get(kn as usize).ok_or(R1Error::BadContent(kstart))?.to_string(),
                        3 => {
                            let raw = self.take(kn as usize)?;
                            let s = core::str::from_utf8(raw).map_err(|_| R1Error::BadContent(kstart))?;
                            if KEYS.contains(&s) {
                                return Err(R1Error::NotInProfile(kstart)); // a tabled name has one form: its number
                            }
                            s.to_string()
                        }
                        _ => return Err(R1Error::NotInProfile(kstart)),
                    };
                    let kbytes = &self.b[kstart..self.i];
                    if let Some(prev) = last {
                        if prev >= kbytes {
                            return Err(R1Error::KeyOrder(kstart));
                        }
                    }
                    last = Some(kbytes);
                    let v = self.value(depth + 1)?;
                    m.insert(key, v);
                }
                Value::Map(m)
            }
            _ => return Err(R1Error::NotInProfile(at)),
        })
    }
}

/// Decode exactly one value from the whole input, refusing anything outside the profile.
pub fn decode(input: &[u8]) -> Result<Value, R1Error> {
    let mut d = Dec { b: input, i: 0 };
    let v = d.value(0)?;
    if d.i != input.len() {
        return Err(R1Error::TrailingBytes(d.i));
    }
    Ok(v)
}

/// Seconds since 1970-01-01T00:00:00Z for a time of the form YYYY-MM-DDTHH:MM:SSZ.
pub fn ts_to_seconds(ts: &str) -> Option<i64> {
    let b = ts.as_bytes();
    if !crate::ledger::is_ts(ts) {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<i64> { core::str::from_utf8(&b[a..z]).ok()?.parse::<i64>().ok() };
    let (y, mo, d, h, mi, s) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let last_day = match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=last_day).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146097 + doe - 719468) * 86400 + h * 3600 + mi * 60 + s)
}

/// An event in positional form: [seq, prev, seconds, actor, auth, kind, body].
pub fn encode_event(ev: &Value) -> Option<Vec<u8>> {
    let auth = match ev.get("auth")?.as_str()? {
        "DECLARED" => 0,
        "HOST" => 1,
        _ => return None,
    };
    let items = alloc::vec![
        ev.get("seq")?.clone(),
        ev.get("prev")?.clone(),
        Value::Int(ts_to_seconds(ev.get("ts")?.as_str()?)?),
        ev.get("actor")?.clone(),
        Value::Int(auth),
        ev.get("kind")?.clone(),
        ev.get("body")?.clone(),
    ];
    Some(encode(&Value::List(items)))
}
