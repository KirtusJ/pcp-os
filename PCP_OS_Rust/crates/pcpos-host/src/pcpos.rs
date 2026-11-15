//! pcpos: PCP OS tools on another operating system, over the same native core as the kernel.
//!
//!     pcpos status <register>                 what is running, what needs a response, what a halt would stop
//!     pcpos verify <register>                 check the chain and replay every guard
//!     pcpos machine [name]                    print a state machine's whole table
//!     pcpos method <directory>                check a method packet against the release this core implements
//!     pcpos init <register> --principal L     begin a register (a person, at a terminal)
//!     pcpos adopt <register> --by L --basis W put ACR in use (a person, acting as themselves)
//!     pcpos halt <register> --by L [--words W]
//!     pcpos standard <register> --by L [--second L2] BASELINE|CRITICAL|key=value ...
//!     pcpos modules <method directory>        the modules found in a method packet, each with its identity
//!     pcpos clause <method directory> <module> [section]   the exact text of a provision, or the module's headings
//!     pcpos workspace <register> --by L [module ...]   the modules this workspace's reviews use (a person; none = Core only)
//!     pcpos mcp                               serve the record to a model over standard input and output
//!     pcpos hook <event>                      record what the host itself observed (reads the host's JSON on standard input)
//!
//! Exit status: 0 intact; 1 intact, with something that needs a response; 3 refused; 6 not intact; 2 usage.
//! A pass is a fact about bytes. It establishes nothing about whether a finding is right.
//!
//! What this program binds, and what it does not. Over `mcp` the caller never names the actor: the
//! session's participant is fixed when the server starts, and events that belong to a person or to the
//! system are not accepted from it. That holds against a model's tool calls. It does not hold against
//! anything that can write the register file directly; on another operating system that is every
//! program running with the user's rights. Only the kernel can close that, and this program is not it.
#![forbid(unsafe_code)]
use pcpos_core::acr::{self, Problem, Record, State, SubmitError};
use pcpos_core::authority::{Edition, Method, ModelFindings, Standard};
use pcpos_core::machine::{self, Delivery, Link, Review, System, Use};
use pcpos_core::value::{ParseError, Value, MAX_DEPTH};
use pcpos_core::{ledger, sha256_hex};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{BufRead, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use std::{env, process::exit};

// ---- reading what other programs send ---------------------------------------------------------------------

/// A reader for JSON this program did not write (a host's hook input). The record's own parser is strict
/// on purpose and refuses floats and duplicate keys; a host's message may contain both. Here a float
/// becomes its whole part, the last of duplicate keys wins, and nothing read this way is written to a
/// register without passing the guards.
mod instruments;

mod lenient {
    use pcpos_core::value::Value;
    use std::collections::BTreeMap;

    struct P<'a> {
        b: &'a [u8],
        i: usize,
    }

    pub fn parse(b: &[u8]) -> Option<Value> {
        let mut p = P { b, i: 0 };
        let v = p.value(0)?;
        p.ws();
        if p.i == b.len() { Some(v) } else { None }
    }

    impl P<'_> {
        fn ws(&mut self) {
            while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.i += 1;
            }
        }

        fn lit(&mut self, t: &[u8], v: Value) -> Option<Value> {
            if self.b[self.i..].starts_with(t) {
                self.i += t.len();
                Some(v)
            } else {
                None
            }
        }

        fn value(&mut self, depth: usize) -> Option<Value> {
            if depth > 64 {
                return None;
            }
            self.ws();
            match *self.b.get(self.i)? {
                b'n' => self.lit(b"null", Value::Null),
                b't' => self.lit(b"true", Value::Bool(true)),
                b'f' => self.lit(b"false", Value::Bool(false)),
                b'"' => self.string().map(Value::Str),
                b'[' => {
                    self.i += 1;
                    let mut out = Vec::new();
                    loop {
                        self.ws();
                        if self.b.get(self.i) == Some(&b']') {
                            self.i += 1;
                            return Some(Value::List(out));
                        }
                        if !out.is_empty() {
                            if self.b.get(self.i) != Some(&b',') {
                                return None;
                            }
                            self.i += 1;
                        }
                        out.push(self.value(depth + 1)?);
                    }
                }
                b'{' => {
                    self.i += 1;
                    let mut out = BTreeMap::new();
                    let mut first = true;
                    loop {
                        self.ws();
                        if self.b.get(self.i) == Some(&b'}') {
                            self.i += 1;
                            return Some(Value::Map(out));
                        }
                        if !first {
                            if self.b.get(self.i) != Some(&b',') {
                                return None;
                            }
                            self.i += 1;
                            self.ws();
                        }
                        first = false;
                        let k = self.string()?;
                        self.ws();
                        if self.b.get(self.i) != Some(&b':') {
                            return None;
                        }
                        self.i += 1;
                        let v = self.value(depth + 1)?;
                        out.insert(k, v);
                    }
                }
                b'-' | b'0'..=b'9' => {
                    let start = self.i;
                    if self.b[self.i] == b'-' {
                        self.i += 1;
                    }
                    while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                        self.i += 1;
                    }
                    let whole = core::str::from_utf8(&self.b[start..self.i]).ok()?.parse::<i64>().unwrap_or(0);
                    while matches!(self.b.get(self.i), Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')) {
                        self.i += 1;
                    }
                    Some(Value::Int(whole))
                }
                _ => None,
            }
        }

        fn hex4(&mut self) -> Option<u32> {
            let t = core::str::from_utf8(self.b.get(self.i..self.i + 4)?).ok()?;
            self.i += 4;
            u32::from_str_radix(t, 16).ok()
        }

        fn string(&mut self) -> Option<String> {
            if self.b.get(self.i) != Some(&b'"') {
                return None;
            }
            self.i += 1;
            let mut out: Vec<u8> = Vec::new();
            loop {
                let c = *self.b.get(self.i)?;
                self.i += 1;
                match c {
                    b'"' => return String::from_utf8(out).ok(),
                    b'\\' => {
                        let e = *self.b.get(self.i)?;
                        self.i += 1;
                        let ch = match e {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'/' => '/',
                            b'b' => '\u{8}',
                            b'f' => '\u{c}',
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            b'u' => {
                                let mut u = self.hex4()?;
                                if (0xd800..0xdc00).contains(&u) && self.b.get(self.i..self.i + 2) == Some(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    u = if (0xdc00..0xe000).contains(&lo) { 0x10000 + ((u - 0xd800) << 10) + (lo - 0xdc00) } else { 0xfffd };
                                }
                                char::from_u32(u).unwrap_or('\u{fffd}')
                            }
                            _ => return None,
                        };
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    _ => out.push(c),
                }
            }
        }
    }
}

// ---- a register file: found safely, locked while it is read or written ------------------------------------

#[cfg(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm")))]
const O_NOFOLLOW: i32 = 0o100000;
#[cfg(all(target_os = "linux", not(any(target_arch = "aarch64", target_arch = "arm"))))]
const O_NOFOLLOW: i32 = 0o400000;
#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
const O_NOFOLLOW: i32 = 0x0100;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))))]
const O_NOFOLLOW: i32 = 0;

/// The largest file the server reads to record as an artifact.
const MAX_ARTIFACT: u64 = 64 * 1024 * 1024;

struct Handle {
    file: File,
    path: PathBuf,
}

/// Open a file named by a caller, inside `root` (which is canonical). The name is checked, and then the
/// object that was actually opened is checked: a regular file, with one name, that is still what the
/// name refers to, under the root. A link is never followed, at the last step or before it.
/// `Ok(None)`: there is no such file and none was to be created.
fn confined(root: &Path, given: &str, write: bool, create: bool) -> Result<Option<Handle>, String> {
    if !given.ends_with(".jsonl") {
        return Err("a register is named by a relative path that ends in .jsonl".to_string());
    }
    confined_file(root, given, write, create, true)
}

/// The same confinement for any file under the root. `single_name`: refuse a file that has a second name
/// (a register has exactly one; material to be read may be hard-linked by the tools that manage it).
fn confined_file(root: &Path, given: &str, write: bool, create: bool, single_name: bool) -> Result<Option<Handle>, String> {
    let rel = Path::new(given);
    if given.is_empty() || given.contains('\0') {
        return Err("a file is named by a relative path".to_string());
    }
    if rel.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(format!("{given} is not a path inside the directory this program serves"));
    }
    let outside = || format!("{given} is outside the directory this program serves");
    let p = root.join(rel);
    let dir = p.parent().ok_or("no parent directory")?;
    if create {
        // Every existing part of the way is resolved first, so a directory is never made through a link that leaves the root.
        let mut at = root.to_path_buf();
        for c in rel.parent().map(|d| d.components().collect::<Vec<_>>()).unwrap_or_default() {
            at.push(c);
            match fs::symlink_metadata(&at) {
                Ok(_) => {
                    if !at.canonicalize().map(|c| c.starts_with(root)).unwrap_or(false) {
                        return Err(outside());
                    }
                }
                Err(_) => {
                    fs::create_dir(&at).map_err(|e| format!("cannot create {}: {}", at.display(), e))?;
                    // Checked again after the fact: a part of the way swapped for a link meanwhile is found here.
                    if !at.canonicalize().map(|c| c.starts_with(root)).unwrap_or(false) {
                        return Err(outside());
                    }
                }
            }
        }
    }
    let dir = match dir.canonicalize() {
        Ok(d) => d,
        Err(_) if !create => return Ok(None),
        Err(e) => return Err(format!("{given}: {e}")),
    };
    if !dir.starts_with(root) {
        return Err(outside());
    }
    let leaf = dir.join(p.file_name().ok_or("no file name")?);
    let exists = match fs::symlink_metadata(&leaf) {
        Ok(m) => {
            if m.file_type().is_symlink() || !m.is_file() {
                return Err(format!("{given} is not a regular file; a link is not followed"));
            }
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(format!("{given}: {e}")),
    };
    if !exists && !create {
        return Ok(None);
    }
    let mut o = OpenOptions::new();
    o.read(true);
    if write {
        o.append(true);
    }
    if !exists {
        o.create_new(true); // fails if anything, a link included, appeared under this name meanwhile
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(O_NOFOLLOW);
    }
    let file = o.open(&leaf).map_err(|e| format!("cannot open {given}: {e}"))?;
    let m = file.metadata().map_err(|e| format!("{given}: {e}"))?;
    if !m.is_file() {
        return Err(format!("{given} is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if single_name && m.nlink() != 1 {
            return Err(format!("{given} has more than one name; a register has exactly one"));
        }
        let again = fs::symlink_metadata(&leaf).map_err(|e| format!("{given}: {e}"))?;
        if again.file_type().is_symlink() || again.dev() != m.dev() || again.ino() != m.ino() {
            return Err(format!("{given} changed while it was being opened"));
        }
    }
    if !leaf.canonicalize().map(|c| c.starts_with(root)).unwrap_or(false) {
        return Err(outside());
    }
    // Where the system can say where the opened object really is, ask it: this is the object that will be
    // read and written, whatever the names did while it was being opened.
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        if let Ok(real) = fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())) {
            if !real.starts_with(root) {
                return Err(outside());
            }
        }
    }
    if !exists {
        // A new name is durable only once its directory is.
        if let Ok(d) = File::open(&dir) {
            let _ = d.sync_all();
        }
    }
    Ok(Some(Handle { file, path: leaf }))
}

/// One writer at a time, and no reader in the middle of a write. The wait is bounded: a held lock is reported, not waited out.
fn lock(h: &Handle, exclusive: bool) -> Result<(), String> {
    let wait = env::var("PCPOS_LOCK_WAIT").ok().and_then(|x| x.parse::<u64>().ok()).unwrap_or(10);
    let deadline = Instant::now() + Duration::from_secs(wait);
    loop {
        let r = if exclusive { h.file.try_lock() } else { h.file.try_lock_shared() };
        match r {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(TryLockError::WouldBlock) => return Err(format!("{} is locked by another program; nothing was read or written", h.path.display())),
            Err(TryLockError::Error(e)) => return Err(format!("cannot lock {}: {}", h.path.display(), e)),
        }
    }
}

fn read_all(h: &mut Handle) -> Result<Vec<u8>, String> {
    let mut raw = Vec::new();
    h.file.seek(SeekFrom::Start(0)).and_then(|_| h.file.read_to_end(&mut raw)).map_err(|e| format!("cannot read {}: {}", h.path.display(), e))?;
    Ok(raw)
}

struct Opened {
    events: Vec<Value>,
    head: String,
    state: State,
    problems: Vec<Problem>,
    raw: Vec<u8>,
}

fn parse(raw: Vec<u8>) -> Result<Opened, String> {
    let (events, head) = ledger::verify(&raw).map_err(|e| format!("the register is not intact: {:?}", e))?;
    let (state, problems) = acr::fold(&events, true).map_err(|e| format!("the register cannot be replayed: {}", e.message))?;
    Ok(Opened { events, head, state, problems, raw })
}

/// Read a register under a shared lock. `Ok(None)`: no such file.
fn open(root: &Path, given: &str) -> Result<Option<Opened>, String> {
    match confined(root, given, false, false)? {
        None => Ok(None),
        Some(mut h) => {
            lock(&h, false)?;
            parse(read_all(&mut h)?).map(Some)
        }
    }
}

/// What became of one offered event.
enum Put {
    Written { seq: u64, head: String, prev: String },
    Refused(acr::Refusal),
}

/// A register held under an exclusive lock from the moment it is read until what was added is on disk.
/// Events are checked against the bytes actually on disk, so two writers cannot both extend the same head.
struct Tx {
    h: Handle,
    rec: Record,
    problems: Vec<Problem>,
    start: usize,
}

/// A record this process has already checked, kept between calls so that a call costs what it adds and
/// not the whole history. It is used again only if the file still begins with exactly the bytes that were
/// checked: the comparison is of the bytes themselves, so a file changed underneath is checked afresh.
struct Checked {
    path: PathBuf,
    rec: Record,
    problems: Vec<Problem>,
}
static CHECKED: std::sync::Mutex<Option<Checked>> = std::sync::Mutex::new(None);

impl Drop for Tx {
    fn drop(&mut self) {
        // Kept only if it holds exactly what is stored: an event offered and not stored is not carried over.
        if self.rec.ledger().bytes().len() == self.start {
            if let Ok(mut c) = CHECKED.lock() {
                *c = Some(Checked { path: self.h.path.clone(), rec: std::mem::take(&mut self.rec), problems: std::mem::take(&mut self.problems) });
            }
        }
    }
}

impl Tx {
    fn begin(root: &Path, given: &str, create: bool) -> Result<Option<Tx>, String> {
        let mut h = match confined(root, given, true, create)? {
            Some(h) => h,
            None => return Ok(None),
        };
        lock(&h, true)?;
        let raw = read_all(&mut h)?;
        let failed = |e: SubmitError| match e {
            SubmitError::Ledger(l) => format!("the register is not intact: {:?}", l),
            SubmitError::Refused(r) => format!("the register cannot be replayed: {}", r.message),
        };
        let kept = CHECKED.lock().ok().and_then(|mut c| c.take()).filter(|c| c.path == h.path && raw.starts_with(c.rec.ledger().bytes()));
        let (rec, problems) = match kept {
            Some(mut c) => {
                let seen = c.rec.ledger().bytes().len();
                let more = c.rec.extend(&raw[seen..]).map_err(failed)?;
                c.problems.extend(more);
                (c.rec, c.problems)
            }
            None => Record::open(&raw).map_err(failed)?,
        };
        Ok(Some(Tx { h, rec, problems, start: raw.len() }))
    }

    fn state(&self) -> &State {
        self.rec.state()
    }

    fn head(&self) -> String {
        self.rec.ledger().head().to_string()
    }

    /// Offer an event. A register that holds an event the guards would refuse is not continued as though
    /// nothing were wrong: only a halt is added to it. A halt is never refused on that ground.
    fn put(&mut self, actor: &str, auth: &str, kind: &str, body: Value) -> Result<Put, String> {
        if !self.problems.is_empty() && kind != "HALTED" {
            return Ok(Put::Refused(acr::Refusal {
                code: acr::INTEGRITY,
                clause: None,
                message: format!("this register holds {} event(s) the guards would refuse (first: event {}: {}). It accepts a halt and nothing else. Continue in a new register that admits this one as a submission by its head digest",
                                 self.problems.len(), self.problems[0].seq, self.problems[0].message),
            }));
        }
        let prev = self.head();
        match self.rec.submit(&now_utc(), actor, auth, kind, body) {
            Ok(head) => {
                let head = head.to_string();
                Ok(Put::Written { seq: self.rec.ledger().len() - 1, head, prev })
            }
            Err(SubmitError::Refused(r)) => Ok(Put::Refused(r)),
            Err(SubmitError::Ledger(e)) => Err(format!("the register refused the event: {:?}", e)),
        }
    }

    /// Write what was added, in one piece, and wait until the system reports it stored.
    fn commit(&mut self) -> Result<(), String> {
        let bytes = &self.rec.ledger().bytes()[self.start..];
        if bytes.is_empty() {
            return Ok(());
        }
        self.h.file.write_all(bytes).and_then(|_| self.h.file.sync_all()).map_err(|e| {
            format!("the event was accepted and could not be stored ({e}). Whether any of it reached the file is unknown: run verify on the register before anything else is done with it")
        })?;
        self.start = self.rec.ledger().bytes().len();
        Ok(())
    }
}

// ---- the view ---------------------------------------------------------------------------------------------

