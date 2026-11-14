//! The record data model and its JSON form.
//!
//! The model is deliberately small: null, booleans, 64-bit integers, text, lists and maps with text keys.
//! There are no floating-point numbers: a float has no single byte form across platforms.
//!
//! The parser is strict where common parsers are lenient, because a lenient parser lets two parties read
//! one record differently: duplicate keys, floats, exponents, leading zeros, unpaired surrogates, trailing
//! bytes and excessive nesting are all refused. The same function validates and loads.
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Greatest nesting depth accepted. Bounded so that hostile input cannot exhaust the stack.
pub const MAX_DEPTH: usize = 32;

/// A record value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Absent.
    Null,
    /// True or false.
    Bool(bool),
    /// A signed 64-bit integer.
    Int(i64),
    /// Text.
    Str(String),
    /// An ordered list.
    List(Vec<Value>),
    /// A map with text keys, kept in key order.
    Map(BTreeMap<String, Value>),
}

/// Why some bytes are not an acceptable record value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The input ended inside a value.
    UnexpectedEnd,
    /// A byte that cannot start or continue a value at this position.
    UnexpectedByte(usize),
    /// A map repeats a key.
    DuplicateKey(usize),
    /// A number with a fraction or an exponent.
    NotAnInteger(usize),
    /// An integer outside the signed 64-bit range, or written with a leading zero.
    BadInteger(usize),
    /// An escape that is malformed or names half of a surrogate pair.
    BadEscape(usize),
    /// Text that is not valid UTF-8 or contains an unescaped control character.
    BadText(usize),
    /// Nesting deeper than [`MAX_DEPTH`].
    TooDeep(usize),
    /// Bytes after the end of the value.
    TrailingBytes(usize),
}

impl Value {
    /// Parse exactly one value from the whole input.
    pub fn parse(input: &[u8]) -> Result<Value, ParseError> {
        let mut p = Parser { b: input, i: 0 };
        let v = p.value(0)?;
        if p.i != input.len() {
            return Err(ParseError::TrailingBytes(p.i));
        }
        Ok(v)
    }

    /// The canonical JSON form: sorted keys, no spaces, ASCII only. One value has one byte form.
    pub fn canonical_json(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut Vec<u8>) {
        match self {
            Value::Null => out.extend_from_slice(b"null"),
            Value::Bool(true) => out.extend_from_slice(b"true"),
            Value::Bool(false) => out.extend_from_slice(b"false"),
            Value::Int(n) => write_int(*n, out),
            Value::Str(s) => write_str(s, out),
            Value::List(items) => {
                out.push(b'[');
                for (n, v) in items.iter().enumerate() {
                    if n > 0 {
                        out.push(b',');
                    }
                    v.write_json(out);
                }
                out.push(b']');
            }
            Value::Map(m) => {
                out.push(b'{');
                for (n, (k, v)) in m.iter().enumerate() {
                    if n > 0 {
                        out.push(b',');
                    }
                    write_str(k, out);
                    out.push(b':');
                    v.write_json(out);
                }
                out.push(b'}');
            }
        }
    }

    /// The value under a key, if this is a map that has it.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.get(key),
            _ => None,
        }
    }

    /// The text, if this is text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The integer, if this is one.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }
}

pub(crate) fn write_int(n: i64, out: &mut Vec<u8>) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let neg = n < 0;
    let mut m = n.unsigned_abs();
    loop {
        i -= 1;
        buf[i] = b'0' + (m % 10) as u8;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    if neg {
        i -= 1;
        buf[i] = b'-';
    }
    out.extend_from_slice(&buf[i..]);
}

fn write_u16_escape(u: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(b"\\u");
    for shift in [12u32, 8, 4, 0] {
        out.push(crate::HEX[((u >> shift) & 15) as usize]);
    }
}

