//! Processes. A program runs in user mode, in an address space of its own, and reaches the kernel only by
//! a system call. The timer takes the processor back from a program that does not give it up. Whatever a
//! program does that the processor refuses ends that program and nothing else.
//!
//! Each process is a task in the core's task table and holds its rights in the core's capability space:
//! a process reaches the console, the display, the keyboard, sound, the loader's files or the store only if it
//! holds a capability for it, it is charged for the timer periods it uses, and it is stopped when it has
//! used more than its budget. What a process wrote is kept by the kernel, so that the kernel's own reading
//! of it, and not the process's word, decides completion.
//!
//! There are two kinds of program. The thirteen built into the kernel are a few instructions each and use
//! the five calls of the first gate; they exist to show what the processor refuses. A program the loader
//! handed over is an ordinary statically linked C program: it is loaded by `exec`, and its calls, made with
//! the `syscall` instruction, are answered by `sys`.
//!
//! This file has no `unsafe`. Everything that touches the processor is in `arch.rs`.
use crate::arch::{self, Frame, Space, Trap};
use crate::{dev, exec, mem, sys};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use pcpos_core::cap::{CapSpace, Handle, Rights};
use pcpos_core::machine::{Task, TaskEvent};
use pcpos_core::sha256;
use pcpos_core::task::{TaskTable, Worker};

/// How a process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// It asked to exit, with this code.
    Exited(u64),
    /// The processor refused something it did: the exception's vector, error code, the address concerned and the instruction.
    Refused { vector: u64, error: u64, address: u64, rip: u64 },
    /// It used more timer periods than its budget and the kernel stopped it.
    OverBudget,
    /// The person stopped it with the kernel's own key.
    Stopped,
}

/// The things a capability can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    Console = 1,
    Display = 2,
    Keys = 3,
    /// The files the loader handed over, to read.
    Files = 4,
    /// Files a program writes, which last until the machine is switched off.
    Store = 5,
    Sound = 6,
    #[cfg(feature = "game-link")]
    GameLink = 7,
    #[cfg(feature = "guest-ethernet")]
    Ethernet,
}

const CAP_SLOTS: usize = Object::Sound as usize + 1 + cfg!(feature = "game-link") as usize + cfg!(feature = "guest-ethernet") as usize;

/// What a program is started with the right to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub console: bool,
    pub display: bool,
    pub keys: bool,
    pub files: bool,
    pub store: bool,
    pub sound: bool,
    #[cfg(feature = "game-link")]
    pub game_link: bool,
    #[cfg(feature = "guest-ethernet")]
    pub ethernet: bool,
}

/// The most of a process's output that is kept.
const MAX_OUTPUT: usize = 16 * 1024;

/// System call answers of the first gate, as a built-in program sees them in RAX.
const E_PERM: u64 = -1i64 as u64;
const E_FAULT: u64 = -14i64 as u64;
const E_INVAL: u64 = -22i64 as u64;
const E_NOSYS: u64 = -38i64 as u64;

/// Whether a process can run now, and if not, what it waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    Nothing,
    /// The tick count to reach.
    Until(u64),
    /// A key event.
    Key,
}

pub struct Process {
    pub pid: u32,
    pub name: String,
    frame: Frame,
    /// Taken when the process's memory has been given back.
    pub space: Option<Space>,
    fpu: Box<arch::FpuArea>,
    pub fs_base: u64,
    caps: [Option<Handle>; CAP_SLOTS],
    pub task: u32,
    pub end: Option<End>,
    pub waiting: Waiting,
    /// What it wrote to the console, as the kernel received it: the first [`MAX_OUTPUT`] bytes, and the count of all of them.
    pub output: Vec<u8>,
    pub written: u64,
    /// Timer periods in which it was running when the timer fired.
    pub periods: u64,
    /// Times the timer took the processor from it.
    pub preempted: u64,
    pub calls: u64,
    /// The state of its task when everything had ended.
    pub settled: Option<Task>,
    /// What its address space's own tables say (see [`Space::audit`]).
    pub audit: (u32, u32, bool, bool),
    pub frames: [usize; 3],
    /// For a loaded program: its open files and its memory, as `sys` keeps them.
    pub linux: Option<sys::Linux>,
    /// For a loaded program: every page it had when it ended, counted from its own tables (see [`Space::census`]).
    pub census: (usize, usize, usize, bool),
    /// Frames given back to the pool when it ended.
    pub released: usize,
    /// Timer ticks in which the processor idled because every program was waiting.
    pub idle: u64,
}