/// Things that need a person's response, most urgent first. Each has one stated response. Anything that
/// needs no response is not listed here; it is in the tables below.
fn attention(o: &Opened) -> Vec<(&'static str, String, &'static str)> {
    let mut a = Vec::new();
    if !o.problems.is_empty() {
        a.push(("HIGH", format!("{} recorded event(s) would not pass the guards (first: event {})", o.problems.len(), o.problems[0].seq),
                "treat findings that rest on them as unestablished until each is examined; this register accepts only a halt"));
    }
    if o.state.system() == Some(System::Safe) {
        a.push(("HIGH", "the system is in SAFE: it found its own integrity in doubt".to_string(), "read the record, stop the system, start it again and let it verify"));
    }
    for (id, t) in &o.state.tasks {
        if t.outcome.is_none() {
            if let Some(n) = t.stop_requested {
                a.push(("HIGH", format!("task {} {}: stop requested at event {}; termination not confirmed", id, t.label, n),
                        "confirm that its worker has stopped; until its end is recorded it may still be acting"));
            }
        }
        if t.outcome.as_deref() == Some("LOST") {
            a.push(("HIGH", format!("task {} {} was LOST: it stopped being heard from", id, t.label), "confirm its process is gone; nothing it produced is established"));
        }
    }
    if o.state.system() == Some(System::Degraded) {
        a.push(("MEDIUM", "the system is DEGRADED: a service is missing".to_string(), "restore the service, or accept the lower ceiling on what may be claimed"));
    }
    if let Some((by, actor)) = o.state.adoption() {
        if by != actor && adoption_basis(o).as_deref() != Some("RELAYED_WHOLE_PROMPT_OBSERVED") {
            a.push(("MEDIUM", format!("ACR use rests on an instruction relayed by {actor}: {by} did not act as themselves, and the host did not observe the instruction"),
                    "have the person confirm it themselves (a typed command, or pcpos adopt), or halt"));
        }
    }
    for r in o.state.reviews.values().filter(|r| matches!(r.lifecycle, Review::Active | Review::Paused)) {
        if o.state.delivery.get(&r.director) == Some(&Delivery::Stale) {
            a.push(("MEDIUM", format!("the Director of review {} ({}) no longer holds the method text: its context was reset, or the release changed", r.label, r.director),
                    "deliver the text again before its next step, and have it read the status first"));
        }
    }
    for (agent, d) in &o.state.delivery {
        if *d == Delivery::Stale && o.state.in_use() == Use::InUse && !o.state.reviews.values().any(|r| &r.director == agent && matches!(r.lifecycle, Review::Active | Review::Paused)) {
            a.push(("MEDIUM", format!("{agent} no longer holds the method text: its context was reset, or the release changed"), "have it read the method text and this status before its next step"));
        }
    }
    if let Some(p) = profile_of(&o.events).filter(|p| !p.modules.is_empty()) {
        for r in o.state.reviews.values().filter(|r| matches!(r.lifecycle, Review::Active | Review::Paused)) {
            let has: Vec<String> = o.events.iter().filter(|e| e.get("kind").and_then(Value::as_str) == Some("REVIEW_COMMISSIONED") && e.get("body").and_then(|b| b.get("label")).and_then(Value::as_str) == Some(r.label.as_str()))
                .filter_map(|e| match e.get("body").and_then(|b| b.get("modules")) { Some(Value::List(l)) => Some(l.iter().filter_map(|m| m.get("id").and_then(Value::as_str).map(|x| x.to_string())).collect::<Vec<_>>()), _ => None })
                .flatten().collect();
            let missing: Vec<&str> = p.modules.iter().map(|(id, _, _)| id.as_str()).filter(|id| !has.iter().any(|h| h == id)).collect();
            if !missing.is_empty() {
                a.push(("MEDIUM", format!("review {} was commissioned without {} which this workspace's reviews use", r.label, missing.join(", ")),
                        "close it and commission it again with the workspace's modules, or have the person change the workspace"));
            }
        }
    }
    if o.state.unclean_starts > 0 {
        a.push(("MEDIUM", format!("{} start(s) found that the run before had not stopped in order", o.state.unclean_starts), "check the last events before each such start"));
    }
    for (id, t) in &o.state.tasks {
        if t.outcome.as_deref() == Some("FAILED") {
            a.push(("MEDIUM", format!("task {} {} FAILED", id, t.label), "its result is absent or is not what it reported; rerun it or record why not"));
        }
    }
    a
}

fn status(label: &str, o: &Opened) -> String {
    let mut s = String::new();
    let st = &o.state;
    let _ = writeln!(s, "REGISTER  {}  |  {} events  |  head {} (prefix; verify prints it in full)", label, o.events.len(), &o.head[..16]);
    let std = st.standard();
    let _ = writeln!(s, "SYSTEM    {}   USE {}   LINK {}{}", st.system().map(|x| x.name()).unwrap_or("not recorded (kept on another system)"), st.in_use().name(),
                     st.link().name(), if st.link() == Link::Offline { " (nothing new can be retrieved)" } else { "" });
    let _ = writeln!(s, "STANDARD  method {:?}, model findings {:?}, human adoption {}, two-person change {}, fixed tasks {}, offline only {}",
                     std.method, std.model_findings, std.human_adoption, std.dual_authorization, std.static_tasks, std.offline_only);
    let _ = writeln!(s, "WORKSPACE {}", profile_words(&profile_of(&o.events)));
    if let Some((by, actor)) = st.adoption() {
        let how = match adoption_basis(o).as_deref() {
            Some("HOST_OBSERVED_PROMPT") => "the host saw the command they typed",
            Some("ENTERED_AT_A_TERMINAL") => "entered at a terminal",
            Some("RELAYED_WHOLE_PROMPT_OBSERVED") => "relayed: the host kept the person's whole prompt, and the participant took it for the instruction",
            Some("RELAYED_NOT_OBSERVED") => "relayed: the host did not see the instruction",
            _ => "as recorded",
        };
        let _ = writeln!(s, "ADOPTION  by {by}, recorded by {actor} ({how})");
    }
    if !st.delivery.is_empty() {
        let d: Vec<String> = st.delivery.iter().map(|(a, d)| format!("{} {}", a, d.name())).collect();
        let _ = writeln!(s, "DELIVERY  {}", d.join(", "));
    }
    let att = attention(o);
    let _ = writeln!(s, "ATTENTION {}", if att.is_empty() { "nothing needs a response".to_string() } else { format!("{} item(s)", att.len()) });
    for (p, what, response) in &att {
        let _ = writeln!(s, "  {:<6} {}\n         -> {}", p, what, response);
    }
    let _ = writeln!(s, "REVIEWS   {:<12} {:<9} {:<17} {:>3} {:>6} {:>8} {:>10}", "LABEL", "PROCEDURE", "STATE", "SES", "GATES", "NO-FIND", "OBJECTIONS");
    for r in st.reviews.values() {
        let _ = writeln!(s, "          {:<12} {:<9} {:<17} {:>3} {:>6} {:>8} {:>10}", r.label, r.procedure, r.lifecycle.name(), r.session,
                         format!("{}/{}", r.passed_gates.len(), r.required_gates.len()), st.claims_without_finding(r).len(), st.open_objections(&r.label).len());
    }
    if !st.reviews.is_empty() {
        let _ = writeln!(s, "          (NO-FIND: claims that have no finding yet. OBJECTIONS: objections still open.)");
    }
    // What each finding in force rests on, read from the events: nothing here is a judgment of the finding.
    let rows = findings_basis(o);
    if !rows.is_empty() {
        let _ = writeln!(s, "FINDINGS  {:<4} {:<10} {:<8} {:<24} RESTS ON", "ID", "REVIEW", "CLAIM", "DISPOSITION");
        for (id, review, claim, disp, rests) in &rows {
            let _ = writeln!(s, "          {:<4} {:<10} {:<8} {:<24} {}", id, clip(review, 10), clip(claim, 8), disp, rests);
        }
        let _ = writeln!(s, "          (A check the host saw run is its observation of the command and its output. A reported check is the participant's account. Neither says the command tests the claim.)");
    }
    // A task's state is what the record observed, in the record's own words: STARTED until its end is
    // recorded; STOP-ASKED when a stop was requested and no end is recorded (termination unknown).
    let _ = writeln!(s, "TASKS     {:<4} {:<16} {:<8} {:<10} {:<10} {:<10} {:>13}  RESULT", "ID", "LABEL", "WORKER", "BY", "REVIEW", "STATE", "SPENT/BUDGET");
    for (id, t) in &st.tasks {
        let state = match (&t.outcome, t.stop_requested) {
            (Some(x), _) => x.as_str(),
            (None, Some(_)) => "STOP-ASKED",
            (None, None) => "STARTED",
        };
        let result = match (t.outcome.as_deref(), &t.result) {
            (Some("DONE"), Some(d)) => format!("read back {}", clip(d, 12)),
            (Some(_), Some(d)) => format!("reported {}, not accepted", clip(d, 12)),
            (Some(_), None) => "none".to_string(),
            (None, _) => "-".to_string(),
        };
        let _ = writeln!(s, "          {:<4} {:<16} {:<8} {:<10} {:<10} {:<10} {:>13}  {}", id, t.label, t.worker, t.actor, t.review.as_deref().unwrap_or("-"),
                         state, format!("{}/{}", t.spent, t.budget), result);
    }
    let open_reviews: Vec<&str> = st.reviews.values().filter(|r| matches!(r.lifecycle, Review::Active | Review::Paused)).map(|r| r.label.as_str()).collect();
    let theirs: Vec<&str> = st.tasks.iter().filter(|(_, t)| t.outcome.is_none() && t.review.as_deref().map(|r| open_reviews.contains(&r)).unwrap_or(false)).map(|(id, _)| id.as_str()).collect();
    let _ = writeln!(s, "A HALT NOW would close {} review(s) as incomplete [{}] and ask their {} running task(s) to stop [{}]. Asking is not stopping: each stays STOP-ASKED until its end is recorded.{}",
                     open_reviews.len(), open_reviews.join(" "), theirs.len(), theirs.join(" "), if st.in_use() == Use::NotInUse { " ACR is not in use." } else { "" });
    let ob = st.open_obligations();
    if !ob.is_empty() {
        let _ = writeln!(s, "OPEN OBLIGATIONS {}", ob.join(" "));
    }
    s
}

