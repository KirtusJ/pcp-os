//! The calls a loaded program makes with the `syscall` instruction. Their numbers, arguments and answers
//! are those of Linux on x86_64, for the calls a statically linked C program needs: so that such a
//! program, built with an ordinary compiler and C library, runs here unchanged. This is a small part of
//! that interface and is not Linux: there are no signals, no threads, no pipes, no sockets, no users, and
//! one flat set of file names. A call this kernel does not answer is refused with the usual code for
//! "no such call" and counted, so that what a program wanted and did not get can be read afterwards.
//!
//! Every address a program passes is one in its own memory, and is reached only through `Space::copy_in`
//! and `Space::copy_out`, which refuse anything else. Every device and file is reached only with a
//! capability the program was started with.
//!
//! This file has no `unsafe`.
use crate::arch::{self, Frame};
use crate::user::{Object, Process, Table, Waiting, World};
use crate::{dev, exec, mem};
use alloc::string::String;
use alloc::vec::Vec;
use pcpos_core::cap::{CapSpace, Rights};

const EPERM: i64 = 1;
const ENOENT: i64 = 2;
#[cfg(feature = "vm-store")]
const EIO: i64 = 5;
const EBADF: i64 = 9;
const EAGAIN: i64 = 11;
const ENOMEM: i64 = 12;
const EACCES: i64 = 13;
const EFAULT: i64 = 14;
const EEXIST: i64 = 17;
const EINVAL: i64 = 22;
const EMFILE: i64 = 24;
const ENOTTY: i64 = 25;
const ENOSPC: i64 = 28;
const ESPIPE: i64 = 29;
const ENAMETOOLONG: i64 = 36;
const ENOSYS: i64 = 38;

fn err(code: i64) -> u64 {
    (-code) as u64
}

/// What the kernel does after a call.
pub enum Outcome {
    /// Answer and let the program go on.
    Return(u64),
    /// The program waits. If the flag is set it makes the same call again when it is woken; if not, the call is answered with zero.
    Wait(Waiting, bool),
    /// Answer zero and let another program run.
    Yield,
    /// The program ends.
    Exit(u64),
}

/// What an open file is.
#[derive(Clone, Debug)]
pub enum Node {
    /// Where a program that may propose effects hands the kernel a proposal and reads the receipt.
    #[cfg(feature = "proposals")]
    Proposal,
    #[cfg(feature = "guest-ethernet")]
    Ethernet,
    #[cfg(feature = "game-link")]
    GameLink,
    Console,
    Display,
    Keys,
    Sound,
    /// The FM synthesiser: timed register writes.
    Music,
    /// Reads nothing and swallows what is written.
    Null,
    /// The one directory there is.
    Directory,
    /// One of the loader's files: its index in `World::files`.
    File(usize),
    /// An open writable-namespace file, even if its name has changed or been removed.
    Stored(crate::regular::File),
}

#[derive(Clone, Debug)]
pub struct Open {
    pub node: Node,
    pub position: usize,
    pub nonblocking: bool,
    pub writable: bool,
    pub append: bool,
}

/// A loaded program's open files and its memory.
pub struct Linux {
    pub files: Vec<Option<Open>>,
    /// Where the heap that `brk` moves begins, and where it ends now.
    heap_start: usize,
    heap_end: usize,
    /// Where the next mapping is placed. Addresses are not used twice.
    next_map: usize,
    /// Address ranges the program has reserved without asking for memory behind them yet: (start, end).
    reserved: Vec<(usize, usize)>,
    /// Pages of memory the program holds beyond its image and stack.
    pub extra_pages: usize,
    /// The program asked (display request 0x5004) to draw to memory of its own that the kernel copies
    /// to the display, and was told that this display needs it.
    shadow_asked: bool,
    /// Where that memory is in the program's address space, once it has mapped the display.
    shadow: Option<usize>,
    #[cfg(feature = "proposals")]
    proposals: Proposals,
}

/// What came of the kernel's question.
#[cfg(feature = "proposals")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Allowed,
    Refused,
    /// The display has no room to show the proposal whole and legibly, so the person was not asked.
    CannotBeShown,
}

/// One effect a program asks the kernel to make: one new file of text, whole and exact.
#[cfg(feature = "proposals")]
#[derive(Clone, Debug)]
pub struct Proposal {
    pub attempt: u64,
    pub number: u64,
    pub name: String,
    pub content: Vec<u8>,
    /// The length of the record the program wrote, which its call is answered with.
    pub length: usize,
}

#[cfg(feature = "proposals")]
#[derive(Default)]
struct Proposals {
    allowed: bool,
    /// Which run of a proposing program this is, counted by the kernel from its start.
    launch: u64,
    pending: Option<Proposal>,
    /// The receipt of the last proposal the kernel took. It stays, and can be read again, until the
    /// next one is taken.
    receipt: Option<[u8; RECEIPT]>,
    /// Every receipt of this run with the digest of the exact request: the same request made again is
    /// answered and not done again; another request under the same numbers is refused.
    kept: Vec<(u64, u64, [u8; 32], [u8; RECEIPT])>,
}

#[cfg(feature = "proposals")]
static LAUNCHES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// The most lines of the kernel's screen a proposal may need (64 pages of 22 lines of 76 characters,
/// the room on the smallest display the question is shown on).
#[cfg(feature = "proposals")]
const PROPOSAL_LINES: usize = 64 * 22;
#[cfg(feature = "proposals")]
const PROPOSAL_ACROSS: usize = 76;
/// The most proposals one run of a program may have answered. Their receipts are kept, so that none can
/// be asked or done twice; when there is room for no more, a new proposal is refused, and no receipt
/// is ever dropped to make room (C3: the list grew without limit in the kernel's own memory).
#[cfg(feature = "proposals")]
const PROPOSALS_KEPT: usize = 64;

/// A receipt, 100 bytes: version (u32, 1); outcome (u32: 1 created, 2 refused by the person, 3 the name
/// was taken, 5 no room in the store, 6 the display could not show it so nobody was asked); the kernel's number for this run (u64); attempt and proposal
/// (u64 each); SHA-256 of the exact request (lengths, name, content); SHA-256 of the content; and
/// whether it outlasts power (u32, 0: it does not).
#[cfg(feature = "proposals")]
pub const RECEIPT: usize = 4 + 4 + 8 + 8 + 8 + 32 + 32 + 4;
#[cfg(feature = "proposals")]
pub const PROPOSAL_NAME: usize = 120;
#[cfg(feature = "proposals")]
pub const PROPOSAL_CONTENT: usize = 16 * 1024;
#[cfg(feature = "proposals")]
pub const E_INVALID: u64 = (-EINVAL) as u64;

