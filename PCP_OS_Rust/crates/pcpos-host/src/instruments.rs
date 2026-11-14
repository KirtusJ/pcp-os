//! Instruments: what a module of the method can have this program compute by itself (Core 14.1, and the
//! "Instruments" section of each specialist Specification). An instrument is defined by what it computes.
//! Each one here is deterministic, uses whole numbers and fractions only, and refuses what it cannot
//! compute exactly; none reads anything but its input and, for two of them, the register.
//!
//! A result is an executed check of the property computed and of nothing else. Whether that property is
//! the criterion in question is not something this file can know.
use pcpos_core::sha256_hex;
use pcpos_core::value::Value;

/// What an instrument found: whether the property it computes holds, and the account of it in words.
pub struct Outcome {
    pub holds: bool,
    pub text: String,
}

/// The instruments this program offers, with the module each belongs to and what it computes.
pub const LIST: [(&str, &str); 9] = [
    ("mathematics.exact", "the value of an expression in whole numbers and fractions, exactly; input: expression"),
    ("mathematics.statement", "whether a Lean theorem's statement is the one that was pinned, and whether it is trivially true; input: source (Lean text), theorem (name), pinned (optional SHA-256)"),
    ("statistics.grim", "whether a reported mean of whole-number observations can arise from the stated count; input: mean (text, as reported), n"),
    ("statistics.percent", "whether a reported percentage can arise from a whole count out of the stated total; input: percent (text, as reported), total"),
    ("engineering.units", "whether a quantity in one unit equals a quantity in another, exactly; input: from, to (each a number and a unit, such as \"25 psi\")"),
    ("cryptography.digest", "the SHA-256 digest of given text, after reproducing published known answers; input: text, claimed (optional)"),
    ("science.order", "from the register: whether one artifact entered the record before another; input: earlier, later (SHA-256 digests)"),
    ("quality.coverage", "from the register: for each claim of a review, whether a finding is recorded and how it was executed; input: review"),
    ("core.calibration", "whether a Calibration Record's counts agree with each other and its thresholds and claim say no more than its counts; input: record"),
];

/// Run one instrument. `events` is the register as it stands, for the instruments that read it.
pub fn run(name: &str, input: &Value, events: &[Value]) -> Result<Outcome, String> {
    let text = |k: &str| input.get(k).and_then(Value::as_str).ok_or(format!("this instrument needs {k}, as text"));
    let whole = |k: &str| input.get(k).and_then(Value::as_int).filter(|n| *n > 0).ok_or(format!("this instrument needs {k}, a whole number above zero"));
    match name {
        "mathematics.exact" => exact(text("expression")?),
        "mathematics.statement" => statement(text("source")?, text("theorem")?, input.get("pinned").and_then(Value::as_str)),
        "statistics.grim" => granularity(text("mean")?, whole("n")?, 1, "mean"),
        "statistics.percent" => granularity(text("percent")?, whole("total")?, 100, "percentage"),
        "engineering.units" => units(text("from")?, text("to")?),
        "cryptography.digest" => digest(text("text")?, input.get("claimed").and_then(Value::as_str)),
        "science.order" => order(text("earlier")?, text("later")?, events),
        "quality.coverage" => coverage(text("review")?, events),
        "core.calibration" => calibration(input.get("record").ok_or("this instrument needs record, the Calibration Record")?),
        _ => Err(format!("there is no instrument named {name}. Those offered: {}", LIST.iter().map(|i| i.0).collect::<Vec<_>>().join(", "))),
    }
}

// ---- exact numbers ------------------------------------------------------------------------------------------

/// A fraction in lowest terms, with a positive denominator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Q {
    n: i128,
    d: i128,
}

const BEYOND: &str = "a value passed beyond what this instrument holds exactly (127 bits); nothing was computed";

fn gcd(a: i128, b: i128) -> u128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    fn new(n: i128, d: i128) -> Result<Q, String> {
        if d == 0 {
            return Err("division by zero".to_string());
        }
        let g = gcd(n, d);
        // A nonzero gcd larger than MAX is |MIN|; the only possibilities here
        // are 0/MIN and MIN/MIN. Reduce before converting back to signed.
        let Ok(g) = i128::try_from(g) else {
            return Ok(Q::int(if n == 0 { 0 } else { 1 }));
        };
        let (n, d) = (n / g, d / g);
        if d < 0 { Ok(Q { n: n.checked_neg().ok_or(BEYOND)?, d: d.checked_neg().ok_or(BEYOND)? }) } else { Ok(Q { n, d }) }
    }
    fn int(n: i128) -> Q {
        Q { n, d: 1 }
    }
    fn add(self, o: Q) -> Result<Q, String> {
        Q::new(self.n.checked_mul(o.d).ok_or(BEYOND)?.checked_add(o.n.checked_mul(self.d).ok_or(BEYOND)?).ok_or(BEYOND)?, self.d.checked_mul(o.d).ok_or(BEYOND)?)
    }
    fn neg(self) -> Result<Q, String> {
        Ok(Q { n: self.n.checked_neg().ok_or(BEYOND)?, d: self.d })
    }
    fn mul(self, o: Q) -> Result<Q, String> {
        // Reduce across first, so that a product that fits is not refused for an intermediate that does not.
        // Each gcd divides a strictly positive, representable denominator.
        let (g1, g2) = (gcd(self.n, o.d) as i128, gcd(o.n, self.d) as i128);
        Q::new((self.n / g1).checked_mul(o.n / g2).ok_or(BEYOND)?, (self.d / g2).checked_mul(o.d / g1).ok_or(BEYOND)?)
    }
    fn div(self, o: Q) -> Result<Q, String> {
        if o.n == 0 {
            return Err("division by zero".to_string());
        }
        self.mul(Q::new(o.d, o.n)?)
    }
    fn cmp(self, o: Q) -> Result<core::cmp::Ordering, String> {
        Ok(self.add(o.neg()?)?.n.cmp(&0))
    }
    fn whole(self) -> Option<i128> {
        if self.d == 1 { Some(self.n) } else { None }
    }
    /// The value in decimal: exactly where the expansion ends within 40 places, otherwise to 12 places and marked.
    pub fn show(self) -> String {
        if self.d == 1 {
            return self.n.to_string();
        }
        let (mut out, mut rem, mut places) = (String::new(), self.n.unsigned_abs() % self.d as u128, 0);
        while rem != 0 && places < 40 {
            let d = self.d as u128;
            let (digit, next) = if let Some(scaled) = rem.checked_mul(10) {
                ((scaled / d) as u8, scaled % d)
            } else {
                // Each sum is below 2*d, which fits u128 even when 10*rem does not.
                let (mut digit, mut residue) = (0, 0);
                for _ in 0..10 {
                    residue += rem;
                    if residue >= d {
                        residue -= d;
                        digit += 1;
                    }
                }
                (digit, residue)
            };
            out.push(char::from(b'0' + digit));
            rem = next;
            places += 1;
        }
        let sign = if self.n < 0 { "-" } else { "" };
        let head = self.n.unsigned_abs() / self.d as u128;
        if rem == 0 { format!("{}/{} (exactly {sign}{head}.{out})", self.n, self.d) } else { format!("{}/{} (about {sign}{head}.{}...)", self.n, self.d, &out[..12]) }
    }
}

/// A decimal number as written ("3.48", "25", "-0.5") as a fraction, and how many decimal places it was written with.
fn decimal(s: &str) -> Result<(Q, u32), String> {
    let t = s.trim().replace('_', "");
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r.to_string()),
        None => (false, t),
    };
    let (a, b) = t.split_once('.').unwrap_or((t.as_str(), ""));
    if a.is_empty() && b.is_empty() || !a.chars().chain(b.chars()).all(|c| c.is_ascii_digit()) || a.len() + b.len() > 30 {
        return Err(format!("{s:?} is not a number written in digits, with at most one decimal point"));
    }
    let n: i128 = format!("{a}{b}").parse().map_err(|_| BEYOND.to_string())?;
    let q = Q::new(if neg { -n } else { n }, 10i128.pow(b.len() as u32))?;
    Ok((q, b.len() as u32))
}