/// For each finding not superseded: what it cites, and how the record came by each check among it.
fn findings_basis(o: &Opened) -> Vec<(String, String, String, String, String)> {
    let body = |e: &Value, k: &str| e.get("body").and_then(|b| b.get(k)).and_then(Value::as_str).map(|x| x.to_string()).unwrap_or_default();
    let of_kind = |k: &'static str| o.events.iter().filter(move |e| e.get("kind").and_then(Value::as_str) == Some(k));
    let superseded: Vec<String> = of_kind("FINDING_RECORDED").map(|e| body(e, "supersedes")).filter(|x| !x.is_empty()).collect();
    let checks: BTreeMap<String, (String, String)> = of_kind("CHECK_RAN").map(|e| (body(e, "id"), (body(e, "observed"), e.get("actor").and_then(Value::as_str).unwrap_or("").to_string()))).collect();
    let mut rows = Vec::new();
    for e in of_kind("FINDING_RECORDED") {
        let id = body(e, "id");
        if superseded.contains(&id) {
            continue;
        }
        let mut cited: Vec<String> = Vec::new();
        let mut others = 0;
        for side in ["basis", "contrary"] {
            if let Some(Value::List(l)) = e.get("body").and_then(|b| b.get(side)) {
                for it in l {
                    match it.get("ref").and_then(|r| r.get("t")).and_then(Value::as_str) {
                        Some("check") => {
                            let k = it.get("ref").and_then(|r| r.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
                            cited.push(match checks.get(&k).map(|(o, a)| (o.as_str(), a.as_str())) {
                                Some(("HOST_OBSERVED_TOOL", _)) => format!("check {k}, which the host saw run"),
                                Some(("RAN_BY_THIS_PROGRAM", _)) => format!("check {k}, which the recording program ran"),
                                Some((_, a)) => format!("check {k}, reported by {a} and not seen by the host"),
                                None => format!("check {k}"),
                            });
                        }
                        _ => others += 1,
                    }
                }
            }
        }
        let rests = match (cited.is_empty(), others) {
            (true, 0) => "nothing cited".to_string(),
            (true, n) => format!("reasoning over {n} inspected item(s); no check was run"),
            (false, 0) => cited.join("; "),
            (false, n) => format!("{}; and {n} inspected item(s)", cited.join("; ")),
        };
        rows.push((id, body(e, "review"), body(e, "claim"), body(e, "disposition"), rests));
    }
    rows
}

fn verify_text(o: &Opened, limit: usize) -> String {
    let mut t = format!("chain intact: {} events, head {}\nguards replayed: {} problem(s)\nThe guards are those of pcpos {}. A register is read under the rules of the program that reads it: an event an earlier build accepted may be reported here, and its bytes are unchanged.\n",
                        o.events.len(), o.head, o.problems.len(), env!("CARGO_PKG_VERSION"));
    for p in o.problems.iter().take(limit) {
        let _ = writeln!(t, "  event {}: {}", p.seq, p.message);
    }
    if o.problems.len() > limit {
        let _ = writeln!(t, "  and {} more", o.problems.len() - limit);
    }
    let _ = writeln!(t, "register file SHA-256: {}", sha256_hex(&o.raw));
    t.push_str("A pass is a fact about these bytes. It authenticates nobody and establishes nothing about whether a finding is right.\n");
    t
}

fn machine_text(name: Option<&str>) -> Result<String, String> {
    let mut all = String::new();
    machine::export(&mut all).map_err(|_| "export failed".to_string())?;
    match name {
        None => Ok(all),
        Some(n) => {
            let n = n.to_ascii_uppercase();
            let t: String = all.lines().filter(|l| l.split('\t').next() == Some(n.as_str())).map(|l| format!("{l}\n")).collect();
            if t.is_empty() { Err(format!("no machine named {n}; the machines are SYSTEM USE REVIEW OBJECTION OBLIGATION TASK DELIVERY LINK")) } else { Ok(t) }
        }
    }
}

// ---- time, for events appended here --------------------------------------------------------------------

fn now_utc() -> String {
    utc_at(std::time::SystemTime::now())
}

/// A time as the register writes it, to the second.
fn utc_at(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil date from a day count (proleptic Gregorian calendar).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

// ---- values ---------------------------------------------------------------------------------------------

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn opt(x: &Option<String>) -> Value {
    x.as_deref().map(s).unwrap_or(Value::Null)
}

fn label_ok(l: &str) -> bool {
    let b = l.as_bytes();
    !b.is_empty() && b.len() <= 64 && b[0].is_ascii_alphanumeric() && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// A label made from text this program did not choose (an agent's type, a directory's name).
fn to_label(text: &str, fallback: &str) -> String {
    let mut l: String = text.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' }).take(64).collect();
    while l.starts_with(|c: char| !c.is_ascii_alphanumeric()) {
        l.remove(0);
    }
    if label_ok(&l) { l } else { fallback.to_string() }
}

fn clip(text: &str, chars: usize) -> String {
    text.chars().filter(|c| !matches!(*c as u32, 0..=8 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f)).take(chars).collect::<String>().trim().to_string()
}

// ---- the session: who this process acts for, fixed when it starts ------------------------------------------

struct Session {
    root: PathBuf,
    register: String,
    /// The label of the participant whose tool calls arrive here. Never chosen by a caller.
    actor: String,
    /// The label of the person directing the work.
    principal: String,
    /// True if that label was configured; otherwise a register's own single directing person is used.
    principal_set: bool,
    method: Option<PathBuf>,
    edition: Option<Edition>,
    model: Option<String>,
    lineage: Option<String>,
    /// The packaged folder this program was started from, where a host names one.
    plugin: Option<PathBuf>,
}

fn setting(names: &[&str]) -> Option<String> {
    names.iter().filter_map(|n| env::var(n).ok()).map(|v| v.trim().to_string()).find(|v| !v.is_empty() && !v.contains("${"))
}

fn edition_of(name: &str) -> Option<Edition> {
    match name.to_ascii_uppercase().as_str() {
        "PERSONAL" => Some(Edition::Personal),
        "COMMERCIAL" => Some(Edition::Commercial),
        "ENTERPRISE" => Some(Edition::Enterprise),
        _ => None,
    }
}

fn session(hint: Option<&str>) -> Result<Session, String> {
    let root = setting(&["PCPOS_ROOT", "CLAUDE_PROJECT_DIR"]).or(hint.map(|h| h.to_string())).map(PathBuf::from).map(Ok).unwrap_or_else(env::current_dir)
        .and_then(|p| p.canonicalize()).map_err(|e| format!("the directory to serve cannot be resolved: {e}"))?;
    let label = |names: &[&str], default: &str| setting(names).filter(|l| label_ok(l) && l != "host").unwrap_or_else(|| default.to_string());
    let actor = label(&["PCPOS_ACTOR", "CLAUDE_PLUGIN_OPTION_SESSION_LABEL"], "assistant");
    let principal = label(&["PCPOS_PRINCIPAL", "CLAUDE_PLUGIN_OPTION_PRINCIPAL_LABEL"], "principal");
    // Where a host starts this program from a packaged folder, the method text and the edition marker beside it are used.
    let plugin = setting(&["CLAUDE_PLUGIN_ROOT"]).map(PathBuf::from);
    if actor == principal {
        return Err("the session's participant and the person directing the work have the same label; they are different participants".to_string());
    }
    Ok(Session {
        principal_set: setting(&["PCPOS_PRINCIPAL", "CLAUDE_PLUGIN_OPTION_PRINCIPAL_LABEL"]).is_some(),
        root,
        register: setting(&["PCPOS_REGISTER", "CLAUDE_PLUGIN_OPTION_REGISTER_PATH"]).unwrap_or_else(|| ".pcp/register.jsonl".to_string()),
        actor,
        principal,
        method: setting(&["PCPOS_METHOD"]).map(PathBuf::from).or_else(|| plugin.as_ref().map(|p| p.join("method")).filter(|m| m.join("MANIFEST.json").is_file())),
        edition: setting(&["PCPOS_EDITION"])
            .or_else(|| plugin.as_ref().and_then(|p| fs::read(p.join("edition").join("EDITION.json")).ok()).and_then(|raw| lenient::parse(&raw)).and_then(|v| v.get("edition").and_then(Value::as_str).map(|e| e.to_string())))
            .and_then(|e| edition_of(e.trim())),
        model: setting(&["PCPOS_MODEL"]),
        lineage: setting(&["PCPOS_LINEAGE"]),
        plugin,
    })
}

impl Session {
    /// The person directing the work, as a register has them: the label configured for the session, or
    /// else the one person the register records as directing it.
    fn principal_in(&self, st: &State) -> String {
        if !self.principal_set {
            let people: Vec<&String> = st.participants.iter().filter(|(_, p)| p.principal && p.kind == "HUMAN").map(|(l, _)| l).collect();
            if people.len() == 1 {
                return people[0].clone();
            }
        }
        self.principal.clone()
    }

    /// Where a person's added modules stand: a folder beside the register, inside what the write fence covers.
    fn added(&self) -> Option<PathBuf> {
        let beside = Path::new(&self.register).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        Some(self.root.join(beside).join("modules")).filter(|d| d.is_dir())
    }

    fn observed(&self) -> String {
        match Path::new(&self.register).parent().filter(|p| !p.as_os_str().is_empty()) {
            Some(d) => format!("{}/observed.jsonl", d.display()),
            None => "observed.jsonl".to_string(),
        }
    }

    /// What the host itself saw shell commands do while ACR was in use: the command, its exit status and the
    /// digests of its output. Never the output itself.
    fn observed_tools(&self) -> String {
        match Path::new(&self.register).parent().filter(|p| !p.as_os_str().is_empty()) {
            Some(d) => format!("{}/observed_tools.jsonl", d.display()),
            None => "observed_tools.jsonl".to_string(),
        }
    }
}

// ---- the method release this core implements -------------------------------------------------------------

struct MethodCheck {
    fingerprint: String,
    listed: usize,
    equal: usize,
    absent: usize,
    /// Files absent that are not among the packet's programs and probes.
    absent_text: usize,
    differing: Vec<String>,
}

impl MethodCheck {
    fn is_native(&self) -> bool {
        self.fingerprint == acr::METHOD_FINGERPRINT
    }

    fn complete(&self) -> bool {
        self.is_native() && self.differing.is_empty() && self.absent_text == 0 && self.equal > 0
    }

    fn text(&self) -> String {
        format!("manifest {} ({} the release this core implements); {} file(s) listed: {} present and equal, {} absent{}, {} differing{}",
                self.fingerprint, if self.is_native() { "is" } else { "IS NOT" }, self.listed, self.equal, self.absent,
                if self.absent > 0 && self.absent_text == 0 { " (all of them the packet's programs and probes, which a plugin does not carry)" } else { "" }, self.differing.len(),
                if self.differing.is_empty() { String::new() } else { format!(" [{}]", self.differing.join(" ")) })
    }
}

/// The parts of the method packet a plugin does not carry: its Python programs and the probes that test a
/// delivery, model answers included. A plugin holds the method's text, each file the packet's own bytes.
const NOT_IN_A_PLUGIN: [&str; 2] = ["INTEGRATION/", "TOOLS/"];

/// Compare a method packet's files with its own manifest, and the manifest with the release this core implements.
fn method_check(dir: &Path) -> Result<MethodCheck, String> {
    let raw = fs::read(dir.join("MANIFEST.json")).map_err(|e| format!("cannot read the manifest in {}: {}", dir.display(), e))?;
    let m = lenient::parse(&raw).ok_or("the manifest is not readable")?;
    let files = match m.get("files") {
        Some(Value::List(l)) => l,
        _ => return Err("the manifest lists no files".to_string()),
    };
    let mut c = MethodCheck { fingerprint: sha256_hex(&raw), listed: files.len(), equal: 0, absent: 0, absent_text: 0, differing: Vec::new() };
    for f in files {
        let (path, want) = match (f.get("path").and_then(Value::as_str), f.get("sha256").and_then(Value::as_str)) {
            (Some(p), Some(w)) if Path::new(p).components().all(|c| matches!(c, Component::Normal(_))) => (p, w),
            _ => return Err("the manifest holds an entry this program does not accept".to_string()),
        };
        match fs::read(dir.join(path)) {
            Ok(b) if sha256_hex(&b) == want => c.equal += 1,
            Ok(_) => c.differing.push(path.to_string()),
            Err(_) => {
                c.absent += 1;
                if !NOT_IN_A_PLUGIN.iter().any(|d| path.starts_with(d)) {
                    c.absent_text += 1;
                }
            }
        }
    }
    Ok(c)
}

// ---- modules: found in the method text at hand, never listed in this program -------------------------------

/// One module of the method, as its own Specification states it.
struct Module {
    key: String,
    id: String,
    version: String,
    sha256: String,
    words: usize,
    /// The Core descriptor a specialist says it was written for, where its header gives one.
    core: Option<String>,
    /// Its Specification.
    file: PathBuf,
    /// True if a person added it beside the register; false if it is a module of the method text.
    added: bool,
}

impl Module {
    /// A specialist written for another Core than the one this program implements.
    fn other_core(&self) -> bool {
        self.key != "core" && self.core.as_deref().map(|c| c != acr::CORE_DESCRIPTOR).unwrap_or(false)
    }

    fn role(&self) -> &'static str {
        if self.key == "core" { "CORE" } else if self.added { "ADDED" } else { "SPECIALIST" }
    }

    fn record(&self) -> Value {
        obj(vec![("id", s(&self.id)), ("version", s(&self.version)), ("sha256", s(&self.sha256))])
    }
}

/// What a Specification's first lines say it is: (identifier, key, version, the Core it was written for).
/// The version is the whole field as written, so that 1.0.1 and 2.4.1-rc.2+build.4 are not read as shorter ones.
fn component_header(text: &str) -> Result<(String, String, String, Option<String>), &'static str> {
    let line = text.lines().take(60).find(|l| l.starts_with("**Component:**")).ok_or("its Specification names no component; not a module")?;
    let id = line.split('`').nth(1).unwrap_or("").to_string();
    let version: String = line.split("Version ").nth(1).unwrap_or("").split_whitespace().next().unwrap_or("").trim_end_matches(['.', ',', ';']).to_string();
    let version_ok = !version.is_empty() && version.len() <= 64 && version.starts_with(|c: char| c.is_ascii_digit()) && version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    // `acr.` names a module of the method. `local.` names one a person added; it can never carry a method module's identifier.
    let key = match id.strip_prefix("acr.").or_else(|| id.strip_prefix("local.")) {
        Some(k) if label_ok(k) && !k.contains('.') && version_ok => k.to_string(),
        _ => return Err("its component line is not `acr.<name>`, Version <n>; not a module"),
    };
    let core = text.lines().take(60).find(|l| l.starts_with("**Required core:**")).and_then(|l| l.split('`').find(|x| x.len() == 64 && x.bytes().all(|b| b.is_ascii_hexdigit())).map(|x| x.to_string()));
    Ok((id, key, version, core))
}

/// The largest Specification this program reads.
const MAX_SPECIFICATION: u64 = 4 * 1024 * 1024;

/// The modules at hand: every folder under the method text's MODULES that holds a Specification naming its
/// own component, and then every module a person added beside the register. Adding or removing a module is
/// adding or removing a folder; nothing here is edited.
fn modules_in(se: &Session) -> Vec<Module> {
    modules_and_oddities(se).0
}

/// The modules, and every folder that is not one, with the reason. Nothing is skipped in silence.
/// An identifier that two folders claim is not served from either: which text a review was held to must
/// never depend on the order of folder names. A module of the method text is never displaced by an added one.
fn modules_and_oddities(se: &Session) -> (Vec<Module>, Vec<String>) {
    let mut odd: Vec<String> = Vec::new();
    let mut out: Vec<Module> = Vec::new();
    let mut ambiguous: Vec<String> = Vec::new();
    let added = se.added().filter(|d| d.canonicalize().map(|c| c.starts_with(&se.root)).unwrap_or(false));
    let sources: Vec<(PathBuf, bool)> = se.method.iter().map(|m| (m.join("MODULES"), false)).chain(added.into_iter().map(|d| (d, true))).collect();
    if se.method.is_some() && !se.method.as_ref().map(|m| m.join("MODULES").is_dir()).unwrap_or(false) {
        odd.push("there is no MODULES folder".to_string());
    }
    for (base, is_added) in sources {
        let mut dirs: Vec<PathBuf> = match fs::read_dir(&base) {
            Ok(d) => d.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
            Err(_) => continue,
        };
        dirs.sort();
        let packet: Vec<String> = out.iter().map(|m| m.key.clone()).chain(ambiguous.iter().cloned()).collect();
        for d in dirs {
            let folder = format!("{}{}", if is_added { "added/" } else { "" }, clip(&d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), 80));
            if !d.is_dir() {
                continue;
            }
            let file = d.join("SOURCE").join("Specification.md");
            let bytes = match fs::metadata(&file).ok().filter(|m| m.is_file() && m.len() <= MAX_SPECIFICATION).and_then(|_| fs::read(&file).ok()) {
                Some(b) => b,
                None => {
                    odd.push(format!("{folder}: no SOURCE/Specification.md that can be read; not a module"));
                    continue;
                }
            };
            let text = String::from_utf8_lossy(&bytes);
            let (id, key, version, core) = match component_header(&text) {
                Ok(h) => h,
                Err(why) => {
                    odd.push(format!("{folder}: {why}"));
                    continue;
                }
            };
            if is_added != id.starts_with("local.") {
                odd.push(format!("{folder}: {id} is not {}; not used", if is_added { "named `local.<name>`, as an added module is" } else { "named `acr.<name>`, as a module of the method is" }));
                continue;
            }
            if is_added && packet.contains(&key) {
                odd.push(format!("{folder}: {id} takes the name of a module of the method text; the added folder is not used"));
                continue;
            }
            if ambiguous.contains(&key) {
                odd.push(format!("{folder}: another folder for {id}; none of them is used"));
                continue;
            }
            if let Some(at) = out.iter().position(|m| m.key == key) {
                let first = out.remove(at);
                odd.push(format!("{folder}: a second folder for {id} (the first is {}); the identifier is ambiguous and neither is used", first.file.parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                ambiguous.push(key);
                continue;
            }
            let m = Module { key, id, version, sha256: sha256_hex(&bytes), words: text.split_whitespace().count(), core, file, added: is_added };
            if m.other_core() {
                odd.push(format!("{folder}: {} says it was written for another Core than this program implements; it can be read, and cannot be selected", m.id));
            }
            out.push(m);
        }
    }
    if se.method.is_some() && !out.iter().any(|m| m.key == "core") {
        odd.push("no usable Core module was found".to_string());
    }
    (out, odd)
}

/// The number of the provision a line begins, if it begins one, and its depth: 2 or 3 for a numbered
/// heading or an appendix, 9 for a numbered requirement ("4.5. ..."). An item of an ordinary list is not one.
fn provision_start(line: &str) -> Option<(String, u8)> {
    let (depth, rest) = match (line.strip_prefix("### "), line.strip_prefix("## ")) {
        (Some(r), _) => (3u8, r),
        (_, Some(r)) => (2u8, r),
        _ => (9u8, line),
    };
    if depth != 9 {
        if let Some(r) = rest.strip_prefix("Appendix ") {
            let letter: String = r.chars().take_while(|c| c.is_ascii_uppercase()).collect();
            return if letter.len() == 1 { Some((format!("Appendix {letter}"), depth)) } else { None };
        }
    }
    let run: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let number = run.trim_end_matches('.');
    if number.is_empty() || !number.starts_with(|c: char| c.is_ascii_digit()) || number.contains("..") || !rest[run.len()..].starts_with(' ') {
        return None;
    }
    if depth == 9 && !(run.ends_with('.') && number.contains('.')) {
        return None;
    }
    Some((number.to_string(), depth))
}

/// The exact text of one provision of a module's Specification, or the list of its headings.
fn clause_text(se: &Session, module: &str, section: Option<&str>) -> Result<String, String> {
    se.method.as_deref().ok_or("no method text is at hand")?;
    let (found, odd) = modules_and_oddities(se);
    let want = module.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    let want = want.strip_prefix("acr.").or_else(|| want.strip_prefix("local.")).unwrap_or(&want).to_string();
    let m = found.iter().find(|m| m.key == want).ok_or_else(|| match odd.iter().find(|o| o.contains(&format!(".{want} ")) && o.contains("ambiguous")) {
        Some(o) => format!("{want} is ambiguous in the method text at hand and no text is served for it ({o}). Remove one of the folders"),
        None => format!("{} is not a module of the method text at hand. The modules are: {}", clip(module, 60), found.iter().map(|m| m.key.as_str()).collect::<Vec<_>>().join(", ")),
    })?;
    let text = fs::read_to_string(&m.file).map_err(|e| format!("the Specification of {} cannot be read: {e}", m.id))?;
    let lines: Vec<&str> = text.lines().collect();
    let marks: Vec<(usize, String, u8)> = lines.iter().enumerate().filter_map(|(i, l)| provision_start(l).map(|(n, d)| (i, n, d))).collect();
    let head = format!("{} {} | Specification SHA-256 {}", m.id, m.version, m.sha256);
    let section = match section.map(|s| s.trim().trim_start_matches('§').trim_end_matches('.').trim()).filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            let mut t = format!("{head}\nHEADINGS\n");
            for (i, _, d) in marks.iter().filter(|(_, _, d)| *d != 9) {
                let _ = writeln!(t, "{}{}", if *d == 3 { "  " } else { "" }, lines[*i].trim_start_matches('#').trim());
            }
            return Ok(t);
        }
    };
    let section = if section.to_ascii_lowercase().starts_with("appendix") { format!("Appendix {}", section[8..].trim().to_ascii_uppercase()) } else { section.to_string() };
    let at = marks.iter().position(|(_, n, _)| *n == section).ok_or(format!("{} has no provision numbered {}. Call this tool without a section for its headings. Nothing was fetched; do not supply the text from memory", m.id, clip(&section, 40)))?;
    let (start, _, depth) = (marks[at].0, &marks[at].1, marks[at].2);
    let end = marks[at + 1..].iter().find(|(_, _, d)| depth == 9 || (*d != 9 && *d <= depth)).map(|(i, _, _)| *i).unwrap_or(lines.len());
    // What is shown is exactly what the digest is of: the provision without the white space around it, and one line end.
    let body = lines[start..end].join("\n").trim().to_string();
    let digest = sha256_hex(format!("{body}\n").as_bytes());
    Ok(format!("{head}\nPROVISION {section} | lines {}..{} | SHA-256 of this text {digest}\n\n{}\n", start + 1, end, clip_long(&body, 60_000)))
}

/// Text passed on whole, up to a limit that only a damaged file would reach.
fn clip_long(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars { text.to_string() } else { format!("{}\n[cut at {chars} characters]", text.chars().take(chars).collect::<String>()) }
}

/// A commission names its modules by identifier, version and digest. Where the method text is at hand,
/// each must be a specialist module found there, with that version and that digest, once.
fn commission_modules(se: &Session, body: &Value) -> Result<(), String> {
    let list = match body.get("modules") {
        Some(Value::List(l)) if !l.is_empty() => l,
        _ => return Ok(()),
    };
    if se.method.is_none() {
        return Ok(()); // nothing to compare with: the record keeps what was declared, and METHOD_PINNED says the text was not at hand
    }
    let found = modules_in(se);
    let mut seen: Vec<&str> = Vec::new();
    for m in list {
        let id = m.get("id").and_then(Value::as_str).unwrap_or("");
        let f = found.iter().find(|f| f.id == id).ok_or(format!("{} is not a module of the method text at hand. Call acr_modules", clip(id, 80)))?;
        if f.key == "core" {
            return Err("Core applies to every review and is not listed among the selected modules".to_string());
        }
        if f.other_core() {
            return Err(format!("{id} was written for another Core than this program implements"));
        }
        if m.get("sha256").and_then(Value::as_str) != Some(f.sha256.as_str()) || m.get("version").and_then(Value::as_str) != Some(f.version.as_str()) {
            return Err(format!("the version or digest given for {id} is not that of its Specification here ({} {}). Copy it from acr_modules", f.version, f.sha256));
        }
        if seen.contains(&id) {
            return Err(format!("{id} is listed twice"));
        }
        seen.push(id);
    }
    Ok(())
}

/// The workspace profile as the record has it: the newest selection of modules, with who made it and how.
struct Profile {
    seq: i64,
    by: String,
    observed: String,
    prompt: Option<String>,
    modules: Vec<(String, String, String)>,
}

fn profile_of(events: &[Value]) -> Option<Profile> {
    events.iter().rev().find_map(|e| {
        let b = e.get("body")?;
        if e.get("kind").and_then(Value::as_str) != Some("HOST_EVENT") || b.get("event").and_then(Value::as_str) != Some("WORKSPACE_PROFILE") {
            return None;
        }
        let modules = match b.get("modules") {
            Some(Value::List(l)) => l.iter().filter_map(|m| Some((m.get("id")?.as_str()?.to_string(), m.get("version")?.as_str()?.to_string(), m.get("sha256")?.as_str()?.to_string()))).collect(),
            _ => Vec::new(),
        };
        Some(Profile { seq: e.get("seq").and_then(Value::as_int).unwrap_or(-1), by: b.get("by").and_then(Value::as_str).unwrap_or("").to_string(),
                       observed: b.get("observed").and_then(Value::as_str).unwrap_or("").to_string(),
                       prompt: b.get("prompt_sha256").and_then(Value::as_str).map(|x| x.to_string()), modules })
    })
}

fn profile_words(p: &Option<Profile>) -> String {
    match p {
        None => "Core only (no selection recorded; a specialist module applies when the person directing the work names it)".to_string(),
        Some(p) if p.modules.is_empty() => format!("Core only (selected by {} at event {}, {})", p.by, p.seq, p.observed),
        Some(p) => format!("Core + {} (selected by {} at event {}, {})", p.modules.iter().map(|(id, _, _)| id.trim_start_matches("acr.")).collect::<Vec<_>>().join(" + "), p.by, p.seq, p.observed),
    }
}

/// Record the modules this workspace's reviews use. A person's act: the caller says how the host knows it.
fn set_profile(tx: &mut Tx, se: &Session, names: &[&str], by: &str, observed: &str, words: &str, digest: Option<&str>) -> Result<Put, String> {
    se.method.as_deref().ok_or("no method text is at hand, so a module cannot be identified by its digest")?;
    let found = modules_in(se);
    let mut chosen: Vec<&Module> = Vec::new();
    for n in names.iter().map(|n| n.trim().trim_end_matches(',')).filter(|n| !n.is_empty() && !matches!(n.to_ascii_lowercase().as_str(), "core" | "acr.core" | "none" | "and" | "+")) {
        let want = n.to_ascii_lowercase().replace('-', "_");
        let want = want.strip_prefix("acr.").or_else(|| want.strip_prefix("local.")).unwrap_or(&want).to_string();
        match found.iter().find(|m| m.key == want && m.key != "core") {
            Some(m) if m.other_core() => return Err(format!("{} was written for another Core than this program implements, and cannot be selected", m.id)),
            Some(m) if !chosen.iter().any(|c| c.key == m.key) => chosen.push(m),
            Some(_) => {}
            None => return Err(format!("{} is not a module of the method text at hand. The modules are: {}", clip(n, 60), found.iter().map(|m| m.key.as_str()).collect::<Vec<_>>().join(", "))),
        }
    }
    let mut body = vec![("event", s("WORKSPACE_PROFILE")), ("by", s(by)), ("observed", s(observed)), ("words", s(&clip(words, 2000))),
                        ("modules", Value::List(chosen.iter().map(|m| m.record()).collect()))];
    if let Some(d) = digest {
        body.push(("prompt_sha256", s(d)));
    }
    tx.put("host", "HOST", "HOST_EVENT", obj(body))
}