#[cfg(feature = "proposals")]
fn receipt(outcome: u32, launch: u64, proposal: &Proposal) -> [u8; RECEIPT] {
    let mut r = [0u8; RECEIPT];
    r[0..4].copy_from_slice(&1u32.to_le_bytes());
    r[4..8].copy_from_slice(&outcome.to_le_bytes());
    r[8..16].copy_from_slice(&launch.to_le_bytes());
    r[16..24].copy_from_slice(&proposal.attempt.to_le_bytes());
    r[24..32].copy_from_slice(&proposal.number.to_le_bytes());
    r[32..64].copy_from_slice(&request_digest(proposal));
    r[64..96].copy_from_slice(&pcpos_core::sha256(&proposal.content));
    r
}

#[cfg(feature = "proposals")]
fn request_digest(proposal: &Proposal) -> [u8; 32] {
    let mut whole = Vec::with_capacity(8 + proposal.name.len() + proposal.content.len());
    whole.extend_from_slice(&(proposal.name.len() as u32).to_le_bytes());
    whole.extend_from_slice(&(proposal.content.len() as u32).to_le_bytes());
    whole.extend_from_slice(proposal.name.as_bytes());
    whole.extend_from_slice(&proposal.content);
    pcpos_core::sha256(&whole)
}

#[cfg(feature = "proposals")]
impl Linux {
    pub fn allow_proposals(&mut self, allowed: bool) {
        self.proposals.allowed = allowed;
        if allowed {
            self.proposals.launch = LAUNCHES.fetch_add(1, core::sync::atomic::Ordering::SeqCst) + 1;
        }
    }

    pub fn proposals_allowed(&self) -> bool {
        self.proposals.allowed
    }

    /// The proposal the program is waiting on, taken once.
    pub fn proposal_taken(&mut self) -> Option<Proposal> {
        self.proposals.pending.take()
    }

    /// The person has answered. On yes the kernel makes the file, with exactly the bytes it showed,
    /// unless the name has been taken meanwhile; then, as on no, nothing is made.
    pub fn proposal_settled(&mut self, proposal: Proposal, answer: Answer, world: &mut World) {
        let allowed = answer == Answer::Allowed;
        let free = world.store.named(&proposal.name).is_none() && !world.files.iter().any(|f| f.0 == proposal.name);
        let room = world.store.len() < 256 && world.store.fits(proposal.content.len());
        let outcome = if answer == Answer::CannotBeShown {
            6
        } else if !allowed {
            2
        } else if !free {
            3
        } else if !room {
            5
        } else {
            // The whole file or nothing: the room was counted first, and the bytes go in in one piece.
            world.store.insert(proposal.name.clone(), proposal.content.clone());
            1
        };
        let r = receipt(outcome, self.proposals.launch, &proposal);
        self.proposals.receipt = Some(r);
        self.proposals.kept.push((proposal.attempt, proposal.number, request_digest(&proposal), r));
        crate::log(format_args!("proposal {} of attempt {}: {} ({} bytes, SHA-256 {}) {}", proposal.number, proposal.attempt, proposal.name, proposal.content.len(),
                    pcpos_core::sha256_hex(&proposal.content), match outcome { 1 => "CREATED after the person allowed it", 2 => "DENIED by the person; nothing made", 3 => "REFUSED: the name was taken; nothing made", 6 => "REFUSED: this display is too small to show it, so nobody was asked; nothing made", _ => "REFUSED: no room in the store; nothing made" }));
        crate::log(format_args!("\n"));
    }
}

/// A program writes one proposal. The record is read whole and checked; a proposal already answered in
/// this run is answered again from its receipt; a new one stops the program until the kernel has asked.
#[cfg(feature = "proposals")]
fn propose(p: &mut Process, world: &World, addr: u64, len: u64) -> Outcome {
    let len = len as usize;
    if !(32..=32 + PROPOSAL_NAME + PROPOSAL_CONTENT).contains(&len) {
        return Outcome::Return(err(EINVAL));
    }
    let Some(bytes) = p.space.as_ref().and_then(|s| s.read(addr as usize, len)) else { return Outcome::Return(err(EFAULT)) };
    let word = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let long = |at: usize| u64::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3], bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]);
    let (name_len, content_len) = (word(24) as usize, word(28) as usize);
    if word(0) != 1 || word(4) != 1 || name_len == 0 || name_len > PROPOSAL_NAME || content_len > PROPOSAL_CONTENT || 32 + name_len + content_len != len {
        return Outcome::Return(err(EINVAL));
    }
    let (name, content) = (&bytes[32..32 + name_len], &bytes[32 + name_len..]);
    // A name is a file in at most three folders, one inside another ("taylor/my-project/notes.txt"):
    // each part of small letters, digits, dot, underscore and hyphen, not empty and not beginning with a
    // dot, and the first not "dev", where the kernel's devices are. Content of printable characters,
    // spaces and new lines: what the kernel's screen can show exactly, and no more.
    let part = |p: &[u8]| !p.is_empty() && p[0] != b'.' && p.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    let named = name.split(|b| *b == b'/').count() <= 4 && name.split(|b| *b == b'/').all(part) && !name.starts_with(b"dev/") && name != b"dev";
    let shown = content.iter().all(|b| (0x20..0x7f).contains(b) || *b == b'\n');
    // What the kernel cannot show whole, in a number of pages a person can turn, it does not ask about.
    let lines: usize = content.split(|b| *b == b'\n').map(|line| line.len().div_ceil(PROPOSAL_ACROSS).max(1)).sum();
    if !named || !shown || lines > PROPOSAL_LINES {
        crate::log(format_args!("a proposal was refused unseen: {}\n", if !named { "its name has a character a name may not have" }
            else if !shown { "its text has a character the kernel's screen cannot show exactly" } else { "it is too long to show" }));
        return Outcome::Return(err(EINVAL));
    }
    let proposal = Proposal { attempt: long(8), number: long(16), name: String::from_utf8_lossy(name).into_owned(), content: content.to_vec(), length: len };
    let Some(l) = p.linux.as_mut() else { return Outcome::Return(err(ENOSYS)) };
    if let Some((_, _, request, kept)) = l.proposals.kept.iter().find(|(a, n, _, _)| *a == proposal.attempt && *n == proposal.number) {
        if *request != request_digest(&proposal) {
            // The same numbers for other bytes: never answered with the earlier result.
            crate::log(format_args!("proposal {} of attempt {} REFUSED unseen: these numbers were used for a different request\n", proposal.number, proposal.attempt));
            return Outcome::Return(err(16));
        }
        l.proposals.receipt = Some(*kept);
        return Outcome::Return(len as u64);
    }
    if l.proposals.kept.len() >= PROPOSALS_KEPT {
        crate::log(format_args!("proposal {} of attempt {} REFUSED unseen: this run has made as many proposals as the kernel keeps receipts for\n", proposal.number, proposal.attempt));
        return Outcome::Return(err(28));
    }
    if world.store.named(&proposal.name).is_some() || world.files.iter().any(|f| f.0 == proposal.name) {
        let r = receipt(3, l.proposals.launch, &proposal);
        l.proposals.receipt = Some(r);
        l.proposals.kept.push((proposal.attempt, proposal.number, request_digest(&proposal), r));
        crate::log(format_args!("proposal {} of attempt {}: {} REFUSED without asking: the name is taken\n", proposal.number, proposal.attempt, proposal.name));
        return Outcome::Return(err(EEXIST));
    }
    l.proposals.receipt = None;
    l.proposals.pending = Some(proposal);
    Outcome::Wait(Waiting::Consent, false)
}

