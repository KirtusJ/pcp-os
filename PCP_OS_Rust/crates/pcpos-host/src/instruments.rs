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
pub const LIST: [(&str, &str); 8] = [
    ("mathematics.exact", "the value of an expression in whole numbers and fractions, exactly; input: expression"),
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
        assert_eq!(LIST.len(), 8);
        for (name, _) in LIST {
            assert!(!matches!(run(name, &v("{}"), &[]), Err(e) if e.starts_with("there is no instrument")), "{name} is listed and must exist");
        }
    }
}