fn modules_text(se: &Session, o: Option<&Opened>) -> String {
    let mut t = String::new();
    match se.method.as_deref() {
        None => t.push_str("MODULES   no method text is at hand\n"),
        Some(_) => {
            let _ = writeln!(t, "MODULES   {:<18} {:<24} {:<8} {:<10} {:>6}  SHA-256 OF ITS SPECIFICATION", "KEY", "ID", "VERSION", "ROLE", "WORDS");
            let (found, odd) = modules_and_oddities(se);
            for m in &found {
                let _ = writeln!(t, "          {:<18} {:<24} {:<8} {:<10} {:>6}  {}", m.key, m.id, m.version, m.role(), m.words, m.sha256);
            }
            for o in &odd {
                let _ = writeln!(t, "  NOTE    {o}");
            }
        }
    }
    let p = o.and_then(|o| profile_of(&o.events));
    let _ = writeln!(t, "WORKSPACE {}", profile_words(&p));
    if let Some(p) = p.as_ref().filter(|p| !p.modules.is_empty()) {
        let list = Value::List(p.modules.iter().map(|(id, v, d)| obj(vec![("id", s(id)), ("version", s(v)), ("sha256", s(d))])).collect());
        let _ = writeln!(t, "For REVIEW_COMMISSIONED in this workspace: \"modules\": {}", String::from_utf8_lossy(&list.canonical_json()));
    }
    t.push_str("Core applies whenever ACR is in use. The presence of a module here selects nothing: a specialist module applies to a review when the person directing the work, or a policy they set, selects it. Read a selected module's Specification before the first step that depends on it.\n");
    t
}

/// The colours of a module's plate, for a terminal: (background, text). The same values as the design tokens.
fn plate(key: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    match key {
        "mathematics" => ((0x70, 0x1A, 0x32), (0xF2, 0xF7, 0xFC)),
        "science" => ((0xA5, 0xE7, 0xF5), (0x10, 0x38, 0x44)),
        "engineering" => ((0xD0, 0x5D, 0x29), (0x21, 0x10, 0x08)),
        "cryptography" => ((0x34, 0x21, 0x4F), (0xF2, 0xF7, 0xFC)),
        "statistics" => ((0xFF, 0xFF, 0xFF), (0x15, 0x21, 0x37)),
        "quality_assurance" => ((0xB5, 0xD9, 0x4B), (0x15, 0x21, 0x37)),
        _ => ((0x10, 0x28, 0x3F), (0xF2, 0xF7, 0xFC)),
    }
}

/// One line that names the workspace, in its modules' colours where the output is a terminal that takes
/// colour (NO_COLOR is honoured). The words carry everything; the colour repeats it.
fn banner(o: &Opened, colour: bool) -> String {
    let p = profile_of(&o.events);
    let keys: Vec<String> = match &p {
        Some(p) if !p.modules.is_empty() => p.modules.iter().map(|(id, _, _)| id.trim_start_matches("acr.").to_string()).collect(),
        _ => vec!["core".to_string()],
    };
    let edition = o.events.iter().find_map(|e| e.get("body").filter(|b| b.get("event").and_then(Value::as_str) == Some("ENVIRONMENT")).and_then(|b| b.get("edition")).and_then(Value::as_str))
        .unwrap_or("NOT_STATED");
    let mut t = format!("PCP OS    {} edition", edition);
    for k in &keys {
        let name = k.replace('_', " ").to_ascii_uppercase();
        if colour {
            let ((r, g, b), (fr, fg, fb)) = plate(k);
            let _ = write!(t, "  \x1b[48;2;{r};{g};{b}m\x1b[38;2;{fr};{fg};{fb}m {name} \x1b[0m");
        } else {
            let _ = write!(t, "  [{name}]");
        }
    }
    t.push_str(if keys == ["core"] { " workspace\n" } else { " workspace (Core applies throughout)\n" });
    t
}

// ---- operations shared by the tool server, the hooks and the command line -----------------------------------

fn refusal_text(r: &acr::Refusal) -> String {
    format!("REFUSE (exit code {}){}\n{}\nNothing was written.\n", r.code, r.clause.map(|c| format!(" [{c}]")).unwrap_or_default(), r.message)
}

fn receipt(seq: u64, head: &str, prev: &str, kind: &str, actor: &str, auth: &str) -> String {
    format!("WRITTEN as event {seq}: {kind} by {actor} ({auth})\nhead {head}\nprevious head {prev}\n")
}

/// Begin a register if there is none, and make sure this session's participant is in it. Written by the
/// host. Returns what was done, in words.
fn begin(tx: &mut Tx, se: &Session, principal_name: Option<&str>, with_session: bool) -> Result<String, String> {
    let mut did = Vec::new();
    let host = |tx: &mut Tx, kind: &str, body: Value| -> Result<(), String> {
        match tx.put("host", "HOST", kind, body)? {
            Put::Written { .. } => Ok(()),
            Put::Refused(r) => Err(format!("{}: {}", kind, r.message)),
        }
    };
    if tx.state().events() == 0 {
        let name = to_label(se.root.file_name().and_then(|n| n.to_str()).unwrap_or(""), "workspace");
        let id = sha256_hex(format!("{}|{}", se.root.display(), now_utc()).as_bytes());
        host(tx, "WORKSPACE_CREATED", obj(vec![("label", s(&name)), ("profile", s("ACR-OS-RECORD-1")), ("program_version", s(env!("CARGO_PKG_VERSION"))),
                                              ("workspace", s(&format!("acr:workspace:{}", &id[..32])))]))?;
        host(tx, "PARTICIPANT_ADDED", obj(vec![("label", s(&se.principal)), ("kind", s("HUMAN")), ("principal", Value::Bool(true)),
                                              ("name", principal_name.map(|n| s(&clip(n, 200))).unwrap_or(Value::Null))]))?;
        let (verified, note) = match se.method.as_deref().map(method_check) {
            Some(Ok(c)) if c.complete() => ("FILES_VERIFIED_AGAINST_OWN_MANIFEST", c.text()),
            Some(Ok(c)) => ("DECLARED_NOT_VERIFIED", c.text()),
            Some(Err(e)) => ("DECLARED_NOT_VERIFIED", e),
            None => ("DECLARED_NOT_VERIFIED", "no method text was at hand to compare; the identity is that of the release this core implements".to_string()),
        };
        host(tx, "METHOD_PINNED", obj(vec![("release_fingerprint", s(acr::METHOD_FINGERPRINT)), ("core_descriptor_sha256", s(acr::CORE_DESCRIPTOR)), ("verified", s(verified)),
                                          ("presence", s("NATIVE")), ("note", s(&clip(&note, 1500)))]))?;
        host(tx, "HOST_EVENT", obj(vec![("event", s("ENVIRONMENT")), ("program", s(concat!("pcpos ", env!("CARGO_PKG_VERSION")))), ("os", s(env::consts::OS)), ("arch", s(env::consts::ARCH)),
                                                 ("edition", s(match se.edition { Some(Edition::Personal) => "PERSONAL", Some(Edition::Commercial) => "COMMERCIAL", Some(Edition::Enterprise) => "ENTERPRISE", None => "NOT_STATED" })),
                                                 ("actor_binding", s("SESSION_LABEL_FIXED_AT_START_NOT_AUTHENTICATED"))]))?;
        did.push(format!("began the register: person directing the work {}, method {} ({})", se.principal, &acr::METHOD_FINGERPRINT[..16], verified));
    }
    if with_session && !tx.state().participants.contains_key(&se.actor) {
        host(tx, "PARTICIPANT_ADDED", obj(vec![("label", s(&se.actor)), ("kind", s("AUTOMATED")), ("principal", Value::Bool(false)),
                                              ("model", s(&clip(se.model.as_deref().unwrap_or("not stated"), 200))), ("lineage", opt(&se.lineage.as_deref().map(|l| clip(l, 200))))]))?;
        did.push(format!("registered this session's participant {}", se.actor));
    }
    Ok(if did.is_empty() { "the register was already begun and this session's participant is in it".to_string() } else { did.join("; ") })
}

/// Where the oldest review still open was commissioned: its sequence number and its time. What a review
/// gathers comes after that point. What the record or the host held before it was not gathered for it.
fn open_review_floor(tx: &Tx) -> Option<(i64, String)> {
    let open: Vec<&str> = tx.state().reviews.values().filter(|r| matches!(r.lifecycle, Review::Active | Review::Paused)).map(|r| r.label.as_str()).collect();
    let o = parse(tx.rec.ledger().bytes().to_vec()).ok()?;
    o.events.iter().filter(|e| e.get("kind").and_then(Value::as_str) == Some("REVIEW_COMMISSIONED")
        && e.get("body").and_then(|b| b.get("label")).and_then(Value::as_str).map(|l| open.contains(&l)).unwrap_or(false))
        .filter_map(|e| Some((e.get("seq").and_then(Value::as_int)?, e.get("ts").and_then(Value::as_str)?.to_string()))).min()
}

/// Whether acr_artifact is to write an event. The argument is true or false; anything else is refused and not read as "write".
fn record_wanted(args: &Value) -> Result<bool, String> {
    match args.get("record") {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err("argument record is true or false".to_string()),
    }
}

/// True if these words stand, in the same order, in a prompt the host itself kept: the person typed them.
fn in_an_observed_prompt(se: &Session, text: &str) -> bool {
    let words = |x: &str| x.split_whitespace().collect::<Vec<_>>().join(" ");
    let want = words(text);
    let read = || -> Option<Vec<u8>> {
        let mut h = confined(&se.root, &se.observed(), false, false).ok()??;
        lock(&h, false).ok()?;
        read_all(&mut h).ok()
    };
    !want.is_empty() && read().unwrap_or_default().split(|b| *b == b'\n').filter(|l| !l.is_empty()).filter_map(|l| Value::parse(l).ok())
        .any(|v| v.get("text").and_then(Value::as_str).map(|t| words(t).contains(&want)).unwrap_or(false))
}

/// The newest prompt the host kept, if it is exactly these words and was typed after the last halt.
/// A fragment of a prompt is not the prompt: "do not adopt ACR" contains "adopt ACR".
fn observed_whole_prompt(se: &Session, basis: &str, after: Option<&str>) -> Option<String> {
    let mut h = confined(&se.root, &se.observed(), false, false).ok()??;
    lock(&h, false).ok()?;
    let raw = read_all(&mut h).ok()?;
    let last = raw.split(|b| *b == b'\n').filter(|l| !l.is_empty()).next_back().and_then(|l| Value::parse(l).ok())?;
    // The same words in the same order. How the words are spaced or broken into lines is not part of an instruction.
    let words = |x: &str| x.split_whitespace().collect::<Vec<_>>().join(" ");
    let same = last.get("text").and_then(Value::as_str).map(|t| words(t) == words(basis)).unwrap_or(false);
    let fresh = match (last.get("ts").and_then(Value::as_str), after) {
        (Some(ts), Some(halt)) => ts > halt,
        (Some(_), None) => true,
        _ => false,
    };
    if same && fresh { last.get("sha256").and_then(Value::as_str).map(|x| x.to_string()) } else { None }
}

/// Put ACR in use on the person's instruction. Only a command the person typed, seen by the host itself,
/// is recorded as the person's own act (`typed`). Anything a participant passes on is the participant's
/// event and says so; where the host kept the person's whole prompt and it is these exact words, the
/// record carries its digest, so a reader can see what the participant took for an instruction.
fn adopt(tx: &mut Tx, se: &Session, basis: &str, typed: Option<String>) -> Result<Put, String> {
    let basis = clip(basis, 4000);
    let who = se.principal_in(tx.state());
    if let Some(d) = typed {
        return tx.put(&who, "HOST", "ADOPTED", obj(vec![("by", s(&who)), ("basis", s(&basis)), ("observed", s("HOST_OBSERVED_PROMPT")), ("prompt_sha256", s(&d))]));
    }
    let last_halt = parse(tx.rec.ledger().bytes().to_vec()).ok().and_then(|o| o.events.iter().rev().find(|e| e.get("kind").and_then(Value::as_str) == Some("HALTED"))
        .and_then(|e| e.get("ts").and_then(Value::as_str).map(|x| x.to_string())));
    match observed_whole_prompt(se, &basis, last_halt.as_deref()) {
        Some(d) => tx.put(&se.actor, "HOST", "ADOPTED", obj(vec![("by", s(&who)), ("basis", s(&basis)), ("observed", s("RELAYED_WHOLE_PROMPT_OBSERVED")), ("prompt_sha256", s(&d))])),
        None => tx.put(&se.actor, "HOST", "ADOPTED", obj(vec![("by", s(&who)), ("basis", s(&basis)), ("observed", s("RELAYED_NOT_OBSERVED"))])),
    }
}

/// How the adoption now in force came about, as its event says.
fn adoption_basis(o: &Opened) -> Option<String> {
    o.events.iter().rev().find(|e| matches!(e.get("kind").and_then(Value::as_str), Some("ADOPTED" | "HALTED"))).filter(|e| e.get("kind").and_then(Value::as_str) == Some("ADOPTED"))
        .and_then(|e| e.get("body").and_then(|b| b.get("observed")).and_then(Value::as_str).map(|x| x.to_string()))
}

/// A session writes as its own participant, and that participant is not a person of this register.
fn session_may_write(tx: &Tx, se: &Session) -> Result<(), String> {
    match tx.state().participants.get(&se.actor) {
        Some(p) if p.kind == "HUMAN" || p.principal => Err(format!("this session's label, {}, is the label of a person in this register. A session does not write as a person: start it under its own label", se.actor)),
        _ => Ok(()),
    }
}

/// The most reviews that may be open at once through the tools. A halt names every review it closes in one
/// event, and an event has a size limit: past this number a halt could not be written.
const MAX_OPEN_REVIEWS: usize = 64;

// ---- the tool server ------------------------------------------------------------------------------------

/// Kinds of event a session's participant may not write through the tools, and why. They belong to a
/// person, to the system, or to this program's own observation.
fn not_from_a_session(kind: &str, body: &Value) -> Option<&'static str> {
    match kind {
        "ADOPTED" => Some("ACR use is adopted by the person directing the work. Use acr_adopt with their instruction in their own words; this tool does not write ADOPTED"),
        "STANDARD_SET" => Some("a standard is set by a person, acting as themselves, outside this session (pcpos standard)"),
        "SYSTEM_STATE" | "LINK_STATE" | "TASK_ENDED" | "TASK_STARTED" => Some("this event is written by the system from what it observes itself, never on a participant's word"),
        "WORKSPACE_CREATED" | "METHOD_PINNED" | "ENVIRONMENT_RECORDED" | "HOST_EVENT" => Some("this event is written by the host (acr_begin)"),
        "HALTED" => Some("a halt is recorded with acr_halt, which states who relayed it; this tool does not write HALTED"),
        k if k.starts_with("X_") => Some("an extension event is the host's, not a participant's"),
        "PARTICIPANT_ADDED" if matches!(body.get("label").and_then(Value::as_str), Some("host" | "principal" | "system")) => Some("that label is reserved"),
        "PARTICIPANT_ADDED" if body.get("principal") == Some(&Value::Bool(true)) || body.get("kind").and_then(Value::as_str) != Some("AUTOMATED") =>
            Some("a session may register an automated participant only; a person is registered by a person (pcpos init)"),
        _ => None,
    }
}

const NO_TOOL: &str = "no tool named ";

fn tools() -> Value {
    let schema = |props: Vec<(&str, &str, &str)>, required: Vec<&str>| {
        obj(vec![("type", s("object")), ("properties", Value::Map(props.into_iter().map(|(k, t, d)| (k.to_string(), obj(vec![("type", s(t)), ("description", s(d))]))).collect())),
                 ("required", Value::List(required.into_iter().map(s).collect()))])
    };
    let reg = ("register", "string", "Optional. Path of the register, relative to the served directory. Omit it to use the session's register.");
    let tool = |name: &str, d: &str, sc: Value| obj(vec![("name", s(name)), ("description", s(d)), ("inputSchema", sc)]);
    Value::List(vec![
        tool("acr_status", "What the record says now: system, ACR use, standard, what needs a response, reviews, tasks, and what a halt would stop. Read this instead of recalling state.", schema(vec![reg], vec![])),
        tool("acr_verify", "Check the register's hash chain and replay every guard. Reports the full head digest and any recorded event the guards would refuse.", schema(vec![reg], vec![])),
        tool("acr_begin", "Begin the session's register if there is none, and register this session's participant. Changes nothing if both are already done. It does not put ACR in use.", schema(vec![reg], vec![])),
        tool("acr_adopt", "Record that the person directing the work asked for ACR in their own words. basis is their whole last message, copied exactly and unchanged. The adoption is recorded as relayed by you. If the host kept that exact message, the record carries its digest; if not, the status asks the person to confirm. Only a command the person types is recorded as the person's own act.",
             schema(vec![reg, ("basis", "string", "The person's whole last message, unchanged")], vec!["basis"])),
        tool("acr_halt", "Stop ACR use now, on the person's instruction or for your own part. Closes open reviews as incomplete and asks their tasks to stop. Never refused for the state of the work.",
             schema(vec![reg, ("words", "string", "Optional. The instruction to stop, in its original words")], vec![])),
        tool("acr_offer", "Ask the guards what they would answer to an event, without writing it: ACCEPT with the exact body that would be written, or REFUSE with the clause.",
             schema(vec![reg, ("kind", "string", "Event kind, e.g. FINDING_RECORDED"), ("body", "object", "The event body")], vec!["kind", "body"])),
        tool("acr_submit", "Offer an event as this session's participant and, if the guards accept it, append it. The actor is fixed by the server; it is not an argument. With expected_head, the event is written only if the register's head is still that digest.",
             schema(vec![reg, ("kind", "string", "Event kind"), ("body", "object", "The event body"), ("expected_head", "string", "Optional. The full head digest the caller last saw")], vec!["kind", "body"])),
        tool("acr_modules", "The modules found in the method text at hand, each with its identifier, version and the digest of its Specification, and the modules this workspace's reviews use as the record has them. Use it to fill 'modules' when commissioning a review. It selects nothing.", schema(vec![reg], vec![])),
        tool("acr_clause", "The exact text of one provision of a module's Specification, read from the method text at hand: a numbered section (15.2), a numbered requirement (4.5) or an appendix (Appendix A). Without a section, the module's headings. Use it instead of recalling a rule. It does not put ACR in use.",
             schema(vec![("module", "string", "core, mathematics, science, engineering, cryptography, statistics, quality_assurance, or another module found by acr_modules"), ("section", "string", "Optional. The provision's number")], vec!["module"])),
        tool("acr_artifact", "Have the server compute the SHA-256 and size of material itself and record it as an artifact, so that the digest is the server's observation, not your report. Give path for a file under the served directory, which the server reads. Give text for words that exist only in the conversation, such as a claim the person typed: pass them exactly, the server keeps those bytes beside the register and records their digest; no file tool and no shell is needed. Give exactly one of path and text. origin is SUPPLIED (default) for material you were given or PRODUCED for something this work made. With record false, nothing is written and the digest is returned.",
             schema(vec![reg, ("path", "string", "A file, relative to the served directory"), ("text", "string", "Words given in the conversation, exactly as given, when there is no file"), ("origin", "string", "SUPPLIED or PRODUCED"), ("name", "string", "Optional. A label for the artifact"), ("record", "boolean", "Optional. false: compute and return the digest, write nothing")], vec![])),
        tool("acr_check", "Record a check on what the host itself saw. First run the command with the shell tool; then call this with exactly the same command text. The server finds the host's own observation of that command (its exit status and the digests of its output) and records the check as HOST_OBSERVED_TOOL. If the host saw no such command, nothing is recorded: a command you could not run is not a check, and the claim that needed it is UNRESOLVED (Core 5.1). Whether the command tests the claim's criterion remains your assessment.",
             schema(vec![reg, ("command", "string", "The command, exactly as it was given to the shell tool"), ("expect_exit", "integer", "Optional. The exit status that would mean the expectation was met; 0 if omitted"), ("name", "string", "Optional. A label for the check")], vec!["command"])),
        tool("acr_instrument", "Call with instrument \"list\" to see the checks this host can run and the input each takes. Each instrument computes one property inside the program, with no shell. With an open review the result can be recorded as a check this program ran and cited by a finding. State why the computed property is the criterion in that review.",
             schema(vec![reg, ("instrument", "string", "Use list for the available instrument names and their inputs, or choose a name from that list"), ("input", "object", "The instrument's input, for example {\"expression\": \"sum(k, 1, 100, k) = 5500\"}"), ("name", "string", "Optional. A label for the check"), ("record", "boolean", "Optional. false computes and writes nothing")], vec!["instrument"])),
        tool("acr_kernel", "The method's kernel text, whole: the operating floor and the findings table, copied without change from the Core Specification, with the SHA-256 of the file. Read it before the first step of a review, and again after your context is reset. This needs no file tool. It does not put ACR in use.", schema(vec![], vec![])),
        tool("acr_events", "The exact body of every event a review writes, in the order a review uses them; each was accepted by the guards when the package was built. Read it before your first acr_submit. This needs no file tool. It does not put ACR in use.", schema(vec![], vec![])),
        tool("acr_machine", "The whole table of one state machine (SYSTEM, USE, REVIEW, OBJECTION, OBLIGATION, TASK, DELIVERY, LINK): for each state and event, what follows or '-' if not accepted.",
             schema(vec![("name", "string", "Machine name")], vec!["name"])),
    ])
}