impl Process {
    /// True if the process holds a capability for the thing with the right asked for.
    pub fn holds(&self, caps: &CapSpace, object: Object, right: Rights) -> bool {
        self.caps[object as usize].map(|h| caps.check(self.pid, h, right) == Ok(object as u64)).unwrap_or(false)
    }

    /// Keep what the process wrote to the console.
    pub fn wrote(&mut self, bytes: &[u8]) {
        self.written += bytes.len() as u64;
        let room = MAX_OUTPUT - self.output.len();
        self.output.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }
}

/// What programs share and the kernel keeps between them: the loader's files and the store.
pub struct World {
    /// The loader's files by name.
    pub files: Vec<(String, &'static [u8])>,
    /// Files programs wrote, by name. An explicitly mounted volume can checkpoint them.
    pub store: Vec<(String, Vec<u8>)>,
    pub volume: Option<crate::store::Volume>,
    /// Seconds since 1970 when the tick count was zero, by the machine's clock.
    pub epoch_at_start: u64,
    /// Calls a program made that this kernel does not answer: (number, times).
    pub unanswered: Vec<(u64, u32)>,
}

pub struct Table {
    pub procs: Vec<Process>,
    current: Option<usize>,
    kernel: Option<Frame>,
    tasks: TaskTable<16>,
    pub caps: CapSpace,
    /// The process whose floating-point registers are in the processor now.
    fpu_owner: Option<usize>,
    /// The FS base loaded in the processor now.
    pub fs_loaded: u64,
    pub world: World,
    /// Timer ticks in which the processor was idling.
    idle: u64,
}

static TABLE: arch::Single<Option<Table>> = arch::Single::new(None);

/// A built-in program to start: its name in `boot.s`, whether it is given the console, and its budget in timer periods.
pub struct Start {
    pub name: &'static str,
    pub console: bool,
    pub budget: u64,
}

/// A program the loader handed over, to start.
pub struct Program<'a> {
    pub name: &'a str,
    pub image: &'a [u8],
    pub args: &'a [&'a str],
    pub grants: Grants,
    /// Timer periods it may use; `u64::MAX` for a program the person stops when they choose.
    pub budget: u64,
}

fn new_table(world: World) -> Table {
    Table { procs: Vec::new(), current: None, kernel: None, tasks: TaskTable::new(), caps: CapSpace::new(64), fpu_owner: None, fs_loaded: 0, world, idle: 0 }
}

fn empty_world() -> World {
    World { files: Vec::new(), store: Vec::new(), volume: None, epoch_at_start: 0, unanswered: Vec::new() }
}