fn write_str(s: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for c in s.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\n' => out.extend_from_slice(b"\\n"),
            '\r' => out.extend_from_slice(b"\\r"),
            '\t' => out.extend_from_slice(b"\\t"),
            '\u{8}' => out.extend_from_slice(b"\\b"),
            '\u{c}' => out.extend_from_slice(b"\\f"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => write_u16_escape(c as u16, out), // every byte outside space..tilde is escaped
            c if (c as u32) < 0x80 => out.push(c as u8),
            c => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    write_u16_escape(*u, out);
                }
            }
        }
    }
    out.push(b'"');
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Result<u8, ParseError> {
        self.b.get(self.i).copied().ok_or(ParseError::UnexpectedEnd)
    }

    fn eat(&mut self, lit: &[u8]) -> Result<(), ParseError> {
        if self.b.len() < self.i + lit.len() {
            return Err(ParseError::UnexpectedEnd);
        }
        if &self.b[self.i..self.i + lit.len()] != lit {
            return Err(ParseError::UnexpectedByte(self.i));
        }
        self.i += lit.len();
        Ok(())
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\n' | b'\r' | b'\t') = self.b.get(self.i) {
            self.i += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError::TooDeep(self.i));
        }
        self.ws();
        let v = match self.peek()? {
            b'n' => self.eat(b"null").map(|_| Value::Null)?,
            b't' => self.eat(b"true").map(|_| Value::Bool(true))?,
            b'f' => self.eat(b"false").map(|_| Value::Bool(false))?,
            b'"' => Value::Str(self.string()?),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.peek()? == b']' {
                    self.i += 1;
                } else {
                    loop {
                        items.push(self.value(depth + 1)?);
                        self.ws();
                        match self.peek()? {
                            b',' => self.i += 1,
                            b']' => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err(ParseError::UnexpectedByte(self.i)),
                        }
                    }
                }
                Value::List(items)
            }
            b'{' => {
                self.i += 1;
                let mut m = BTreeMap::new();
                self.ws();
                if self.peek()? == b'}' {
                    self.i += 1;
                } else {
                    loop {
                        self.ws();
                        let at = self.i;
                        if self.peek()? != b'"' {
                            return Err(ParseError::UnexpectedByte(self.i));
                        }
                        let k = self.string()?;
                        self.ws();
                        self.eat(b":")?;
                        let v = self.value(depth + 1)?;
                        if m.insert(k, v).is_some() {
                            return Err(ParseError::DuplicateKey(at));
                        }
                        self.ws();
                        match self.peek()? {
                            b',' => self.i += 1,
                            b'}' => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err(ParseError::UnexpectedByte(self.i)),
                        }
                    }
                }
                Value::Map(m)
            }
            b'-' | b'0'..=b'9' => self.int()?,
            _ => return Err(ParseError::UnexpectedByte(self.i)),
        };
        self.ws();
        Ok(v)
    }

    fn int(&mut self) -> Result<Value, ParseError> {
        let start = self.i;
        let neg = self.b[self.i] == b'-';
        if neg {
            self.i += 1;
        }
        let digits = self.i;
        while let Some(b'0'..=b'9') = self.b.get(self.i) {
            self.i += 1;
        }
        if self.i == digits {
            return Err(ParseError::BadInteger(start));
        }
        if let Some(b'.' | b'e' | b'E') = self.b.get(self.i) {
            return Err(ParseError::NotAnInteger(start));
        }
        if self.b[digits] == b'0' && self.i - digits > 1 {
            return Err(ParseError::BadInteger(start));
        }
        let mut acc: i64 = 0;
        for &d in &self.b[digits..self.i] {
            let d = (d - b'0') as i64;
            acc = acc.checked_mul(10).ok_or(ParseError::BadInteger(start))?;
            acc = if neg { acc.checked_sub(d) } else { acc.checked_add(d) }.ok_or(ParseError::BadInteger(start))?;
        }
        Ok(Value::Int(acc))
    }

    fn hex4(&mut self) -> Result<u16, ParseError> {
        let at = self.i;
        if self.b.len() < self.i + 4 {
            return Err(ParseError::UnexpectedEnd);
        }
        let mut u = 0u16;
        for _ in 0..4 {
            let c = self.b[self.i];
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return Err(ParseError::BadEscape(at)),
            };
            u = (u << 4) | d as u16;
            self.i += 1;
        }
        Ok(u)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(&c) = self.b.get(self.i) {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            let chunk = core::str::from_utf8(&self.b[start..self.i]).map_err(|_| ParseError::BadText(start))?;
            out.push_str(chunk);
            match self.peek()? {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    let at = self.i;
                    self.i += 1;
                    let c = self.peek()?;
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                self.eat(b"\\u").map_err(|_| ParseError::BadEscape(at))?;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(ParseError::BadEscape(at));
                                }
                                0x10000 + (((hi as u32) - 0xD800) << 10) + ((lo as u32) - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(ParseError::BadEscape(at));
                            } else {
                                hi as u32
                            };
                            out.push(char::from_u32(cp).ok_or(ParseError::BadEscape(at))?);
                        }
                        _ => return Err(ParseError::BadEscape(at)),
                    }
                }
                _ => return Err(ParseError::BadText(self.i)),
            }
        }
    }
}