fn call(se: &Session, name: &str, args: &Value) -> Result<String, String> {
    let arg = |k: &str| args.get(k).and_then(Value::as_str).ok_or(format!("argument {k} is required and is text"));
    let text_arg = |k: &str| match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Str(x)) => Ok(Some(x.as_str())),
        Some(_) => Err(format!("argument {k} is text")),
    };
    if args.get("actor").is_some() {
        return Err(format!("the actor is not an argument. This server acts for one participant, {}, fixed when it started", se.actor));
    }
    let reg = text_arg("register")?.unwrap_or(&se.register).to_string();
    let none = || format!("there is no register at {reg}. ACR is not in use here and nothing is recorded. Begin one with acr_begin when the person directing the work asks for ACR");
    match name {
        "acr_machine" => machine_text(Some(arg("name")?)),
        "acr_kernel" => {
            // The same bytes a file tool would read, served by the program that sits beside them: a session whose
            // file tools cannot reach the packaged folder still holds the operating floor.
            let dir = se.method.as_deref().ok_or("no method text is at hand")?;
            let raw = fs::read(dir.join("KERNEL.md")).map_err(|e| format!("the kernel text cannot be read: {e}"))?;
            let text = String::from_utf8(raw.clone()).map_err(|_| "the kernel text is not UTF-8".to_string())?;
            Ok(format!("KERNEL.md | SHA-256 {} | {} bytes | this digest is the `text` of the DELIVERY_STATE event that records that you hold it\n\n{}", sha256_hex(&raw), raw.len(), clip_long(&text, 80_000)))
        }
        "acr_events" => {
            let dir = se.plugin.as_deref().ok_or("this program was not started from a packaged folder, so it has no list of event bodies; acr_offer answers what the guards would accept")?;
            let raw = fs::read(dir.join("skills").join("review").join("events.md")).map_err(|e| format!("the list of event bodies cannot be read: {e}. acr_offer answers what the guards would accept"))?;
            let text = String::from_utf8(raw.clone()).map_err(|_| "the list of event bodies is not UTF-8".to_string())?;
            Ok(format!("events.md | SHA-256 {} | {} bytes\n\n{}", sha256_hex(&raw), raw.len(), clip_long(&text, 80_000)))
        }
        "acr_modules" => Ok(modules_text(se, open(&se.root, &reg)?.as_ref())),
        "acr_clause" => clause_text(se, arg("module")?, text_arg("section")?),
        "acr_artifact" => {
            let mut origin = text_arg("origin")?.unwrap_or("SUPPLIED");
            let mut origin_note = "";
            if let Some(text) = text_arg("text")? {
                // Words that exist only in the conversation. The server did not read them from anywhere: it was handed them, and says so.
                if text_arg("path")?.is_some() {
                    return Err("give path or text, not both".to_string());
                }
                if !matches!(origin, "SUPPLIED" | "PRODUCED") {
                    return Err("origin is SUPPLIED or PRODUCED".to_string());
                }
                if text.is_empty() || text.len() > 1024 * 1024 {
                    return Err("text is between 1 byte and 1 MiB; record anything larger as a file".to_string());
                }
                let digest = sha256_hex(text.as_bytes());
                if !record_wanted(args)? {
                    return Ok(format!("sha256 {digest}\nsize {}\ncomputed by the server from the text it was handed. Nothing was written.\n", text.len()));
                }
                let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
                session_may_write(&tx, se)?;
                // Words handed over while a review is open are the session's own unless the host saw the person type them.
                // Calling them supplied does not make them supplied (Core 4.3).
                if origin == "SUPPLIED" && open_review_floor(&tx).is_some() && !in_an_observed_prompt(se, text) {
                    origin = "PRODUCED";
                    origin_note = "Recorded as PRODUCED, not SUPPLIED: a review is open and the host saw no prompt of the person's that holds these words, so they are this session's own. A finding cannot rest on them alone.\n";
                }
                let beside = Path::new(&reg).parent().map(|p| p.to_path_buf()).unwrap_or_default();
                let kept = beside.join("artifacts").join(&digest);
                let kept_name = kept.to_string_lossy().to_string();
                let mut body = vec![("sha256", s(&digest)), ("size", Value::Int(text.len() as i64)), ("origin", s(origin)), ("stored", Value::Bool(false)),
                                    ("read_by", s("HOST_FROM_TOOL_ARGUMENT")), ("kept_at", s(&kept_name))];
                if let Some(n) = text_arg("name")? {
                    body.push(("name", s(n)));
                }
                return match tx.put(&se.actor, "HOST", "ARTIFACT_ADDED", obj(body))? {
                    Put::Refused(r) => Ok(refusal_text(&r)),
                    Put::Written { seq, head, prev } => {
                        // Kept only once the event is accepted, and never through a link.
                        let dir = se.root.join(&beside).join("artifacts");
                        let safe = fs::create_dir_all(&dir).is_ok() && dir.canonicalize().map(|c| c.starts_with(&se.root)).unwrap_or(false)
                            && !fs::symlink_metadata(se.root.join(&kept)).map(|m| m.file_type().is_symlink()).unwrap_or(false);
                        if !safe || fs::write(se.root.join(&kept), text.as_bytes()).is_err() {
                            return Err("the text could not be kept beside the register; nothing was recorded".to_string());
                        }
                        tx.commit()?;
                        Ok(format!("{}sha256 {digest}\nsize {}\nThe server computed the digest from the text it was handed and kept those bytes at {kept_name}. It did not read them from a file.\n{origin_note}", receipt(seq, &head, &prev, "ARTIFACT_ADDED", &se.actor, "HOST"), text.len()))
                    }
                };
            }
            let path = arg("path").map_err(|_| "give path (a file under the served directory) or text (words given in the conversation)".to_string())?;
            if !matches!(origin, "SUPPLIED" | "PRODUCED") {
                return Err("origin is SUPPLIED or PRODUCED. A retrieved source is recorded with acr_submit, with where and when it was retrieved".to_string());
            }
            let mut h = confined_file(&se.root, path, false, false, false)?.ok_or(format!("there is no file at {path} under the served directory"))?;
            let meta = h.file.metadata().map_err(|e| e.to_string())?;
            let (size, changed) = (meta.len(), meta.modified().ok().map(utc_at));
            if size > MAX_ARTIFACT {
                return Err(format!("the file is {size} bytes; this tool reads at most {MAX_ARTIFACT}. Record a larger file with acr_submit and a digest you computed"));
            }
            let bytes = read_all(&mut h)?;
            let digest = sha256_hex(&bytes);
            if !record_wanted(args)? {
                return Ok(format!("sha256 {digest}\nsize {}\nread by the server from {path}. Nothing was written.\n", bytes.len()));
            }
            let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
            session_may_write(&tx, se)?;
            // A file is material the review was given only if its last write is shown to be earlier than the commissioning.
            // Both times are known to the second. A write in the commissioning second itself may have come after it, so it
            // is not shown to be earlier; and a file whose time cannot be read is shown to be nothing.
            if let (true, Some((_, since))) = (origin == "SUPPLIED", open_review_floor(&tx)) {
                match changed.as_ref() {
                    Some(at) if *at < since => {}
                    Some(at) if *at == since => {
                        origin = "PRODUCED";
                        origin_note = "Recorded as PRODUCED, not SUPPLIED: the file was last written in the same second as the open review was commissioned, and times here are known only to the second, so it is not shown to be material the review was given. A finding cannot rest on it alone.\n";
                    }
                    Some(_) => {
                        origin = "PRODUCED";
                        origin_note = "Recorded as PRODUCED, not SUPPLIED: the file was last written after the open review was commissioned, so it is not material the review was given. A finding cannot rest on it alone.\n";
                    }
                    None => {
                        origin = "PRODUCED";
                        origin_note = "Recorded as PRODUCED, not SUPPLIED: the time of the file's last write could not be read, so it is not shown to be material the review was given. A finding cannot rest on it alone.\n";
                    }
                }
            }
            let mut body = vec![("sha256", s(&digest)), ("size", Value::Int(bytes.len() as i64)), ("origin", s(origin)), ("stored", Value::Bool(false)),
                                ("filename", s(&clip(Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(""), 200))), ("read_by", s("HOST"))];
            if let Some(n) = text_arg("name")? {
                body.push(("name", s(n)));
            }
            match tx.put(&se.actor, "HOST", "ARTIFACT_ADDED", obj(body))? {
                Put::Refused(r) => Ok(refusal_text(&r)),
                Put::Written { seq, head, prev } => {
                    tx.commit()?;
                    Ok(format!("{}sha256 {digest}\nsize {}\nThe server read the file and computed the digest itself.\n{origin_note}", receipt(seq, &head, &prev, "ARTIFACT_ADDED", &se.actor, "HOST"), bytes.len()))
                }
            }
        }
        "acr_instrument" => {
            let which = arg("instrument")?;
            if which == "list" {
                let mut out = String::from("Instruments this program runs itself. Each computes one property and nothing else (Core 14.1):\n");
                for (name, what) in instruments::LIST {
                    let _ = writeln!(out, "  {name}: {what}");
                }
                return Ok(out);
            }
            let input = args.get("input").cloned().unwrap_or(Value::Map(BTreeMap::new()));
            if !matches!(input, Value::Map(_)) {
                return Err("argument input is an object".to_string());
            }
            let opened = open(&se.root, &reg)?;
            let found = instruments::run(which, &input, opened.as_ref().map(|o| o.events.as_slice()).unwrap_or(&[]))?;
            let text = format!("{which}\n{}\n", found.text);
            if !record_wanted(args)? || opened.is_none() {
                return Ok(format!("{text}Nothing was written{}.\n", if opened.is_none() { ": there is no register here" } else { "" }));
            }
            let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
            session_may_write(&tx, se)?;
            if open_review_floor(&tx).is_none() {
                return Ok(format!("{text}No review is open, so this was not recorded as a check: a check is run for a review.\nNothing was written.\n"));
            }
            let canonical = String::from_utf8_lossy(&input.canonical_json()).to_string();
            let mut body = vec![("argv", Value::List(vec![s("instrument"), s(which), s(&clip(&canonical, 4000))])), ("via", s("INSTRUMENT")), ("exit", Value::Int(if found.holds { 0 } else { 1 })), ("expect_exit", Value::Int(0)),
                                ("timeout", Value::Bool(false)), ("stdout", s(&sha256_hex(found.text.as_bytes()))), ("stderr", s(&sha256_hex(b""))), ("output", s("INSTRUMENT_TEXT")), ("observed", s("RAN_BY_THIS_PROGRAM")),
                                ("instrument", s(which)), ("result", s(&clip(&found.text, 1500)))];
            if let Some(n) = text_arg("name")? {
                body.push(("name", s(n)));
            }
            match tx.put(&se.actor, "HOST", "CHECK_RAN", obj(body))? {
                Put::Refused(r) => Ok(refusal_text(&r)),
                Put::Written { seq, head, prev } => {
                    tx.commit()?;
                    let id = tx.state().last_check().unwrap_or_default();
                    Ok(format!("{}check {id}: this program ran the instrument itself; the property it computes {}.\n{text}Cite it in a finding as {{\"ref\":{{\"t\":\"check\",\"id\":\"{id}\"}}}}. The instrument computed exactly what is shown above. Whether that is the criterion in question is your assessment, and you state it (Core 5.1, 14.1).\n",
                               receipt(seq, &head, &prev, "CHECK_RAN", &se.actor, "HOST"), if found.holds { "HOLDS" } else { "DOES NOT HOLD" }))
                }
            }
        }
        "acr_check" => {
            let command = arg("command")?;
            let expect = match args.get("expect_exit") {
                None | Some(Value::Null) => 0,
                Some(Value::Int(n)) => *n,
                Some(_) => return Err("argument expect_exit is a whole number".to_string()),
            };
            let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
            session_may_write(&tx, se)?;
            let used: Vec<String> = parse(tx.rec.ledger().bytes().to_vec())?.events.iter().filter(|e| e.get("kind").and_then(Value::as_str) == Some("CHECK_RAN"))
                .filter_map(|e| e.get("body").and_then(|b| b.get("tool_use_id")).and_then(Value::as_str).map(|x| x.to_string())).collect();
            let floor = match open_review_floor(&tx) {
                Some((seq, _)) => seq,
                None => return Ok(format!("REFUSE (exit code {}) [Core 5.1]\nNo review is open. A check is run for a review: commission the review, run the command, then call acr_check.\nNothing was written.\n", acr::REFUSED)),
            };
            let all = observed_unrecorded(se, &used);
            // What the host saw before the open review was commissioned was not run for it, and is not offered.
            let stale = all.iter().any(|v| v.get("command").and_then(Value::as_str) == Some(command) && v.get("at_event").and_then(Value::as_int).map(|n| n <= floor).unwrap_or(true));
            let unrecorded: Vec<Value> = all.into_iter().filter(|v| v.get("at_event").and_then(Value::as_int).map(|n| n > floor).unwrap_or(false)).collect();
            let seen = match unrecorded.iter().rev().find(|v| v.get("command").and_then(Value::as_str) == Some(command)) {
                Some(v) => v.clone(),
                None => {
                    // The exact text matters, and a command is easily passed on with something added. What the host did see is the session's own commands: it is shown them.
                    let mut did = String::new();
                    for v in unrecorded.iter().rev().take(5) {
                        let _ = writeln!(did, "  {}", clip(v.get("command").and_then(Value::as_str).unwrap_or(""), 400));
                    }
                    let did = if did.is_empty() { "The host has seen no shell command here that is not already recorded as a check.\n".to_string() }
                              else { format!("Shell commands the host did see and that are not yet recorded as checks, newest first, each exactly as it was given:\n{did}If one of these is the check you mean, call acr_check with that text.\n") };
                    return Ok(format!("REFUSE (exit code {}) [Core 5.1]\nThe host has seen no shell command with exactly this text that is not already recorded as a check. Run it with the shell tool while ACR is in use, then call acr_check with the same text. If you cannot run it, no check was executed: do not record one, and give the claim that needed it the disposition its evidence warrants, which without the check is UNRESOLVED.\n{}{did}Nothing was written.\n", acr::REFUSED, if stale { "The host did see this command, but before the review now open was commissioned: that run was not a check for this review. Run it again.\n" } else { "" }));
                }
            };
            let take = |k: &str| seen.get(k).cloned().unwrap_or(Value::Null);
            let mut body = vec![("argv", Value::List(vec![s(command)])), ("via", s("HOST_SHELL_TOOL")), ("exit", take("exit")), ("expect_exit", Value::Int(expect)), ("timeout", take("timeout")),
                                ("stdout", take("stdout")), ("stderr", take("stderr")), ("output", take("output")), ("observed", s("HOST_OBSERVED_TOOL")), ("observed_at", take("ts")),
                                ("tool_use_id", take("tool_use_id")), ("duration_ms", take("duration_ms"))];
            if let Some(n) = text_arg("name")? {
                body.push(("name", s(n)));
            }
            match tx.put(&se.actor, "HOST", "CHECK_RAN", obj(body))? {
                Put::Refused(r) => Ok(refusal_text(&r)),
                Put::Written { seq, head, prev } => {
                    tx.commit()?;
                    let id = tx.state().last_check().unwrap_or_default();
                    let exit = seen.get("exit").and_then(Value::as_int);
                    let met = exit == Some(expect) && seen.get("timeout") == Some(&Value::Bool(false));
                    Ok(format!("{}check {id}: the host saw this command run; exit {}, expected {expect}: the expectation was {}. stdout {}\nThe digests are of the output as the host reported it to this program{}. Cite it in a finding as {{\"ref\":{{\"t\":\"check\",\"id\":\"{id}\"}}}}. That the command ran says nothing about whether it tests the criterion: that is your assessment, and you state it (Core 5.1).\n",
                               receipt(seq, &head, &prev, "CHECK_RAN", &se.actor, "HOST"), exit.map(|e| e.to_string()).unwrap_or_else(|| "not reported".to_string()), if met { "met" } else { "NOT met" },
                               seen.get("stdout").and_then(Value::as_str).unwrap_or(""),
                               if seen.get("output").and_then(Value::as_str) == Some("ONE_TEXT_ON_FAILURE") { " (for a failed command the host reports one text, so stdout here is that whole text after its first line)" } else { "" }))
                }
            }
        }
        "acr_status" => Ok(status(&reg, &open(&se.root, &reg)?.ok_or_else(none)?)),
        "acr_verify" => Ok(verify_text(&open(&se.root, &reg)?.ok_or_else(none)?, 20)),
        "acr_begin" => {
            let mut tx = Tx::begin(&se.root, &reg, true)?.ok_or("the register could not be created")?;
            session_may_write(&tx, se)?;
            let did = begin(&mut tx, se, None, true)?;
            tx.commit()?;
            Ok(format!("{did}\nhead {}\nACR is {}.\n", tx.head(), tx.state().in_use().name()))
        }
        "acr_adopt" => {
            let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
            session_may_write(&tx, se)?;
            if tx.state().in_use() == Use::InUse {
                return Ok("ACR is already in use in this register; nothing was written. Read acr_status.\n".to_string());
            }
            match adopt(&mut tx, se, arg("basis")?, None)? {
                Put::Refused(r) => Ok(refusal_text(&r)),
                Put::Written { seq, head, prev } => {
                    tx.commit()?;
                    let (by, actor) = tx.state().adoption().map(|(b, a)| (b.to_string(), a.to_string())).unwrap_or_default();
                    Ok(format!("{}{}\n", receipt(seq, &head, &prev, "ADOPTED", &actor, "HOST"),
                               if by == actor { "Recorded as the person's own act." }
                               else if adoption_basis(&parse(tx.rec.ledger().bytes().to_vec())?).as_deref() == Some("RELAYED_WHOLE_PROMPT_OBSERVED") { "Recorded as relayed by you. The host kept the person's last prompt and it is exactly these words; the record carries its digest. Only a command the person types is recorded as their own act." }
                               else { "Recorded as relayed by you: these are not exactly the person's last prompt as the host kept it. The status lists it for the person to confirm. Pass their whole message, unchanged, or have them type the command." }))
                }
            }
        }
        "acr_halt" => {
            let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
            // The person's words are kept under either name: a model that has just used acr_adopt tends to say `basis` here too.
            let words = text_arg("words")?.or(text_arg("basis")?).map(|w| s(&clip(w, 2000))).unwrap_or(Value::Null);
            // A participant registered here halts as itself, naming the person whose instruction it carries; one that is not registered cannot write, and says so.
            // A halt is never lost to a label: where the configured person is not in this register, the host halts in its own name.
            let who = Some(se.principal_in(tx.state())).filter(|w| tx.state().participants.contains_key(w)).unwrap_or_else(|| "host".to_string());
            let session = tx.state().participants.get(&se.actor).map(|p| p.kind != "HUMAN" && !p.principal).unwrap_or(false);
            let actor = if session { se.actor.clone() } else { "host".to_string() };
            let who = if actor == "host" { "host".to_string() } else { who };
            match tx.put(&actor, "HOST", "HALTED", obj(vec![("by", s(&who)), ("words", words), ("relayed_by", s(&se.actor))]))? {
                Put::Refused(r) => Ok(refusal_text(&r)),
                Put::Written { seq, head, prev } => {
                    tx.commit()?;
                    let asked: Vec<String> = tx.state().tasks.iter().filter(|(_, t)| t.outcome.is_none() && t.stop_requested.is_some()).map(|(id, t)| format!("{id} {}", t.label)).collect();
                    Ok(format!("{}ACR use has stopped. Tasks asked to stop, termination UNKNOWN until each end is recorded: [{}].\n", receipt(seq, &head, &prev, "HALTED", &actor, "HOST"), asked.join(", ")))
                }
            }
        }
        "acr_offer" | "acr_submit" => {
            let kind = arg("kind")?;
            let body = args.get("body").ok_or("argument body is required")?;
            // A check offered here is the participant's own account of a command: the host did not see it run, and the record says so.
            // Only acr_check, which reads the host's own observation, records a check as observed.
            let reported;
            let body = match (kind, body) {
                ("CHECK_RAN", Value::Map(m)) => {
                    let mut m = m.clone();
                    m.insert("observed".to_string(), s("REPORTED_BY_PARTICIPANT"));
                    reported = Value::Map(m);
                    &reported
                }
                _ => body,
            };
            if let Some(why) = not_from_a_session(kind, body) {
                return Ok(format!("REFUSE (exit code {})\n{}\nNothing was written.\n", acr::REFUSED, why));
            }
            let mut tx = Tx::begin(&se.root, &reg, false)?.ok_or_else(none)?;
            session_may_write(&tx, se)?;
            if name == "acr_submit" && kind == "ARTIFACT_ADDED" && body.get("origin").and_then(Value::as_str) == Some("SUPPLIED") && open_review_floor(&tx).is_some() {
                return Ok(format!("REFUSE (exit code {}) [Core 4.3]\nA review is open, and material recorded now as SUPPLIED on a digest alone is your report that it was supplied: the server did not read it. Use acr_artifact with its path or its text, so that the server reads it and says how it came by it; record a source you retrieved as RETRIEVED, with where and when; record your own working as PRODUCED.\nNothing was written.\n", acr::REFUSED));
            }
            if kind == "REVIEW_COMMISSIONED" {
                if let Err(why) = commission_modules(se, body) {
                    return Ok(format!("REFUSE (exit code {}) [Core 14.1]\n{why}\nNothing was written.\n", acr::REFUSED));
                }
            }
            if kind == "REVIEW_COMMISSIONED" && tx.state().reviews.values().filter(|r| matches!(r.lifecycle, Review::Active | Review::Paused)).count() >= MAX_OPEN_REVIEWS {
                return Ok(format!("REFUSE (exit code {})\n{MAX_OPEN_REVIEWS} reviews are open. Close one first: a halt names every review it closes in one event, and must always fit.\nNothing was written.\n", acr::REFUSED));
            }
            if name == "acr_offer" {
                return match acr::guard(tx.state(), kind, body, &se.actor) {
                    Ok(out) => Ok(format!("ACCEPT\nbody that would be written: {}\n", String::from_utf8_lossy(&out.canonical_json()))),
                    Err(r) => Ok(format!("REFUSE (exit code {}){}\n{}\n", r.code, r.clause.map(|c| format!(" [{c}]")).unwrap_or_default(), r.message)),
                };
            }
            if let Some(want) = text_arg("expected_head")? {
                if want != tx.head() {
                    return Ok(format!("REFUSE (exit code {})\nthe register's head is {}, not the expected {}: something was written since you last read it. Read acr_status, then decide again.\nNothing was written.\n",
                                      acr::REFUSED, tx.head(), want));
                }
            }
            match tx.put(&se.actor, "HOST", kind, body.clone())? {
                // A session that names itself by anything but its label is told its label: a model tends to give its own name.
                Put::Refused(r) if kind == "DELIVERY_STATE" && body.get("agent").and_then(Value::as_str) != Some(se.actor.as_str()) =>
                    Ok(format!("{}This session's participant is `{}`. Give that label as agent.\n", refusal_text(&r), se.actor)),
                Put::Refused(r) => Ok(refusal_text(&r)),
                Put::Written { seq, head, prev } => {
                    tx.commit()?;
                    Ok(receipt(seq, &head, &prev, kind, &se.actor, "HOST"))
                }
            }
        }
        _ => Err(format!("{NO_TOOL}{name}")),
    }
}

