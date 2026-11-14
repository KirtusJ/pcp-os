//! Read the kernel's COM2 observations on the host.
use pcpos_core::{boot_report::expand_positional, machine::System, r1, value::Value};
use std::{
    fmt,
    io::{self, Read, Write},
};

pub enum Format {
    Json,
    Cbor,
    Cbor2,
}

pub enum View {
    Admission,
    Health,
}

#[derive(Debug)]
pub enum Error {
    Report(String),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Report(message) => message.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

type Result<T> = std::result::Result<T, Error>;

fn invalid(reason: impl Into<String>) -> Error {
    Error::Report(reason.into())
}

/// Bound both the current record and the bytes consumed through the requested observation.
struct Records<R, const N: usize> {
    input: R,
    // Only the CBOR file translator infers its layout from the first observation.
    format: Option<Format>,
    bytes: [u8; N],
    filled: usize,
    remaining: usize,
}

fn read(input: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match input.read(buffer) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

impl<R: Read, const N: usize> Records<R, N> {
    fn new(input: R, format: Option<Format>, remaining: usize) -> Self {
        Self { input, format, bytes: [0; N], filled: 0, remaining }
    }

    fn record<T>(
        &mut self,
        event: &str,
        fields: usize,
        parse: fn(&Value) -> Option<T>,
    ) -> Result<(Value, T)> {
        let limit = self.remaining.min(N);
        let mut scanned = 0;
        loop {
            let bytes = &self.bytes[..self.filled];
            let decoded = match self.format {
                Some(Format::Json) => {
                    let end = bytes[scanned..].iter().position(|&b| b == b'\n').map(|i| scanned + i);
                    scanned = self.filled;
                    match end {
                        Some(i) => Some((
                            Value::parse(&bytes[..i]).map_err(|_| invalid("invalid JSON record"))?,
                            i + 1,
                        )),
                        None => None,
                    }
                }
                None | Some(Format::Cbor | Format::Cbor2) => match r1::decode_prefix(bytes) {
                    Ok((value, rest)) => Some((value, bytes.len() - rest.len())),
                    Err(r1::R1Error::UnexpectedEnd) => None,
                    Err(_) => return Err(invalid(format!("invalid {event} CBOR message"))),
                },
            };
            if let Some((row, used)) = decoded {
                let layout = self.format.get_or_insert_with(|| {
                    if matches!(&row, Value::List(_)) { Format::Cbor2 } else { Format::Cbor }
                });
                let row = match layout {
                    Format::Cbor2 => expand_positional(row)
                        .ok_or_else(|| invalid(format!("invalid positional {event} message")))?,
                    Format::Cbor | Format::Json => row,
                };
                if row.get("v") != Some(&Value::Int(1)) {
                    return Err(invalid("missing or unsupported boot report version"));
                }
                if row.get("event").and_then(Value::as_str) != Some(event) {
                    return Err(invalid(format!("expected the {event} message")));
                }
                let typed = parse(&row)
                    .filter(|_| matches!(&row, Value::Map(m) if m.len() == fields))
                    .ok_or_else(|| invalid(format!("invalid fields in {event} message")))?;
                self.bytes.copy_within(used..self.filled, 0);
                self.filled -= used;
                self.remaining -= used;
                return Ok((row, typed));
            }
            if self.filled == limit {
                return Err(invalid(format!("oversized {event} message or capture")));
            }
            let n = read(&mut self.input, &mut self.bytes[self.filled..limit])?;
            if n == 0 {
                return Err(invalid(format!("incomplete capture: expected a complete {event} message")));
            }
            self.filled += n;
        }
    }

    fn end(&mut self) -> Result<()> {
        if self.filled != 0 || read(&mut self.input, &mut self.bytes[..1])? != 0 {
            return Err(invalid("unexpected bytes after the stop message"));
        }
        Ok(())
    }
}

fn count(row: &Value, key: &str) -> Option<u32> {
    u32::try_from(row.get(key)?.as_int()?).ok()
}

fn boolean(row: &Value, key: &str) -> Option<bool> {
    match row.get(key)? {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

fn digest(row: &Value, key: &str) -> Option<()> {
    let s = row.get(key)?.as_str()?;
    (s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))).then_some(())
}

fn image(row: &Value) -> Option<&'static str> {
    digest(row, "image")?;
    match row.get("match")? {
        Value::Null => Some("unchecked"),
        Value::Bool(true) => Some("matched"),
        Value::Bool(false) => Some("mismatch"),
        _ => None,
    }
}

struct Session {
    admitted: bool,
    system: Option<System>,
    pass: u32,
    fail: u32,
}

fn session(row: &Value) -> Option<Session> {
    let system = match row.get("system")? {
        Value::Null => None,
        Value::Str(s) => Some(System::from_name(s)?),
        _ => return None,
    };
    Some(Session {
        admitted: boolean(row, "admit")?,
        system,
        pass: count(row, "pass")?,
        fail: count(row, "fail")?,
    })
}

struct Stop {
    pass: u32,
    fail: u32,
    replay: bool,
    events: i64,
}

fn stop(row: &Value) -> Option<Stop> {
    digest(row, "head")?;
    let events = row.get("events")?.as_int()?;
    if events < 0 {
        return None;
    }
    Some(Stop {
        pass: count(row, "pass")?,
        fail: count(row, "fail")?,
        replay: boolean(row, "replay")?,
        events,
    })
}

fn write_line(output: &mut impl Write, value: &Value) -> io::Result<()> {
    let mut line = value.canonical_json();
    line.push(b'\n');
    output.write_all(&line)?;
    output.flush()
}

/// Forward every structurally valid CBOR observation, including a reported failure.
pub fn translate(input: impl Read, mut output: impl Write) -> Result<()> {
    let mut records = Records::<_, 1024>::new(input, None, 3 * 1024);
    write_line(&mut output, &records.record("boot", 4, image)?.0)?;
    write_line(&mut output, &records.record("session", 6, session)?.0)?;
    write_line(&mut output, &records.record("stop", 7, stop)?.0)?;
    records.end()
}

fn verdict<R: Read, const N: usize>(mut records: Records<R, N>, view: &View) -> Result<(bool, Value)> {
    let (_, image) = records.record("boot", 4, image)?;
    let (_, session) = records.record("session", 6, session)?;
    if session.admitted != (session.fail == 0)
        || (session.admitted && (session.system != Some(System::Operational) || image == "mismatch"))
    {
        return Err(invalid("inconsistent admission"));
    }

    let mut fields = vec![("image", super::s(image))];
    let ok = match view {
        View::Admission => {
            fields.push(("admit", Value::Bool(session.admitted)));
            session.admitted
        }
        View::Health => {
            let (_, stop) = records.record("stop", 7, stop)?;
            records.end()?;
            // The final replay itself increments the cumulative pass or failure count.
            if (stop.replay && stop.events == 0)
                || stop.pass < session.pass
                || (stop.replay && stop.pass == session.pass)
                || stop.fail < session.fail
                || (!stop.replay && stop.fail == session.fail)
            {
                return Err(invalid("inconsistent shutdown results"));
            }
            let healthy = session.admitted && stop.replay && stop.fail == 0;
            fields.push(("boot", super::s(if healthy { "ok" } else { "failed" })));
            if !healthy {
                fields.extend([
                    ("admit", Value::Bool(session.admitted)),
                    ("replay", Value::Bool(stop.replay)),
                    ("fail", Value::Int(i64::from(stop.fail))),
                ]);
            }
            healthy
        }
    };
    if !ok {
        if matches!(view, View::Admission) {
            fields.push(("fail", Value::Int(i64::from(session.fail))));
        }
        fields.push(("system", session.system.map(|s| super::s(s.name())).unwrap_or(Value::Null)));
    }
    Ok((ok, super::obj(fields)))
}

pub fn summarize(input: impl Read, format: Format, view: View) -> i32 {
    // Both binary layouts fit within 128 bytes for each observation. JSON permits whitespace.
    let result = match format {
        Format::Json => verdict(Records::<_, 4096>::new(input, Some(format), 4096), &view),
        Format::Cbor | Format::Cbor2 => verdict(Records::<_, 128>::new(input, Some(format), 4096), &view),
    };
    let (code, summary) = match result {
        Ok((ok, summary)) => (if ok { 0 } else { 1 }, summary),
        Err(error) => {
            eprintln!("pcpos boot-report: {error}");
            let unknown = match view {
                View::Admission => super::obj(vec![("admit", Value::Null)]),
                View::Health => super::obj(vec![("boot", super::s("unknown"))]),
            };
            (if matches!(error, Error::Io(_)) { 2 } else { 6 }, unknown)
        }
    };
    if let Err(e) = write_line(&mut io::stdout().lock(), &summary) {
        eprintln!("pcpos boot-report: {e}");
        return 2;
    }
    code
}