/// Limits for the `brk` heap and for a single mapping.
const HEAP_LIMIT: usize = 1 << 30;
const MAP_LIMIT: usize = 1 << 30;
const MAX_FILES: usize = 32;
/// Nanoseconds in one timer tick: 1193 / 1193182 of a second.
const TICK_NS: u64 = 999_847;

impl Linux {
    pub fn new(image_end: usize) -> Linux {
        let console = || Some(Open { node: Node::Console, position: 0, nonblocking: false, writable: true, append: false });
        Linux { files: alloc::vec![console(), console(), console()], heap_start: image_end, heap_end: image_end, next_map: arch::USER_BASE + (64 << 30), reserved: Vec::new(), extra_pages: 0, shadow_asked: false, shadow: None, #[cfg(feature = "proposals")] proposals: Proposals::default() }
    }
}

fn text(p: &Process, addr: u64) -> Result<String, u64> {
    let space = p.space.as_ref().ok_or(err(EFAULT))?;
    let mut out = Vec::new();
    for i in 0..256usize {
        let mut b = [0u8; 1];
        if !space.copy_in(addr as usize + i, &mut b) {
            return Err(err(EFAULT));
        }
        if b[0] == 0 {
            return String::from_utf8(out).map_err(|_| err(ENOENT));
        }
        out.push(b[0]);
    }
    Err(err(ENAMETOOLONG))
}

/// A path as a name: no leading slash, no empty or `.` components. A path that climbs with `..` names nothing.
fn name_of(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => return None,
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

fn put(p: &Process, addr: u64, bytes: &[u8]) -> bool {
    p.space.as_ref().map(|s| s.copy_out(addr as usize, bytes)).unwrap_or(false)
}

fn get<const N: usize>(p: &Process, addr: u64) -> Option<[u8; N]> {
    let mut b = [0u8; N];
    if p.space.as_ref()?.copy_in(addr as usize, &mut b) { Some(b) } else { None }
}

fn word(p: &Process, addr: u64) -> Option<u64> {
    get::<8>(p, addr).map(u64::from_le_bytes)
}

/// The 144 bytes Linux calls `struct stat`, for a file of the kind and size given.
fn stat_bytes(mode: u32, size: usize, links: u64) -> [u8; 144] {
    let mut s = [0u8; 144];
    s[16..24].copy_from_slice(&links.to_le_bytes());
    s[24..28].copy_from_slice(&mode.to_le_bytes());
    s[48..56].copy_from_slice(&(size as u64).to_le_bytes());
    s[56..64].copy_from_slice(&4096u64.to_le_bytes());
    s[64..72].copy_from_slice(&(((size + 511) / 512) as u64).to_le_bytes());
    s
}

fn node_stat(world: &World, node: &Node) -> Option<[u8; 144]> {
    Some(match node {
        #[cfg(feature = "proposals")]
        Node::Proposal => stat_bytes(0o020660, 0, 1),
        #[cfg(feature = "guest-ethernet")]
        Node::Ethernet => stat_bytes(0o020660, 0, 1),
        Node::Console | Node::Display | Node::Keys | Node::Sound | Node::Music | Node::Null => stat_bytes(0o020666, 0, 1),
        #[cfg(feature = "game-link")]
        Node::GameLink => stat_bytes(0o020666, 0, 1),
        Node::Directory => stat_bytes(0o040755, 0, 1),
        Node::File(i) => stat_bytes(0o100444, world.files.get(*i)?.1.len(), 1),
        Node::Stored(f) => stat_bytes(0o100644, f.len(), world.store.linked(f) as u64),
    })
}

/// What a path names, for a program holding the capabilities it holds.
fn lookup(p: &Process, caps: &CapSpace, world: &World, path: &str) -> Result<Node, u64> {
    let name = name_of(path).ok_or(err(ENOENT))?;
    let need = |object: Object, right: Rights, node: Node| if p.holds(caps, object, right) { Ok(node) } else { Err(err(EACCES)) };
    match name.as_str() {
        "" => Ok(Node::Directory),
        #[cfg(feature = "guest-ethernet")]
        "dev/ethernet" => need(Object::Ethernet, Rights::READ.with(Rights::APPEND), Node::Ethernet),
        #[cfg(feature = "game-link")]
        "dev/game-link" => need(Object::GameLink, Rights::READ.with(Rights::APPEND), Node::GameLink),
        "dev/null" => Ok(Node::Null),
        #[cfg(feature = "proposals")]
        "dev/proposal" => if p.linux.as_ref().is_some_and(|l| l.proposals.allowed) { Ok(Node::Proposal) } else { Err(err(EACCES)) },
        "dev/console" | "dev/tty" => need(Object::Console, Rights::APPEND, Node::Console),
        "dev/fb0" => need(Object::Display, Rights::APPEND, Node::Display),
        "dev/kbd" => need(Object::Keys, Rights::READ, Node::Keys),
        "dev/audio" => need(Object::Sound, Rights::APPEND, Node::Sound),
        "dev/fm" if dev::fm_state().0 => need(Object::Sound, Rights::APPEND, Node::Music),
        _ => {
            if let Some(file) = world.store.named(&name) {
                return need(Object::Store, Rights::READ, Node::Stored(file.clone()));
            }
            match world.files.iter().position(|f| f.0 == name) {
                Some(i) => need(Object::Files, Rights::READ, Node::File(i)),
                None => Err(err(ENOENT)),
            }
        }
    }
}

fn open(p: &mut Process, caps: &CapSpace, world: &mut World, path: u64, flags: u64) -> u64 {
    let path = match text(p, path) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let (writing, create, truncate) = (flags & 3 != 0, flags & 0x40 != 0, flags & 0x200 != 0);
    enum Target { Existing(Node), Create(String) }
    let target = match lookup(p, caps, world, &path) {
        Ok(_) if create && flags & 0x80 != 0 => return err(EEXIST), // O_CREAT | O_EXCL
        #[cfg(feature = "guest-ethernet")]
        Ok(Node::Ethernet) if flags & 3 != 2 => return err(EINVAL), // raw packet sessions require O_RDWR
        Ok(Node::File(_)) if writing => return err(EACCES), // the loader's files are read and never written
        Ok(n) => {
            if writing && matches!(n, Node::Stored(_)) && !p.holds(caps, Object::Store, Rights::APPEND) {
                return err(EACCES);
            }
            Target::Existing(n)
        }
        Err(e) if e == err(ENOENT) && create => {
            let name = match name_of(&path) {
                Some(n) if !n.is_empty() && !n.starts_with("dev/") && n.len() <= 200 => n,
                _ => return err(EACCES),
            };
            if !p.holds(caps, Object::Store, Rights::APPEND) {
                return err(EACCES);
            }
            if world.store.len() >= 256 {
                return err(ENOSPC);
            }
            Target::Create(name)
        }
        Err(e) => return e,
    };
    let linux = match p.linux.as_mut() {
        Some(l) => l,
        None => return err(ENOSYS),
    };
    let fd = linux.files.iter().position(Option::is_none).unwrap_or(linux.files.len());
    if fd >= MAX_FILES {
        return err(EMFILE);
    }
    // All refusal paths precede the creation or truncation of a stored file.
    let node = match target {
        Target::Existing(node) => {
            if let Node::Stored(f) = &node {
                if writing && truncate {
                    f.truncate();
                }
            }
            node
        }
        Target::Create(name) => Node::Stored(world.store.insert(name, Vec::new())),
    };
    let entry = Some(Open { node, position: 0, nonblocking: flags & 0x800 != 0, writable: writing, append: flags & 0x400 != 0 });
    if fd == linux.files.len() {
        linux.files.push(entry);
    } else {
        linux.files[fd] = entry;
    }
    fd as u64
}

fn file(p: &Process, fd: u64) -> Option<Open> {
    p.linux.as_ref()?.files.get(fd as usize)?.clone()
}

fn set_position(p: &mut Process, fd: u64, position: usize) {
    if let Some(Some(f)) = p.linux.as_mut().and_then(|l| l.files.get_mut(fd as usize)) {
        f.position = position;
    }
}

fn read(p: &mut Process, world: &World, fd: u64, addr: u64, len: u64) -> Outcome {
    let f = match file(p, fd) {
        Some(f) => f,
        None => return Outcome::Return(err(EBADF)),
    };
    let len = len.min(1 << 30) as usize;
    let stored;
    #[cfg(feature = "proposals")]
    if matches!(f.node, Node::Proposal) {
        // The receipt of the last proposal, once.
        // It is not used up by being read, and a read too small for it takes nothing.
        let Some(r) = p.linux.as_ref().and_then(|l| l.proposals.receipt) else { return Outcome::Return(0) };
        return Outcome::Return(if len < RECEIPT { err(EINVAL) } else if put(p, addr, &r) { RECEIPT as u64 } else { err(EFAULT) });
    }
    let bytes: &[u8] = match &f.node {
        #[cfg(feature = "proposals")]
        Node::Proposal => return Outcome::Return(0),
        #[cfg(feature = "guest-ethernet")]
        Node::Ethernet => {
            let mut frame = [0u8; dev::virtio_tx::guest::MTU];
            let result = if len < frame.len() { err(EINVAL) } else {
                match dev::virtio_tx::guest::receive(&mut frame) {
                    Ok(Some(n)) if put(p, addr, &frame[..n]) => n as u64,
                    Ok(Some(_)) => err(EFAULT),
                    Ok(None) => err(EAGAIN),
                    Err(_) => err(5),
                }
            };
            return Outcome::Return(result);
        }
        #[cfg(feature = "game-link")]
        Node::GameLink => {
            let mut buf = [0u8; 64];
            let mut n = 0;
            for slot in buf.iter_mut().take(len) {
                let Some(byte) = arch::game_link_read() else { break };
                *slot = byte;
                n += 1;
            }
            let result = if n == 0 && len > 0 { err(EAGAIN) } else if put(p, addr, &buf[..n]) { n as u64 } else { err(EFAULT) };
            return Outcome::Return(result);
        }
        Node::Console | Node::Null => return Outcome::Return(0),
        Node::Display | Node::Directory | Node::Sound | Node::Music => return Outcome::Return(err(EINVAL)),
        Node::Keys => {
            // Each key event is two bytes: the key's number, then bit 0 set if it was released and bit 1 if it is of the second group.
            let mut out = Vec::new();
            while out.len() + 2 <= len.min(128) {
                match dev::key_take() {
                    Some(k) => {
                        out.push(k.code);
                        out.push(k.released as u8 | (k.extended as u8) << 1);
                    }
                    None => break,
                }
            }
            if out.is_empty() {
                return if f.nonblocking || len < 2 { Outcome::Return(err(EAGAIN)) } else { Outcome::Wait(Waiting::Key, true) };
            }
            return Outcome::Return(if put(p, addr, &out) { out.len() as u64 } else { err(EFAULT) });
        }
        Node::File(i) => match world.files.get(*i) {
            Some(file) => file.1,
            None => return Outcome::Return(err(EBADF)),
        },
        Node::Stored(file) => {
            stored = file.bytes();
            &stored
        }
    };
    let from = f.position.min(bytes.len());
    let n = len.min(bytes.len() - from);
    if !put(p, addr, &bytes[from..from + n]) {
        return Outcome::Return(err(EFAULT));
    }
    set_position(p, fd, from + n);
    Outcome::Return(n as u64)
}

fn write(p: &mut Process, caps: &CapSpace, fd: u64, addr: u64, len: u64) -> u64 {
    let f = match file(p, fd) {
        Some(f) => f,
        None => return err(EBADF),
    };
    let len = len.min(1 << 24) as usize;
    match &f.node {
        #[cfg(feature = "proposals")]
        Node::Proposal => err(EINVAL), // a proposal is taken where the call is answered, before this
        Node::Null => len as u64,
        #[cfg(feature = "guest-ethernet")]
        Node::Ethernet => {
            if !f.writable || !p.holds(caps, Object::Ethernet, Rights::APPEND) { return err(EBADF); }
            if !(14..=dev::virtio_tx::guest::MTU).contains(&len) { return err(EINVAL); }
            let Some(bytes) = p.space.as_ref().and_then(|s| s.read(addr as usize, len)) else { return err(EFAULT) };
            if dev::virtio_tx::guest::send(&bytes).is_ok() { len as u64 } else { err(5) }
        }

        #[cfg(feature = "game-link")]
        Node::GameLink => {
            if !f.writable || !p.holds(caps, Object::GameLink, Rights::APPEND) { return err(EBADF); }
            let Some(bytes) = p.space.as_ref().and_then(|s| s.read(addr as usize, len.min(16))) else { return err(EFAULT) };
            let mut n = 0;
            for b in bytes { if !arch::game_link_write(b) { break; } n += 1; }
            if n == 0 && len > 0 { err(EAGAIN) } else { n }
        }
        Node::Console => {
            if !p.holds(caps, Object::Console, Rights::APPEND) {
                return err(EPERM);
            }
            let mut done = 0;
            while done < len {
                let n = (len - done).min(4096);
                let bytes = match p.space.as_ref().and_then(|s| s.read(addr as usize + done, n)) {
                    Some(b) => b,
                    None => return if done > 0 { done as u64 } else { err(EFAULT) },
                };
                crate::program_output(&bytes);
                p.wrote(&bytes);
                done += n;
            }
            len as u64
        }
        Node::Stored(file) => {
            if !f.writable || !p.holds(caps, Object::Store, Rights::APPEND) {
                return err(EBADF);
            }
            let bytes = match p.space.as_ref().and_then(|s| s.read(addr as usize, len.min(1 << 20))) {
                Some(b) => b,
                None => return err(EFAULT),
            };
            if bytes.is_empty() { return 0; }
            let at = if f.append { file.len() } else { f.position };
            let Some(end) = file.write(at, &bytes) else { return err(ENOSPC) };
            set_position(p, fd, end);
            bytes.len() as u64
        }
        Node::Music => {
            if !p.holds(caps, Object::Sound, Rights::APPEND) {
                return err(EPERM);
            }
            let n = len.min(1 << 14) & !7;
            match p.space.as_ref().and_then(|s| s.read(addr as usize, n)) {
                Some(bytes) => match dev::fm_write(&bytes) {
                    0 if n > 0 => err(EAGAIN),
                    taken => taken as u64,
                },
                None => err(EFAULT),
            }
        }
        Node::Sound => {
            if !p.holds(caps, Object::Sound, Rights::APPEND) {
                return err(EPERM);
            }
            // Whole pairs of samples only, and no more than the device will take now: the rest is the program's to offer again.
            let (room, _) = dev::sound_room();
            let n = len.min(room).min(1 << 16) & !3;
            if n == 0 {
                return if len < 4 { err(EINVAL) } else { err(EAGAIN) };
            }
            match p.space.as_ref().and_then(|s| s.read(addr as usize, n)) {
                Some(bytes) => dev::sound_write(&bytes) as u64,
                None => err(EFAULT),
            }
        }
        Node::File(_) | Node::Keys | Node::Directory => err(EBADF),
        Node::Display => err(EINVAL),
    }
}

/// `readv` and `writev`: one call for several pieces of memory.
fn vector(p: &mut Process, caps: &CapSpace, world: &mut World, fd: u64, iov: u64, count: u64, writing: bool) -> Outcome {
    let mut total = 0u64;
    for n in 0..count.min(16) {
        let (base, len) = match (word(p, iov + n * 16), word(p, iov + n * 16 + 8)) {
            (Some(b), Some(l)) => (b, l),
            _ => return Outcome::Return(err(EFAULT)),
        };
        if len == 0 {
            continue;
        }
        let done = if writing {
            write(p, caps, fd, base, len)
        } else {
            match read(p, world, fd, base, len) {
                Outcome::Return(v) => v,
                other => return if total > 0 { Outcome::Return(total) } else { other },
            }
        };
        if (done as i64) < 0 {
            return Outcome::Return(if total > 0 { total } else { done });
        }
        total += done;
        if done < len {
            break;
        }
    }
    Outcome::Return(total)
}

fn seek(p: &mut Process, world: &World, fd: u64, offset: u64, whence: u64) -> u64 {
    let f = match file(p, fd) {
        Some(f) => f,
        None => return err(EBADF),
    };
    let size = match &f.node {
        Node::File(i) => world.files.get(*i).map(|x| x.1.len()).unwrap_or(0),
        Node::Stored(file) => file.len(),
        _ => return err(ESPIPE),
    };
    let base = match whence {
        0 => 0i64,
        1 => f.position as i64,
        2 => size as i64,
        _ => return err(EINVAL),
    };
    match base.checked_add(offset as i64) {
        Some(to) if to >= 0 => {
            set_position(p, fd, to as usize);
            to as u64
        }
        _ => err(EINVAL),
    }
}

fn pages_of(len: usize) -> usize {
    (len + 4095) / 4096
}

/// Give the program `pages` pages of zeroed memory from `start`. On failure, what was given is taken back.
fn grow(p: &mut Process, start: usize, pages: usize, write: bool, exec: bool) -> bool {
    let space = match p.space.as_mut() {
        Some(s) => s,
        None => return false,
    };
    for n in 0..pages {
        let ok = match mem::take() {
            Some(frame) => {
                let mapped = space.map(start + n * 4096, frame, write, exec);
                if !mapped {
                    mem::give(frame);
                }
                mapped
            }
            None => false,
        };
        if !ok {
            for back in 0..n {
                if let Some(frame) = space.unmap(start + back * 4096) {
                    mem::give(frame);
                }
            }
            return false;
        }
    }
    if let Some(l) = p.linux.as_mut() {
        l.extra_pages += pages;
    }
    true
}

fn shrink(p: &mut Process, start: usize, pages: usize) {
    let mut freed = 0;
    if let Some(space) = p.space.as_mut() {
        for n in 0..pages {
            if let Some(frame) = space.unmap(start + n * 4096) {
                mem::give(frame);
                freed += 1;
            }
        }
    }
    if let Some(l) = p.linux.as_mut() {
        l.extra_pages = l.extra_pages.saturating_sub(freed);
    }
}

fn brk(p: &mut Process, to: u64) -> u64 {
    let (start, end) = match p.linux.as_ref() {
        Some(l) => (l.heap_start, l.heap_end),
        None => return 0,
    };
    let to = to as usize;
    if to < start || to > start + HEAP_LIMIT {
        return end as u64;
    }
    let (old_top, new_top) = ((end + 4095) & !4095, (to + 4095) & !4095);
    if new_top > old_top {
        if !grow(p, old_top, (new_top - old_top) / 4096, true, false) {
            return end as u64;
        }
    } else if new_top < old_top {
        shrink(p, new_top, (old_top - new_top) / 4096);
    }
    if let Some(l) = p.linux.as_mut() {
        l.heap_end = to;
    }
    to as u64
}

fn mmap(p: &mut Process, caps: &CapSpace, len: u64, prot: u64, flags: u64, fd: u64) -> u64 {
    let len = len as usize;
    if len == 0 || len > MAP_LIMIT || flags & 0x10 != 0 {
        return err(EINVAL); // a mapping at an address of the program's choosing is not offered
    }
    let (write, exec) = (prot & 2 != 0, prot & 4 != 0);
    if write && exec {
        return err(EACCES); // never both
    }
    let pages = pages_of(len);
    let start = match p.linux.as_mut() {
        Some(l) => {
            let s = l.next_map;
            l.next_map += (pages + 1) * 4096; // a page that is never mapped lies after every mapping
            s
        }
        None => return err(ENOSYS),
    };
    if flags & 0x20 != 0 {
        if prot == 0 {
            // Reserved and not yet backed: the program asks for the memory later, with `mprotect`.
            if let Some(l) = p.linux.as_mut() {
                l.reserved.push((start, start + pages * 4096));
            }
            return start as u64;
        }
        return if grow(p, start, pages, write, exec) { start as u64 } else { err(ENOMEM) };
    }
    // The only file that can be mapped is the display.
    match file(p, fd).map(|f| f.node) {
        Some(Node::Display) if p.holds(caps, Object::Display, Rights::APPEND) => {
            let d = match dev::display() {
                Some(d) => d,
                None => return err(EINVAL),
            };
            let bytes = d.pitch as usize * d.height as usize * d.pages as usize;
            if pages * 4096 > (bytes + 4095) & !4095 {
                return err(EINVAL);
            }
            // A program that asked for it, on a display whose memory cannot be given whole, draws to
            // memory of its own: the picture's size, once, and never the device.
            if shadow_wanted(p, &d) {
                if p.linux.as_ref().map(|l| l.shadow.is_some()).unwrap_or(true) || pages != pages_of(bytes) {
                    return err(EINVAL);
                }
                if !grow(p, start, pages, true, false) {
                    return err(ENOMEM);
                }
                if let Some(l) = p.linux.as_mut() {
                    l.shadow = Some(start);
                }
                return start as u64;
            }
            let space = match p.space.as_mut() {
                Some(s) => s,
                None => return err(EFAULT),
            };
            // All of it or none: a picture that ends partway through a page cannot be mapped whole.
            if !space.map_device(start, d.memory, pages) {
                return err(ENOMEM);
            }
            start as u64
        }
        Some(_) => err(EACCES),
        None => err(EBADF),
    }
}

fn mprotect(p: &mut Process, addr: u64, len: u64, prot: u64) -> u64 {
    let (addr, len) = (addr as usize, len as usize);
    if addr % 4096 != 0 || len > MAP_LIMIT {
        return err(EINVAL);
    }
    let (write, exec) = (prot & 2 != 0, prot & 4 != 0);
    if write && exec {
        return err(EACCES);
    }
    for n in 0..pages_of(len) {
        let page = addr + n * 4096;
        let changed = p.space.as_mut().map(|s| s.set_protection(page, write, exec)).unwrap_or(false);
        if !changed {
            // Not mapped: allowed only inside a range the program reserved, which is backed now.
            let inside = p.linux.as_ref().map(|l| l.reserved.iter().any(|&(s, e)| page >= s && page < e)).unwrap_or(false);
            if !inside || prot == 0 {
                if prot == 0 {
                    continue;
                }
                return err(ENOMEM);
            }
            if !grow(p, page, 1, write, exec) {
                return err(ENOMEM);
            }
        }
    }
    0
}

fn clock_ns(world: &World, clock: u64) -> u64 {
    let since_start = arch::ticks() * TICK_NS;
    if clock == 0 { world.epoch_at_start * 1_000_000_000 + since_start } else { since_start }
}

fn sleep(p: &Process, world: &World, request: u64, absolute_on: Option<u64>) -> Outcome {
    let (sec, nsec) = match (word(p, request), word(p, request + 8)) {
        (Some(s), Some(n)) => (s, n),
        _ => return Outcome::Return(err(EFAULT)),
    };
    if nsec >= 1_000_000_000 || sec > 1 << 32 {
        return Outcome::Return(err(EINVAL));
    }
    let mut ns = sec * 1_000_000_000 + nsec;
    if let Some(clock) = absolute_on {
        ns = ns.saturating_sub(clock_ns(world, clock));
    }
    if ns == 0 {
        return Outcome::Yield;
    }
    Outcome::Wait(Waiting::Until(arch::ticks() + (ns + TICK_NS - 1) / TICK_NS), false)
}

/// Whether this display's memory cannot be mapped whole into a program: one page, ending partway
/// through a page of memory (a loader's framebuffer of 800 by 600 is 1,920,000 bytes).
fn display_needs_shadow(d: &dev::Display) -> bool {
    let bytes = d.pitch as usize * d.height as usize;
    d.pages == 1 && !arch::device_grantable(d.memory, pages_of(bytes))
}

fn shadow_wanted(p: &Process, d: &dev::Display) -> bool {
    p.linux.as_ref().map(|l| l.shadow_asked).unwrap_or(false) && display_needs_shadow(d)
}

/// Copy a program's own picture to the display, row by row: only the pixels of the picture, through
/// the kernel's bounded write. False if any row is not the program's memory or would leave the device.
fn shadow_present(p: &Process, from: usize, d: &dev::Display) -> bool {
    let space = match p.space.as_ref() {
        Some(s) => s,
        None => return false,
    };
    let mut bytes = alloc::vec![0u8; d.width as usize * 4];
    let mut row = alloc::vec![0u32; d.width as usize];
    for y in 0..d.height as usize {
        if !space.copy_in(from + y * d.pitch as usize, &mut bytes) {
            return false;
        }
        for (word, four) in row.iter_mut().zip(bytes.chunks_exact(4)) {
            *word = u32::from_le_bytes([four[0], four[1], four[2], four[3]]);
        }
        if !arch::device_write32(d.memory + y * d.pitch as usize, &row) {
            return false;
        }
    }
    true
}

fn display_call(p: &mut Process, caps: &CapSpace, request: u64, arg: u64) -> u64 {
    if !p.holds(caps, Object::Display, Rights::APPEND) {
        return err(EACCES);
    }
    let describe = |d: dev::Display| -> [u8; 24] {
        let mut b = [0u8; 24];
        for (n, v) in [d.width, d.height, d.pitch, d.pages, d.shown, (d.kind == dev::DisplayKind::Bochs) as u32].iter().enumerate() {
            b[n * 4..n * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    };
    match request {
        0x5001 => match dev::display() {
            Some(d) if put(p, arg, &describe(d)) => 0,
            Some(_) => err(EFAULT),
            None => err(EINVAL),
        },
        0x5002 => {
            let want = match get::<8>(p, arg) {
                Some(w) => w,
                None => return err(EFAULT),
            };
            let (w, h) = (u32::from_le_bytes([want[0], want[1], want[2], want[3]]), u32::from_le_bytes([want[4], want[5], want[6], want[7]]));
            match dev::display_set(w, h) {
                Some(d) if put(p, arg, &describe(d)) => 0,
                Some(_) => err(EFAULT),
                None => err(EINVAL),
            }
        }
        0x5003 => {
            // A program that asked to draw through the kernel, on a display that needs it, shows only
            // what it has drawn in the memory it mapped for that. Before the map there is nothing to
            // show, and it is not let through to the device's own page (B3's fixture found it was).
            if let Some(d) = dev::display().filter(|d| shadow_wanted(p, d)) {
                let from = match p.linux.as_ref().and_then(|l| l.shadow) {
                    Some(from) if arg == 0 => from,
                    _ => return err(EINVAL),
                };
                if !shadow_present(p, from, &d) {
                    return err(EINVAL);
                }
                dev::display_count_show();
                return 0;
            }
            if dev::display_show(arg as u32) {
                dev::display_count_show();
                0
            } else {
                err(EINVAL)
            }
        }
        // Opt in to drawing through the kernel where the display cannot be mapped whole. 0: draw
        // directly, as before. 1: map the display as usual and show page 0 after every picture.
        0x5004 => {
            if arg != 0 {
                return err(EINVAL);
            }
            match dev::display() {
                Some(d) => {
                    let needed = display_needs_shadow(&d);
                    if let Some(l) = p.linux.as_mut() {
                        l.shadow_asked = true;
                    }
                    needed as u64
                }
                None => err(EINVAL),
            }
        }
        _ => err(ENOTTY),
    }
}

/// Answer one call of process `i`.
pub fn call(t: &mut Table, i: usize, frame: &Frame) -> Outcome {
    let (number, a) = frame.linux_call();
    let Table { procs, caps, world, fs_loaded, .. } = t;
    let p = &mut procs[i];
    let ret = Outcome::Return;
    match number {
        0 => read(p, world, a[0], a[1], a[2]),
        #[cfg(feature = "proposals")]
        1 if file(p, a[0]).is_some_and(|f| matches!(f.node, Node::Proposal)) => propose(p, world, a[1], a[2]),
        1 => ret(write(p, caps, a[0], a[1], a[2])),
        2 => ret(open(p, caps, world, a[0], a[1])),
        257 => ret(open(p, caps, world, a[1], a[2])), // openat: every path is taken from the one directory
        3 => ret(match p.linux.as_mut().and_then(|l| l.files.get_mut(a[0] as usize)) {
            Some(slot) if slot.is_some() => {
                *slot = None;
                0
            }
            _ => err(EBADF),
        }),
        4 | 6 | 262 => {
            // stat, lstat, fstatat: what a path names.
            let (path, buf) = if number == 262 { (a[1], a[2]) } else { (a[0], a[1]) };
            ret(match text(p, path).and_then(|t| lookup(p, caps, world, &t)) {
                Ok(node) => match node_stat(world, &node) {
                    Some(s) if put(p, buf, &s) => 0,
                    _ => err(EFAULT),
                },
                Err(e) => e,
            })
        }
        5 => ret(match file(p, a[0]).and_then(|f| node_stat(world, &f.node)) {
            Some(s) => if put(p, a[1], &s) { 0 } else { err(EFAULT) },
            None => err(EBADF),
        }),
        8 => ret(seek(p, world, a[0], a[1], a[2])),
        9 => ret(mmap(p, caps, a[1], a[2], a[3], a[4])),
        10 => ret(mprotect(p, a[0], a[1], a[2])),
        11 => {
            if a[0] % 4096 == 0 && a[1] <= MAP_LIMIT as u64 {
                shrink(p, a[0] as usize, pages_of(a[1] as usize));
                ret(0)
            } else {
                ret(err(EINVAL))
            }
        }
        12 => ret(brk(p, a[0])),
        13 | 14 | 131 => ret(0), // signal handlers, the signal mask, an alternate stack: accepted and without effect, since no signal is ever delivered
        16 => ret(match file(p, a[0]).map(|f| f.node) {
            Some(Node::Display) => display_call(p, caps, a[1], a[2]),
            Some(Node::Console) if a[1] == 0x5413 => {
                // "How large is this terminal?" A C library asks in order to learn that it is one, and then
                // sends what a program prints line by line and not in blocks that may never be sent.
                let size: [u8; 8] = [25, 0, 80, 0, 0, 0, 0, 0];
                if put(p, a[2], &size) { 0 } else { err(EFAULT) }
            }
            Some(Node::Sound) if a[1] == 0x5011 => {
                // What the sound device plays, how much it will take now and how much is waiting, in bytes.
                let (room, waiting) = dev::sound_room();
                let mut b = [0u8; 20];
                for (n, v) in [dev::SOUND_RATE, 2, 16, room as u32, waiting as u32].iter().enumerate() {
                    b[n * 4..n * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
                if put(p, a[2], &b) { 0 } else { err(EFAULT) }
            }
            Some(_) => err(ENOTTY),
            None => err(EBADF),
        }),
        19 => vector(p, caps, world, a[0], a[1], a[2], false),
        20 => vector(p, caps, world, a[0], a[1], a[2], true),
        21 | 269 => {
            let path = if number == 269 { a[1] } else { a[0] };
            ret(match text(p, path).and_then(|t| lookup(p, caps, world, &t)) {
                Ok(_) => 0,
                Err(e) => e,
            })
        }
        24 => Outcome::Yield,
        25 => ret(err(ENOMEM)), // moving a mapping is not offered; a C library falls back to a new one
        28 => ret(0),           // advice about memory: accepted and ignored
        35 => sleep(p, world, a[0], None),
        230 => sleep(p, world, a[2], if a[1] & 1 != 0 { Some(a[0]) } else { None }),
        39 | 186 | 218 => ret(p.pid as u64),
        102 | 104 | 107 | 108 | 110 => ret(0), // there are no users: every identity is zero
        60 | 231 => Outcome::Exit(a[0] & 0xff),
        62 | 200 | 234 => Outcome::Exit(128 + (if number == 234 { a[2] } else { a[1] } & 0x7f)), // a signal the program sends itself ends it
        63 => {
            let mut b = [0u8; 390];
            for (n, s) in ["PCPOS", "pcpos", env!("CARGO_PKG_VERSION"), "PCP OS kernel", "x86_64", ""].iter().enumerate() {
                b[n * 65..n * 65 + s.len()].copy_from_slice(s.as_bytes());
            }
            ret(if put(p, a[0], &b) { 0 } else { err(EFAULT) })
        }
        72 => ret(match p.linux.as_mut().and_then(|l| l.files.get_mut(a[0] as usize)).and_then(|f| f.as_mut()) {
            Some(f) => match a[1] {
                3 => (f.nonblocking as u64) << 11 | (f.writable as u64),
                4 => {
                    f.nonblocking = a[2] & 0x800 != 0;
                    0
                }
                _ => 0,
            },
            None => err(EBADF),
        }),
        #[cfg(feature = "vm-store")]
        74 => {
            let Some(volume) = world.volume.as_mut() else { return ret(err(ENOSYS)); };
            match file(p, a[0]) {
                None => return ret(err(EBADF)),
                Some(open) if !matches!(open.node, Node::Stored(_) | Node::Directory) =>
                    return ret(err(EINVAL)),
                _ => {}
            }
            if !p.holds(caps, Object::Store, Rights::APPEND) { return ret(err(EACCES)); }
            ret(match volume.commit(&world.store) {
                Ok(()) => 0,
                Err(crate::store::Failure::Full) => err(ENOSPC),
                Err(crate::store::Failure::Io) => err(EIO),
            })
        }
        79 => ret(if a[1] >= 2 && put(p, a[0], b"/\0") { 2 } else { err(EINVAL) }),
        82 => {
            // rename: of a file in the store.
            let (from, to) = match (text(p, a[0]).ok().and_then(|t| name_of(&t)), text(p, a[1]).ok().and_then(|t| name_of(&t))) {
                (Some(f), Some(t)) if !t.is_empty() && !t.starts_with("dev/") => (f, t),
                _ => return ret(err(ENOENT)),
            };
            if !p.holds(caps, Object::Store, Rights::APPEND) {
                return ret(err(EACCES));
            }
            ret(if world.store.rename(&from, to) { 0 } else { err(ENOENT) })
        }
        83 | 258 => ret(if p.holds(caps, Object::Store, Rights::APPEND) { 0 } else { err(EACCES) }), // mkdir: names may contain slashes; there is nothing to make
        87 | 263 => {
            let path = if number == 263 { a[1] } else { a[0] };
            let name = match text(p, path).ok().and_then(|t| name_of(&t)) {
                Some(n) => n,
                None => return ret(err(ENOENT)),
            };
            if !p.holds(caps, Object::Store, Rights::APPEND) {
                return ret(err(EACCES));
            }
            ret(if world.store.unlink(&name) { 0 } else { err(ENOENT) })
        }
        96 => {
            let ns = clock_ns(world, 0);
            let mut b = [0u8; 16];
            b[..8].copy_from_slice(&(ns / 1_000_000_000).to_le_bytes());
            b[8..].copy_from_slice(&(ns % 1_000_000_000 / 1000).to_le_bytes());
            ret(if a[0] == 0 || put(p, a[0], &b) { 0 } else { err(EFAULT) })
        }
        201 => {
            let s = clock_ns(world, 0) / 1_000_000_000;
            ret(if a[0] == 0 || put(p, a[0], &s.to_le_bytes()) { s } else { err(EFAULT) })
        }
        228 => {
            let ns = clock_ns(world, a[0]);
            let mut b = [0u8; 16];
            b[..8].copy_from_slice(&(ns / 1_000_000_000).to_le_bytes());
            b[8..].copy_from_slice(&(ns % 1_000_000_000).to_le_bytes());
            ret(if put(p, a[1], &b) { 0 } else { err(EFAULT) })
        }
        158 => match a[0] {
            0x1002 => {
                // Where the C library keeps its per-thread data.
                if arch::set_user_fs(a[1]) {
                    p.fs_base = a[1];
                    *fs_loaded = a[1];
                    ret(0)
                } else {
                    ret(err(EPERM))
                }
            }
            0x1003 => ret(if put(p, a[1], &p.fs_base.to_le_bytes()) { 0 } else { err(EFAULT) }),
            _ => ret(err(EINVAL)),
        },
        other => {
            match world.unanswered.iter().position(|u| u.0 == other) {
                Some(at) => world.unanswered[at].1 += 1,
                None if world.unanswered.len() < 64 => world.unanswered.push((other, 1)),
                None => {}
            }
            ret(err(ENOSYS))
        }
    }
}

/// The largest stack a loaded program has: for the kernel's report.
pub fn stack_bytes() -> usize {
    exec::STACK_PAGES * 4096
}