fn serve(se: &Session) {
    const MAX_LINE: usize = 8 * 1024 * 1024;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut out = std::io::stdout();
    let empty = Value::Map(BTreeMap::new());
    let error = |id: Value, code: i64, m: &str| obj(vec![("jsonrpc", s("2.0")), ("id", id), ("error", obj(vec![("code", Value::Int(code)), ("message", s(m))]))]);
    let mut line: Vec<u8> = Vec::new();
    loop {
        line.clear();
        // Bytes, not text: a line that is not UTF-8 is answered as an error and the server goes on.
        match (&mut input).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.len() > MAX_LINE {
            // Too long to be a message of this server: drop the rest of the line and say so.
            let mut rest = Vec::new();
            while !line.ends_with(b"\n") {
                rest.clear();
                match (&mut input).take(MAX_LINE as u64).read_until(b'\n', &mut rest) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => line = std::mem::take(&mut rest),
                }
            }
            let mut bytes = error(Value::Null, -32600, "the message is longer than this server accepts").canonical_json();
            bytes.push(b'\n');
            if out.write_all(&bytes).and_then(|_| out.flush()).is_err() {
                break;
            }
            continue;
        }
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let reply = match Value::parse(&line) {
            // Not in the record's strict form: say so to whoever asked, if the message can be read at all.
            Err(reason) => {
                let id = lenient::parse(&line).and_then(|m| m.get("id").cloned());
                Some(match (reason, id) {
                    (ParseError::TooDeep(_), id) => {
                        let code = if id.is_some() { -32600 } else { -32700 };
                        error(id.unwrap_or(Value::Null), code, &format!("the message exceeds the maximum nesting depth of {MAX_DEPTH}"))
                    }
                    (_, Some(id)) => error(id, -32600, "the message holds a fraction, an exponent or a repeated key; this server accepts whole numbers and distinct keys only"),
                    (_, None) => error(Value::Null, -32700, "the message is not JSON this server accepts"),
                })
            }
            Ok(Value::Map(_)) => None,
            Ok(_) => Some(error(Value::Null, -32600, "one message is one JSON object; batches are not accepted")),
        };
        let reply = match (reply, Value::parse(&line)) {
            (Some(r), _) => Some(r),
            (None, Ok(msg)) => {
                let id = msg.get("id").cloned();
                let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
                let params = msg.get("params").unwrap_or(&empty);
                let result: Option<Result<Value, (i64, String)>> = match method {
                    _ if id.is_none() => None, // a notification: nothing is done on it and nothing is answered
                    "initialize" => {
                        let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
                        let version = if PROTOCOL_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSIONS[PROTOCOL_VERSIONS.len() - 1] };
                        Some(Ok(obj(vec![("protocolVersion", s(version)), ("capabilities", obj(vec![("tools", empty.clone())])),
                                         ("serverInfo", obj(vec![("name", s("pcpos")), ("title", s("PCP OS")), ("version", s(env!("CARGO_PKG_VERSION")))]))])))
                    }
                    "ping" => Some(Ok(empty.clone())),
                    "tools/list" => Some(Ok(obj(vec![("tools", tools())]))),
                    "tools/call" => {
                        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                        let args = params.get("arguments").unwrap_or(&empty);
                        match call(se, name, args) {
                            Err(e) if e.starts_with(NO_TOOL) => Some(Err((-32602, e))),
                            r => {
                                let (text, is_error) = match r {
                                    Ok(t) => (t, false),
                                    Err(e) => (e, true),
                                };
                                Some(Ok(obj(vec![("content", Value::List(vec![obj(vec![("type", s("text")), ("text", s(&text))])])), ("isError", Value::Bool(is_error))])))
                            }
                        }
                    }
                    _ => Some(Err((-32601, format!("method {} is not provided", clip(method, 80))))),
                };
                match (id, result) {
                    (Some(id), Some(Ok(r))) => Some(obj(vec![("jsonrpc", s("2.0")), ("id", id), ("result", r)])),
                    (Some(id), Some(Err((code, m)))) => Some(error(id, code, &m)),
                    _ => None,
                }
            }
            (None, Err(_)) => None,
        };
        if let Some(r) = reply {
            let mut bytes = r.canonical_json();
            bytes.push(b'\n');
            if out.write_all(&bytes).and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
}

/// The protocol revisions this server speaks, oldest first. An unknown one is answered with the newest.
const PROTOCOL_VERSIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

// ---- hooks: what the host itself observed ------------------------------------------------------------------

/// The plugin command a typed prompt invoked, if any: ("pcp-os", "review", arguments).
fn typed_command(v: &Value) -> Option<(String, String, String)> {
    let (name, args) = match v.get("command_name").and_then(Value::as_str) {
        Some(n) => (n.to_string(), v.get("command_args").and_then(Value::as_str).unwrap_or("").to_string()),
        None => {
            let p = v.get("prompt").and_then(Value::as_str)?.trim_start();
            let first = p.split_whitespace().next()?;
            (first.to_string(), p[first.len()..].trim().to_string())
        }
    };
    let name = name.strip_prefix('/').unwrap_or(&name).to_string();
    let (plugin, command) = name.split_once(':')?;
    if plugin != "pcp-os" {
        return None;
    }
    Some((plugin.to_string(), command.to_string(), args))
}

fn short_status(o: &Opened) -> String {
    let st = &o.state;
    let mut t = format!("ACR record: use {}; {} events; head {}.", st.in_use().name(), o.events.len(), &o.head[..16]);
    if st.in_use() == Use::InUse {
        t.push_str(" ACR is in use here: if the person says to stop, pause or drop it, in any words, call acr_halt at once (Core 15.2).");
    }
    for r in st.reviews.values().filter(|r| matches!(r.lifecycle, Review::Active | Review::Paused)) {
        let _ = write!(t, " Review {} {} {} (session {}, gates {}/{}, {} claim(s) without a finding, {} open objection(s)).", r.label, r.procedure, r.lifecycle.name(), r.session,
                       r.passed_gates.len(), r.required_gates.len(), st.claims_without_finding(r).len(), st.open_objections(&r.label).len());
    }
    for (p, what, response) in attention(o) {
        let _ = write!(t, " {p}: {what} -> {response}.");
    }
    t
}

/// Bind the hook identity before clipping its presentation label. Canonical JSON
/// keeps component boundaries and a missing session distinct.
fn hook_attempt_key(input: &Value, agent: &str) -> String {
    let session_id = input.get("session_id").and_then(Value::as_str).map(s).unwrap_or(Value::Null);
    sha256_hex(&Value::List(vec![session_id, s(agent)]).canonical_json())
}

fn active_hook_identity(t: &acr::TaskState, key: &str, legacy_id: Option<&str>) -> bool {
    match t.hook_attempt.as_ref() {
        Some(acr::HookAttempt::Key(stored)) => stored == key,
        Some(acr::HookAttempt::Legacy) => legacy_id.is_some() && t.attempt.as_deref() == legacy_id,
        None => false,
    }
}

