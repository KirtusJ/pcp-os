//! The PCP OS kernel. Early skeleton: it starts, protects its own memory and proves that the protection
//! is enforced, takes over exceptions and the timer, runs the integrity core and the native ACR guards on
//! the machine, runs programs in user mode as processes it can preempt and stop, keeps a register of its own
//! start, verification, processes and stop, reports, and stops. It has no drivers beyond a serial port and
//! a timer, no storage and no means of loading a program from outside its own image. See README.md.
#![no_std]
#![no_main]
#![deny(unsafe_code)]

extern crate alloc;

mod arch;
mod dev;
mod exec;
mod font;
mod mem;
#[cfg(feature = "virtio-worker-probe")]
mod worker_wire;
#[cfg(feature = "virtio-register-probe")]
mod register_wire;
mod session;
mod sys;
mod ui;
mod user;

use pcpos_core::acr::{self, Record};
use pcpos_core::authority::{authority_of, Edition, Kind, Roles};
use pcpos_core::cap::{CapError, CapSpace, CapTable, Rights};
use pcpos_core::ledger::{self, Ledger};
use pcpos_core::machine::{self, System, SystemEvent, Task, TaskEvent};
use pcpos_core::task::{TaskTable, Worker};
use pcpos_core::value::Value;
use pcpos_core::{r1, sha256_hex};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::{self, Write};

#[global_allocator]
static ALLOCATOR: arch::Heap = arch::Heap;

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                arch::serial_put(b'\r');
            }
            arch::serial_put(b);
        }
        Ok(())
    }
}

macro_rules! say {
    ($($arg:tt)*) => {{ let _ = writeln!(Console, $($arg)*); }};
}

/// One line on the serial port, for the other files of the kernel.
pub fn log(args: fmt::Arguments) {
    let _ = Console.write_fmt(args);
    let _ = Console.write_str("\n");
}