impl Table {
    fn admit(&mut self, name: &str, space: Space, frame: Frame, grants: Grants, budget: u64, linux: Option<sys::Linux>) -> Option<usize> {
        let pid = self.procs.len() as u32 + 1;
        let now = arch::ticks();
        let mut caps = [None; CAP_SLOTS];
        for (wanted, object, rights) in [(grants.console, Object::Console, Rights::APPEND), (grants.display, Object::Display, Rights::APPEND),
                                         (grants.keys, Object::Keys, Rights::READ), (grants.files, Object::Files, Rights::READ),
                                         (grants.store, Object::Store, Rights::READ.with(Rights::APPEND)), (grants.sound, Object::Sound, Rights::APPEND),
                  #[cfg(feature = "game-link")] (grants.game_link, Object::GameLink, Rights::READ.with(Rights::APPEND)),
                  #[cfg(feature = "guest-ethernet")] (grants.ethernet, Object::Ethernet, Rights::READ.with(Rights::APPEND))] {
            if wanted {
                caps[object as usize] = self.caps.install(pid, object as u64, rights).ok();
            }
        }
        let task = match self.tasks.admit(name, Worker::Program, 0, now, u64::MAX, budget).and_then(|task| self.tasks.event(task, TaskEvent::Start, now).map(|_| task)) {
            Ok(task) => task,
            Err(_) => {
                // Not a task, so not a process: the memory it was given goes back to the pool now, since nothing else ever would give it back.
                space.release();
                return None;
            }
        };
        self.procs.push(Process { pid, name: name.to_string(), frame, audit: space.audit(), frames: space.frames(), space: Some(space), fpu: Box::new(arch::FpuArea::clean()), fs_base: 0,
                                  caps, task, end: None, waiting: Waiting::Nothing, output: Vec::new(), written: 0, periods: 0, preempted: 0, calls: 0, settled: None, linux,
                                  census: (0, 0, 0, false), released: 0, idle: 0 });
        Some(self.procs.len() - 1)
    }
}

/// Run whatever the table holds until no process is left, then settle and give back every process's memory.
fn run_table(t: Table) -> Option<Table> {
    TABLE.with(|slot| *slot = Some(t));
    arch::enter_user();
    let mut t = TABLE.with(|slot| slot.take())?;
    // Completion is settled by the kernel's own reading of what each process wrote, after it exited.
    for p in t.procs.iter_mut() {
        if p.end == Some(End::Exited(0)) {
            let _ = t.tasks.settle(p.task, Some(sha256(&p.output)));
        }
        p.settled = t.tasks.get(p.task).map(|r| r.state);
        p.idle = t.idle;
        t.caps.revoke_holder(p.pid);
        if let Some(space) = p.space.take() {
            p.census = space.census();
            p.released = space.release();
        }
    }
    Some(t)
}

/// Start each built-in program as a process, run them all until none is left, and return what the kernel observed.
/// Interrupts must be off when this is called; it returns with them off.
pub fn run(programs: &[Start]) -> Option<Vec<Process>> {
    let mut t = new_table(empty_world());
    let admitted = programs.iter().try_for_each(|p| {
        let space = Space::new(arch::user_program(p.name)?)?;
        t.admit(p.name, space, Frame::user(arch::USER_BASE, arch::USER_STACK_TOP), Grants { console: p.console, ..Grants::default() }, p.budget, None)?;
        Some(())
    });
    if admitted.is_none() {
        // The group has not entered user mode. Return every earlier admission's frames
        // while still on the kernel tables; the local task and capability tables drop here.
        for p in t.procs {
            if let Some(space) = p.space {
                space.release();
            }
        }
        return None;
    }
    run_table(t).map(|t| t.procs)
}

/// Load one program the loader handed over and run it until it ends. Returns what the kernel observed of
/// it, or why it could not be started, and the world as the program left it.
/// Interrupts must be off when this is called; it returns with them off.
pub fn run_program(program: &Program, world: World) -> (Result<Process, &'static str>, World) {
    let mut t = new_table(world);
    let mut space = match Space::empty() {
        Some(s) => s,
        None => return (Err("out of memory"), t.world),
    };
    let (seed, _) = arch::seed();
    let loaded = match exec::load(&mut space, program.image, program.args, seed) {
        Ok(l) => l,
        Err(e) => {
            space.release();
            return (Err(e), t.world);
        }
    };
    let linux = sys::Linux::new(loaded.image_end);
    if t.admit(program.name, space, Frame::user(loaded.entry, loaded.stack), program.grants, program.budget, Some(linux)).is_none() {
        return (Err("the program could not be admitted as a task"), t.world);
    }
    dev::keys_forget();
    let _ = dev::stop_requested(); // a stop asked for before the program existed is not a stop of it
    match run_table(t) {
        Some(mut t) => (t.procs.pop().ok_or("the process table was lost"), t.world),
        None => (Err("the process table was lost"), empty_world()),
    }
}