/// Run one hook. Returns what to print on standard output (which the host adds to the model's context
/// for the events that allow it). Nothing here stops the host's work: a failure is reported on standard
/// error and the hook still ends normally, except the one hook whose purpose is to refuse a write.
fn hook(event: &str, input: &Value) -> Result<String, String> {
    let se = session(input.get("cwd").and_then(Value::as_str))?;
    let reg = se.register.clone();
    let field = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    match event {
        // The host starts, resumes, clears or compacts a session.
        "session-start" => {
            let mut tx = match Tx::begin(&se.root, &reg, false)? {
                Some(tx) => tx,
                None => return Ok(String::new()), // no register: ACR is not in use here, and its presence changes nothing
            };
            let mut se = se;
            if se.model.is_none() && !field("model").is_empty() {
                se.model = Some(field("model").to_string());
            }
            let live = tx.state().in_use() == Use::InUse || tx.state().reviews.values().any(|r| matches!(r.lifecycle, Review::Active | Review::Paused));
            let mut note = String::new();
            if live && tx.problems.is_empty() {
                begin(&mut tx, &se, None, true)?;
                if matches!(field("source"), "compact" | "clear") && tx.state().delivery.get(&se.actor) == Some(&Delivery::Delivered) {
                    if let Put::Written { seq, .. } = tx.put("host", "HOST", "DELIVERY_STATE", obj(vec![("agent", s(&se.actor)), ("event", s("CONTEXT_RESET")), ("observed", s(&format!("SESSION_START_{}", field("source").to_ascii_uppercase())))]))? {
                        note = format!(" The host recorded at event {seq} that your context was reset: you no longer hold the method text. Call acr_kernel to read the kernel again, and acr_status, before any step of a review.");
                    }
                }
                tx.commit()?;
            }
            if !live {
                return Ok(String::new());
            }
            let o = parse(tx.rec.ledger().bytes().to_vec())?;
            Ok(format!("{}{}\n", short_status(&o), note))
        }
        // The person submits a prompt, or a command they typed expands into one.
        "prompt" => {
            if !matches!(field("source"), "" | "user") {
                return Ok(String::new()); // injected by a program, not typed by the person
            }
            let command = typed_command(input);
            let text = match &command {
                Some((p, c, a)) => format!("/{p}:{c} {a}").trim().to_string(),
                None => field("prompt").to_string(),
            };
            // A prompt that is nothing but an instruction to stop (Core 15.2 names the words) is a halt request
            // the host has observed itself. It is looked at only when the prompt is one of those few words, so
            // an ordinary prompt costs nothing here.
            let plain: String = text.trim().trim_end_matches(['.', '!']).to_ascii_lowercase();
            const STOP_WORDS: [&str; 16] = ["stop", "halt", "pause", "stop acr", "halt acr", "pause acr", "drop acr", "end acr", "stop pcp", "halt pcp", "pause pcp",
                                            "stop the review", "end the review", "pause the review", "forget the protocol", "-acr halt"];
            let bare_stop = command.is_none() && STOP_WORDS.contains(&plain.as_str());
            if bare_stop {
                // The word is a halt only where there is something to halt. Otherwise it is the person's own word and nothing is kept.
                let live = matches!(Tx::begin(&se.root, &reg, false), Ok(Some(tx)) if tx.state().in_use() == Use::InUse || tx.state().reviews.values().any(|r| matches!(r.lifecycle, Review::Active | Review::Paused)));
                if !live {
                    return Ok(String::new());
                }
            }
            let command = if bare_stop { Some(("pcp-os".to_string(), "halt".to_string(), String::new())) } else { command };
            let stop_note = if bare_stop { " The person typed only that. If the review was the whole of the task, both have stopped and there is nothing to ask. Only if other work was under way that they could have meant, ask one short question about which, after stopping ACR (Core 15.2)." } else { "" };
            // Whole words only: "ACRYLIC" is not about ACR. Nothing else the person types is kept.
            let lower = text.to_ascii_lowercase();
            let word = |w: &str| text.split(|c: char| !c.is_ascii_alphanumeric()).any(|x| x == w);
            let about_acr = command.is_some() || word("ACR") || word("PCP") || lower.contains("adversarial claim review") || lower.contains("parrot council");
            if !about_acr || text.trim().is_empty() {
                return Ok(String::new()); // nothing else the person types is kept
            }
            let digest = sha256_hex(text.as_bytes());
            if let Some(mut h) = confined(&se.root, &se.observed(), true, true)? {
                lock(&h, true)?;
                let mut line = obj(vec![("ts", s(&now_utc())), ("sha256", s(&digest)), ("text", s(&clip(&text, 4000))), ("event", s(field("hook_event_name")))]).canonical_json();
                line.push(b'\n');
                h.file.write_all(&line).and_then(|_| h.file.sync_all()).map_err(|e| e.to_string())?;
            }
            let mut said = String::new();
            match command.as_ref().map(|(_, c, _)| c.as_str()) {
                Some("halt") => match Tx::begin(&se.root, &reg, false) {
                    Err(e) => said = format!("The host could NOT record the halt the person typed: {e}. Stop every ACR-structured act now, tell the person the halt is not in the record, and call acr_halt."),
                    Ok(None) => {}
                    Ok(Some(mut tx)) => {
                        let live = tx.state().in_use() == Use::InUse || tx.state().reviews.values().any(|r| matches!(r.lifecycle, Review::Active | Review::Paused));
                        if live {
                            let who = Some(se.principal_in(tx.state())).filter(|w| tx.state().participants.contains_key(w)).unwrap_or_else(|| "host".to_string());
                            said = match tx.put(&who, "HOST", "HALTED", obj(vec![("by", s(&who)), ("words", s(&clip(&text, 2000))), ("observed", s("HOST_OBSERVED_PROMPT")), ("prompt_sha256", s(&digest))])).and_then(|p| match p {
                                Put::Written { seq, .. } => tx.commit().map(|_| Ok(seq)),
                                Put::Refused(r) => Ok(Err(r.message)),
                            }) {
                                Ok(Ok(seq)) => format!("The host recorded the halt at event {seq} from what the person typed. ACR use has stopped; the guards now refuse every review step.{stop_note}"),
                                Ok(Err(m)) | Err(m) => format!("The host could NOT record the halt the person typed: {m}. Stop every ACR-structured act now, tell the person the halt is not in the record, and call acr_halt."),
                            };
                        }
                    }
                },
                Some("workspace") => {
                    let args = command.as_ref().map(|(_, _, a)| a.clone()).unwrap_or_default();
                    let names: Vec<&str> = args.split(|c: char| c.is_whitespace() || c == ',' || c == '+').filter(|n| !n.is_empty()).collect();
                    if !names.is_empty() {
                        let mut tx = Tx::begin(&se.root, &reg, true)?.ok_or("the register could not be created")?;
                        // The host reports one typed command through more than one event. The same prompt is recorded once.
                        let before = parse(tx.rec.ledger().bytes().to_vec()).ok().and_then(|o| profile_of(&o.events));
                        if let Some(p) = before.as_ref().filter(|p| p.prompt.as_deref() == Some(digest.as_str())) {
                            said = format!("The host recorded at event {}, from the command the person typed, the modules this workspace's reviews use: {}. Do not record it again. This does not put ACR in use.", p.seq, profile_words(&before));
                        } else if tx.problems.is_empty() {
                            begin(&mut tx, &se, None, true)?;
                            let who = se.principal_in(tx.state());
                            said = match set_profile(&mut tx, &se, &names, &who, "HOST_OBSERVED_PROMPT", &text, Some(&digest)) {
                                Ok(Put::Written { seq, .. }) => {
                                    tx.commit()?;
                                    let o = parse(tx.rec.ledger().bytes().to_vec())?;
                                    format!("The host recorded at event {seq}, from the command the person typed, the modules this workspace's reviews use: {}. Do not record it again. This does not put ACR in use.", profile_words(&profile_of(&o.events)))
                                }
                                Ok(Put::Refused(r)) => {
                                    tx.commit()?;
                                    format!("The host could not record the workspace's modules: {}", r.message)
                                }
                                Err(e) => {
                                    tx.commit()?;
                                    format!("The host recorded no change to the workspace's modules: {e}")
                                }
                            };
                        }
                    }
                }
                Some("review") | Some("adopt") => {
                    let mut tx = Tx::begin(&se.root, &reg, true)?.ok_or("the register could not be created")?;
                    if tx.problems.is_empty() && tx.state().in_use() == Use::NotInUse {
                        begin(&mut tx, &se, None, true)?;
                        match adopt(&mut tx, &se, &text, Some(digest))? {
                            Put::Written { seq, .. } => {
                                tx.commit()?;
                                said = format!("The host recorded at event {seq} that the person adopted ACR use, from the command they typed. Do not record the adoption again.");
                            }
                            Put::Refused(r) => {
                                tx.commit()?;
                                said = format!("The host could not record the adoption: {}", r.message);
                            }
                        }
                    }
                }
                _ => {}
            }
            Ok(if said.is_empty() { said } else { format!("{said}\n") })
        }
        // A subagent starts: one task, one attempt, owned by this session.
        "subagent-start" => {
            let mut tx = match Tx::begin(&se.root, &reg, false)? {
                Some(tx) if tx.state().in_use() == Use::InUse && tx.problems.is_empty() => tx,
                _ => return Ok(String::new()),
            };
            let agent = field("agent_id");
            if agent.is_empty() {
                return Ok(String::new()); // no attempt to match an end to: recording it would leave a task that can never be seen to end
            }
            let key = hook_attempt_key(input, agent);
            let clipped = clip(agent, 200);
            if tx.state().tasks.values().any(|t| t.outcome.is_none() &&
                active_hook_identity(t, &key, Some(&clipped))) {
                return Ok(String::new());
            }
            begin(&mut tx, &se, None, true)?;
            let active: Vec<String> = tx.state().reviews.values().filter(|r| r.lifecycle == Review::Active).map(|r| r.label.clone()).collect();
            let review = if active.len() == 1 { s(&active[0]) } else { Value::Null };
            let body = obj(vec![("label", s(&to_label(field("agent_type"), "subagent"))), ("worker", s("MODEL")), ("review", review), ("quiet_limit", Value::Int(0)), ("budget", Value::Int(0)),
                                ("attempt", s(&clipped)), ("hook_attempt_key", s(&key)), ("observed", s("SUBAGENT_START"))]);
            if let Put::Refused(r) = tx.put(&se.actor, "HOST", "TASK_STARTED", body)? {
                eprintln!("pcpos hook: the task was not recorded: {}", r.message);
            }
            tx.commit()?;
            Ok(String::new())
        }
        // A subagent stops: the exit is observed, and its final message is read back by the host.
        "subagent-stop" => {
            let mut tx = match Tx::begin(&se.root, &reg, false)? {
                Some(tx) if tx.problems.is_empty() => tx,
                _ => return Ok(String::new()),
            };
            let agent = field("agent_id");
            if agent.is_empty() { return Ok(String::new()); }
            let key = hook_attempt_key(input, agent);
            // Earlier versions may have clipped or trimmed this identifier.
            // Do not assign an exit to a legacy row from a lossy match.
            let legacy_id = (agent.chars().count() < 200 && clip(agent, 200) == agent).then_some(agent);
            let mut candidates = tx.state().tasks.iter().filter(|(_, t)| t.outcome.is_none() &&
                active_hook_identity(t, &key, legacy_id))
                .map(|(id, t)| (id.clone(), t.stop_requested.is_some()));
            let (id, stop_asked) = match candidates.next() {
                Some(x) => x,
                None => return Ok(String::new()),
            };
            if candidates.next().is_some() {
                eprintln!("pcpos hook: several live tasks match this stop; no exit was assigned");
                return Ok(String::new());
            }
            let (bytes, source): (Option<Vec<u8>>, &str) = match input.get("last_assistant_message").and_then(Value::as_str) {
                Some(m) if !m.is_empty() => (Some(m.as_bytes().to_vec()), "the final message"),
                _ => match fs::read(field("agent_transcript_path")) {
                    Ok(b) if !b.is_empty() && !field("agent_transcript_path").is_empty() => (Some(b), "the transcript file"),
                    _ => (None, "nothing"),
                },
            };
            let digest = bytes.as_ref().map(|b| sha256_hex(b));
            let end = |outcome: &str, note: String| obj(vec![("id", s(&id)), ("outcome", s(outcome)), ("result", opt(&digest)), ("read_back", opt(&digest)), ("spent", Value::Int(0)), ("note", s(&note)), ("observed", s("SUBAGENT_STOP"))]);
            let body = match (&bytes, stop_asked) {
                (_, true) => end("STOPPED", format!("exit observed after a stop was requested; the host read back {source}, which is not accepted as a result")),
                (Some(b), false) => {
                    let d = digest.clone().unwrap_or_default();
                    match tx.put(&se.actor, "HOST", "ARTIFACT_ADDED", obj(vec![("sha256", s(&d)), ("size", Value::Int(b.len() as i64)), ("origin", s("PRODUCED")), ("stored", Value::Bool(false)), ("task", s(&id))]))? {
                        Put::Written { .. } => end("DONE", format!("exit observed; the result is the SHA-256 of {source} as the host read it back. The worker's claim and the read-back are one observation here")),
                        Put::Refused(r) => end("FAILED", format!("exit observed; the result could not be recorded: {}", clip(&r.message, 300))),
                    }
                }
                (None, false) => end("FAILED", "exit observed; nothing could be read back as a result".to_string()),
            };
            if let Put::Refused(r) = tx.put("host", "HOST", "TASK_ENDED", body)? {
                let failed = obj(vec![("id", s(&id)), ("outcome", s("FAILED")), ("result", Value::Null), ("read_back", Value::Null), ("spent", Value::Int(0)), ("observed", s("SUBAGENT_STOP")),
                                      ("note", s(&format!("exit observed; its completion was refused by the guards: {}", clip(&r.message, 300))))]);
                if let Put::Refused(r2) = tx.put("host", "HOST", "TASK_ENDED", failed)? {
                    eprintln!("pcpos hook: the end of task {id} was not recorded: {}", r2.message);
                }
            }
            tx.commit()?;
            Ok(String::new())
        }
        // A file tool is about to write. The register and what stands beside it are written by this program only.
        "pre-tool" => {
            let target = input.get("tool_input").and_then(|t| t.get("file_path").or_else(|| t.get("notebook_path"))).and_then(Value::as_str).unwrap_or("");
            if target.is_empty() {
                return Ok(String::new());
            }
            let t = Path::new(target);
            let t = if t.is_absolute() { t.to_path_buf() } else { se.root.join(t) };
            let resolved = match t.parent().and_then(|d| d.canonicalize().ok()) {
                Some(d) => d.join(t.file_name().unwrap_or_default()),
                None => t.clone(),
            };
            let flat = |p: &Path| -> PathBuf {
                let mut o = PathBuf::new();
                for c in p.components() {
                    match c {
                        Component::ParentDir => {
                            o.pop();
                        }
                        Component::CurDir => {}
                        other => o.push(other.as_os_str().to_string_lossy().to_lowercase()),
                    }
                }
                o
            };
            let register = se.root.join(&se.register);
            let keep = register.parent().map(|d| d.to_path_buf()).filter(|d| d != &se.root);
            let (ft, fr) = (flat(&t), flat(&register));
            let by_name = ft == fr || ft == flat(&se.root.join(se.observed())) || ft == flat(&se.root.join(se.observed_tools())) || keep.as_ref().map(|d| ft.starts_with(flat(d))).unwrap_or(false);
            let fenced = by_name || resolved == register || resolved == se.root.join(se.observed()) || resolved == se.root.join(se.observed_tools()) || keep.map(|d| resolved.starts_with(&d)).unwrap_or(false)
                || resolved.canonicalize().ok().zip(register.canonicalize().ok()).map(|(a, b)| a == b).unwrap_or(false);
            if !fenced {
                return Ok(String::new());
            }
            let deny = obj(vec![("hookSpecificOutput", obj(vec![("hookEventName", s("PreToolUse")), ("permissionDecision", s("deny")),
                ("permissionDecisionReason", s("The ACR register and the files beside it are written only through the acr tools, which check every event. Use acr_submit; do not edit the record."))]))]);
            Ok(String::from_utf8_lossy(&deny.canonical_json()).to_string())
        }
        // A shell command has run, or has failed. While ACR is in use the host keeps what it saw: the command, its exit
        // status and the digests of its output, so that a check can be recorded on the host's observation and not on a
        // participant's report. Outside ACR use nothing is kept.
        "post-tool" => {
            if field("tool_name") != "Bash" {
                return Ok(String::new());
            }
            let at_event = match open(&se.root, &reg) {
                Ok(Some(o)) if o.state.in_use() == Use::InUse => o.events.len() as i64,
                _ => return Ok(String::new()),
            };
            let command = input.get("tool_input").and_then(|t| t.get("command")).and_then(Value::as_str).unwrap_or("");
            if command.is_empty() || command.len() > MAX_COMMAND {
                return Ok(String::new());
            }
            // Only a report in the form the host gives it is kept. Input that names no tool call, or that is neither of the
            // host's two events, or that lacks what that event carries, is not an observation of a command: nothing is kept,
            // and acr_check then finds nothing. A missing result is never read as a success, and neither is a missing or
            // wrongly typed interruption flag read as "it was not interrupted": the host gives that flag as true or false
            // with both events, and a report without it in that form says nothing about whether the command finished.
            let failed = match field("hook_event_name") {
                "PostToolUse" => false,
                "PostToolUseFailure" => true,
                _ => return Ok(String::new()),
            };
            let reported = if failed { matches!(input.get("error"), Some(Value::Str(e)) if !e.is_empty()) }
                           else { matches!(input.get("tool_response"), Some(Value::Map(_))) && ["stdout", "stderr"].iter().all(|k| matches!(input.get("tool_response").and_then(|r| r.get(k)), Some(Value::Str(_)))) };
            let flag = if failed { input.get("is_interrupt") } else { input.get("tool_response").and_then(|r| r.get("interrupted")) };
            let interrupted = match flag {
                Some(Value::Bool(b)) => *b,
                _ => return Ok(String::new()),
            };
            if field("tool_use_id").is_empty() || !reported {
                return Ok(String::new());
            }
            let (exit, timeout, out, err, form): (Value, bool, String, String, &str) = if failed {
                // The host reports a failed command as one text: its first line names the exit status, the rest is the output.
                let e = field("error");
                let (first, rest) = e.split_once('\n').unwrap_or((e, ""));
                let code = first.strip_prefix("Exit code ").and_then(|n| n.trim().parse::<i64>().ok());
                match code {
                    // A report of failure that names exit status 0 contradicts itself. It is not kept, and so it is never a passed check.
                    Some(0) => return Ok(String::new()),
                    Some(c) => (Value::Int(c), interrupted, rest.to_string(), String::new(), "ONE_TEXT_ON_FAILURE"),
                    None => (Value::Null, true, e.to_string(), String::new(), "ONE_TEXT_ON_FAILURE"),
                }
            } else {
                let r = input.get("tool_response");
                let part = |k: &str| r.and_then(|r| r.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
                (if interrupted { Value::Null } else { Value::Int(0) }, interrupted, part("stdout"), part("stderr"), "STDOUT_AND_STDERR")
            };
            let line = obj(vec![("ts", s(&now_utc())), ("tool_use_id", s(&clip(field("tool_use_id"), 200))), ("command", s(command)), ("exit", exit), ("timeout", Value::Bool(timeout)),
                                ("stdout", s(&sha256_hex(out.as_bytes()))), ("stderr", s(&sha256_hex(err.as_bytes()))), ("output", s(form)), ("output_bytes", Value::Int((out.len() + err.len()) as i64)), ("at_event", Value::Int(at_event)),
                                ("duration_ms", input.get("duration_ms").and_then(Value::as_int).map(Value::Int).unwrap_or(Value::Null))]).canonical_json();
            if let Some(mut h) = confined(&se.root, &se.observed_tools(), true, true)? {
                lock(&h, true)?;
                // The newest observations only: this is a working file beside the record, not the record.
                let old = read_all(&mut h)?;
                let mut keep: Vec<&[u8]> = old.split(|b| *b == b'\n').filter(|l| !l.is_empty()).collect();
                keep.push(&line);
                let from = keep.len().saturating_sub(MAX_OBSERVED_TOOLS);
                let mut all: Vec<u8> = Vec::new();
                for l in &keep[from..] {
                    all.extend_from_slice(l);
                    all.push(b'\n');
                }
                h.file.set_len(0).and_then(|_| h.file.seek(SeekFrom::Start(0))).and_then(|_| h.file.write_all(&all)).and_then(|_| h.file.sync_all()).map_err(|e| e.to_string())?;
            }
            Ok(String::new())
        }
        _ => Err(format!("no hook named {event}")),
    }
}

/// The longest shell command the host keeps an observation of, and how many observations it keeps.
const MAX_COMMAND: usize = 16 * 1024;
const MAX_OBSERVED_TOOLS: usize = 200;

/// What the host saw of shell commands that no check has yet been recorded from, oldest first.
fn observed_unrecorded(se: &Session, used: &[String]) -> Vec<Value> {
    let read = || -> Option<Vec<u8>> {
        let mut h = confined(&se.root, &se.observed_tools(), false, false).ok()??;
        lock(&h, false).ok()?;
        read_all(&mut h).ok()
    };
    read().unwrap_or_default().split(|b| *b == b'\n').filter(|l| !l.is_empty()).filter_map(|l| Value::parse(l).ok())
        .filter(|v| !matches!(v.get("tool_use_id").and_then(Value::as_str), Some(id) if used.iter().any(|u| u == id))).collect()
}

// ---- the command line --------------------------------------------------------------------------------------

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// A register named at a terminal: its own directory is the directory served.
fn at_terminal(path: &str, create: bool) -> Result<(PathBuf, String), String> {
    let p = Path::new(path);
    let name = p.file_name().and_then(|n| n.to_str()).ok_or("no file name")?.to_string();
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if create {
        let _ = fs::create_dir_all(dir);
    }
    Ok((dir.canonicalize().map_err(|e| format!("{}: {}", dir.display(), e))?, name))
}

/// Keep the historical fragment name when available. Never open an occupied name, even to retry a
/// repair: it may be a link, an unrelated file, or a partial copy left after an earlier failure.
fn save_fragment(register: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let extension = format!("fragment-{}", &sha256_hex(bytes)[..12]);
    let mut aside = register.with_extension(&extension);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    // A recovered line can contain private register data. Restrict access at creation, before writing.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&aside) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
            aside = register.with_extension(format!("{extension}-{}-{stamp}", std::process::id()));
            options.open(&aside)?
        }
        Err(e) => return Err(e),
    };
    file.write_all(bytes)?;
    file.sync_all()?;
    // On Unix the new directory entry is a separate persistence step from the file contents.
    #[cfg(unix)]
    File::open(aside.parent().unwrap_or(Path::new(".")))?.sync_all()?;
    Ok(aside)
}