// ---- mathematics.exact ----------------------------------------------------------------------------------------

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    /// Names bound by an enclosing sum or product, innermost last.
    bound: Vec<(String, Q)>,
    /// Steps left: every operation costs one, so that no expression runs without end.
    budget: u32,
    depth: u32,
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }
    fn eat(&mut self, t: &str) -> bool {
        self.skip();
        if self.s[self.at..].starts_with(t.as_bytes()) {
            self.at += t.len();
            true
        } else {
            false
        }
    }
    fn step(&mut self) -> Result<(), String> {
        self.budget = self.budget.checked_sub(1).ok_or("the expression takes more than two million steps; nothing was computed")?;
        Ok(())
    }
    fn word(&mut self) -> Option<String> {
        self.skip();
        let start = self.at;
        while self.at < self.s.len() && (self.s[self.at].is_ascii_alphabetic() || (self.at > start && self.s[self.at].is_ascii_digit())) {
            self.at += 1;
        }
        if self.at > start { Some(String::from_utf8_lossy(&self.s[start..self.at]).to_string()) } else { None }
    }
    fn sum(&mut self) -> Result<Q, String> {
        let mut v = self.term()?;
        loop {
            if self.eat("+") {
                v = v.add(self.term()?)?;
            } else if self.eat("-") {
                v = v.add(self.term()?.neg()?)?;
            } else {
                return Ok(v);
            }
            self.step()?;
        }
    }
    fn term(&mut self) -> Result<Q, String> {
        let mut v = self.unary()?;
        loop {
            self.skip();
            if self.eat("*") {
                v = v.mul(self.unary()?)?;
            } else if self.eat("/") {
                v = v.div(self.unary()?)?;
            } else if self.s[self.at..].starts_with(b"mod") && !self.s.get(self.at + 3).map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
                self.at += 3;
                let m = self.unary()?;
                match (v.whole(), m.whole()) {
                    (Some(_), Some(-1)) => v = Q::int(0),
                    (Some(a), Some(b)) if b != 0 => v = Q::int(a.rem_euclid(b)),
                    _ => return Err("mod is defined here for whole numbers, the second not zero".to_string()),
                }
            } else {
                return Ok(v);
            }
            self.step()?;
        }
    }
    fn unary(&mut self) -> Result<Q, String> {
        if self.eat("-") {
            return self.unary()?.neg();
        }
        let base = self.postfix()?;
        if self.eat("^") {
            let e = self.unary()?.whole().ok_or("a power is defined here for a whole-number exponent")?;
            let magnitude = e.unsigned_abs();
            if magnitude > 4096 {
                return Err(BEYOND.to_string());
            }
            let mut v = Q::int(1);
            for _ in 0..magnitude {
                v = v.mul(base)?;
                self.step()?;
            }
            return if e < 0 { Q::int(1).div(v) } else { Ok(v) };
        }
        Ok(base)
    }
    fn postfix(&mut self) -> Result<Q, String> {
        let mut v = self.primary()?;
        while self.eat("!") {
            if self.s.get(self.at) == Some(&b'=') {
                self.at -= 1; // that was the first character of "!=", not a factorial
                break;
            }
            let n = v.whole().filter(|n| *n >= 0).ok_or("a factorial is defined here for a whole number, zero or more")?;
            let mut f = Q::int(1);
            for k in 2..=n {
                f = f.mul(Q::int(k))?;
                self.step()?;
            }
            v = f;
        }
        Ok(v)
    }
    fn args(&mut self, count: usize) -> Result<Vec<Q>, String> {
        let mut out = Vec::new();
        for i in 0..count {
            out.push(self.sum()?);
            if !self.eat(if i + 1 == count { ")" } else { "," }) {
                return Err(format!("expected {} here: ...{}", if i + 1 == count { "a closing bracket" } else { "a comma" }, self.rest()));
            }
        }
        Ok(out)
    }
    fn rest(&self) -> String {
        String::from_utf8_lossy(&self.s[self.at..self.s.len().min(self.at + 24)]).to_string()
    }
    fn primary(&mut self) -> Result<Q, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("the expression is nested more than 64 deep; nothing was computed".to_string());
        }
        self.skip();
        let v = if self.eat("(") {
            let v = self.sum()?;
            if !self.eat(")") {
                return Err(format!("a bracket is not closed: ...{}", self.rest()));
            }
            v
        } else if self.at < self.s.len() && (self.s[self.at].is_ascii_digit() || self.s[self.at] == b'.') {
            let start = self.at;
            while self.at < self.s.len() && (self.s[self.at].is_ascii_digit() || self.s[self.at] == b'.' || self.s[self.at] == b'_') {
                self.at += 1;
            }
            decimal(&String::from_utf8_lossy(&self.s[start..self.at]))?.0
        } else {
            let name = self.word().ok_or(format!("a number, a name or a bracket was expected here: ...{}", self.rest()))?;
            if !self.eat("(") {
                self.bound.iter().rev().find(|b| b.0 == name).map(|b| b.1).ok_or(format!("{name} is not a number and is not the variable of an enclosing sum or product"))?
            } else {
                match name.as_str() {
                    "sum" | "prod" => {
                        let var = self.word().ok_or("sum and prod take a variable, a first value, a last value and an expression: sum(k, 1, 100, k)")?;
                        if !self.eat(",") {
                            return Err("expected a comma after the variable".to_string());
                        }
                        let lo = self.sum()?.whole().ok_or("the first value of a range is a whole number")?;
                        if !self.eat(",") {
                            return Err("expected a comma after the first value".to_string());
                        }
                        let hi = self.sum()?.whole().ok_or("the last value of a range is a whole number")?;
                        if !self.eat(",") {
                            return Err("expected a comma after the last value".to_string());
                        }
                        let body = self.at;
                        let mut acc = Q::int(if name == "sum" { 0 } else { 1 });
                        let mut end = body;
                        if hi < lo {
                            // An empty range still has a body to pass over: evaluate it once with a value, and discard the result.
                            self.bound.push((var.clone(), Q::int(lo)));
                            let _ = self.sum()?;
                            self.bound.pop();
                            end = self.at;
                        }
                        for k in lo..=hi {
                            self.at = body;
                            self.bound.push((var.clone(), Q::int(k)));
                            let term = self.sum()?;
                            self.bound.pop();
                            end = self.at;
                            acc = if name == "sum" { acc.add(term)? } else { acc.mul(term)? };
                            self.step()?;
                        }
                        self.at = end;
                        if !self.eat(")") {
                            return Err(format!("a bracket is not closed: ...{}", self.rest()));
                        }
                        acc
                    }
                    "gcd" | "lcm" => {
                        let a = self.args(2)?;
                        match (a[0].whole(), a[1].whole()) {
                            (Some(x), Some(y)) => {
                                let g = gcd(x, y);
                                let magnitude = if name == "gcd" {
                                    g
                                } else if x == 0 || y == 0 {
                                    0
                                } else {
                                    (x.unsigned_abs() / g).checked_mul(y.unsigned_abs()).ok_or(BEYOND)?
                                };
                                Q::int(i128::try_from(magnitude).map_err(|_| BEYOND)?)
                            }
                            _ => return Err(format!("{name} is defined here for whole numbers")),
                        }
                    }
                    "abs" => {
                        let a = self.args(1)?[0];
                        if a.n < 0 { a.neg()? } else { a }
                    }
                    "min" | "max" => {
                        let a = self.args(2)?;
                        let first_smaller = a[0].cmp(a[1])? == core::cmp::Ordering::Less;
                        if (name == "min") == first_smaller { a[0] } else { a[1] }
                    }
                    "floor" | "ceil" => {
                        let a = self.args(1)?[0];
                        let f = a.n.div_euclid(a.d);
                        Q::int(if name == "ceil" && a.d != 1 { f + 1 } else { f })
                    }
                    _ => return Err(format!("{name} is not a function this instrument has. It has: sum, prod, gcd, lcm, abs, min, max, floor, ceil")),
                }
            }
        };
        self.depth -= 1;
        Ok(v)
    }
}

fn exact(expression: &str) -> Result<Outcome, String> {
    if expression.len() > 2000 || !expression.is_ascii() {
        return Err("the expression is longer than 2000 characters or has characters outside ASCII; nothing was computed".to_string());
    }
    let mut p = Parser { s: expression.as_bytes(), at: 0, bound: Vec::new(), budget: 2_000_000, depth: 0 };
    let left = p.sum()?;
    p.skip();
    let relation = ["!=", "<=", ">=", "=", "<", ">"].into_iter().find(|r| p.eat(r));
    let out = match relation {
        None => Outcome { holds: true, text: format!("{expression}\n= {}", left.show()) },
        Some(r) => {
            let right = p.sum()?;
            let o = left.cmp(right)?;
            use core::cmp::Ordering::*;
            let holds = match r {
                "=" => o == Equal,
                "!=" => o != Equal,
                "<" => o == Less,
                "<=" => o != Greater,
                ">" => o == Greater,
                _ => o != Less,
            };
            Outcome { holds, text: format!("{expression}\n{}: the left side is {}, the right side is {}", if holds { "TRUE" } else { "FALSE" }, left.show(), right.show()) }
        }
    };
    p.skip();
    if p.at != p.s.len() {
        return Err(format!("the expression does not end where it should: ...{}", p.rest()));
    }
    Ok(out)
}

// ---- mathematics.statement -------------------------------------------------------------------------------------