/// Entered from `arch` on every entry that concerns programs. On return, `frame` is what the processor resumes.
pub fn trap(frame: &mut Frame, trap: Trap, from_user: bool) {
    TABLE.with(|slot| match slot.as_mut() {
        Some(t) => {
            // An interrupt that arrived in the kernel concerns programs only if the processor was idling
            // because every program was waiting; the kernel's own code is otherwise left to go on.
            if from_user || trap == Trap::Enter || (t.current.is_none() && t.kernel.is_some()) {
                t.handle(frame, trap)
            }
        }
        None if from_user => crate::fault(255, 0, 0, 0), // an entry from user mode with no process table cannot happen; stop if it does
        None => {}
    })
}

impl Table {
    fn handle(&mut self, frame: &mut Frame, trap: Trap) {
        let now = arch::ticks();
        if trap == Trap::Enter {
            self.kernel = Some(*frame);
            return self.next(frame, 0);
        }
        // The person's own key ends every program, whatever it was doing.
        if dev::stop_requested() {
            for i in 0..self.procs.len() {
                if self.procs[i].end.is_none() {
                    self.end(i, End::Stopped, now);
                }
            }
            return self.next(frame, 0);
        }
        let i = match self.current {
            Some(i) => i,
            None => {
                // The processor was idling: see who can go on.
                if trap == Trap::Timer {
                    self.idle += 1;
                }
                return self.next(frame, 0);
            }
        };
        let from = i + 1;
        match trap {
            Trap::Enter => {}
            Trap::Device => {
                // The program that was running goes on unless something else is ready; a program that waits has been woken by `next`.
                self.procs[i].frame = *frame;
                self.next(frame, i)
            }
            Trap::Timer => {
                let p = &mut self.procs[i];
                p.periods += 1;
                p.preempted += 1;
                p.frame = *frame;
                // The charge is what stops a program that never gives the processor up.
                if self.tasks.charge(p.task, 1, now) == Ok(false) {
                    self.end(i, End::OverBudget, now);
                }
                self.next(frame, from)
            }
            Trap::Fault { vector, error, address } => {
                self.end(i, End::Refused { vector, error, address, rip: frame.rip }, now);
                self.next(frame, from)
            }
            Trap::Syscall => {
                self.procs[i].calls += 1;
                if self.procs[i].linux.is_none() {
                    // A built-in program has no business with this entry.
                    frame.answer(E_NOSYS);
                    return;
                }
                match sys::call(self, i, frame) {
                    sys::Outcome::Return(value) => frame.answer(value),
                    sys::Outcome::Wait(waiting, retry) => {
                        if retry {
                            frame.retry_call();
                        } else {
                            frame.answer(0);
                        }
                        let p = &mut self.procs[i];
                        p.waiting = waiting;
                        p.frame = *frame;
                        self.next(frame, from)
                    }
                    sys::Outcome::Yield => {
                        frame.answer(0);
                        self.procs[i].frame = *frame;
                        self.next(frame, from)
                    }
                    sys::Outcome::Exit(code) => {
                        self.end(i, End::Exited(code), now);
                        self.next(frame, from)
                    }
                }
            }
            Trap::Call => {
                let (number, a, b) = frame.call();
                let held = self.procs[i].holds(&self.caps, Object::Console, Rights::APPEND);
                let p = &mut self.procs[i];
                p.calls += 1;
                match number {
                    0 => {
                        self.end(i, End::Exited(a), now);
                        self.next(frame, from)
                    }
                    1 => {
                        let answer = if !held {
                            E_PERM
                        } else if b > 4096 || p.output.len() + b as usize > MAX_OUTPUT {
                            E_INVAL
                        } else {
                            match p.space.as_ref().and_then(|s| s.read(a as usize, b as usize)) {
                                Some(bytes) => {
                                    p.wrote(&bytes);
                                    b
                                }
                                None => E_FAULT,
                            }
                        };
                        frame.answer(answer)
                    }
                    2 => {
                        frame.answer(0);
                        p.frame = *frame;
                        self.next(frame, from)
                    }
                    3 => frame.answer(p.pid as u64),
                    4 => frame.answer(now),
                    _ => frame.answer(E_NOSYS),
                }
            }
        }
    }