/// Set aside an unfinished last line, which an interrupted write can leave. Nothing that was stored whole is
/// touched: the fragment is moved to a file beside the register and the register ends at its last whole event.
fn repair(path: &str, sync_register: impl FnOnce(&File) -> std::io::Result<()>) -> Result<String, String> {
    let (root, name) = at_terminal(path, false)?;
    let mut h = confined(&root, &name, true, false)?.ok_or(format!("there is no register at {path}"))?;
    lock(&h, true)?;
    let raw = read_all(&mut h)?;
    if raw.is_empty() || raw.ends_with(b"\n") {
        return Ok("the register ends with a whole line; nothing was changed. Run verify.\n".to_string());
    }
    let keep = raw.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
    ledger::verify(&raw[..keep]).map_err(|e| format!("the part before the unfinished line is not intact either ({:?}); nothing was changed", e))?;
    let aside = save_fragment(&h.path, &raw[keep..]).map_err(|e| format!("the fragment could not be saved ({e}); the register was not shortened"))?;
    h.file.set_len(keep as u64).map_err(|e| format!(
        "saved the unfinished fragment to {}. The host could not shorten the register ({e}). Run verify before retrying repair", aside.display()
    ))?;
    sync_register(&h.file).map_err(|e| format!(
        "saved the unfinished fragment to {}. The register was shortened to {keep} bytes, but the final sync failed ({e}). Its durability is unknown. Run verify on the register",
        aside.display()
    ))?;
    Ok(format!("an unfinished last line of {} byte(s) was moved to {}. The register now ends at its last whole event. Run verify, and record what happened.\n", raw.len() - keep, aside.display()))
}

/// A person adds, removes or lists the modules that stand beside a register. An added module is text the
/// person supplies; it is identified in every review that uses it by the digest of its Specification, and
/// it never displaces a module of the method text.
fn module_command(args: &[String]) -> Result<i32, String> {
    let verb = args.get(1).map(String::as_str).unwrap_or("");
    let (root, name) = at_terminal(args.get(2).ok_or("usage")?, verb == "add")?;
    let mut se = session(root.to_str())?;
    se.root = root;
    se.register = name.clone();
    if let Some(m) = flag(args, "--method") {
        se.method = Some(PathBuf::from(m));
    }
    let home = se.root.join("modules");
    if verb == "list" {
        print!("{}", modules_text(&se, open(&se.root, &name)?.as_ref()));
        return Ok(0);
    }
    let by = flag(args, "--by").filter(|l| label_ok(l) && l != "host").ok_or("name the person acting with --by <label>")?;
    se.principal = by.clone();
    let what = args.get(3).filter(|a| !a.starts_with("--")).ok_or("usage")?;
    let event = |tx: &mut Tx, event: &str, m: Value| tx.put("host", "HOST", "HOST_EVENT", obj(vec![("event", s(event)), ("by", s(&by)), ("observed", s("ENTERED_AT_A_TERMINAL")), ("module", m)]));
    let put = match verb {
        "add" => {
            se.method.as_ref().ok_or("no method text is at hand to compare with: give its folder with --method <directory>")?;
            let src = Path::new(what);
            let file = [src.join("SOURCE").join("Specification.md"), src.join("Specification.md"), src.to_path_buf()].into_iter().find(|p| p.is_file()).ok_or(format!("there is no Specification.md at {what}"))?;
            if fs::metadata(&file).map(|m| m.len()).unwrap_or(u64::MAX) > MAX_SPECIFICATION {
                return Err(format!("{} is larger than {MAX_SPECIFICATION} bytes", file.display()));
            }
            let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let text = String::from_utf8(bytes.clone()).map_err(|_| "the Specification is not UTF-8 text".to_string())?;
            let (id, key, version, core) = component_header(&text).map_err(|w| format!("{}: {w}", file.display()))?;
            if !id.starts_with("local.") {
                return Err(format!("{id}: an added module is named `local.<name>`. Identifiers beginning `acr.` belong to the modules of the method"));
            }
            if core.as_deref().map(|c| c != acr::CORE_DESCRIPTOR).unwrap_or(false) {
                return Err(format!("{id} says it was written for another Core than this program implements ({})", acr::CORE_DESCRIPTOR));
            }
            if modules_in(&se).iter().any(|m| m.key == key) || modules_and_oddities(&se).1.iter().any(|o| o.contains(&format!(".{key} ")) || o.contains(&format!(".{key};"))) {
                return Err(format!("{key} is already the name of a module here. Remove it first (pcpos module remove), or give the new one its own identifier"));
            }
            let mut tx = Tx::begin(&se.root, &name, true)?.ok_or("the register could not be created")?;
            if !tx.problems.is_empty() {
                return Err("the register does not verify; nothing was added".to_string());
            }
            begin(&mut tx, &se, None, false)?;
            let dest = home.join(&key).join("SOURCE");
            if fs::symlink_metadata(&home).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
                return Err("the modules folder beside the register is a link; nothing was added".to_string());
            }
            fs::create_dir_all(&dest).and_then(|_| fs::write(dest.join("Specification.md"), &bytes)).map_err(|e| format!("the module could not be stored: {e}"))?;
            if let Some(over) = file.parent().map(|p| p.join("Overview.md")).filter(|p| p.is_file() && fs::metadata(p).map(|m| m.len() <= MAX_SPECIFICATION).unwrap_or(false)) {
                let _ = fs::copy(over, dest.join("Overview.md"));
            }
            println!("added {id} {version} to {}\nSHA-256 of its Specification {}\nIt selects nothing: name it in a workspace or a review to use it.", home.join(&key).display(), sha256_hex(&bytes));
            event(&mut tx, "MODULE_ADDED", obj(vec![("id", s(&id)), ("version", s(&version)), ("sha256", s(&sha256_hex(&bytes)))])).map(|p| (tx, p))?
        }
        "remove" => {
            let want = what.to_ascii_lowercase().replace('-', "_");
            let want = want.strip_prefix("acr.").or_else(|| want.strip_prefix("local.")).unwrap_or(&want).to_string();
            if !label_ok(&want) || !home.join(&want).is_dir() {
                return Err(format!("{what} is not an added module here. A module of the method text is not removed"));
            }
            let found = modules_in(&se).into_iter().find(|m| m.added && m.key == want);
            let mut tx = Tx::begin(&se.root, &name, false)?.ok_or(format!("there is no register at {}", args[2]))?;
            fs::remove_dir_all(home.join(&want)).map_err(|e| format!("the module could not be removed: {e}"))?;
            println!("removed the added module {want}. Reviews already commissioned with it keep its digest in the record.");
            let m = found.map(|m| m.record()).unwrap_or_else(|| obj(vec![("id", s(&format!("local.{want}")))]));
            event(&mut tx, "MODULE_REMOVED", m).map(|p| (tx, p))?
        }
        _ => return Err("usage".to_string()),
    };
    match put {
        (mut tx, Put::Written { seq, head, .. }) => {
            tx.commit()?;
            println!("WRITTEN as event {seq}\nhead {head}");
            Ok(0)
        }
        (_, Put::Refused(r)) => {
            print!("{}", refusal_text(&r));
            Ok(acr::REFUSED as i32)
        }
    }
}

fn person(args: &[String]) -> Result<i32, String> {
    let (root, name) = at_terminal(args.get(1).ok_or("a register is named")?, args[0] == "init")?;
    let by = flag(args, "--by").or(flag(args, "--principal")).filter(|l| label_ok(l) && l != "host").ok_or("name the person acting with --by <label> (or --principal for init)")?;
    let mut se = session(root.to_str())?;
    se.root = root;
    se.register = name.clone();
    se.principal = by.clone();
    let mut tx = Tx::begin(&se.root, &name, args[0] == "init")?.ok_or(format!("there is no register at {}", args[1]))?;
    let put = match args[0].as_str() {
        "init" => {
            let did = begin(&mut tx, &se, flag(args, "--name").as_deref(), false)?;
            tx.commit()?;
            println!("{did}\nhead {}", tx.head());
            return Ok(0);
        }
        "adopt" => tx.put(&by, "DECLARED", "ADOPTED", obj(vec![("by", s(&by)), ("basis", s(&clip(&flag(args, "--basis").ok_or("give the instruction with --basis")?, 2000))), ("observed", s("ENTERED_AT_A_TERMINAL"))]))?,
        "halt" => tx.put(&by, "DECLARED", "HALTED", obj(vec![("by", s(&by)), ("words", flag(args, "--words").map(|w| s(&clip(&w, 2000))).unwrap_or(Value::Null))]))?,
        "workspace" => {
            let names: Vec<&str> = args.iter().skip(2).filter(|a| !a.starts_with("--") && Some(*a) != flag(args, "--by").as_ref() && Some(*a) != flag(args, "--method").as_ref()).map(String::as_str).collect();
            if let Some(m) = flag(args, "--method") {
                se.method = Some(PathBuf::from(m));
            }
            set_profile(&mut tx, &se, &names, &by, "ENTERED_AT_A_TERMINAL", &names.join(" "), None)?
        }
        "standard" => {
            let mut st = tx.state().standard();
            let mut tasks: Vec<Value> = Vec::new();
            for a in args.iter().skip(2).filter(|a| !a.starts_with("--") && Some(*a) != flag(args, "--by").as_ref() && Some(*a) != flag(args, "--second").as_ref()) {
                match (a.as_str(), a.split_once('=')) {
                    ("BASELINE", _) => st = Standard::BASELINE,
                    ("CRITICAL", _) => st = Standard::CRITICAL,
                    (_, Some(("method", v))) => st.method = if v == "NATIVE_ONLY" { Method::NativeOnly } else { Method::NativeOrPinned },
                    (_, Some(("model_findings", v))) => st.model_findings = match v { "MECHANICAL_ONLY" => ModelFindings::MechanicalOnly, "HUMAN_CLOSES" => ModelFindings::HumanCloses, _ => ModelFindings::AsMethod },
                    (_, Some(("human_adoption", v))) => st.human_adoption = v == "true",
                    (_, Some(("dual_authorization", v))) => st.dual_authorization = v == "true",
                    (_, Some(("static_tasks", v))) => st.static_tasks = v == "true",
                    (_, Some(("offline_only", v))) => st.offline_only = v == "true",
                    (_, Some(("tasks", v))) => tasks = v.split(',').filter(|t| !t.is_empty()).map(s).collect(),
                    _ => return Err(format!("{a} is not a standard or a field of one")),
                }
            }
            if let Some(e) = se.edition {
                if !st.available_in(e) {
                    return Err("this edition is licensed to one person, so it cannot meet a requirement for two people; that standard is not available in it".to_string());
                }
            }
            tx.put(&by, "DECLARED", "STANDARD_SET", obj(vec![("by", s(&by)), ("second", opt(&flag(args, "--second"))),
                ("method", s(if st.method == Method::NativeOnly { "NATIVE_ONLY" } else { "NATIVE_OR_PINNED" })),
                ("model_findings", s(match st.model_findings { ModelFindings::MechanicalOnly => "MECHANICAL_ONLY", ModelFindings::HumanCloses => "HUMAN_CLOSES", ModelFindings::AsMethod => "AS_METHOD" })),
                ("human_adoption", Value::Bool(st.human_adoption)), ("dual_authorization", Value::Bool(st.dual_authorization)), ("static_tasks", Value::Bool(st.static_tasks)),
                ("offline_only", Value::Bool(st.offline_only)), ("tasks", Value::List(tasks))]))?
        }
        _ => return Err("usage".to_string()),
    };
    match put {
        Put::Written { seq, head, prev } => {
            tx.commit()?;
            if args[0] == "workspace" {
                println!("WRITTEN as event {seq}: the modules this workspace's reviews use, selected by {by} at a terminal\nhead {head}\nprevious head {prev}");
                println!("WORKSPACE {}", profile_words(&profile_of(&parse(tx.rec.ledger().bytes().to_vec())?.events)));
            } else {
                print!("{}", receipt(seq, &head, &prev, &args[0].to_ascii_uppercase(), &by, "DECLARED"));
            }
            Ok(0)
        }
        Put::Refused(r) => {
            print!("{}", refusal_text(&r));
            Ok(acr::REFUSED as i32)
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let usage = || -> ! {
        eprintln!("usage: pcpos status|verify <register> | machine [name] | method <directory> | repair <register> | modules <method directory> | clause <method directory> <module> [section] | instrument list | instrument <name> <input as JSON> [register] | workspace <register> --by <label> [--method <directory>] [module ...] | module list <register> | module add <register> <folder or Specification.md> --by <label> [--method <directory>] | module remove <register> <name> --by <label> | init <register> --principal <label> [--name <name>] | adopt <register> --by <label> --basis <words> | halt <register> --by <label> [--words <words>] | standard <register> --by <label> [--second <label>] BASELINE|CRITICAL|key=value... | mcp | hook <event>");
        exit(2)
    };
    match args.first().map(String::as_str) {
        Some("mcp") => match session(None) {
            Ok(se) => {
                // standard error: standard output carries protocol messages only
                eprintln!("pcpos mcp: serving registers under {} as participant {} (person directing the work: {})", se.root.display(), se.actor, se.principal);
                serve(&se)
            }
            Err(e) => {
                eprintln!("pcpos: {e}");
                exit(2)
            }
        },
        Some("hook") if args.len() != 2 => {
            eprintln!("pcpos hook: one event name is expected");
            exit(0)
        }
        Some("hook") => {
            let mut raw = Vec::new();
            let _ = std::io::stdin().lock().read_to_end(&mut raw);
            let input = lenient::parse(&raw).unwrap_or(Value::Map(BTreeMap::new()));
            match hook(&args[1], &input) {
                Ok(out) => print!("{out}"),
                Err(e) => eprintln!("pcpos hook {}: {e}", args[1]),
            }
            exit(0) // a hook of this program never stops the host's work by failing
        }
        Some("machine") => match machine_text(args.get(1).map(String::as_str)) {
            Ok(t) => print!("{t}"),
            Err(e) => {
                eprintln!("pcpos: {e}");
                exit(2)
            }
        },
        Some("instrument") if args.len() >= 2 => {
            // An instrument at a terminal: computed and printed, never recorded. `pcpos instrument list`, or a name and its input as JSON.
            if args[1] == "list" {
                for (name, what) in instruments::LIST {
                    println!("{name}: {what}");
                }
                exit(0);
            }
            let input = match Value::parse(args.get(2).map(|s| s.as_str()).unwrap_or("{}").as_bytes()) {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("pcpos: the input is not JSON");
                    exit(2);
                }
            };
            let events = match args.get(3).and_then(|r| fs::read(r).ok()).and_then(|raw| parse(raw).ok()) {
                Some(o) => o.events,
                None => Vec::new(),
            };
            match instruments::run(&args[1], &input, &events) {
                Ok(found) => {
                    println!("{}\n{}", args[1], found.text);
                    exit(if found.holds { 0 } else { 1 });
                }
                Err(e) => {
                    eprintln!("pcpos: {e}");
                    exit(2);
                }
            }
        }
        Some("clause") if args.len() >= 3 => {
            let mut se = match session(None) {
                Ok(se) => se,
                Err(e) => {
                    eprintln!("pcpos: {e}");
                    exit(2)
                }
            };
            se.method = Some(PathBuf::from(&args[1]));
            match clause_text(&se, &args[2], args.get(3).map(String::as_str)) {
                Ok(t) => print!("{t}"),
                Err(e) => {
                    eprintln!("pcpos: {e}");
                    exit(6)
                }
            }
        }
        Some("modules") if args.len() == 2 => {
            let mut se = match session(None) {
                Ok(se) => se,
                Err(e) => {
                    eprintln!("pcpos: {e}");
                    exit(2)
                }
            };
            se.method = Some(PathBuf::from(&args[1]));
            print!("{}", modules_text(&se, None));
        }
        Some("method") if args.len() == 2 => match method_check(Path::new(&args[1])) {
            Ok(c) => {
                println!("{}", c.text());
                exit(if c.complete() { 0 } else if c.is_native() && c.differing.is_empty() { 1 } else { 6 })
            }
            Err(e) => {
                eprintln!("pcpos: {e}");
                exit(6)
            }
        },
        Some("repair") if args.len() == 2 => match repair(&args[1], File::sync_all) {
            Ok(t) => print!("{t}"),
            Err(e) => {
                eprintln!("pcpos: {e}");
                exit(6)
            }
        },
        Some("module") if args.len() >= 3 => match module_command(&args) {
            Ok(code) => exit(code),
            Err(e) if e == "usage" => usage(),
            Err(e) => {
                eprintln!("pcpos: {e}");
                exit(2)
            }
        },
        Some("init" | "adopt" | "halt" | "standard" | "workspace") if args.len() >= 2 => match person(&args) {
            Ok(code) => exit(code),
            Err(e) if e == "usage" => usage(),
            Err(e) => {
                eprintln!("pcpos: {e}");
                exit(2)
            }
        },
        Some(cmd @ ("status" | "verify")) if args.len() == 2 => {
            let found = at_terminal(&args[1], false).and_then(|(root, name)| open(&root, &name)?.ok_or(format!("there is no register at {}", args[1])));
            match found {
                Err(e) => {
                    eprintln!("pcpos: {e}");
                    exit(6)
                }
                Ok(o) => {
                    if cmd == "status" {
                        let colour = std::io::stdout().is_terminal() && env::var_os("NO_COLOR").map(|v| v.is_empty()).unwrap_or(true) && env::var("TERM").map(|t| t != "dumb").unwrap_or(true);
                        print!("{}{}", banner(&o, colour), status(&args[1], &o));
                    } else {
                        print!("{}", verify_text(&o, usize::MAX));
                        let n = attention(&o).len();
                        if n > 0 {
                            println!("exit status 1: the bytes verify, and {n} item(s) in the record need a response. Run status to read them.");
                        }
                    }
                    exit(if attention(&o).is_empty() { 0 } else { 1 })
                }
            }
        }
        _ => usage(),
    }
}

#[cfg(test)]
mod repair_tests {
    use super::*;

    #[test]
    fn repair_reports_the_saved_fragment_when_the_final_sync_fails() {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = env::temp_dir().join(format!("pcpos-repair-final-sync-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let register = dir.join("register.jsonl");
        let mut ledger = ledger::Ledger::new();
        ledger.append("2026-11-14T00:00:00Z", "test", "DECLARED", "X_TEST", obj(vec![])).unwrap();
        let prefix = ledger.bytes();
        let tail = b"unfinished";
        fs::write(&register, [prefix, tail].concat()).unwrap();
        let preferred = register.with_extension(format!("fragment-{}", &sha256_hex(tail)[..12]));
        fs::write(&preferred, b"earlier backup").unwrap();

        let error = repair(register.to_str().unwrap(), |file| {
            assert_eq!(file.metadata().unwrap().len(), prefix.len() as u64);
            Err(std::io::Error::other("injected final sync failure"))
        }).unwrap_err();
        let saved: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path())
            .filter(|p| p != &register && p != &preferred).collect();
        assert_eq!(saved.len(), 1);
        assert_eq!(fs::read(&saved[0]).unwrap(), tail);
        assert_eq!(fs::read(&preferred).unwrap(), b"earlier backup");
        assert_eq!(fs::read(&register).unwrap(), prefix);
        fs::remove_dir_all(dir).unwrap();

        assert!(error.contains(&saved[0].display().to_string()), "{error}");
        assert!(error.contains(&format!("was shortened to {} bytes", prefix.len())), "{error}");
        assert!(error.contains("durability is unknown") && error.contains("injected final sync failure"), "{error}");
    }
}