/// What a program writes to the console goes to the serial port as it is.
pub fn program_output(bytes: &[u8]) {
    for b in bytes {
        if *b == b'\n' {
            arch::serial_put(b'\r');
        }
        arch::serial_put(*b);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    say!("KERNEL STOPPED: {}", info);
    arch::stop(false)
}

/// Called by `arch` for an exception nothing was prepared for. Report and stop: a fault in the kernel is
/// never continued past.
pub fn fault(vector: u64, error: u64, rip: u64, address: u64) -> ! {
    say!("KERNEL STOPPED: exception {} (error code {:#x}) at instruction {:#x}, address {:#x}", vector, error, rip, address);
    let guard = arch::stack_guard();
    if (address as usize) >= guard && (address as usize) < guard + 4096 {
        say!("the address is in the unmapped page below the kernel stack{}", if vector == 8 { ", and the fault could not be delivered there: the stack is exhausted" } else { "" });
    }
    arch::stop(false)
}

/// The vectors written by the native implementation, and a register written by the Python implementation.
const MACHINES: &str = include_str!("../../../vectors/machines.tsv");
const SCENARIO_REGISTER: &[u8] = include_bytes!("../../../vectors/register_scenario.jsonl");
const SCENARIO_EXPECTED: &[u8] = include_bytes!("../../../vectors/register_scenario.expected.json");
/// A register written by the Python implementation on another operating system, and the head it reported.
const REFERENCE_REGISTER: &[u8] = include_bytes!("../../../vectors/register_reference.jsonl");
const REFERENCE_HEAD: &str = "df54563c8abc6187b9df104b13e4665b82436ee466884212c08e9765cc07287a";
const EXERCISE_REGISTER: &[u8] = include_bytes!("../../../vectors/register_fphpm_exercise.jsonl");
const EXERCISE_HEAD: &str = "836469bc0d3893695c38e501a9846d5153ad49846d6a9e33219e137278f31c57";

pub(crate) fn logged<T, E: core::fmt::Debug>(what: &str, r: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &r {
        say!("  note the register refused {}: {:?}", what, e);
    }
    r
}

static PASSED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

fn check(name: &str, ok: bool, failed: &mut u32) {
    say!("  {} {}", if ok { "pass" } else { "FAIL" }, name);
    if ok {
        PASSED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    } else {
        *failed += 1;
    }
}

fn self_test() -> u32 {
    let mut failed = 0;

    // 1. The kernel reads another implementation's records exactly as that implementation does.
    let a = ledger::verify(REFERENCE_REGISTER);
    check("a register written by the Python implementation verifies to the same head",
          matches!(&a, Ok((ev, head)) if ev.len() == 4 && head == REFERENCE_HEAD), &mut failed);
    let b = ledger::verify(EXERCISE_REGISTER);
    check("the register of the research-baseline exercise verifies to the same head",
          matches!(&b, Ok((ev, head)) if ev.len() == 13 && head == EXERCISE_HEAD), &mut failed);

    // 2. History cannot be edited without detection.
    let mut edited = REFERENCE_REGISTER.to_vec();
    if let Some(at) = edited.windows(6).position(|w| w == b"second") {
        edited[at] = b'S';
    }
    check("one changed byte in an earlier event is refused", ledger::verify(&edited).is_err(), &mut failed);
    check("an incomplete final line is refused", ledger::verify(&REFERENCE_REGISTER[..REFERENCE_REGISTER.len() - 1]).is_err(), &mut failed);

    // 3. Appending in the kernel yields bytes any other implementation accepts.
    let mut l = Ledger::new();
    let mut body = BTreeMap::new();
    body.insert("text".to_string(), Value::Str("written inside the kernel".to_string()));
    let appended = l.append("2026-11-12T00:00:00Z", "kernel", "HOST", "NOTE", Value::Map(body)).is_ok()
        && l.append("2026-11-12T00:00:01Z", "kernel", "HOST", "NOTE", Value::Map(BTreeMap::new())).is_ok();
    check("the kernel appends events and its own register verifies",
          appended && matches!(ledger::verify(l.bytes()), Ok((ev, head)) if ev.len() == 2 && head == l.head()), &mut failed);
    check("an event with a malformed time or an undefined authority is refused and nothing is written",
          l.append("yesterday", "kernel", "HOST", "NOTE", Value::Map(BTreeMap::new())).is_err()
              && l.append("2026-11-12T00:00:02Z", "kernel", "ROOT", "NOTE", Value::Map(BTreeMap::new())).is_err() && l.len() == 2, &mut failed);

    // 4. Capabilities narrow and never widen.
    let mut t = CapTable::new(16);
    let ok = (|| -> Result<bool, CapError> {
        let root = t.install(1, Rights::READ.with(Rights::APPEND).with(Rights::GRANT))?;
        let reader = t.derive(root, Rights::READ)?;
        let mid = t.derive(root, Rights::READ.with(Rights::GRANT))?;
        let leaf = t.derive(mid, Rights::READ)?;
        Ok(t.check(reader, Rights::APPEND) == Err(CapError::Denied)
            && t.derive(root, Rights::DIRECT) == Err(CapError::WouldWiden)
            && t.derive(reader, Rights::READ) == Err(CapError::Denied)
            && t.revoke(mid) == Ok(2)
            && t.check(leaf, Rights::READ) == Err(CapError::Invalid)
            && t.check(reader, Rights::READ) == Ok(1))
    })();
    check("a derived capability has the same rights or fewer, and revocation reaches everything derived", ok == Ok(true), &mut failed);

    // 5. The draft compact form decodes to exactly what was encoded.
    let mut json = 0usize;
    let mut compact = 0usize;
    let mut round = true;
    if let Ok((events, _)) = &b {
        for ev in events {
            let enc = r1::encode(ev);
            round &= r1::decode(&enc).as_ref() == Ok(ev);
            json += ev.canonical_json().len() + 1;
            compact += r1::encode_event(ev).map(|e| e.len()).unwrap_or(usize::MAX / 2);
        }
    }
    check("the draft compact record form round-trips every event", round && compact < json, &mut failed);
    say!("  note the same 13 events: {} bytes as JSON, {} bytes in the draft compact form", json, compact);

    // 6. The state machines in this image are well formed and are the tables of the vectors.
    let mut tables = String::new();
    let exported = machine::export(&mut tables).is_ok();
    check("every state machine is a complete, well-formed table, equal to the published tables",
          exported && tables == MACHINES && machine::ALL.iter().all(|m| m.defects() == 0), &mut failed);
    check("from every running state an integrity fault leads to SAFE, and SAFE is left only by stopping",
          System::ALL.iter().all(|st| match st { System::Stopped => true, System::Stopping => st.step(SystemEvent::IntegrityFault) == Ok(System::Stopped),
                                                 _ => st.step(SystemEvent::IntegrityFault) == Ok(System::Safe) })
              && SystemEvent::ALL.iter().all(|e| matches!(System::Safe.step(*e), Err(_) | Ok(System::Safe) | Ok(System::Stopping))), &mut failed);
    check("a task becomes DONE only from EXITED by a verified result",
          Task::ALL.iter().all(|st| TaskEvent::ALL.iter().all(|e| st.step(*e) != Ok(Task::Done) || (*st == Task::Exited && *e == TaskEvent::ResultVerified))), &mut failed);

    // 7. ACR, native: the guards replay a register that uses every kind of event, and find nothing.
    let replay = ledger::verify(SCENARIO_REGISTER).ok().and_then(|(ev, head)| acr::fold(&ev, true).ok().map(|(st, problems)| (st.events(), problems.len(), head)));
    let want = Value::parse(SCENARIO_EXPECTED).ok().and_then(|v| v.get("head").and_then(Value::as_str).map(|h| h.to_string()));
    check("the native ACR guards replay the scenario register (82 events, every kind) without a problem",
          matches!((&replay, &want), (Some((82, 0, head)), Some(w)) if head == w), &mut failed);

    // 8. Authority: exhaustive over kinds of identity, sets of roles and editions.
    let forbidden = Rights::DIRECT.with(Rights::ADMIN).with(Rights::SYSTEM);
    let mut ok = true;
    for edition in [Edition::Personal, Edition::Commercial, Edition::Enterprise] {
        for kind in [Kind::Program, Kind::Model] {
            for bits in 0u16..128 {
                if let Ok(a) = authority_of(kind, Roles(bits), edition) {
                    ok &= a.union().0 & forbidden.0 == 0;
                }
            }
        }
    }
    check("no combination of roles hands a model or a program the means to start ACR use, administer or change the system", ok, &mut failed);
    let mut sp = CapSpace::new(16);
    let ok = (|| -> Result<bool, CapError> {
        let root = sp.install(1, 9, Rights::APPEND.with(Rights::GRANT))?;
        let reg = sp.grant(1, root, 2, Rights::APPEND.with(Rights::GRANT))?;
        let agent = sp.grant(2, reg, 3, Rights::APPEND)?;
        Ok(sp.check(4, agent, Rights::APPEND) == Err(CapError::Invalid) && sp.revoke(3, reg) == Err(CapError::Invalid)
            && sp.revoke(1, reg) == Ok(2) && sp.check(3, agent, Rights::APPEND) == Err(CapError::Invalid) && sp.check(1, root, Rights::APPEND) == Ok(9))
    })();
    check("revoking a grant takes back what was handed on from it, in every holder", ok == Ok(true), &mut failed);
    failed
}

pub(crate) fn now() -> String {
    let (y, mo, d, h, mi, s) = arch::clock();
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

pub(crate) fn obj(pairs: &[(&str, Value)]) -> Value {
    Value::Map(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

pub(crate) fn text(t: &str) -> Value {
    Value::Str(t.to_string())
}

/// Record a transition of the system in the kernel's own register. The guard holds the transition to the
/// SYSTEM table; a transition the table does not have is refused and reported.
fn system_event(rec: &mut Record, event: &str, reason: &str, failed: &mut u32) {
    let body = obj(&[("event", text(event)), ("reason", text(reason))]);
    if let Err(e) = rec.submit(&now(), "host", "HOST", "SYSTEM_STATE", body) {
        say!("  FAIL the system event {} was not recorded: {:?}", event, e);
        *failed += 1;
    }
}

/// Memory protection, observed: each access is attempted and the processor's answer is the result.
fn protection_probes(failed: &mut u32) {
    use arch::Probe::Refused;
    let [text_start, _te, ro_start, _re, _end] = arch::layout();
    let w = arch::probe_write_same(text_start);
    check("a write to kernel code is refused by the processor", matches!(w, Refused(e) if e & 3 == 3), failed);
    let w = arch::probe_write_same(ro_start);
    check("a write to read-only data is refused by the processor", matches!(w, Refused(e) if e & 3 == 3), failed);
    let x = arch::probe_execute_data();
    check("executing an instruction that lies in writable data is refused by the processor", matches!(x, Refused(e) if e & 0x10 != 0), failed);
    let r = arch::probe_read_at(8);
    check("a read near address zero is refused by the processor", matches!(r, Refused(e) if e & 1 == 0), failed);
    static CONTROL: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(7);
    let c = arch::probe_write_same(core::ptr::addr_of!(CONTROL) as usize);
    check("control: a write to ordinary data is allowed, so the probe can tell the difference", c == arch::Probe::Allowed, failed);
    let w = arch::probe_write_same(arch::interrupt_table());
    check("a write to the interrupt table is refused by the processor", matches!(w, Refused(e) if e & 3 == 3), failed);
    let w = arch::probe_write_same(arch::segment_tables());
    check("a write to the segment table and task state segment is refused by the processor", matches!(w, Refused(e) if e & 3 == 3), failed);
    let all = arch::guard_pages().iter().all(|g| matches!(arch::probe_read_at(*g), Refused(e) if e & 1 == 0));
    check("a read of the page below each of the six stacks is refused by the processor", all, failed);
    let (_, pool_end) = arch::pool();
    let outside = [0xB8000usize, 0x9FC00, pool_end, pool_end + (2 << 20), (1 << 30) - 4096, 0xFD00_0000, (4 << 30) - 4096];
    check("memory the kernel does not use is not mapped: the text display, the low mebibyte, what lies above the frame memory, and where devices are",
          outside.iter().all(|a| matches!(arch::probe_read_at(*a), Refused(e) if e & 1 == 0)), failed);
    let frame = mem::take();
    let (wr, ex) = (frame.map(arch::probe_write_same), frame.map(arch::probe_execute_at));
    if let Some(f) = frame {
        mem::give(f);
    }
    check("frame memory is writable by the kernel and an instruction in it is refused by the processor",
          wr == Some(arch::Probe::Allowed) && matches!(ex, Some(Refused(e)) if e & 0x10 != 0), failed);
}

/// The timer and the watchdog: of two tasks, one reports each tick and one is silent.
fn watchdog(failed: &mut u32) {
    arch::interrupts_on();
    let start = arch::ticks();
    let advanced = arch::wait_until(start + 2);
    check("the timer interrupts and the tick count advances", advanced, failed);
    let mut tasks: TaskTable<8> = TaskTable::new();
    let t0 = arch::ticks();
    let (a, b) = (tasks.admit("reports", Worker::Program, 0, t0, 3, 100), tasks.admit("silent", Worker::Model, 0, t0, 3, 100));
    let mut ok = advanced && a.is_ok() && b.is_ok();
    if let (Ok(a), Ok(b)) = (a, b) {
        ok &= tasks.event(a, TaskEvent::Start, t0).is_ok() && tasks.event(b, TaskEvent::Start, t0).is_ok();
        for n in 1..=6 {
            ok &= arch::wait_until(t0 + n);
            ok &= tasks.event(a, TaskEvent::Heartbeat, arch::ticks()).is_ok();
            tasks.sweep(arch::ticks());
        }
        ok &= tasks.get(a).map(|r| r.state) == Some(Task::Running) && tasks.get(b).map(|r| r.state) == Some(Task::Lost);
        // A task's own report of success is not completion; the result read back decides.
        ok &= tasks.exit_ok(a, [1; 32], arch::ticks()) == Ok(Task::Exited) && tasks.settle(a, Some([2; 32])) == Ok(Task::Failed);
        ok &= tasks.attention() == (1, 1, 0);
    }
    arch::interrupts_off();
    check("a task not heard from within its quiet limit is LOST at the next sweep; one that reports is not; a claimed result that differs is FAILED", ok, failed);
}


/// The frame pool and the heap: what is handed out is handed out once, comes back, and is clean.
fn memory_tests(failed: &mut u32) {
    let (frames, free0, _, _) = mem::census();
    let a = mem::take();
    let b = mem::take();
    let distinct = matches!((a, b), (Some(x), Some(y)) if x != y);
    let (lo, hi) = arch::pool();
    let inside = [a, b].iter().all(|f| matches!(f, Some(x) if *x >= lo && *x < hi && x % 4096 == 0));
    // A frame is dirtied, given back and taken again: it comes back zeroed. Every other frame of the pool is
    // taken on the way, so this also shows that the pool hands out each frame once and then has none left.
    let dirty = a.map(|f| arch::frame_write(f, 0, &[0xAB; 4096])).unwrap_or(false);
    let back = a.map(mem::give).unwrap_or(false);
    let twice = a.map(mem::give).unwrap_or(true);
    let mut seen = alloc::vec::Vec::new();
    let mut again = None;
    while let Some(f) = mem::take() {
        if Some(f) == a {
            again = Some(f);
        } else {
            seen.push(f);
        }
    }
    let exhausted = mem::census().1 == 0 && seen.len() + 2 == frames;
    seen.sort();
    let once = seen.windows(2).all(|w| w[0] != w[1]) && !seen.contains(&b.unwrap_or(0));
    for f in seen {
        mem::give(f);
    }
    let mut clean = false;
    if let Some(f) = again {
        match arch::Space::empty() {
            Some(mut s) => {
                clean = s.map(arch::USER_BASE, f, true, false) && s.read(arch::USER_BASE, 4096).map(|v| v.iter().all(|x| *x == 0)).unwrap_or(false);
                let _ = s.release(); // gives `f` back with the tables
            }
            None => {
                mem::give(f);
            }
        }
    }
    if let Some(f) = b {
        mem::give(f);
    }
    let foreign = mem::give(0x1000) || mem::give(lo + 1) || arch::frame_zero(0x101000) || arch::frame_write(0x101000, 0, &[0]);
    check("a frame is handed out to one holder, comes back zeroed, cannot be given back twice, and memory outside the pool is not a frame",
          frames > 0 && distinct && inside && dirty && back && !twice && exhausted && once && clean && !foreign && mem::census().1 == free0, failed);
    // The heap reuses what is freed and never hands one block to two holders.
    let (live0, carved0) = arch::heap_used();
    let mut blocks: alloc::vec::Vec<alloc::vec::Vec<u8>> = alloc::vec::Vec::new();
    for n in 0..64usize {
        blocks.push(alloc::vec![n as u8; 100 + n * 37]);
    }
    let own = blocks.iter().enumerate().all(|(n, b)| b.iter().all(|x| *x == n as u8));
    let (_, carved1) = arch::heap_used();
    drop(blocks);
    let mut again: alloc::vec::Vec<alloc::vec::Vec<u8>> = alloc::vec::Vec::new();
    for n in 0..64usize {
        again.push(alloc::vec![n as u8; 100 + n * 37]);
    }
    let (_, carved2) = arch::heap_used();
    drop(again);
    check("the kernel's heap gives each holder its own block and uses freed blocks again instead of new memory",
          own && carved1 > carved0 && carved2 == carved1 && arch::heap_used().0 == live0, failed);
}

/// Processes, observed: thirteen programs are run in user mode and what each did is what the kernel saw.
fn processes(rec: &mut Record, measured: &str, failed: &mut u32) {
    use user::{End, Start};
    let starts = [Start { name: "hello", console: true, budget: 50 }, Start { name: "polite", console: true, budget: 50 },
                  Start { name: "spinner", console: true, budget: 200 }, Start { name: "spinner", console: true, budget: 200 },
                  Start { name: "read_kernel", console: true, budget: 50 }, Start { name: "write_kernel", console: true, budget: 50 },
                  Start { name: "privileged", console: true, budget: 50 }, Start { name: "port", console: true, budget: 50 },
                  Start { name: "exec_data", console: true, budget: 50 }, Start { name: "overflow", console: true, budget: 50 },
                  Start { name: "bad_calls", console: true, budget: 50 }, Start { name: "mute", console: false, budget: 50 },
                  Start { name: "hog", console: true, budget: 12 }];
    // Each process is a task in the kernel's register, started by the kernel as a participant of its own record.
    let mut recorded = rec.submit(&now(), "host", "HOST", "PARTICIPANT_ADDED", obj(&[("label", text("kernel")), ("kind", text("AUTOMATED")), ("principal", Value::Bool(false))])).is_ok();
    for (i, s) in starts.iter().enumerate() {
        recorded &= logged("TASK_STARTED", rec.submit(&now(), "kernel", "HOST", "TASK_STARTED",
                               obj(&[("label", text(s.name)), ("worker", text("PROGRAM")), ("review", Value::Null), ("quiet_limit", Value::Int(0)), ("budget", Value::Int(s.budget as i64)),
                                     ("attempt", text(&format!("process {}", i + 1))), ("observed", text("PROCESS_CREATED"))]))).is_ok();
    }
    arch::interrupts_off();
    let before = arch::ticks();
    let procs = match user::run(&starts) {
        Some(p) => p,
        None => {
            check("the programs for user mode were started", false, failed);
            return;
        }
    };
    let p = |n: usize| &procs[n - 1];
    let refused = |n: usize, want_vector: u64, test: &dyn Fn(u64, u64) -> bool| matches!(p(n).end, Some(End::Refused { vector, error, address, .. }) if vector == want_vector && test(error, address));
    say!("  note {} processes ran for {} timer periods; the timer took the processor back {} times", procs.len(), arch::ticks() - before, procs.iter().map(|x| x.preempted).sum::<u64>());
    check("a program runs in user mode in its own address space, writes through a system call and exits; the kernel kept what it wrote",
          p(1).end == Some(End::Exited(0)) && p(1).output == b"hello from user mode" && p(1).settled == Some(Task::Done), failed);
    check("a program that yields is resumed where it left off, three times",
          p(2).end == Some(End::Exited(0)) && p(2).output == b"a line, then a yielda line, then a yielda line, then a yield" && p(2).calls == 7, failed);
    check("two programs that never yield are each preempted by the timer, at least three times, and each finishes",
          p(3).end == Some(End::Exited(0)) && p(4).end == Some(End::Exited(0)) && p(3).preempted >= 3 && p(4).preempted >= 3, failed);
    check("each of the two wrote its own number at the same address and found it unchanged: their memory is separate",
          p(3).output == b"my memory is my own" && p(4).output == b"my memory is my own" && p(3).frames.iter().all(|f| !p(4).frames.contains(f)), failed);
    check("a program's read of kernel memory is refused by the processor and ends that program only",
          refused(5, 14, &|e, a| e & 5 == 5 && e & 2 == 0 && a == 0x101000), failed);
    check("a program's write to kernel memory is refused by the processor", refused(6, 14, &|e, a| e & 7 == 7 && a == 0x101000), failed);
    check("an instruction reserved to the kernel is refused in user mode", refused(7, 13, &|_, _| true), failed);
    check("a program reaches no device port", refused(8, 13, &|_, _| true), failed);
    check("a program cannot execute its own writable data", refused(9, 14, &|e, a| e & 0x15 == 0x15 && a as usize == arch::USER_DATA), failed);
    check("a program that exhausts its stack meets the absent page below it and is ended",
          refused(10, 14, &|e, a| e & 1 == 0 && (a as usize) < arch::USER_STACK_TOP - 0x1000 && (a as usize) >= arch::USER_STACK_TOP - 0x2000), failed);
    check("a system call that names kernel memory, unmapped memory or an impossible length is refused, and so is an undefined call",
          p(11).end == Some(End::Exited(0)) && p(11).output.is_empty() && p(11).calls == 5, failed);
    check("a program without a capability for the console cannot write to it", p(12).end == Some(End::Exited(0)) && p(12).output.is_empty(), failed);
    check("a program that never yields and never ends is stopped by the kernel when its budget is spent",
          p(13).end == Some(End::OverBudget) && p(13).periods == 13 && p(13).settled == Some(Task::Stopped), failed);
    check("in every address space a program reaches three pages, none both writable and executable, no kernel page, and the page below its stack is absent",
          procs.iter().all(|x| x.audit == (3, 0, false, true)), failed);
    let mut frames: alloc::vec::Vec<usize> = procs.iter().flat_map(|x| x.frames).collect();
    frames.sort();
    frames.dedup();
    check("no page of memory belongs to two processes", frames.len() == procs.len() * 3, failed);
    check("after all of it the kernel is in its own address space and its code and read-only data measure as before",
          arch::in_kernel_space() && sha256_hex(arch::image()) == measured, failed);
    // The record: DONE only by the kernel's own reading of what the process wrote; a refused one FAILED; the stopped one STOPPED.
    let mut ends = [0u32; 3];
    for x in &procs {
        let id = format!("T{}", x.pid);
        let digest = sha256_hex(&x.output);
        let (outcome, result, note) = match x.end {
            Some(End::Exited(0)) => {
                recorded &= rec.submit(&now(), "kernel", "HOST", "ARTIFACT_ADDED", obj(&[("sha256", text(&digest)), ("size", Value::Int(x.output.len() as i64)), ("origin", text("PRODUCED")),
                                                                                         ("stored", Value::Bool(false)), ("task", text(&id))])).is_ok();
                ends[0] += 1;
                ("DONE", text(&digest), format!("exit 0 observed; the result is what the kernel itself received through the write call; {} system call(s), {} timer period(s)", x.calls, x.periods))
            }
            Some(End::Exited(code)) => {
                ends[1] += 1;
                ("FAILED", Value::Null, format!("exit {} observed", code))
            }
            Some(End::Refused { vector, error, address, rip }) => {
                ends[1] += 1;
                ("FAILED", Value::Null, format!("ended by the kernel: the processor refused an act of the program (exception {}, error {:#x}, address {:#x}, instruction {:#x})", vector, error, address, rip))
            }
            Some(End::OverBudget) => {
                ends[2] += 1;
                ("STOPPED", Value::Null, format!("stopped by the kernel after {} timer period(s): over its budget", x.periods))
            }
            Some(End::Stopped) => {
                ends[2] += 1;
                ("STOPPED", Value::Null, format!("stopped by the person after {} timer period(s)", x.periods))
            }
            None => ("LOST", Value::Null, "no end was observed".to_string()),
        };
        recorded &= logged("TASK_ENDED", rec.submit(&now(), "host", "HOST", "TASK_ENDED", obj(&[("id", text(&id)), ("outcome", text(outcome)), ("result", result.clone()), ("read_back", result), ("spent", Value::Int(x.periods as i64)),
                                                                              ("note", text(&note)), ("observed", text("PROCESS_END"))]))).is_ok();
    }
    check("every process is a task in the kernel's register: six DONE by the kernel's own reading of the output, six FAILED, one STOPPED",
          recorded && ends == [6, 6, 1], failed);
    // A worker's word is not completion: a DONE that names no recorded result is refused.
    let unrecorded = rec.submit(&now(), "kernel", "HOST", "TASK_STARTED", obj(&[("label", text("claims")), ("worker", text("PROGRAM")), ("review", Value::Null), ("quiet_limit", Value::Int(0)), ("budget", Value::Int(1))])).is_ok()
        && rec.submit(&now(), "host", "HOST", "TASK_ENDED", obj(&[("id", text("T14")), ("outcome", text("DONE")), ("result", text(&"ab".repeat(32))), ("read_back", text(&"ab".repeat(32))), ("spent", Value::Int(0)), ("note", text("claimed"))])).is_err()
        && rec.submit(&now(), "host", "HOST", "TASK_ENDED", obj(&[("id", text("T14")), ("outcome", text("LOST")), ("result", Value::Null), ("read_back", Value::Null), ("spent", Value::Int(0)), ("note", text("never run"))])).is_ok();
    check("a task is not DONE on a claim: completion that names no result in the record is refused", unrecorded, failed);
}

/// A program of `boot.s` behind the smallest ELF header the loader accepts: position-independent, one segment
/// that is readable and executable, the entry at the program's first byte.
fn elf_of(name: &str) -> Option<alloc::vec::Vec<u8>> {
    let code = arch::user_program(name)?;
    let mut f = alloc::vec![0u8; 120];
    f[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
    let total = (120 + code.len()) as u64;
    for (at, value, width) in [(16usize, 3u64, 2usize), (18, 62, 2), (20, 1, 4), (24, 120, 8), (32, 64, 8), (52, 64, 2), (54, 56, 2), (56, 1, 2),
                               (64, 1, 4), (68, 5, 4), (96, total, 8), (104, total, 8), (112, 4096, 8)] {
        f[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
    }
    f.extend_from_slice(code);
    Some(f)
}

/// Scan codes: Ctrl, Alt and Delete pressed in that order and then released.
const STOP_KEYS: [u8; 6] = [0x1D, 0x38, 0x53, 0xD3, 0xB8, 0x9D];

/// Let whatever is left of a key script reach the controller and the keyboard's handler, then forget the keys and
/// any stop they asked for. Called and left with interrupts off. Report a missed wake or unfinished script.
fn script_drain() -> Result<(), &'static str> {
    let woke_on_interrupt = dev::keyboard_script_finish();
    arch::interrupts_on();
    let mut done = false;
    for _ in 0..200 {
        if dev::keyboard_script_left() == 0 {
            done = true;
            break;
        }
        arch::wait_until(arch::ticks() + 1);
    }
    arch::wait_until(arch::ticks() + 2); // the last byte's interrupt
    arch::interrupts_off();
    dev::keys_forget();
    let _ = dev::stop_requested();
    if !done {
        Err("the key script did not finish")
    } else if !woke_on_interrupt {
        Err("a key was queued but the reader was still idle on the next timer tick")
    } else {
        Ok(())
    }
}

/// The asynchronous side, observed: sleeping, waiting for a key, and the stop key arriving while a program runs,
/// sleeps or waits. Keys arrive through the keyboard's interrupt (see `dev::keyboard_script`). Interrupts are off
/// when this is called and when it returns. None of these runs is a task in the kernel's register.
fn asynchronous(failed: &mut u32) {
    use dev::KeyPacing::{BlockedRead, Sleeping, Timed};
    use user::{End, Grants, Program, Start, World};
    let world = || World { files: alloc::vec::Vec::new(), store: alloc::vec::Vec::new(), epoch_at_start: 0, unanswered: alloc::vec::Vec::new() };
    let grants = Grants { console: true, keys: true, ..Grants::default() };
    let loaded = |name: &str, args: &[&str], keys: &[u8], after: u64, pacing| -> (Result<user::Process, &'static str>, usize, usize, u64) {
        let free0 = user::frames_free();
        let result = match elf_of(name) {
            Some(image) => {
                let t0 = arch::ticks();
                // The first scan code is due no earlier than `after` ticks from here.
                dev::keyboard_script(t0 + after, keys, pacing);
                let (r, _) = user::run_program(&Program { name: args[0], image: &image, args, grants, budget: u64::MAX }, world());
                (r, arch::ticks() - t0)
            }
            None => (Err("no such test program"), 0),
        };
        (script_drain().and(result.0), free0, user::frames_free(), result.1)
    };

    // Independent rollback controls: an admitted earlier group member never enters user mode.
    for (label, group) in [("single-unknown", alloc::vec!["absent-test-member"]),
                            ("valid-then-unknown", alloc::vec!["hog", "absent-test-member"]),
                            ("capacity-plus-one", alloc::vec!["hog"; 17])] {
        let specifications: alloc::vec::Vec<_> = group.iter().map(|&name| Start { name, console: true, budget: 2 }).collect();
        let before = user::frames_free();
        let rejected = user::run(&specifications).is_none();
        let after = user::frames_free();
        say!("  note refused group {}: refused {}, free frames {} before and {} after", label, rejected, before, after);
        check("rejected batch returns the frames of every member", rejected && before == after, failed);
    }

    // 1. The stop key while a program runs. The program never yields and never ends; its budget is far away.
    dev::keyboard_script(arch::ticks() + 5, &STOP_KEYS, Timed);
    let t0 = arch::ticks();
    let procs = user::run(&[Start { name: "hog", console: true, budget: 100_000 }]);
    let took = arch::ticks() - t0;
    let drained = script_drain().is_ok();
    let (end, settled) = procs.as_ref().and_then(|p| p.first()).map(|p| (p.end, p.settled)).unwrap_or((None, None));
    say!("  note stop key while running: end {:?}, task {:?}, after {} timer periods", end, settled, took);
    check("the stop key ends a program that never yields, long before its budget", drained && end == Some(End::Stopped) && took < 100, failed);
    check("a program the person stopped is STOPPED in the kernel's task table, as it is in what the kernel observed of it", settled == Some(Task::Stopped), failed);

    // 2. Sleep and wake. 50 ms is 50,000,000 ns; a tick is 999,847 ns; the deadline is the tick of the call plus
    // ceil(50,000,000 / 999,847) = 51. The processor idles through exactly those 51 ticks if the program is woken at its tick.
    let (r, free0, free1, took) = loaded("sleeper", &["sleeper"], &[], 0, Timed);
    let (end, idle) = r.as_ref().map(|p| (p.end, p.idle)).unwrap_or((None, 0));
    say!("  note sleep of 50 ms: {:?}, end {:?}, idle {} of {} timer periods", r.as_ref().err(), end, idle, took);
    check("a program that sleeps 50 ms is woken at its millisecond: the processor idled 51 timer periods, no fewer and no more, and the sleep answered 0",
          end == Some(End::Exited(0)) && idle == 51 && took >= 51 && free0 == free1, failed);

    // 3. The reader checks each two-byte event in order: Shift down, Shift up, then A down.
    // Eight calls: open; then for each of Shift down, Shift up and the key, a read that waits and the same read made again.
    // Each injection is on a timer tick spent idle in a blocked read. Startup can use part of the initial 20 ticks.
    // The eighth call is exit. The remaining key release drains after the process has ended.
    let (r, free0, free1, took) = loaded("key_reader", &["key_reader"], &[0x2A, 0xAA, 0x1E, 0x9E], 20, BlockedRead);
    let (end, idle, calls) = r.as_ref().map(|p| (p.end, p.idle, p.calls)).unwrap_or((None, 0, 0));
    say!("  note key while waiting: {:?}, end {:?}, idle {} of {} timer periods, {} calls", r.as_ref().err(), end, idle, took, calls);
    check("a program waiting in a read of the keyboard is woken by each key's interrupt and reads the keys in order: it idled until the first and exited 0 on the third",
          end == Some(End::Exited(0)) && idle >= 3 && calls == 8 && free0 == free1, failed);

    // 4. The stop key while a program is asleep, with the longest sleep the kernel accepts (2^32 s and 999,999,999 ns).
    const SLEEP_IDLE_TICKS: u64 = 10;
    const STOP_TIMEOUT: u64 = 100;
    let pacing = Sleeping { idle_ticks: SLEEP_IDLE_TICKS, timeout: STOP_TIMEOUT };
    let (r, free0, free1, took) = loaded("sleeper_long", &["sleeper_long"], &STOP_KEYS, 0, pacing);
    let (end, idle, calls) = r.as_ref().map(|p| (p.end, p.idle, p.calls)).unwrap_or((None, 0, 0));
    say!("  note stop key while asleep: {:?}, end {:?}, idle {} of {} timer periods, {} calls, free frames {} before and {} after",
         r.as_ref().err(), end, idle, took, calls, free0, free1);
    check("the stop key ends a program that is asleep (for the longest sleep accepted: its deadline does not overflow) at once, and all its memory comes back",
          end == Some(End::Stopped) && idle >= SLEEP_IDLE_TICKS && took < STOP_TIMEOUT && calls == 1 && free0 == free1, failed);

    // 5. The stop key while a program waits in a read of the keyboard. Ctrl and Alt wake it and it waits again; Delete ends it.
    // Six calls: open, and three reads that wait, of which the first two are made again and answered with Ctrl and with Alt.
    let (r, free0, free1, took) = loaded("key_reader", &["key_reader", "stop"], &STOP_KEYS, 10, BlockedRead);
    let (end, idle, calls) = r.as_ref().map(|p| (p.end, p.idle, p.calls)).unwrap_or((None, 0, 0));
    say!("  note stop key while waiting for a key: {:?}, end {:?}, idle {} of {} timer periods, {} calls", r.as_ref().err(), end, idle, took, calls);
    check("the stop key ends a program that waits in a read of the keyboard; it had read Ctrl and Alt and was never handed Delete; all its memory comes back",
          end == Some(End::Stopped) && idle >= 3 && took < 100 && calls == 6 && free0 == free1, failed);

    // 6. A stop asked for before a program exists is not a stop of it.
    arch::interrupts_on();
    dev::keyboard_script(arch::ticks(), &STOP_KEYS, Timed);
    for _ in 0..200 {
        if dev::keyboard_script_left() == 0 {
            break;
        }
        arch::wait_until(arch::ticks() + 1);
    }
    arch::wait_until(arch::ticks() + 2);
    arch::interrupts_off();
    let (r, free0, free1, _) = loaded("sleeper", &["sleeper"], &[], 0, Timed);
    let end = r.as_ref().map(|p| p.end).unwrap_or(None);
    check("a stop key pressed before a program is started does not stop it", end == Some(End::Exited(0)) && free0 == free1, failed);

    // 7. A name the task table refuses as a label (33 bytes; it takes 32). The program must be refused, with the
    // admission error and no other, and no memory may be lost. The control before it is the same program under a
    // name one byte shorter, which must run: it shows that the refusal is the name's and not the loader's.
    let (r, free0, free1, _) = loaded("sleeper", &["a-name-of-thirty-three-character"], &[], 0, Timed);
    let end = r.as_ref().map(|p| p.end).unwrap_or(None);
    say!("  note a 32-byte name: {:?}; frames free {} before, {} after", r.as_ref().map(|p| p.end), free0, free1);
    check("a program under a name of 32 bytes is admitted and runs to its end", end == Some(End::Exited(0)) && free0 == free1, failed);
    let (r, free0, free1, _) = loaded("sleeper", &["a-name-of-thirty-three-characters"], &[], 0, Timed);
    say!("  note a 33-byte name: {:?}; frames free {} before, {} after", r.as_ref().map(|p| p.end), free0, free1);
    let refused = matches!(r, Err("the program could not be admitted as a task"));
    check("a program under a name of 33 bytes is refused admission as a task", refused, failed);
    check("a program that cannot be admitted as a task leaves no memory behind: as many frames are free after as before", free0 == free1, failed);
}

/// Opt-in allocation-boundary test. Reserving the pool touches every free frame.
fn check_space_cutoffs(failed: &mut u32) {
    // Leave each possible number of frames below one built-in address space's requirement.
    // Restore the fixture before checking the total so any leak remains visible.
    let before = user::frames_free();
    let mut reserved = alloc::vec::Vec::with_capacity(before);
    while let Some(frame) = mem::take() {
        reserved.push(frame);
    }
    let mut restored = true;
    for available in 0..=7 {
        if available > 0 {
            restored &= reserved.pop().map(mem::give) == Some(true);
        }
        let space = arch::Space::new(&[0xc3]);
        restored &= space.is_some() == (available == 7);
        if let Some(space) = space {
            restored &= space.release() == 7;
        }
        restored &= user::frames_free() == available;
    }
    for frame in reserved {
        restored &= mem::give(frame);
    }
    check("a built-in address space gives its frames back at every allocation cutoff and after a successful construction",
          restored && user::frames_free() == before, failed);
}

/// The kernel proper, entered from `arch` in 64-bit mode with interrupts off.
pub fn kernel_start(magic: u32, info: u32) -> ! {
    arch::serial_init();
    say!("PCP OS kernel {} | x86_64 | new kernel, written in Rust | early", env!("CARGO_PKG_VERSION"));
    arch::interrupts_init();
    let mut boot = arch::boot_info(magic, info);
    match boot.memory {
        Some((lower, upper)) => say!("memory reported by the loader: {} KiB below 1 MiB, {} KiB above", lower, upper),
        None => say!("the loader reported no memory size"),
    }
    let [ts, te, rs, re, end] = arch::layout();
    say!("kernel image: code {:#x}..{:#x}, read-only data {:#x}..{:#x}, end {:#x}", ts, te, rs, re, end);
    let (cr0, _cr4, efer) = arch::protection_registers();
    say!("protections: paging {}, supervisor write protection {}, no-execute {}, long mode {}",
         cr0 >> 31 & 1 == 1, cr0 >> 16 & 1 == 1, efer >> 11 & 1 == 1, efer >> 10 & 1 == 1);
    let (pool_start, pool_end) = arch::protect(&mut boot);
    let frames = mem::init();
    arch::fpu_init();
    arch::syscall_init();
    say!("memory for programs: {} frames of 4 KiB, {:#x}..{:#x}", frames, pool_start, pool_end);
    for m in &boot.modules {
        say!("file from the loader: {} ({} bytes at {:#x}){}", m.name, m.end - m.start, m.start, if m.usable { "" } else { ": NOT USABLE, it lies where the kernel cannot map it" });
    }
    if !boot.command_line.is_empty() {
        say!("command line: {}", boot.command_line);
    }
    let (offered, enabled) = arch::harden();
    let on = |bit: u32| if offered >> bit & 1 == 0 { "not offered by this processor" } else if enabled >> bit & 1 == 1 { "on" } else { "OFFERED AND NOT ON" };
    say!("further protections: supervisor execution prevention {}, supervisor access prevention {}, user-mode instruction prevention {}", on(20), on(21), on(11));
    let measured = sha256_hex(arch::image());
    // K-12: the measurement is compared with a value the kernel did not compute, where whoever started it named one
    // (`image=<SHA-256>` on the loader's command line; tools/measure.py computes it from the image file without
    // running it). The command line is not a root of trust: this shows that the image running is the one that
    // person expected, and nothing about who that person is.
    let expected: Option<&str> = boot.command_line.split_whitespace().find_map(|w| w.strip_prefix("image="));
    let as_expected = expected.map(|e| e.eq_ignore_ascii_case(&measured));
    match (expected, as_expected) {
        (Some(_), Some(true)) => say!("running image (code and read-only data), SHA-256: {} (measured; equal to the value the loader named)", measured),
        (Some(e), _) => say!("running image (code and read-only data), SHA-256: {} (measured; NOT the value the loader named, which is {})", measured, e),
        _ => say!("running image (code and read-only data), SHA-256: {} (measured; the loader named no value to compare it with)", measured),
    }
    let mut failed = 0;

    // The kernel's own register: START, then VERIFY.
    let mut rec = Record::new();
    let created = rec.submit(&now(), "host", "HOST", "WORKSPACE_CREATED",
                             obj(&[("profile", text("ACR-OS-RECORD-1")), ("workspace", text("acr:workspace:kernel")), ("label", text("kernel"))])).is_ok();
    // The method's identity is the release this core implements. The image's measurement is a fact about
    // the kernel and is recorded as one; it is not the method's identity and is not put in its place.
    let pinned = rec.submit(&now(), "host", "HOST", "METHOD_PINNED",
                            obj(&[("release_fingerprint", text(acr::METHOD_FINGERPRINT)), ("core_descriptor_sha256", text(acr::CORE_DESCRIPTOR)),
                                  ("verified", text("DECLARED_NOT_VERIFIED")), ("presence", text("NATIVE")),
                                  ("note", text("native: the guards compiled into this image implement this release; no method text is on this machine to compare"))])).is_ok()
        && rec.submit(&now(), "host", "HOST", "HOST_EVENT",
                      obj(&[("event", text("ENVIRONMENT")), ("program", text(concat!("pcpos-kernel ", env!("CARGO_PKG_VERSION")))), ("arch", text("x86_64")), ("os", text("pcpos")),
                            ("image_sha256", text(&measured)), ("image_compared_with", expected.map(text).unwrap_or(Value::Null)), ("machines_sha256", text(&sha256_hex(MACHINES.as_bytes())))])).is_ok();
    system_event(&mut rec, "BOOT", "started by the loader", &mut failed);
    system_event(&mut rec, "BEGIN_VERIFY", "self-tests", &mut failed);

    let map = arch::kernel_map_audit();
    say!("memory map replaced, read back from the page tables: {} pages of the kernel image, {} pages of the loader's files, {} KiB of frame memory, {} KiB of device memory; writable and executable at once: {}",
         map.image_pages, map.module_pages, map.pool_bytes / 1024, map.device_bytes / 1024, map.writable_and_executable);
    say!("self-test:");
    check("the kernel opens its own register and records the native method by measurement", created && pinned, &mut failed);
    if let Some(equal) = as_expected {
        check("the running image measures as the value the loader named", equal, &mut failed);
    }
    failed += self_test();
    let image_pages = ((end - ts) >> 12) as u32 + ((ts - (1 << 20)) >> 12) as u32 - arch::guard_pages().len() as u32;
    let module_pages: u32 = boot.modules.iter().filter(|m| m.usable).map(|m| (((m.end + 4095) >> 12) - (m.start >> 12)) as u32).sum();
    for (name, ok) in [("no memory is both writable and executable", map.writable_and_executable == 0), ("code is executable and not writable", map.code_executable_not_writable),
                       ("read-only data is neither writable nor executable", map.read_only_data_neither), ("data is writable and not executable", map.data_writable_not_executable),
                       ("address zero is not mapped, nor anything below the image", map.address_zero_unmapped && map.low_mebibyte_unmapped),
                       ("what is mapped is the kernel image, the loader's files read-only, and frame memory that is never executable; nothing else, each at its own address, none of it for user mode",
                        map.image_pages == image_pages && map.module_pages == module_pages && map.module_pages_not_read_only == 0 && map.pool_bytes == pool_end - pool_start
                            && map.pool_or_device_executable == 0 && map.outside == 0 && map.not_identity == 0 && map.user_reachable == 0 && map.other_top_entries == 0 && map.device_bytes == 0),
                       ("every further protection the processor offers is on", offered == enabled)] {
        check(name, ok, &mut failed);
    }
    memory_tests(&mut failed);
    protection_probes(&mut failed);
    watchdog(&mut failed);

    // VERIFY ends in OPERATIONAL or in SAFE, and nothing else.
    if failed == 0 {
        system_event(&mut rec, "VERIFIED_OK", "every self-test passed", &mut failed);
    } else {
        system_event(&mut rec, "VERIFY_FAILED", "a self-test failed", &mut failed);
    }
    let state = rec.state().system();
    check("the system is OPERATIONAL after a verification that passed, and only then", (failed == 0) == (state == Some(System::Operational)), &mut failed);
    // The composition rule, on the live record: a review step is refused because ACR is not in use, with its own exit code.
    let refused = rec.submit(&now(), "host", "HOST", "GATE_COMMITTED", obj(&[("review", text("R")), ("gate", text("G1"))]));
    check("an event the guards refuse leaves the register unchanged",
          refused.is_err() && rec.ledger().len() == 6, &mut failed);
    // A kernel that did not verify itself runs nothing: not the processes built into it, and (session.rs) no program.
    // Until build 22 they were run all the same; the guards refused to record them as tasks of a system in SAFE, so the
    // record was right and the kernel's conduct was not.
    if failed == 0 {
        processes(&mut rec, &measured, &mut failed);
    } else {
        say!("the kernel did not verify itself: the processes built into it are not run");
    }
    // Devices are looked for only now: the kernel has verified itself with nothing but itself mapped.
    let bus = dev::bus_scan();
    for d in &bus {
        say!("device on the bus: {:02x}:{:02x}.{} maker {:04x} device {:04x} class {:02x}.{:02x} line {}", d.bus, d.slot, d.function, d.vendor, d.device, d.class, d.subclass, d.line);
    }
    let display = dev::display_init(&bus, boot.display);
    match display {
        Some(d) => say!("display: {:?}, {}x{}, {} page(s), {} KiB of its memory mapped at {:#x}", d.kind, d.width, d.height, d.pages, d.mapped / 1024, d.memory),
        None => say!("display: none this kernel can drive"),
    }
    let keyboard = dev::keyboard_init();
    say!("keyboard: {}", if keyboard { "a PS/2 controller answered" } else { "no controller" });
    let sound = dev::sound_init(&bus);
    say!("sound: {}", if sound { "an AC'97 device, 48000 pairs of 16-bit samples a second" } else { "no device this kernel drives" });
    let music = dev::fm_init();
    say!("music: {}", if music { "an FM synthesiser of the AdLib kind answered at its usual ports" } else { "no FM synthesiser" });
    if display.is_some() {
        // With device memory mapped the kernel's map is read back once more: what was added is the display's
        // memory, never executable, and nothing else changed.
        let after = arch::kernel_map_audit();
        check("with the display mapped, the kernel's memory map is as it was plus the display's memory, which is never executable",
              after.device_bytes == display.map(|d| d.mapped).unwrap_or(0) && after.pool_or_device_executable == 0 && after.writable_and_executable == 0 && after.outside == 0
                  && after.user_reachable == 0 && after.not_identity == 0 && after.image_pages == map.image_pages && after.module_pages == map.module_pages && after.pool_bytes == map.pool_bytes, &mut failed);
    }
    // The asynchronous side needs the keyboard's interrupt, so it is tested here, and only by a kernel that verified itself.
    if failed == 0 && keyboard {
        asynchronous(&mut failed);
        if boot.command_line.split_whitespace().any(|word| word == "test=frame-cutoffs") {
            check_space_cutoffs(&mut failed);
        }
    } else {
        say!("the asynchronous self-tests were not run: {}", if keyboard { "the kernel did not verify itself" } else { "there is no keyboard controller" });
    }
    #[cfg(feature = "virtio-tx-probe")]
    if failed == 0 && boot.command_line.split_whitespace().any(|word| word == "test=virtio-tx") {
        let transmit = dev::virtio_tx::transmit(&bus);
        say!("virtio TX fixture: {:?}", transmit);
        check("the isolated virtio-net TX queue completed its single frame", transmit.is_ok(), &mut failed);
    }
    #[cfg(feature = "virtio-rx-probe")]
    if failed == 0 && boot.command_line.split_whitespace().any(|word| word == "test=virtio-reply") {
        let reply = dev::virtio_tx::reply(&bus, &boot.command_line);
        say!("virtio reply fixture: {:?}", reply);
        check("the isolated virtio-net RX queue received the exact reply", reply.is_ok(), &mut failed);
    }
    #[cfg(feature = "virtio-register-probe")]
    if failed == 0 && boot.command_line.split_whitespace().any(|word| word == "test=virtio-register") {
        let result = register_wire::fixture(&bus, &boot.command_line);
        say!("virtio register fixture: {:?}", result);
        check("the isolated peer supplied the verified register extension", result.is_ok(), &mut failed);
    }
    #[cfg(feature = "virtio-worker-probe")]
    if failed == 0 {
        let words = || boot.command_line.split_whitespace();
        if words().any(|s| s == "test=worker-director") {
            let result = worker_wire::director(&bus, &boot.command_line);
            say!("virtio worker director: {:?}", result);
            check("the native worker director verified two bound outcomes", result.is_ok(), &mut failed);
        } else if words().any(|s| s == "test=worker-receiver") {
            let result = worker_wire::receiver(&bus, &boot);
            say!("virtio worker receiver: {:?}", result);
            check("the native worker receiver finished its bounded dispatch", result.is_ok(), &mut failed);
        }
    }
    // Process and device checks run after VERIFIED_OK. Record a late failure before the launcher reads the register.
    if failed != 0 && rec.state().system() == Some(System::Operational) {
        system_event(&mut rec, "INTEGRITY_FAULT", "a self-test failed after initial verification", &mut failed);
    }
    let start = session::Start { boot: &boot, display, keyboard, sound: sound || music, passed: PASSED.load(core::sync::atomic::Ordering::Relaxed), failed, tasks: 14, version: env!("CARGO_PKG_VERSION") };
    let _tasks = session::run(&mut rec, &start, &mut failed);
    system_event(&mut rec, "SHUTDOWN", "end of the session", &mut failed);
    system_event(&mut rec, "STOPPED_OK", "stopped in order", &mut failed);
    let reopened = Record::open(rec.ledger().bytes());
    check("the kernel's register of its own start, verification and stop verifies and replays without a problem",
          matches!(&reopened, Ok((r, problems)) if problems.is_empty() && r.state().system() == Some(System::Stopped) && r.ledger().head() == rec.ledger().head()), &mut failed);
    say!("kernel register: {} events, head {}", rec.ledger().len(), rec.ledger().head());
    #[cfg(feature = "overflow-demo")]
    {
        say!("overflow demonstration: recursing until the stack is exhausted");
        say!("{}", arch::exhaust_stack(0));
    }
    #[cfg(feature = "fault-demo")]
    {
        say!("fault demonstration: reading the page below the stack with nothing prepared to catch it");
        arch::read_unguarded(arch::stack_guard());
    }
    let (live, carved) = arch::heap_used();
    let (pool_frames, pool_free, taken, given) = mem::census();
    say!("kernel heap: {} bytes in use, {} bytes ever carved; frames: {} of {} free, {} handed out and {} given back since the start", live, carved, pool_free, pool_frames, taken, given);
    if failed == 0 {
        #[cfg(not(feature = "guest-ethernet"))]
        say!("ALL SELF-TESTS PASS. Present: user mode and preemptive processes; programs loaded from the loader's files; a display, a keyboard, sound and music where the machine has ones this kernel drives. Not present: storage that outlasts power, a mouse, network, a second processor.");
        #[cfg(feature = "guest-ethernet")]
        say!("ALL SELF-TESTS PASS. Guest Ethernet support is compiled in. Access requires a matching image grant and a supported NIC.");
    } else {
        say!("{} SELF-TEST(S) FAILED", failed);
    }
    arch::stop(failed == 0)
}