    /// Record how a process ended. Its memory is given back when the kernel is next in its own address space.
    fn end(&mut self, i: usize, end: End, now: u64) {
        let p = &mut self.procs[i];
        p.end = Some(end);
        p.waiting = Waiting::Nothing;
        let _ = match end {
            End::Exited(0) => self.tasks.exit_ok(p.task, sha256(&p.output), now).map(|_| ()),
            End::Exited(_) | End::Refused { .. } => self.tasks.event(p.task, TaskEvent::ExitFail, now).map(|_| ()),
            End::OverBudget => self.tasks.event(p.task, TaskEvent::StopConfirmed, now).map(|_| ()),
            // The charge that found a program over its budget asked its task to stop. The person's key asks here:
            // the task table confirms a stop only of a task that was asked.
            End::Stopped => self.tasks.event(p.task, TaskEvent::StopRequest, now).and_then(|_| self.tasks.event(p.task, TaskEvent::StopConfirmed, now)).map(|_| ()),
        };
        if self.fpu_owner == Some(i) {
            self.fpu_owner = None; // its registers are nobody's now, and are replaced before any program runs again
        }
        dev::sound_quiet();
        dev::fm_quiet();
        if end == End::Stopped || matches!(end, End::Refused { .. }) || end == End::OverBudget {
            // A program that did not leave of its own accord may have left the display in a state of its own.
            dev::display_show(0);
        }
    }

    /// Resume the next process that can run, looking from `from` round the table. With none able to run
    /// and one waiting, idle until an interrupt. With none left, resume the kernel where it asked to run
    /// them, in its own address space.
    fn next(&mut self, frame: &mut Frame, from: usize) {
        let n = self.procs.len();
        let now = arch::ticks();
        let key = dev::keys_waiting();
        for p in self.procs.iter_mut() {
            match p.waiting {
                Waiting::Until(t) if now >= t => p.waiting = Waiting::Nothing,
                Waiting::Key if key => p.waiting = Waiting::Nothing,
                _ => {}
            }
        }
        for k in 0..n {
            let i = (from + k) % n;
            if self.procs[i].end.is_none() && self.procs[i].waiting == Waiting::Nothing {
                return self.resume(i, frame);
            }
        }
        self.current = None;
        arch::kernel_space();
        if self.procs.iter().any(|p| p.end.is_none()) {
            *frame = Frame::idle();
            return;
        }
        match self.kernel.take() {
            Some(k) => *frame = k,
            None => crate::fault(255, 2, 0, 0),
        }
    }

    /// Make process `i` the one the processor runs: its address space, its floating-point registers, its FS base.
    fn resume(&mut self, i: usize, frame: &mut Frame) {
        self.current = Some(i);
        if let Some(space) = self.procs[i].space.as_ref() {
            space.activate();
        }
        if self.fpu_owner != Some(i) {
            if let Some(o) = self.fpu_owner {
                arch::fpu_save(&mut self.procs[o].fpu);
            }
            arch::fpu_restore(&self.procs[i].fpu);
            self.fpu_owner = Some(i);
        }
        if self.fs_loaded != self.procs[i].fs_base {
            arch::set_user_fs(self.procs[i].fs_base);
            self.fs_loaded = self.procs[i].fs_base;
        }
        *frame = self.procs[i].frame;
    }
}

/// How many frames the pool has free: for the kernel's accounting before and after a program.
pub fn frames_free() -> usize {
    mem::census().1
}