/// Lean text, character by character: 0 for code, 1 for a comment, 2 for the inside of a string or character literal.
fn lean_marks(c: &[char]) -> Result<Vec<u8>, String> {
    let mut k = vec![0u8; c.len()];
    let mut i = 0;
    while i < c.len() {
        if c[i] == '-' && c.get(i + 1) == Some(&'-') {
            while i < c.len() && c[i] != '\n' {
                k[i] = 1;
                i += 1;
            }
        } else if c[i] == '/' && c.get(i + 1) == Some(&'-') {
            // Block comments nest.
            let (start, mut depth) = (i, 0u32);
            loop {
                if i >= c.len() {
                    return Err(format!("a block comment opened at character {start} of the source is not closed; nothing was read"));
                }
                let two = c.get(i + 1).copied();
                k[i] = 1;
                if c[i] == '/' && two == Some('-') {
                    depth += 1;
                } else if c[i] == '-' && two == Some('/') {
                    depth -= 1;
                } else {
                    i += 1;
                    continue;
                }
                k[i + 1] = 1;
                i += 2;
                if depth == 0 {
                    break;
                }
            }
        } else if c[i] == '"' {
            let start = i;
            k[i] = 2;
            i += 1;
            loop {
                if i >= c.len() {
                    return Err(format!("a string opened at character {start} of the source is not closed; nothing was read"));
                }
                k[i] = 2;
                if c[i] == '\\' && i + 1 < c.len() {
                    k[i + 1] = 2;
                    i += 2;
                } else {
                    i += 1;
                    if c[i - 1] == '"' {
                        break;
                    }
                }
            }
        } else if c[i] == '\'' && !(i > 0 && lean_wordy(c[i - 1])) {
            // A character literal ('a', '"', '\n'); a prime that ends a name is not one.
            let close = if c.get(i + 1) == Some(&'\\') { (i + 3..c.len().min(i + 12)).find(|j| c[*j] == '\'') } else if c.get(i + 2) == Some(&'\'') { Some(i + 2) } else { None };
            match close {
                Some(j) => {
                    k[i..=j].fill(2);
                    i = j + 1;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    Ok(k)
}

fn lean_wordy(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '\''
}

/// The word of code that starts at `i`, if one starts there.
fn lean_word(c: &[char], k: &[u8], i: usize) -> Option<String> {
    if k[i] != 0 || !lean_wordy(c[i]) || (i > 0 && k[i - 1] == 0 && (lean_wordy(c[i - 1]) || c[i - 1] == '.')) {
        return None;
    }
    Some(c[i..].iter().zip(&k[i..]).take_while(|(ch, m)| **m == 0 && lean_wordy(**ch)).map(|(ch, _)| *ch).collect())
}

/// The name that follows a keyword: from the first character of code after `from` to white space, a bracket or a colon.
fn lean_name(c: &[char], k: &[u8], from: usize) -> (usize, usize) {
    let mut a = from;
    while a < c.len() && (k[a] == 1 || (k[a] == 0 && c[a].is_whitespace())) {
        a += 1;
    }
    let mut b = a;
    while b < c.len() && k[b] == 0 && !c[b].is_whitespace() && !"({[:⦃⟨".contains(c[b]) {
        b += 1;
    }
    if b > a && c[b - 1] == '.' && c.get(b) == Some(&'{') {
        b -= 1; // universe parameters, name.{u}, belong to the statement
    }
    (a, b)
}

/// Comments removed, every run of white space outside a string made one space, the ends trimmed. Each
/// character keeps whether it stood inside a string.
fn lean_norm(c: &[char], k: &[u8], a: usize, b: usize) -> Vec<(char, bool)> {
    let (mut out, mut gap) = (Vec::new(), false);
    for i in a..b {
        if k[i] == 1 || (k[i] == 0 && c[i].is_whitespace()) {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() {
            out.push((' ', false));
        }
        gap = false;
        out.push((c[i], k[i] == 2));
    }
    out
}

const LEAN_OPEN: &str = "([{⟨⦃";
const LEAN_CLOSE: &str = ")]}⟩⦄";

fn lean_text(s: &[(char, bool)]) -> String {
    s.iter().map(|x| x.0).collect()
}

/// For each character: is it code at bracket depth zero, and not itself a bracket?
fn lean_top(s: &[(char, bool)]) -> Vec<bool> {
    let mut depth = 0usize;
    s.iter().map(|&(ch, quoted)| {
        if quoted {
            false
        } else if LEAN_OPEN.contains(ch) {
            depth += 1;
            false
        } else if LEAN_CLOSE.contains(ch) {
            depth = depth.saturating_sub(1);
            false
        } else {
            depth == 0
        }
    }).collect()
}

/// Without the spaces at its ends and without round brackets that enclose the whole of it.
fn lean_bare(mut s: &[(char, bool)]) -> &[(char, bool)] {
    loop {
        while s.first() == Some(&(' ', false)) {
            s = &s[1..];
        }
        while s.last() == Some(&(' ', false)) {
            s = &s[..s.len() - 1];
        }
        if s.len() < 2 || s[0] != ('(', false) || s[s.len() - 1] != (')', false) {
            return s;
        }
        // The first bracket must be the one the last closes.
        let mut depth = 0usize;
        for (i, &(ch, quoted)) in s.iter().enumerate() {
            if quoted {
                continue;
            }
            if LEAN_OPEN.contains(ch) {
                depth += 1;
            } else if LEAN_CLOSE.contains(ch) {
                depth = depth.saturating_sub(1);
                if depth == 0 && i + 1 != s.len() {
                    return s;
                }
            }
        }
        s = &s[1..s.len() - 1];
    }
}

/// The places at depth zero where `s` divides at a colon (0), at a comma or an arrow (1), or at an arrow only (2): each as (start, width).
fn lean_cuts(s: &[(char, bool)], what: u8) -> Vec<(usize, usize)> {
    let top = lean_top(s);
    let at = |i: usize| s.get(i).filter(|x| !x.1).map(|x| x.0);
    let mut out = Vec::new();
    for i in (0..s.len()).filter(|i| top[*i]) {
        let before = if i > 0 { at(i - 1) } else { None };
        match s[i].0 {
            ':' if what == 0 && before != Some(':') && !matches!(at(i + 1), Some(':') | Some('=')) => out.push((i, 1)),
            ',' if what == 1 => out.push((i, 1)),
            '→' if what >= 1 => out.push((i, 1)),
            '-' if what >= 1 && at(i + 1) == Some('>') && before != Some('<') => out.push((i, 2)),
            _ => {}
        }
    }
    out
}

/// Is this proposition true by its form alone: True, or X = X, X ↔ X, X ≤ X with both sides the same text?
fn lean_trivial(s: &[(char, bool)]) -> Option<String> {
    let s = lean_bare(s);
    if lean_text(s) == "True" {
        return Some("it is True".to_string());
    }
    let top = lean_top(s);
    let at = |i: usize| s.get(i).filter(|x| !x.1).map(|x| x.0);
    for i in (0..s.len()).filter(|i| top[*i]) {
        // An equation or an inequality binds tighter than the connectives; a side that has one at depth zero is not a side.
        let (width, looser) = match s[i].0 {
            '↔' => (1, ","),
            '<' if at(i + 1) == Some('-') && at(i + 2) == Some('>') => (3, ","),
            '≤' => (1, ",∧∨→↔¬∀∃"),
            '<' if at(i + 1) == Some('=') => (2, ",∧∨→↔¬∀∃"),
            '=' if !(i > 0 && at(i - 1).map(|p| "<>!:=".contains(p)).unwrap_or(false)) && !matches!(at(i + 1), Some('>') | Some('=')) => (1, ",∧∨→↔¬∀∃"),
            _ => continue,
        };
        let (left, right) = (lean_bare(&s[..i]), lean_bare(&s[i + width..]));
        let plain = |side: &[(char, bool)]| !lean_top(side).iter().zip(side).any(|(t, x)| *t && looser.contains(x.0)) && lean_cuts(side, 2).is_empty() && !lean_text(side).contains("/\\") && !lean_text(side).contains("\\/");
        if !left.is_empty() && lean_text(left) == lean_text(right) && plain(left) && plain(right) {
            return Some(format!("it is {} {} {}, the two sides the same text", lean_text(left), lean_text(&s[i..i + width]), lean_text(right)));
        }
    }
    None
}

/// Words at the start of a line that begin something other than the proof of the declaration before them.
const LEAN_BEGINS: [&str; 24] = ["theorem", "lemma", "def", "example", "instance", "structure", "inductive", "class", "abbrev", "axiom", "opaque", "namespace", "section", "end", "mutual", "macro", "syntax", "notation", "elab", "private", "protected", "noncomputable", "universe", "variable"];

fn statement(source: &str, name: &str, pinned: Option<&str>) -> Result<Outcome, String> {
    if source.len() > 4_000_000 {
        return Err("the source is longer than four million bytes; nothing was read".to_string());
    }
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Err("theorem is one name, such as energy_conserved or Chain.energy_conserved".to_string());
    }
    let c: Vec<char> = source.chars().collect();
    let k = lean_marks(&c)?;
    // One pass: every theorem and lemma with its full name, and every place where a declaration begins.
    let mut spaces: Vec<Option<String>> = Vec::new();
    let mut declared: Vec<(String, usize)> = Vec::new();
    let mut begins: Vec<usize> = Vec::new();
    let (mut i, mut fresh) = (0, true);
    while i < c.len() {
        let first = fresh;
        fresh = c[i] == '\n' || (fresh && (c[i] == ' ' || c[i] == '\t'));
        if k[i] != 0 {
            i += 1;
            continue;
        }
        if first && (c[i] == '#' || (c[i] == '@' && c.get(i + 1) == Some(&'['))) {
            begins.push(i);
        }
        let Some(word) = lean_word(&c, &k, i) else {
            i += 1;
            continue;
        };
        let after = i + word.chars().count();
        match word.as_str() {
            "theorem" | "lemma" => {
                begins.push(i);
                let (a, b) = lean_name(&c, &k, after);
                if b > a {
                    let own: String = c[a..b].iter().collect();
                    declared.push((spaces.iter().flatten().map(|s| s.as_str()).chain([own.as_str()]).collect::<Vec<_>>().join("."), b));
                }
            }
            "namespace" if first => {
                begins.push(i);
                let (a, b) = lean_name(&c, &k, after);
                spaces.push(Some(c[a..b].iter().collect()));
            }
            "section" if first => {
                begins.push(i);
                spaces.push(None);
            }
            "end" if first => {
                begins.push(i);
                spaces.pop();
            }
            w if first && LEAN_BEGINS.contains(&w) => begins.push(i),
            _ => {}
        }
        i = after;
    }
    // The name as given, or the name as given inside the namespaces it was declared in.
    let mut found: Vec<&(String, usize)> = declared.iter().filter(|d| d.0 == name).collect();
    if found.is_empty() {
        found = declared.iter().filter(|d| d.0.ends_with(&format!(".{name}"))).collect();
    }
    let (full, from) = match found.as_slice() {
        [one] => (one.0.as_str(), one.1),
        [] => return Err(format!("no theorem or lemma named {name} is in the source ({} declared{}); nothing was compared", declared.len(), declared.iter().take(12).map(|d| format!(" {}", d.0)).collect::<String>())),
        many => return Err(format!("{name} names {} declarations in the source ({}); nothing was compared", many.len(), many.iter().map(|d| d.0.as_str()).collect::<Vec<_>>().join(", "))),
    };
    let end = begins.iter().copied().find(|b| *b >= from).unwrap_or(c.len());
    // The signature ends at the first := at bracket depth zero.
    let (mut depth, mut bar, mut cut) = (0usize, false, None);
    for j in from..end {
        if k[j] != 0 {
            continue;
        }
        if LEAN_OPEN.contains(c[j]) {
            depth += 1;
        } else if LEAN_CLOSE.contains(c[j]) {
            depth = depth.checked_sub(1).ok_or(format!("a bracket closes that was not opened, in the statement of {full}; nothing was compared"))?;
        } else if depth == 0 {
            let next = c.get(j + 1).filter(|_| k.get(j + 1) == Some(&0)).copied();
            if c[j] == ':' && next == Some('=') {
                cut = Some(j);
                break;
            }
            bar |= c[j] == '|';
            if lean_word(&c, &k, j).as_deref() == Some("where") {
                return Err(format!("{full} is given with where and no := ends its signature; this instrument reads a statement that ends at :="));
            }
            if bar && c[j] == '=' && next == Some('>') {
                return Err(format!("{full} is given by pattern-matching alternatives (|) and no := ends its signature; this instrument reads a statement that ends at :="));
            }
        }
    }
    let cut = cut.ok_or(format!("no := at bracket depth zero ends the signature of {full} before the next declaration; nothing was compared"))?;
    let stated = lean_norm(&c, &k, from, cut);
    let colon = lean_cuts(&stated, 0).last().map(|x| x.0).ok_or(format!("the statement of {full} has no colon at bracket depth zero, and so no conclusion; nothing was compared"))?;
    let conclusion = lean_bare(&stated[colon + 1..]);
    let got = sha256_hex(lean_text(&stated).as_bytes());
    let mut fired = 0;
    let mut probe = |what: &str, found: Option<String>, clear: String| {
        fired += found.is_some() as u32;
        format!("\nprobe {what}: {}", found.map(|f| format!("FIRES: {f}")).unwrap_or(format!("clear: {clear}")))
    };
    // (a) sorry or admit, anywhere from the name to the next declaration, outside comments and strings.
    let gap = (from..end).find_map(|j| lean_word(&c, &k, j).filter(|w| ["sorry", "admit", "sorryAx"].contains(&w.as_str())));
    let mut lines = probe("(a), a proof left open", gap.map(|w| format!("{w} stands in the statement or proof of {full}")), "no sorry and no admit between the name and the next declaration".to_string());
    // (b) the conclusion, and what remains of it after its last comma or arrow at depth zero.
    let last = if lean_top(conclusion).iter().zip(conclusion).any(|(t, x)| *t && x.0 == '↔') { None } else { lean_cuts(conclusion, 1).last().map(|x| &conclusion[x.0 + x.1..]) };
    let trivial = lean_trivial(conclusion).or_else(|| last.and_then(lean_trivial).map(|t| format!("after its last comma or arrow {t}")));
    lines += &probe("(b), a conclusion true by its form", trivial, format!("the conclusion is {}", lean_text(conclusion)));
    // (c) a hypothesis that is False: a binder (h : False), or False before an arrow of the conclusion.
    let mut absurd = None;
    let (mut depth, mut open) = (0usize, 0);
    for (j, x) in stated.iter().enumerate().filter(|(_, x)| !x.1) {
        if LEAN_OPEN.contains(x.0) {
            if depth == 0 {
                open = j;
            }
            depth += 1;
        } else if LEAN_CLOSE.contains(x.0) && depth > 0 {
            depth -= 1;
            let inner = &stated[open + 1..j];
            if depth == 0 && absurd.is_none() && lean_cuts(inner, 0).last().map(|x| lean_text(lean_bare(&inner[x.0 + 1..])) == "False").unwrap_or(false) {
                absurd = Some(format!("the binder {} assumes False", lean_text(&stated[open..=j])));
            }
        }
    }
    let arrows = lean_cuts(conclusion, 2);
    let mut at = 0;
    for (a, w) in &arrows {
        if lean_text(lean_bare(&conclusion[at..*a])) == "False" && absurd.is_none() {
            absurd = Some("False stands before an arrow of the conclusion".to_string());
        }
        at = a + w;
    }
    lines += &probe("(c), a hypothesis that is False", absurd, "no binder is False".to_string());
    let (matches, pin) = match pinned {
        Some(p) => {
            let m = p.trim().eq_ignore_ascii_case(&got);
            (m, format!("{}: the digest pinned is {}", if m { "THE PINNED STATEMENT" } else { "NOT THE PINNED STATEMENT" }, p.trim()))
        }
        None => (true, format!("NOT PINNED: the digest to pin for this statement is {got}")),
    };
    Ok(Outcome { holds: matches && fired == 0, text: format!("{full}\nstatement: {}\nSHA-256 of the statement, comments removed and white space collapsed: {got}\n{pin}{lines}\nThis instrument read text. It did not run Lean: whether the proof is accepted, and whether the statement says what is meant, it does not know.", lean_text(&stated)) })
}

// ---- statistics.grim, statistics.percent -----------------------------------------------------------------------

/// Can `reported` (a mean, or with scale 100 a percentage), at the precision it was written with, arise as
/// a whole count divided by `n`? It can if some whole count rounds to it, whichever way halves are rounded.
fn granularity(reported: &str, n: i64, scale: i128, what: &str) -> Result<Outcome, String> {
    let (value, places) = decimal(reported)?;
    if value.n < 0 || n > 1_000_000_000 {
        return Err("this instrument is for a value of zero or more and a count of at most a thousand million".to_string());
    }
    let n = n as i128;
    // value = count * scale / n, within half a unit of the last place written.
    let unit = Q::new(1, 10i128.pow(places))?;
    let centre = value.mul(Q::int(n))?.div(Q::int(scale))?;
    let nearest = (centre.n * 2 + centre.d).div_euclid(centre.d * 2);
    let mut fits = Vec::new();
    let mut near = Vec::new();
    for count in [nearest - 1, nearest, nearest + 1] {
        if count < 0 || (scale == 100 && count > n) {
            continue;
        }
        let got = Q::int(count).mul(Q::int(scale))?.div(Q::int(n))?;
        let off = got.add(value.neg()?)?;
        let off = if off.n < 0 { off.neg()? } else { off };
        // within half a unit of the last place, the half included
        if off.mul(Q::int(2))?.cmp(unit)? != core::cmp::Ordering::Greater {
            fits.push(count);
        }
        near.push(format!("{count} gives {}", got.show()));
    }
    let holds = !fits.is_empty();
    Ok(Outcome { holds, text: format!("a {what} reported as {reported} (to {places} decimal place(s)) from {n} whole-number observation(s)\n{}\nThe nearest whole counts: {}",
        if holds { format!("CAN ARISE: from a count of {}", fits.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" or ")) } else { format!("CANNOT ARISE: no whole count out of {n} rounds to {reported}") },
        near.join("; ")) })
}

// ---- engineering.units -----------------------------------------------------------------------------------------

/// A unit: its name, how many SI base units one of it is (as a fraction, exact by definition), and what kind of quantity it measures.
fn unit(name: &str) -> Option<(Q, &'static str)> {
    let q = |n: i128, d: i128| Q::new(n, d).ok();
    let pound = q(45_359_237, 100_000_000)?; // kilograms, by definition
    let gravity = q(980_665, 100_000)?; // metres per second squared, the standard value
    let inch = q(254, 10_000)?; // metres, by definition
    let lbf = pound.mul(gravity).ok()?;
    Some(match name {
        "m" => (q(1, 1)?, "length"),
        "km" => (q(1000, 1)?, "length"),
        "cm" => (q(1, 100)?, "length"),
        "mm" => (q(1, 1000)?, "length"),
        "in" => (inch, "length"),
        "ft" => (inch.mul(q(12, 1)?).ok()?, "length"),
        "yd" => (inch.mul(q(36, 1)?).ok()?, "length"),
        "mi" => (inch.mul(q(63_360, 1)?).ok()?, "length"),
        "kg" => (q(1, 1)?, "mass"),
        "g" => (q(1, 1000)?, "mass"),
        "lb" => (pound, "mass"),
        "oz" => (pound.div(q(16, 1)?).ok()?, "mass"),
        "s" => (q(1, 1)?, "time"),
        "min" => (q(60, 1)?, "time"),
        "h" => (q(3600, 1)?, "time"),
        "N" => (q(1, 1)?, "force"),
        "kN" => (q(1000, 1)?, "force"),
        "lbf" => (lbf, "force"),
        "Pa" => (q(1, 1)?, "pressure"),
        "kPa" => (q(1000, 1)?, "pressure"),
        "MPa" => (q(1_000_000, 1)?, "pressure"),
        "bar" => (q(100_000, 1)?, "pressure"),
        "psi" => (lbf.div(inch.mul(inch).ok()?).ok()?, "pressure"),
        _ => return None,
    })
}

fn quantity(s: &str) -> Result<(Q, u32, Q, &'static str, String), String> {
    let (number, name) = s.trim().split_once(' ').ok_or(format!("{s:?} is not a number, a space and a unit"))?;
    let (value, places) = decimal(number)?;
    let (factor, kind) = unit(name.trim()).ok_or(format!("{} is not a unit this instrument has. It has: m km cm mm in ft yd mi, kg g lb oz, s min h, N kN lbf, Pa kPa MPa bar psi", name.trim()))?;
    Ok((value, places, factor, kind, name.trim().to_string()))
}

fn units(from: &str, to: &str) -> Result<Outcome, String> {
    let (a, _, fa, ka, na) = quantity(from)?;
    let (b, places, fb, kb, nb) = quantity(to)?;
    if ka != kb {
        return Ok(Outcome { holds: false, text: format!("{from} and {to}\nCANNOT BE COMPARED: {na} measures {ka} and {nb} measures {kb}") });
    }
    let converted = a.mul(fa)?.div(fb)?;
    let off = converted.add(b.neg()?)?;
    let off = if off.n < 0 { off.neg()? } else { off };
    let holds = off.mul(Q::int(2))?.cmp(Q::new(1, 10i128.pow(places))?)? != core::cmp::Ordering::Greater;
    Ok(Outcome { holds, text: format!("{from} and {to}\n{}: {from} is {} {nb}; the second quantity is given to {places} decimal place(s)", if holds { "EQUAL to the last digit given" } else { "NOT EQUAL" }, converted.show()) })
}

// ---- cryptography.digest ---------------------------------------------------------------------------------------

fn digest(text: &str, claimed: Option<&str>) -> Result<Outcome, String> {
    // Published known answers for SHA-256 (FIPS 180-4, the examples of the standard's validation material).
    let known = [("", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
                 ("abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
                 ("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq", "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")];
    if let Some((m, want)) = known.iter().find(|(m, want)| sha256_hex(m.as_bytes()) != *want) {
        return Err(format!("this program's SHA-256 does not reproduce the published answer for {m:?} ({want}); nothing it computes is to be relied on"));
    }
    let got = sha256_hex(text.as_bytes());
    let line = format!("SHA-256 of the {} bytes given: {got}\ncomputed after this program's SHA-256 reproduced {} published known answers", text.len(), known.len());
    Ok(match claimed {
        Some(c) => {
            let holds = c.trim().eq_ignore_ascii_case(&got);
            Outcome { holds, text: format!("{line}\n{}: the digest claimed is {}", if holds { "MATCHES" } else { "DOES NOT MATCH" }, c.trim()) }
        }
        None => Outcome { holds: true, text: line },
    })
}

// ---- science.order, quality.coverage: what the register holds ---------------------------------------------------

fn field<'a>(e: &'a Value, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Value::as_str)
}
fn body<'a>(e: &'a Value, k: &str) -> Option<&'a Value> {
    e.get("body").and_then(|b| b.get(k))
}

fn order(earlier: &str, later: &str, events: &[Value]) -> Result<Outcome, String> {
    let first = |digest: &str| events.iter().find(|e| field(e, "kind") == Some("ARTIFACT_ADDED") && body(e, "sha256").and_then(Value::as_str) == Some(digest))
        .map(|e| (e.get("seq").and_then(Value::as_int).unwrap_or(-1), field(e, "ts").unwrap_or("").to_string(), body(e, "origin").and_then(Value::as_str).unwrap_or("").to_string()));
    let (a, b) = (first(earlier), first(later));
    match (a, b) {
        (Some(a), Some(b)) => {
            let holds = a.0 < b.0;
            Ok(Outcome { holds, text: format!("{earlier} entered the record at event {} ({}, recorded as {})\n{later} entered the record at event {} ({}, recorded as {})\n{}",
                a.0, a.1, a.2, b.0, b.1, b.2, if holds { "IN THAT ORDER: the first was in the record before the second. This is the order of entry into this record, not of anything outside it." } else { "NOT IN THAT ORDER" }) })
        }
        (a, b) => Err(format!("{} not in this register as an artifact; nothing was compared", match (a.is_some(), b.is_some()) {
            (false, false) => "neither digest is",
            (false, true) => "the first digest is",
            _ => "the second digest is",
        })),
    }
}

fn coverage(review: &str, events: &[Value]) -> Result<Outcome, String> {
    let commissioned = events.iter().find(|e| field(e, "kind") == Some("REVIEW_COMMISSIONED") && body(e, "label").and_then(Value::as_str) == Some(review)).ok_or(format!("no review {review} was commissioned in this register"))?;
    let mut claims: Vec<String> = Vec::new();
    if let Some(Value::List(l)) = body(commissioned, "claims") {
        claims.extend(l.iter().filter_map(|c| c.get("label").and_then(Value::as_str)).map(|s| s.to_string()));
    }
    for e in events.iter().filter(|e| field(e, "kind") == Some("CLAIM_ADDED") && body(e, "review").and_then(Value::as_str) == Some(review)) {
        if let Some(l) = body(e, "claim").and_then(|c| c.get("label")).and_then(Value::as_str).or_else(|| body(e, "label").and_then(Value::as_str)) {
            claims.push(l.to_string());
        }
    }
    let findings: Vec<&Value> = events.iter().filter(|e| field(e, "kind") == Some("FINDING_RECORDED") && body(e, "review").and_then(Value::as_str) == Some(review)).collect();
    let mut cited: Vec<String> = Vec::new();
    for f in &findings {
        for part in ["basis", "contrary"] {
            if let Some(Value::List(l)) = body(f, part) {
                for item in l {
                    if let Some(r) = item.get("ref") {
                        if r.get("t").and_then(Value::as_str) == Some("check") {
                            cited.extend(r.get("id").and_then(Value::as_str).map(|s| s.to_string()));
                        }
                    }
                }
            }
        }
    }
    let mut lines = Vec::new();
    let mut without = 0;
    for c in &claims {
        match findings.iter().rev().find(|f| body(f, "claim").and_then(Value::as_str) == Some(c)) {
            Some(f) => lines.push(format!("claim {c}: finding {} ({})", body(f, "disposition").and_then(Value::as_str).unwrap_or("?"), body(f, "execution").and_then(Value::as_str).unwrap_or("execution not recorded"))),
            None => {
                without += 1;
                lines.push(format!("claim {c}: NO FINDING"));
            }
        }
    }
    let checks: Vec<String> = events.iter().filter(|e| field(e, "kind") == Some("CHECK_RAN")).filter_map(|e| body(e, "id").and_then(Value::as_str).map(|s| s.to_string())).collect();
    let uncited: Vec<&String> = checks.iter().filter(|k| !cited.contains(k)).collect();
    lines.push(format!("checks in the record: {}; cited by no finding of this review: {}", checks.len(), if uncited.is_empty() { "none".to_string() } else { uncited.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") }));
    Ok(Outcome { holds: without == 0 && !claims.is_empty(), text: format!("review {review}: {} claim(s), {} without a finding\n{}\nThis is what the record holds. It is not a statement that any finding is right.", claims.len(), without, lines.join("\n")) })
}

// ---- core.calibration ------------------------------------------------------------------------------------------

fn calibration(record: &Value) -> Result<Outcome, String> {
    let mut issues: Vec<String> = Vec::new();
    let nat = |v: Option<&Value>| v.and_then(Value::as_int).filter(|n| *n >= 0);
    for f in ["subject", "configuration", "assessor", "instrument", "acquisition"] {
        if !matches!(record.get(f), Some(Value::Map(_))) {
            issues.push(format!("{f}: missing"));
        }
    }
    let relation = record.get("assessor").and_then(|a| a.get("relation")).and_then(Value::as_str).unwrap_or("");
    if !["SELF", "SAME_PARTY", "HOST_OR_TOOL", "SEPARATE_PARTY"].contains(&relation) {
        issues.push("assessor.relation: SELF, SAME_PARTY, HOST_OR_TOOL or SEPARATE_PARTY".to_string());
    }
    let delivery = record.get("configuration").and_then(|c| c.get("delivery")).and_then(Value::as_str).unwrap_or("");
    if !["ABSORBED_TEXT", "RETRIEVAL", "TOOL_ASSISTED", "HOST_ENFORCED", "UNKNOWN"].contains(&delivery) {
        issues.push("configuration.delivery: a Core 15.8 value".to_string());
    }
    // What the measurement withheld from the participant or gave it beyond its delivery as used (Core 14.4).
    let confinement = record.get("configuration").and_then(|c| c.get("confinement"));
    let confinement_stated = confinement.and_then(Value::as_str).map(|s| !s.trim().is_empty()).unwrap_or(false);
    if confinement.is_some() && !confinement_stated {
        issues.push("configuration.confinement: say in words what the measurement withheld from the participant or gave it beyond its delivery as used, or NONE".to_string());
    }
    let retrieval = record.get("retrieval");
    let application = record.get("application");
    if let Some(r @ Value::Map(_)) = retrieval {
        match (nat(r.get("items")), nat(r.get("for")), nat(r.get("against")), nat(r.get("unscored"))) {
            (Some(i), Some(f), Some(a), Some(u)) => {
                let runs = nat(r.get("runs")).unwrap_or(1);
                if f + a + u != i * runs {
                    issues.push(format!("retrieval: for + against + unscored must equal items times runs ({f} + {a} + {u} is not {})", i * runs));
                }
            }
            _ => issues.push("retrieval: items, for, against and unscored are whole numbers".to_string()),
        }
    } else if retrieval.and_then(Value::as_str) != Some("NOT_MEASURED") {
        issues.push("retrieval: counts, or NOT_MEASURED".to_string());
    }
    let control = |name: &str| application.and_then(|a| a.get(name)).and_then(|c| Some((nat(c.get("passed"))?, nat(c.get("runs"))?)));
    if let Some(a @ Value::Map(_)) = application {
        if relation == "SELF" {
            issues.push("application: scored by the participant itself is a self-check; record NOT_MEASURED (Core 14.4)".to_string());
        }
        match (nat(a.get("runs")), nat(a.get("passed")), nat(a.get("failed")), nat(a.get("unscored"))) {
            (Some(r), Some(p), Some(f), Some(u)) if p + f + u == r => {}
            _ => issues.push("application: passed + failed + unscored must equal runs, each a whole number".to_string()),
        }
        for c in ["negative_controls", "positive_controls"] {
            match control(c) {
                Some((p, r)) if p <= r && r > 0 => {}
                _ => issues.push(format!("application.{c}: at least one run, and passed no more than runs")),
            }
        }
        // A reading can be wrong: runs read a second time by another assessor, and how many of those readings agreed.
        match a.get("second_reading") {
            None => {}
            Some(Value::Str(s)) if s == "NONE" => {}
            Some(s @ Value::Map(_)) => match (nat(s.get("runs")), nat(s.get("agreed"))) {
                (Some(r), Some(g)) if g <= r => {
                    if let Some(read) = nat(a.get("runs")).filter(|read| r > *read) {
                        issues.push(format!("application.second_reading: it covers {r} runs and only {read} were read"));
                    }
                }
                _ => issues.push("application.second_reading: runs and agreed, with agreed no more than runs".to_string()),
            },
            Some(_) => issues.push("application.second_reading: runs and agreed, or NONE".to_string()),
        }
    } else if application.and_then(Value::as_str) != Some("NOT_MEASURED") {
        issues.push("application: counts, or NOT_MEASURED".to_string());
    }
    let measure = |name: &str| -> Option<(i64, i64)> {
        match name {
            "retrieval.for" | "retrieval.invented" => {
                let r = retrieval.filter(|r| matches!(r, Value::Map(_)))?;
                Some((nat(r.get(&name[10..]))?, nat(r.get("items"))? * nat(r.get("runs")).unwrap_or(1)))
            }
            "application.passed" => Some((nat(application?.get("passed"))?, nat(application?.get("runs"))?)),
            "application.negative_controls" | "application.positive_controls" => control(&name[12..]),
            _ => None,
        }
    };
    let mut results = Vec::new();
    if let Some(Value::List(thresholds)) = record.get("thresholds") {
        for (n, t) in thresholds.iter().enumerate() {
            let name = t.get("measure").and_then(Value::as_str).unwrap_or("");
            // A proportion is written as text ("0.9"): the register holds whole numbers and text, not fractions.
            let bound = |k: &str| t.get(k).and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_int().map(|i| i.to_string()))).and_then(|s| decimal(&s).ok()).map(|d| d.0);
            let (least, most) = (bound("at_least"), bound("at_most"));
            let found = match (measure(name), least, most) {
                (_, None, None) | (_, Some(_), Some(_)) => {
                    issues.push(format!("thresholds[{n}]: state at_least or at_most, one of them, as a proportion from 0 to 1"));
                    continue;
                }
                (Some((count, runs)), l, m) if runs > 0 => {
                    let rate = Q::new(count as i128, runs as i128)?;
                    let met = match (l, m) {
                        (Some(l), _) => rate.cmp(l)? != core::cmp::Ordering::Less,
                        (_, Some(m)) => rate.cmp(m)? != core::cmp::Ordering::Greater,
                        _ => false,
                    };
                    if met { "MET" } else { "NOT_MET" }
                }
                _ => "NOT_MEASURED",
            };
            results.push(found);
            if t.get("result").and_then(Value::as_str) != Some(found) {
                issues.push(format!("thresholds[{n}]: the counts give {found} for {name}, and the record says {:?}", t.get("result").and_then(Value::as_str).unwrap_or("nothing")));
            }
        }
    }
    if record.get("claim").and_then(|c| c.get("disposition")).and_then(Value::as_str) == Some("SUPPORTED_IN_SCOPE") {
        if results.is_empty() {
            issues.push("claim: without thresholds stated before the run a Calibration reports counts and makes no finding about onboarding (Core 14.4)".to_string());
        } else if results.iter().any(|r| *r != "MET") {
            issues.push(format!("claim: SUPPORTED_IN_SCOPE needs every stated threshold MET; the counts give {}", results.join(", ")));
        }
        if relation == "SELF" {
            issues.push("claim: onboarding is never established by the participant's own scoring (Core 14.4, 15.8)".to_string());
        }
        if !confinement_stated {
            issues.push("claim: SUPPORTED_IN_SCOPE needs configuration.confinement stated: what was measured is the configuration as used only where the record says what the measurement changed (Core 14.4)".to_string());
        }
        for c in ["negative_controls", "positive_controls"] {
            if !matches!(control(c), Some((p, r)) if r > 0 && p == r) {
                issues.push(format!("claim: SUPPORTED_IN_SCOPE needs the {} to have run and passed", c.replace('_', " ")));
            }
        }
    }
    let counts = ["retrieval.for", "retrieval.invented", "application.passed", "application.negative_controls", "application.positive_controls"].iter()
        .map(|m| format!("{m}: {}", measure(m).map(|(c, r)| format!("{c} of {r}")).unwrap_or_else(|| "NOT_MEASURED".to_string()))).collect::<Vec<_>>().join("; ");
    let holds = issues.is_empty();
    Ok(Outcome { holds, text: format!("{}\n{counts}{}\nThe values are the assertions of whoever wrote the record. A record whose numbers agree is not evidence of onboarding.",
        if holds { "CONSISTENT AS DECLARED" } else { "INCONSISTENT" }, issues.iter().map(|i| format!("\n- {i}")).collect::<String>()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(json: &str) -> Value {
        Value::parse(json.as_bytes()).unwrap()
    }
    fn go(name: &str, input: &str) -> Outcome {
        run(name, &v(input), &[]).unwrap_or_else(|e| panic!("{name} {input}: {e}"))
    }

    #[test]
    fn exact_arithmetic_is_exact_and_says_when_it_cannot_be() {
        let o = go("mathematics.exact", r#"{"expression":"sum(k, 1, 100, k) = 5500"}"#);
        assert!(!o.holds && o.text.contains("FALSE") && o.text.contains("the left side is 5050"), "{}", o.text);
        assert!(go("mathematics.exact", r#"{"expression":"sum(k, 1, 100, k) = 100 * 101 / 2"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"1/3 + 1/6 = 0.5"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"0.1 + 0.2 = 0.3"}"#).holds, "decimals are fractions here, not binary approximations");
        assert!(go("mathematics.exact", r#"{"expression":"2^10 = 1024"}"#).holds && go("mathematics.exact", r#"{"expression":"2^-2 = 1/4"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"10! = 3628800"}"#).holds && go("mathematics.exact", r#"{"expression":"5! != 100"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"prod(k, 1, 5, k) = 5!"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"sum(i, 1, 3, sum(j, 1, i, j)) = 10"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"gcd(84, 36) = 12"}"#).holds && go("mathematics.exact", r#"{"expression":"17 mod 5 = 2"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"-3 + 5 * (2 - 4) <= -13"}"#).holds);
        assert!(go("mathematics.exact", r#"{"expression":"sum(k, 5, 4, k) = 0"}"#).holds, "an empty sum is zero");
        assert!(go("mathematics.exact", r#"{"expression":"7 / 2"}"#).text.contains("7/2 (exactly 3.5)"));
        assert!(go("mathematics.exact", r#"{"expression":"1 / 3"}"#).text.contains("about 0.333333333333..."));
        for bad in ["1 / 0", "2 ^ 0.5", "x + 1", "sum(k, 1, 3000000, k)", "30!!", "2 +", "(1 + 2", "1 2", "sqrt(2)", "170! * 170!"] {
            assert!(run("mathematics.exact", &Value::parse(format!(r#"{{"expression":"{bad}"}}"#).as_bytes()).unwrap(), &[]).is_err(), "{bad} must be refused, not answered");
        }
    }

    const LEAN: &str = "namespace Chain\n/-- Not this one: theorem foo : True := trivial /- nested -/ still a comment := -/\ntheorem foo_bar (n : Nat) : n + 0 = n := by simp\n\ntheorem foo (c₀ c₁ : ℝ) (h : 0 < c₀) (x : Nat := 3) : c₀ * c₁ ≤ c₁ * c₀ ∧ ∀ ε > 0, ∃ δ, δ < ε := by\n  constructor\n  · exact le_of_eq (mul_comm _ _)\n  · intro ε hε; exact ⟨ε / 2, by linarith⟩\nend Chain\n";
    const STATED: &str = "(c₀ c₁ : ℝ) (h : 0 < c₀) (x : Nat := 3) : c₀ * c₁ ≤ c₁ * c₀ ∧ ∀ ε > 0, ∃ δ, δ < ε";

    #[test]
    fn a_lean_statement_is_the_pinned_one_or_it_is_not() {
        let pin = sha256_hex(STATED.as_bytes());
        // Not pinned: the statement and the digest to pin. A := in a default argument does not end it; the statement is not ASCII.
        let o = statement(LEAN, "foo", None).unwrap();
        assert!(o.holds && o.text.starts_with("Chain.foo\n") && o.text.contains(&format!("statement: {STATED}\n")) && o.text.contains(&format!("NOT PINNED: the digest to pin for this statement is {pin}")), "{}", o.text);
        assert_eq!(o.text.matches("clear:").count(), 3, "{}", o.text);
        // Pinned and matching, the digest in either case, the name with or without its namespace.
        let o = statement(LEAN, "Chain.foo", Some(&pin.to_uppercase())).unwrap();
        assert!(o.holds && o.text.contains("\nTHE PINNED STATEMENT"), "{}", o.text);
        // Other white space and inserted comments, nested ones among them: the same digest.
        let spaced = LEAN.replace("(h : 0 < c₀) ", "\n    (h :   0 <\tc₀) -- positive := here\n  /- a /- nested -/ comment : True := -/ ");
        assert!(spaced != LEAN);
        let o = statement(&spaced, "foo", Some(&pin)).unwrap();
        assert!(o.holds && o.text.contains(&pin), "{}", o.text);
        // A changed hypothesis: another digest, and it does not hold against the old pin.
        let o = statement(&LEAN.replace("0 < c₀", "0 ≤ c₀"), "foo", Some(&pin)).unwrap();
        assert!(!o.holds && o.text.contains("NOT THE PINNED STATEMENT") && o.text.contains(&sha256_hex(STATED.replace("0 < c₀", "0 ≤ c₀").as_bytes())), "{}", o.text);
        assert_eq!(o.text.matches("clear:").count(), 3, "only the pin fails: {}", o.text);
        // A name that begins another name: each is found, and only itself.
        let o = statement(LEAN, "foo_bar", None).unwrap();
        assert!(o.holds && o.text.contains("statement: (n : Nat) : n + 0 = n\n"), "{}", o.text);
        // White space inside a string is the statement's own, and a string may hold anything.
        let (one, two) = ("theorem s (h : t = \"a :=  -- b\") : t ≠ \"\" := by simp [h]", "theorem s (h : t = \"a := -- b\") : t ≠ \"\" := by simp [h]");
        let (a, b) = (statement(one, "s", None).unwrap(), statement(two, "s", None).unwrap());
        assert!(a.holds && b.holds && a.text.contains("statement: (h : t = \"a :=  -- b\") : t ≠ \"\"\n") && a.text.lines().nth(2) != b.text.lines().nth(2), "{}", a.text);
        // Through the list, as a caller reaches it.
        let o = go("mathematics.statement", r#"{"source":"theorem t (a b : Nat) :\n  a + b = b + a := by\n  omega","theorem":"t"}"#);
        assert!(o.holds && o.text.contains("statement: (a b : Nat) : a + b = b + a\n"), "{}", o.text);
        assert!(!go("mathematics.statement", &format!(r#"{{"source":"theorem t (a b : Nat) : a + b = b + a := by omega","theorem":"t","pinned":"{pin}"}}"#)).holds);
    }

    #[test]
    fn a_lean_statement_true_by_its_form_or_left_open_is_said_to_be() {
        let fires = |source: &str, probe: &str| {
            let o = statement(source, "foo", None).unwrap();
            assert!(!o.holds && o.text.contains(&format!("probe ({probe}")) && o.text.matches("FIRES").count() == 1 && o.text.lines().any(|l| l.starts_with(&format!("probe ({probe})")) && l.contains("FIRES")), "{source}: {}", o.text);
        };
        // The same name, the statement weakened until it says nothing.
        fires("theorem foo (c0 : Nat) (h : 0 < c0) : c0 = c0 := rfl", "b");
        fires("theorem foo (c0 : Nat) : ∀ x : Nat, 0 < x → (c0 + x = c0 + x) := fun _ _ => rfl", "b");
        fires("theorem foo (p : Prop) : p ↔ p := Iff.rfl", "b");
        fires("theorem foo (n : Nat) : n ≤ n := Nat.le_refl n", "b");
        fires("theorem foo (n : Nat) (h : 0 < n) : True := trivial", "b");
        // What only looks like it: the two sides are not the sides.
        for honest in ["theorem foo (a b : Nat) : a = b ∧ b = a ∧ b := by omega", "theorem foo (a b : Nat) (h : a = b) : b = a := h.symm", "theorem foo (a : Nat) : a + 1 ≥ a := by omega"] {
            assert!(statement(honest, "foo", None).unwrap().holds, "{honest}");
        }
        // A proof left open, and what is not one: the word in a comment, in a longer name, in the next declaration.
        fires("theorem foo (a b : Nat) : a + b = b + a := by\n  sorry", "a");
        fires("lemma foo (a b : Nat) : a + b = b + a := by\n  cases a\n  · admit\n  · omega", "a");
        assert!(statement("theorem foo (a b : Nat) : a + b = b + a := by\n  -- sorry, not yet /- sorry -/\n  exact sorry_free a b\n\ntheorem next : 1 = 2 := sorry", "foo", None).unwrap().holds);
        // A hypothesis nothing satisfies.
        fires("theorem foo (a b : Nat) (h : False) : a + b = 7 := h.elim", "c");
        fires("theorem foo (a b : Nat) : False → a + b = 7 := False.elim", "c");
    }

    #[test]
    fn a_lean_statement_that_cannot_be_read_is_refused() {
        let refused = |source: &str, name: &str, why: &str| match statement(source, name, None) {
            Err(e) => assert!(e.contains(why), "{name}: {e}"),
            Ok(o) => panic!("{name} in {source:?} must be refused, not answered: {}", o.text),
        };
        refused(LEAN, "fo", "no theorem or lemma named fo ");
        refused(LEAN, "Other.foo", "no theorem or lemma named Other.foo");
        refused(LEAN, "", "one name");
        refused("", "foo", "(0 declared)");
        refused("theorem foo : 1 + 1 = 2 := rfl\nnamespace A\ntheorem foo : 2 + 2 = 4 := rfl\nend A\nnamespace B\ntheorem foo : 2 + 2 = 4 := rfl\nend B", "A.fo", "no theorem");
        refused("namespace A\ntheorem foo : 2 + 2 = 4 := rfl\nend A\nnamespace B\ntheorem foo : 2 + 2 = 4 := rfl\nend B", "foo", "names 2 declarations in the source (A.foo, B.foo)");
        refused("theorem foo : ∀ n : Nat, n + 0 = n\n  | 0 => rfl\n  | n + 1 => rfl", "foo", "pattern-matching");
        refused("theorem foo : Inhabited Nat where\n  default := 0", "foo", "where");
        refused("theorem foo (a : Nat) : a + 0 = a\n\ndef next := 3", "foo", "no := at bracket depth zero");
        refused("theorem foo (a : Nat)) : a + 0 = a := rfl", "foo", "a bracket closes");
        refused("theorem foo : 2 + 2 = 4 := rfl /- never closed /- inner -/", "foo", "block comment");
        refused("theorem foo (h : s = \"open) : 2 + 2 = 4 := rfl", "foo", "string");
        // Declared once at the root and once in a namespace: the name as given is the one at the root.
        assert!(statement("theorem foo : 1 + 1 = 2 := rfl\nnamespace A\ntheorem foo : 2 + 2 = 4 := rfl\nend A", "foo", None).unwrap().text.contains("statement: : 1 + 1 = 2\n"));
        assert!(run("mathematics.statement", &v(r#"{"source":"theorem foo : 2 + 2 = 4 := rfl"}"#), &[]).is_err(), "a name is needed");
    }

    #[test]
    fn a_lean_statement_of_any_text_is_answered_or_refused_and_never_a_panic() {
        // Fixed seed: 20000 texts made of the pieces the reader treats specially, each cut short at a random place.
        let pieces = ["theorem foo", "lemma foo", " ", "\n", ":=", ":", "(", ")", "⟨", "⟩", "{", "}", "[", "]", "/-", "-/", "--", "\"", "\\", "'", "|", "=>", "where", "sorry", "False", "True", "=", "↔", "≤", "<->", "→", ",", "x", "namespace A", "end", "c₀", ".", "@[", "#"];
        let (mut seed, mut answered, mut refused) = (20261114u64, 0, 0);
        for _ in 0..20000 {
            let mut text = String::from("theorem foo ");
            for _ in 0..24 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push_str(pieces[(seed >> 33) as usize % pieces.len()]);
            }
            match statement(&text, "foo", Some("00")) {
                Ok(o) => answered += (!o.holds) as u32,
                Err(e) => refused += (!e.is_empty()) as u32,
            }
        }
        assert_eq!(answered + refused, 20000, "every text is answered as not the pinned statement, or refused with a reason");
        assert!(answered > 0 && refused > 0, "{answered} answered, {refused} refused");
    }

    #[test]
    fn exact_signed_boundaries_answer_or_refuse_without_overflow() {
        for expression in [
            "gcd((-2)^127,0) < 0", "gcd(0,(-2)^127)", "lcm((-2)^127,1)",
            "2^((-2)^127)",
        ] {
            assert_eq!(exact(expression).err().as_deref(), Some(BEYOND), "{expression}");
        }
        for expression in [
            "gcd((-2)^127,2^126) = 2^126", "lcm((-2)^127,0) = 0",
            "((-2)^127) mod (-1) = 0", "((-2)^127) mod 3 = 1",
            "sum(k,(2^126-1)*2+1,(2^126-1)*2+1,1) = 1",
        ] {
            assert!(exact(expression).unwrap().holds, "{expression}");
        }
        assert_eq!(Q::new(i128::MIN, i128::MIN).unwrap(), Q::int(1));
        assert_eq!(Q::new(0, i128::MIN).unwrap(), Q::int(0));
        assert_eq!(Q::new(1, i128::MIN).unwrap_err(), BEYOND);
        assert_eq!(exact("1-1/(2^126)").unwrap().text,
                   "1-1/(2^126)\n= 85070591730234615865843651857942052863/85070591730234615865843651857942052864 (about 0.999999999999...)");
        let terminating = Q::new(-(7 * 10i128.pow(37) + 1), 10i128.pow(38)).unwrap();
        assert!(terminating.show().ends_with("(exactly -0.70000000000000000000000000000000000001)"));
    }

    #[test]
    fn reported_statistics_are_tested_against_whole_counts() {
        assert!(go("statistics.grim", r#"{"mean":"3.48","n":25}"#).holds, "87 / 25 = 3.48");
        let o = go("statistics.grim", r#"{"mean":"3.47","n":25}"#);
        assert!(!o.holds && o.text.contains("CANNOT ARISE"), "{}", o.text);
        assert!(go("statistics.grim", r#"{"mean":"5.19","n":28}"#).text.contains("CANNOT ARISE"), "the published example of the test: no sum of 28 whole numbers gives 5.19");
        assert!(go("statistics.grim", r#"{"mean":"5.18","n":28}"#).holds);
        assert!(go("statistics.percent", r#"{"percent":"37.5","total":48}"#).holds, "18 of 48");
        assert!(!go("statistics.percent", r#"{"percent":"37.4","total":48}"#).holds);
        assert!(go("statistics.percent", r#"{"percent":"33","total":3}"#).holds && go("statistics.percent", r#"{"percent":"100","total":7}"#).holds);
        assert!(!go("statistics.percent", r#"{"percent":"101","total":7}"#).holds, "no count is more than the total");
        assert!(run("statistics.grim", &v(r#"{"mean":"three","n":25}"#), &[]).is_err() && run("statistics.grim", &v(r#"{"mean":"3.4","n":0}"#), &[]).is_err());
    }

    #[test]
    fn units_are_converted_exactly_and_kinds_are_not_mixed() {
        assert!(go("engineering.units", r#"{"from":"1 in","to":"25.4 mm"}"#).holds);
        assert!(go("engineering.units", r#"{"from":"1 mi","to":"1.609344 km"}"#).holds);
        assert!(go("engineering.units", r#"{"from":"1 lb","to":"0.45359237 kg"}"#).holds);
        assert!(go("engineering.units", r#"{"from":"1 lbf","to":"4.4482216152605 N"}"#).holds, "the pound-force is exact by definition");
        assert!(go("engineering.units", r#"{"from":"25 psi","to":"172.369 kPa"}"#).holds);
        let o = go("engineering.units", r#"{"from":"25 psi","to":"172.4 kPa"}"#);
        assert!(o.holds, "equal to the one decimal place given: {}", o.text);
        assert!(!go("engineering.units", r#"{"from":"25 psi","to":"172.5 kPa"}"#).holds);
        let o = go("engineering.units", r#"{"from":"4.45 N","to":"1 lbf"}"#);
        assert!(o.holds, "a force in newtons and the same force given to no decimal place in pounds-force: {}", o.text);
        let o = go("engineering.units", r#"{"from":"1 lbf","to":"1 N"}"#);
        assert!(!o.holds && o.text.contains("NOT EQUAL"), "the two units a spacecraft was lost between: {}", o.text);
        let o = go("engineering.units", r#"{"from":"3 m","to":"3 s"}"#);
        assert!(!o.holds && o.text.contains("CANNOT BE COMPARED"), "{}", o.text);
        assert!(run("engineering.units", &v(r#"{"from":"3 furlongs","to":"3 m"}"#), &[]).is_err());
    }

    #[test]
    fn a_digest_is_computed_only_after_the_known_answers() {
        let o = go("cryptography.digest", r#"{"text":"abc","claimed":"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"}"#);
        assert!(o.holds && o.text.contains("reproduced 3 published known answers"), "{}", o.text);
        assert!(!go("cryptography.digest", r#"{"text":"abd","claimed":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}"#).holds);
    }

    #[test]
    fn the_register_says_what_entered_it_first_and_what_each_claim_has() {
        let (a, b) = ("a".repeat(64), "b".repeat(64));
        let ev = vec![
            v(&format!(r#"{{"seq":3,"ts":"2026-11-13T00:00:00Z","kind":"ARTIFACT_ADDED","body":{{"sha256":"{a}","origin":"SUPPLIED"}}}}"#)),
            v(r#"{"seq":4,"ts":"2026-11-13T00:00:01Z","kind":"REVIEW_COMMISSIONED","body":{"label":"R1","claims":[{"label":"A"},{"label":"B"}]}}"#),
            v(r#"{"seq":5,"ts":"2026-11-13T00:00:02Z","kind":"CHECK_RAN","body":{"id":"K1"}}"#),
            v(r#"{"seq":6,"ts":"2026-11-13T00:00:02Z","kind":"CHECK_RAN","body":{"id":"K2"}}"#),
            v(&format!(r#"{{"seq":7,"ts":"2026-11-13T00:00:03Z","kind":"ARTIFACT_ADDED","body":{{"sha256":"{b}","origin":"PRODUCED"}}}}"#)),
            v(r#"{"seq":8,"ts":"2026-11-13T00:00:04Z","kind":"FINDING_RECORDED","body":{"review":"R1","claim":"A","disposition":"REFUTED_IN_SCOPE","execution":"EXECUTED","basis":[],"contrary":[{"ref":{"t":"check","id":"K1"}}]}}"#),
        ];
        let o = run("science.order", &v(&format!(r#"{{"earlier":"{a}","later":"{b}"}}"#)), &ev).unwrap();
        assert!(o.holds && o.text.contains("event 3") && o.text.contains("event 7"), "{}", o.text);
        assert!(!run("science.order", &v(&format!(r#"{{"earlier":"{b}","later":"{a}"}}"#)), &ev).unwrap().holds);
        assert!(run("science.order", &v(&format!(r#"{{"earlier":"{a}","later":"{}"}}"#, "c".repeat(64))), &ev).is_err(), "what is not in the record is not compared");
        let o = run("quality.coverage", &v(r#"{"review":"R1"}"#), &ev).unwrap();
        assert!(!o.holds && o.text.contains("claim A: finding REFUTED_IN_SCOPE (EXECUTED)") && o.text.contains("claim B: NO FINDING") && o.text.contains("cited by no finding of this review: K2"), "{}", o.text);
        assert!(run("quality.coverage", &v(r#"{"review":"R9"}"#), &ev).is_err());
    }

    #[test]
    fn a_calibration_record_cannot_say_more_than_its_counts() {
        let good = r#"{"subject":{},"configuration":{"delivery":"TOOL_ASSISTED"},"assessor":{"relation":"HOST_OR_TOOL"},"instrument":{},"acquisition":{},
            "retrieval":{"items":22,"runs":2,"for":40,"against":4,"unscored":0,"invented":1},"application":"NOT_MEASURED",
            "thresholds":[{"measure":"retrieval.for","at_least":"0.9","result":"MET"},{"measure":"retrieval.invented","at_most":"0","result":"NOT_MET"},{"measure":"application.passed","at_least":"0.8","result":"NOT_MEASURED"}]}"#;
        let o = go("core.calibration", &format!(r#"{{"record":{good}}}"#));
        assert!(o.holds && o.text.contains("retrieval.for: 40 of 44") && o.text.contains("application.passed: NOT_MEASURED"), "{}", o.text);
        let wrong = good.replace(r#""result":"NOT_MET""#, r#""result":"MET""#);
        assert!(go("core.calibration", &format!(r#"{{"record":{wrong}}}"#)).text.contains("the counts give NOT_MET"));
        let claimed = good.replace(r#""application":"NOT_MEASURED","#, r#""application":"NOT_MEASURED","claim":{"disposition":"SUPPORTED_IN_SCOPE"},"#);
        let o = go("core.calibration", &format!(r#"{{"record":{claimed}}}"#));
        assert!(!o.holds && o.text.contains("every stated threshold MET") && o.text.contains("negative controls to have run and passed"), "{}", o.text);
        let own = good.replace("HOST_OR_TOOL", "SELF").replace(r#""application":"NOT_MEASURED","#, r#""application":{"runs":4,"passed":4,"failed":0,"unscored":0,"negative_controls":{"runs":1,"passed":1},"positive_controls":{"runs":1,"passed":1}},"#);
        assert!(go("core.calibration", &format!(r#"{{"record":{own}}}"#)).text.contains("self-check"));
        let sums = good.replace(r#""for":40"#, r#""for":41"#);
        assert!(go("core.calibration", &format!(r#"{{"record":{sums}}}"#)).text.contains("for + against + unscored"));
        // What was measured is the configuration as used only where the record says what the measurement changed.
        let measured = r#"{"subject":{},"configuration":{"delivery":"RETRIEVAL"},"assessor":{"relation":"SEPARATE_PARTY"},"instrument":{},"acquisition":{},
            "retrieval":{"items":22,"runs":1,"for":22,"against":0,"unscored":0,"invented":0},
            "application":{"runs":52,"passed":50,"failed":2,"unscored":0,"negative_controls":{"runs":4,"passed":4},"positive_controls":{"runs":2,"passed":2},"second_reading":{"runs":52,"agreed":49}},
            "thresholds":[{"measure":"application.passed","at_least":"0.8","result":"MET"}],"claim":{"disposition":"SUPPORTED_IN_SCOPE"}}"#;
        let o = go("core.calibration", &format!(r#"{{"record":{measured}}}"#));
        assert!(!o.holds && o.text.contains("needs configuration.confinement stated"), "{}", o.text);
        let stated = measured.replace(r#""delivery":"RETRIEVAL""#, r#""delivery":"RETRIEVAL","confinement":"NONE""#);
        assert!(go("core.calibration", &format!(r#"{{"record":{stated}}}"#)).holds);
        let blank = measured.replace(r#""delivery":"RETRIEVAL""#, r#""delivery":"RETRIEVAL","confinement":" ""#);
        assert!(go("core.calibration", &format!(r#"{{"record":{blank}}}"#)).text.contains("configuration.confinement: say in words"));
        let more = stated.replace(r#""second_reading":{"runs":52,"agreed":49}"#, r#""second_reading":{"runs":60,"agreed":49}"#);
        assert!(go("core.calibration", &format!(r#"{{"record":{more}}}"#)).text.contains("only 52 were read"));
        let over = stated.replace(r#""second_reading":{"runs":52,"agreed":49}"#, r#""second_reading":{"runs":10,"agreed":11}"#);
        assert!(go("core.calibration", &format!(r#"{{"record":{over}}}"#)).text.contains("agreed no more than runs"));
        let none = stated.replace(r#""second_reading":{"runs":52,"agreed":49}"#, r#""second_reading":"NONE""#);
        assert!(go("core.calibration", &format!(r#"{{"record":{none}}}"#)).holds);
    }

    #[test]
    fn an_instrument_that_does_not_exist_is_not_invented() {
        assert!(run("mathematics.prove", &v("{}"), &[]).is_err());
        assert_eq!(LIST.len(), 9);
        for (name, _) in LIST {
            assert!(!matches!(run(name, &v("{}"), &[]), Err(e) if e.starts_with("there is no instrument")), "{name} is listed and must exist");
        }
    }
}
