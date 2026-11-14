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
const EBADF: i64 = 9;
const EAGAIN: i64 = 11;
const ENOMEM: i64 = 12;
const EACCES: i64 = 13;
const EFAULT: i64 = 14;
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
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
    /// A file a program wrote: its name in `World::store`.
    Stored(String),
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
}

/// The most the `brk` heap may grow to, the most one mapping may be, and the most the store may hold.
const HEAP_LIMIT: usize = 1 << 30;
const MAP_LIMIT: usize = 1 << 30;
const STORE_LIMIT: usize = 16 << 20;
const MAX_FILES: usize = 32;
/// Nanoseconds in one timer tick: 1193 / 1193182 of a second.
const TICK_NS: u64 = 999_847;

impl Linux {
    pub fn new(image_end: usize) -> Linux {
        let console = || Some(Open { node: Node::Console, position: 0, nonblocking: false, writable: true, append: false });
        Linux { files: alloc::vec![console(), console(), console()], heap_start: image_end, heap_end: image_end, next_map: arch::USER_BASE + (64 << 30), reserved: Vec::new(), extra_pages: 0 }
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
fn stat_bytes(mode: u32, size: usize) -> [u8; 144] {
    let mut s = [0u8; 144];
    s[16..24].copy_from_slice(&1u64.to_le_bytes()); // one name
    s[24..28].copy_from_slice(&mode.to_le_bytes());
    s[48..56].copy_from_slice(&(size as u64).to_le_bytes());
    s[56..64].copy_from_slice(&4096u64.to_le_bytes());
    s[64..72].copy_from_slice(&(((size + 511) / 512) as u64).to_le_bytes());
    s
}

fn node_stat(world: &World, node: &Node) -> Option<[u8; 144]> {
    Some(match node {
        #[cfg(feature = "guest-ethernet")]
        Node::Ethernet => stat_bytes(0o020660, 0),
        Node::Console | Node::Display | Node::Keys | Node::Sound | Node::Music | Node::Null => stat_bytes(0o020666, 0),
        #[cfg(feature = "game-link")]
        Node::GameLink => stat_bytes(0o020666, 0),
        Node::Directory => stat_bytes(0o040755, 0),
        Node::File(i) => stat_bytes(0o100444, world.files.get(*i)?.1.len()),
        Node::Stored(n) => stat_bytes(0o100644, world.store.iter().find(|f| &f.0 == n)?.1.len()),
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
        "dev/console" | "dev/tty" => need(Object::Console, Rights::APPEND, Node::Console),
        "dev/fb0" => need(Object::Display, Rights::APPEND, Node::Display),
        "dev/kbd" => need(Object::Keys, Rights::READ, Node::Keys),
        "dev/audio" => need(Object::Sound, Rights::APPEND, Node::Sound),
        "dev/fm" if dev::fm_state().0 => need(Object::Sound, Rights::APPEND, Node::Music),
        _ => {
            if world.store.iter().any(|f| f.0 == name) {
                return need(Object::Store, Rights::READ, Node::Stored(name));
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
    let node = match lookup(p, caps, world, &path) {
        Ok(Node::File(_)) if writing => return err(EACCES), // the loader's files are read and never written
        Ok(n) => n,
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
            world.store.push((name.clone(), Vec::new()));
            Node::Stored(name)
        }
        Err(e) => return e,
    };
    if let Node::Stored(name) = &node {
        if writing && !p.holds(caps, Object::Store, Rights::APPEND) {
            return err(EACCES);
        }
        if writing && truncate {
            if let Some(f) = world.store.iter_mut().find(|f| &f.0 == name) {
                f.1 = Vec::new();
            }
        }
    }
    let linux = match p.linux.as_mut() {
        Some(l) => l,
        None => return err(ENOSYS),
    };
    let entry = Open { node, position: 0, nonblocking: flags & 0x800 != 0, writable: writing, append: flags & 0x400 != 0 };
    match linux.files.iter().position(|f| f.is_none()) {
        Some(fd) => {
            linux.files[fd] = Some(entry);
            fd as u64
        }
        None if linux.files.len() < MAX_FILES => {
            linux.files.push(Some(entry));
            (linux.files.len() - 1) as u64
        }
        None => err(EMFILE),
    }
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
    let bytes: &[u8] = match &f.node {
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
        Node::Stored(name) => match world.store.iter().find(|s| &s.0 == name) {
            Some(file) => &file.1,
            None => return Outcome::Return(0), // removed while open: nothing more to read
        },
    };
    let from = f.position.min(bytes.len());
    let n = len.min(bytes.len() - from);
    if !put(p, addr, &bytes[from..from + n]) {
        return Outcome::Return(err(EFAULT));
    }
    set_position(p, fd, from + n);
    Outcome::Return(n as u64)
}

fn write(p: &mut Process, caps: &CapSpace, world: &mut World, fd: u64, addr: u64, len: u64) -> u64 {
    let f = match file(p, fd) {
        Some(f) => f,
        None => return err(EBADF),
    };
    let len = len.min(1 << 24) as usize;
    match &f.node {
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
        Node::Stored(name) => {
            if !f.writable || !p.holds(caps, Object::Store, Rights::APPEND) {
                return err(EBADF);
            }
            let bytes = match p.space.as_ref().and_then(|s| s.read(addr as usize, len.min(1 << 20))) {
                Some(b) => b,
                None => return err(EFAULT),
            };
            let total: usize = world.store.iter().map(|s| s.1.len()).sum();
            let file = match world.store.iter_mut().find(|s| &s.0 == name) {
                Some(file) => file,
                None => return err(EBADF),
            };
            let at = if f.append { file.1.len() } else { f.position };
            let end = at + bytes.len();
            if end > file.1.len() && total + (end - file.1.len()) > STORE_LIMIT {
                return err(ENOSPC);
            }
            if end > file.1.len() {
                file.1.resize(end, 0);
            }
            file.1[at..end].copy_from_slice(&bytes);
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
            write(p, caps, world, fd, base, len)
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
        Node::Stored(n) => world.store.iter().find(|s| &s.0 == n).map(|s| s.1.len()).unwrap_or(0),
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
            let space = match p.space.as_mut() {
                Some(s) => s,
                None => return err(EFAULT),
            };
            for n in 0..pages {
                if !space.map_device(start + n * 4096, d.memory + n * 4096) {
                    return err(ENOMEM);
                }
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

fn display_call(p: &Process, caps: &CapSpace, request: u64, arg: u64) -> u64 {
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
            if dev::display_show(arg as u32) {
                dev::display_count_show();
                0
            } else {
                err(EINVAL)
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
        1 => ret(write(p, caps, world, a[0], a[1], a[2])),
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
            if !world.store.iter().any(|f| f.0 == from) {
                return ret(err(ENOENT));
            }
            world.store.retain(|f| f.0 != to);
            if let Some(f) = world.store.iter_mut().find(|f| f.0 == from) {
                f.0 = to;
            }
            ret(0)
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
            let before = world.store.len();
            world.store.retain(|f| f.0 != name);
            ret(if world.store.len() < before { 0 } else { err(ENOENT) })
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
