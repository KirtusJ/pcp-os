//! The state machines of ACR and of the system, in one form.
//!
//! Every machine here is a complete table: for each state and each event the table says either which state
//! follows or that the event is not accepted. Nothing is left to a default, so "what happens if this arrives
//! now" always has an answer that can be read off and tested exhaustively.
//!
//! Three rules hold for all of them, and are what lets them be composed without surprises:
//!
//! 1. A machine has no memory besides its state. Its state is a function of the register: fold the events,
//!    get the state. Two parties with the same register compute the same state.
//! 2. Machines contain one another (system > use of ACR > review > task). A contained machine accepts an
//!    event only if every machine around it is in a state that permits that class of event ([`permits`]),
//!    and a containing machine's transition may force one event on what it contains ([`cascade_of_halt`],
//!    [`cascade_of_system`]). There is no other coupling.
//! 3. Machines beside the containment tree (connectivity, delivery of the method text) never stop work.
//!    They change what may be claimed, which is computed elsewhere from their state.
//!
//! This module uses `core` only: no heap, no `unsafe`, no dependency.

/// What a machine answers when it is offered an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The event is accepted; the machine is now in this state (possibly the same one).
    To(u8),
    /// The event is not accepted in this state; nothing changes.
    No,
}

/// A machine as data: names and one complete table.
pub struct Spec {
    /// The machine's name, as it appears in records and reports.
    pub name: &'static str,
    /// State names, in table order.
    pub states: &'static [&'static str],
    /// Event names, in table order.
    pub events: &'static [&'static str],
    /// The state a new instance starts in.
    pub initial: u8,
    /// Final states, as a bit set. A final state accepts nothing.
    pub finals: u32,
    /// `table[state][event]`.
    pub table: &'static [&'static [Step]],
}

/// An event that a machine did not accept, with the names needed to say so plainly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The machine.
    pub machine: &'static str,
    /// The state it was in.
    pub state: &'static str,
    /// The event it was offered.
    pub event: &'static str,
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {} is not accepted in state {}", self.machine, self.event, self.state)
    }
}

/// Defects [`Spec::defects`] can find in a table.
pub mod defect {
    /// The table does not have one row per state and one cell per event, or has more than 32 states.
    pub const SHAPE: u32 = 1;
    /// A cell names a state that does not exist.
    pub const TARGET: u32 = 2;
    /// A state cannot be reached from the initial state.
    pub const UNREACHABLE: u32 = 4;
    /// A final state accepts an event.
    pub const FINAL_ACCEPTS: u32 = 8;
    /// The machine has final states and some state cannot reach any of them.
    pub const TRAP: u32 = 16;
    /// An event is accepted nowhere.
    pub const DEAD_EVENT: u32 = 32;
}

impl Spec {
    /// The table's answer. An index outside the table is answered `No`.
    pub fn step(&self, state: u8, event: u8) -> Step {
        match self.table.get(state as usize).and_then(|row| row.get(event as usize)) {
            Some(s) => *s,
            None => Step::No,
        }
    }

    /// The bit set of all states.
    pub fn all(&self) -> u32 {
        if self.states.len() >= 32 {
            u32::MAX
        } else {
            (1u32 << self.states.len()) - 1
        }
    }

    fn successors(&self, set: u32) -> u32 {
        let mut out = 0u32;
        for s in 0..self.states.len().min(32) {
            if set & (1 << s) != 0 {
                for e in 0..self.events.len() {
                    if let Step::To(n) = self.step(s as u8, e as u8) {
                        if (n as usize) < 32 {
                            out |= 1 << n;
                        }
                    }
                }
            }
        }
        out
    }

    /// States reachable from the initial state.
    pub fn reachable(&self) -> u32 {
        let mut set = 1u32 << self.initial;
        loop {
            let next = set | self.successors(set);
            if next == set {
                return set;
            }
            set = next;
        }
    }

    /// States from which some state of `targets` can be reached (including the targets themselves).
    pub fn can_reach(&self, targets: u32) -> u32 {
        let mut set = targets & self.all();
        loop {
            let mut next = set;
            for s in 0..self.states.len().min(32) {
                if self.successors(1 << s) & set != 0 {
                    next |= 1 << s;
                }
            }
            if next == set {
                return set;
            }
            set = next;
        }
    }

    /// A bit set of [`defect`] values; zero means the table is well formed.
    pub fn defects(&self) -> u32 {
        let mut d = 0;
        let n = self.states.len();
        if n == 0 || n > 32 || self.table.len() != n || self.table.iter().any(|r| r.len() != self.events.len()) || self.initial as usize >= n {
            return defect::SHAPE;
        }
        for row in self.table {
            for cell in *row {
                if let Step::To(t) = cell {
                    if *t as usize >= n {
                        d |= defect::TARGET;
                    }
                }
            }
        }
        if d != 0 {
            return d;
        }
        if self.reachable() != self.all() {
            d |= defect::UNREACHABLE;
        }
        for s in 0..n {
            if self.finals & (1 << s) != 0 && self.table[s].iter().any(|c| *c != Step::No) {
                d |= defect::FINAL_ACCEPTS;
            }
        }
        if self.finals != 0 && self.can_reach(self.finals) != self.all() {
            d |= defect::TRAP;
        }
        for e in 0..self.events.len() {
            if self.table.iter().all(|row| row[e] == Step::No) {
                d |= defect::DEAD_EVENT;
            }
        }
        d
    }
}

macro_rules! cell {
    ($S:ident, -) => {
        Step::No
    };
    ($S:ident, $s:ident) => {
        Step::To($S::$s as u8)
    };
}

macro_rules! machine {
    ($(#[$m:meta])* $SPEC:ident = $name:literal;
     $(#[$sm:meta])* states $S:ident { $($sv:ident = $sn:literal),+ $(,)? }
     $(#[$em:meta])* events $E:ident { $($ev:ident = $en:literal),+ $(,)? }
     initial $init:ident; finals [$($f:ident),*];
     table { $($row:ident: [$($c:tt),+ $(,)?]),+ $(,)? }) => {
        $(#[$sm])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum $S { $(#[doc = $sn] $sv),+ }
        $(#[$em])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum $E { $(#[doc = $en] $ev),+ }
        impl $S {
            /// Every state, in table order.
            pub const ALL: &'static [$S] = &[$($S::$sv),+];
            /// The state's name as it appears in records.
            pub const fn name(self) -> &'static str { match self { $($S::$sv => $sn),+ } }
            /// The state with this name.
            pub fn from_name(n: &str) -> Option<$S> { $(if n == $sn { return Some($S::$sv); })+ None }
            /// Offer an event. Either the state that follows, or the refusal; nothing else can happen.
            pub fn step(self, e: $E) -> Result<$S, Refusal> {
                match $SPEC.step(self as u8, e as u8) {
                    Step::To(n) => Ok(Self::ALL[n as usize]),
                    Step::No => Err(Refusal { machine: $name, state: self.name(), event: e.name() }),
                }
            }
            /// True for a final state.
            pub const fn is_final(self) -> bool { $SPEC.finals & (1u32 << (self as u8)) != 0 }
        }
        impl $E {
            /// Every event, in table order.
            pub const ALL: &'static [$E] = &[$($E::$ev),+];
            /// The event's name as it appears in records.
            pub const fn name(self) -> &'static str { match self { $($E::$ev => $en),+ } }
            /// The event with this name.
            pub fn from_name(n: &str) -> Option<$E> { $(if n == $en { return Some($E::$ev); })+ None }
        }
        const _: () = {
            let rows = [$($S::$row as u8),+];
            let mut i = 0;
            while i < rows.len() { assert!(rows[i] as usize == i, "table rows are in state order"); i += 1; }
        };
        $(#[$m])*
        pub static $SPEC: Spec = Spec {
            name: $name,
            states: &[$($sn),+],
            events: &[$($en),+],
            initial: $S::$init as u8,
            finals: 0 $(| (1u32 << ($S::$f as u8)))*,
            table: &[$(&[$(cell!($S, $c)),+]),+],
        };
    };
}

machine! {
    /// The system as a whole, from the first instruction to the last.
    ///
    /// `SAFE` is the state the system takes when it finds its own integrity in doubt: effects stop, the record
    /// is kept, reading and halting remain available. It is left only by stopping; operation resumes only
    /// through a fresh start and a fresh verification. After maintenance the system verifies itself again
    /// before it operates.
    SYSTEM = "SYSTEM";
    /// States of the system.
    states System { Start = "START", Verify = "VERIFY", Operational = "OPERATIONAL", Degraded = "DEGRADED",
                    Maintenance = "MAINTENANCE", Safe = "SAFE", Stopping = "STOPPING", Stopped = "STOPPED" }
    /// Events of the system.
    events SystemEvent { BeginVerify = "BEGIN_VERIFY", VerifiedOk = "VERIFIED_OK", VerifyFailed = "VERIFY_FAILED",
                         ServiceLost = "SERVICE_LOST", ServiceRestored = "SERVICE_RESTORED",
                         MaintenanceBegin = "MAINTENANCE_BEGIN", MaintenanceEnd = "MAINTENANCE_END",
                         IntegrityFault = "INTEGRITY_FAULT", Shutdown = "SHUTDOWN", StoppedOk = "STOPPED_OK" }
    initial Start; finals [Stopped];
    table {
        //             BEGIN_V  VER_OK       VER_FAIL SVC_LOST  SVC_REST     MAINT_B      MAINT_E FAULT    SHUTDOWN  STOPPED_OK
        Start:       [ Verify,  -,           -,       -,        -,           -,           -,      Safe,    Stopping, -       ],
        Verify:      [ -,       Operational, Safe,    -,        -,           -,           -,      Safe,    Stopping, -       ],
        Operational: [ -,       -,           -,       Degraded, -,           Maintenance, -,      Safe,    Stopping, -       ],
        Degraded:    [ -,       -,           -,       Degraded, Operational, Maintenance, -,      Safe,    Stopping, -       ],
        Maintenance: [ -,       -,           -,       -,        -,           -,           Verify, Safe,    Stopping, -       ],
        Safe:        [ -,       -,           -,       -,        -,           -,           -,      Safe,    Stopping, -       ],
        Stopping:    [ -,       -,           -,       -,        -,           -,           -,      Stopped, -,        Stopped ],
        Stopped:     [ -,       -,           -,       -,        -,           -,           -,      -,       -,        -       ],
    }
}

machine! {
    /// Whether ACR is in use in a workspace. Use begins and ends by the direction of the person directing
    /// the work, or of the host. A halt is accepted in every state.
    USE = "USE";
    /// States of ACR use.
    states Use { NotInUse = "NOT_IN_USE", InUse = "IN_USE" }
    /// Events of ACR use.
    events UseEvent { Adopt = "ADOPT", Halt = "HALT" }
    initial NotInUse; finals [];
    table {
        NotInUse: [ InUse, NotInUse ],
        InUse:    [ InUse, NotInUse ],
    }
}

machine! {
    /// One session of a review. The three closed states seal the session's assessment: nothing is added to
    /// it, and renewed assessment is a successor session.
    REVIEW = "REVIEW";
    /// States of a review session.
    states Review { Active = "ACTIVE", Paused = "PAUSED", ClosedComplete = "CLOSED_COMPLETE",
                    ClosedIncomplete = "CLOSED_INCOMPLETE", Aborted = "ABORTED" }
    /// Events of a review session.
    events ReviewEvent { Pause = "PAUSE", Resume = "RESUME", CloseComplete = "CLOSE_COMPLETE",
                         CloseIncomplete = "CLOSE_INCOMPLETE", Abort = "ABORT", Halt = "HALT", OpenSession = "OPEN_SESSION" }
    initial Active; finals [];
    table {
        //                  PAUSE   RESUME  CLOSE_COMPLETE  CLOSE_INCOMPLETE  ABORT    HALT              OPEN_SESSION
        Active:           [ Paused, -,      ClosedComplete, ClosedIncomplete, Aborted, ClosedIncomplete, -      ],
        Paused:           [ -,      Active, -,              ClosedIncomplete, Aborted, ClosedIncomplete, -      ],
        ClosedComplete:   [ -,      -,      -,              -,                -,       -,                Active ],
        ClosedIncomplete: [ -,      -,      -,              -,                -,       -,                Active ],
        Aborted:          [ -,      -,      -,              -,                -,       -,                Active ],
    }
}

machine! {
    /// An objection raised in a review.
    OBJECTION = "OBJECTION";
    /// States of an objection.
    states Objection { Open = "OPEN", Resolved = "RESOLVED", Withdrawn = "WITHDRAWN", OpenRecorded = "OPEN_RECORDED" }
    /// Events of an objection.
    events ObjectionEvent { Resolve = "RESOLVE", Withdraw = "WITHDRAW", RecordOpen = "RECORD_OPEN" }
    initial Open; finals [Resolved, Withdrawn, OpenRecorded];
    table {
        Open:         [ Resolved, Withdrawn, OpenRecorded ],
        Resolved:     [ -, -, - ],
        Withdrawn:    [ -, -, - ],
        OpenRecorded: [ -, -, - ],
    }
}

machine! {
    /// An obligation carried by the work.
    OBLIGATION = "OBLIGATION";
    /// States of an obligation.
    states Obligation { Open = "OPEN", Closed = "CLOSED" }
    /// Events of an obligation.
    events ObligationEvent { Discharge = "DISCHARGE" }
    initial Open; finals [Closed];
    table {
        Open:   [ Closed ],
        Closed: [ - ],
    }
}

machine! {
    /// A task: one piece of work done by one process, a model-driven one or not.
    ///
    /// Two things are built into the table. A task that reports its own end is `EXITED`, not `DONE`: it is
    /// `DONE` only when its result has been read back and matches what the task reported
    /// (`RESULT_VERIFIED`). And a task that stops being heard from before its deadline is `LOST`, which is
    /// a different fact from `FAILED` and is never silently turned into either success or failure.
    TASK = "TASK";
    /// States of a task.
    states Task { Created = "CREATED", Running = "RUNNING", Waiting = "WAITING", Stopping = "STOPPING",
                  Exited = "EXITED", Done = "DONE", Failed = "FAILED", Stopped = "STOPPED", Lost = "LOST" }
    /// Events of a task.
    events TaskEvent { Start = "START", Block = "BLOCK", Wake = "WAKE", Heartbeat = "HEARTBEAT", ExitOk = "EXIT_OK",
                       ExitFail = "EXIT_FAIL", ResultVerified = "RESULT_VERIFIED", ResultMissing = "RESULT_MISSING",
                       StopRequest = "STOP_REQUEST", StopConfirmed = "STOP_CONFIRMED", Deadline = "DEADLINE" }
    initial Created; finals [Done, Failed, Stopped, Lost];
    table {
        //          START    BLOCK    WAKE     HEARTBEAT EXIT_OK  EXIT_FAIL RES_VER RES_MISS STOP_REQ  STOP_CONF DEADLINE
        Created:  [ Running, -,       -,       -,        -,       -,        -,      -,       Stopped,  -,        Lost   ],
        Running:  [ -,       Waiting, -,       Running,  Exited,  Failed,   -,      -,       Stopping, -,        Lost   ],
        Waiting:  [ -,       -,       Running, Waiting,  -,       Failed,   -,      -,       Stopping, -,        Lost   ],
        Stopping: [ -,       -,       -,       -,        Stopped, Stopped,  -,      -,       Stopping, Stopped,  Lost   ],
        Exited:   [ -,       -,       -,       -,        -,       -,        Done,   Failed,  -,        -,        Failed ],
        Done:     [ -,       -,       -,       -,        -,       -,        -,      -,       -,        -,        -      ],
        Failed:   [ -,       -,       -,       -,        -,       -,        -,      -,       -,        -,        -      ],
        Stopped:  [ -,       -,       -,       -,        -,       -,        -,      -,       -,        -,        -      ],
        Lost:     [ -,       -,       -,       -,        -,       -,        -,      -,       -,        -,        -      ],
    }
}

machine! {
    /// Whether an agent has been given the method text in its present context. `STALE` means it was given
    /// and the context has since been reset, or the pinned release has changed: the agent may no longer
    /// hold what was delivered, and nothing here assumes that it does.
    DELIVERY = "DELIVERY";
    /// States of delivery.
    states Delivery { NotDelivered = "NOT_DELIVERED", Delivered = "DELIVERED", Stale = "STALE" }
    /// Events of delivery.
    events DeliveryEvent { Deliver = "DELIVER", ContextReset = "CONTEXT_RESET", ReleaseChanged = "RELEASE_CHANGED" }
    initial NotDelivered; finals [];
    table {
        NotDelivered: [ Delivered, NotDelivered, NotDelivered ],
        Delivered:    [ Delivered, Stale,        Stale        ],
        Stale:        [ Delivered, Stale,        Stale        ],
    }
}

machine! {
    /// Whether retrieval from outside the machine is available. The system starts `OFFLINE`: nothing is on
    /// the network until someone puts it there.
    LINK = "LINK";
    /// States of connectivity.
    states Link { Offline = "OFFLINE", Online = "ONLINE" }
    /// Events of connectivity.
    events LinkEvent { Up = "UP", Down = "DOWN" }
    initial Offline; finals [];
    table {
        Offline: [ Online, Offline ],
        Online:  [ Online, Offline ],
    }
}

/// Every machine, for checks that run over all of them.
pub static ALL: [&Spec; 8] = [&SYSTEM, &USE, &REVIEW, &OBJECTION, &OBLIGATION, &TASK, &DELIVERY, &LINK];

// ---- gates: an ordered prefix ---------------------------------------------------------------------

/// The six gates, in order.
pub const GATES: [&str; 6] = ["G1", "G2", "G3", "G4", "G5", "G6"];
/// Gates every procedure requires, as a bit set over [`GATES`]: G1, G5 and G6.
pub const MINIMUM_GATES: u8 = 0b110001;
/// All six gates as a bit set.
pub const ALL_GATES: u8 = 0b111111;

/// Why a gate cannot be committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateRefusal {
    /// The gate is not one of this review's required gates.
    NotRequired,
    /// The gate is already committed in this session.
    AlreadyCommitted,
    /// An earlier required gate is not yet committed; this is its index in [`GATES`].
    Before(u8),
    /// The set of required gates lacks G1, G5 or G6, or names a gate that does not exist.
    BadRequiredSet,
}

/// Commit gate `gate` (an index into [`GATES`]) in a session whose required gates are `required` and whose
/// committed gates are `passed`, both bit sets. Committed gates always form an ordered prefix of the
/// required ones. Returns the new committed set.
pub fn gate_step(required: u8, passed: u8, gate: u8) -> Result<u8, GateRefusal> {
    if required & !ALL_GATES != 0 || required & MINIMUM_GATES != MINIMUM_GATES || passed & !required != 0 || gate > 5 {
        return Err(GateRefusal::BadRequiredSet);
    }
    let bit = 1u8 << gate;
    if required & bit == 0 {
        return Err(GateRefusal::NotRequired);
    }
    if passed & bit != 0 {
        return Err(GateRefusal::AlreadyCommitted);
    }
    let pending = required & !passed;
    let next = pending.trailing_zeros() as u8;
    if next != gate {
        return Err(GateRefusal::Before(next));
    }
    Ok(passed | bit)
}

// ---- composition ----------------------------------------------------------------------------------

/// Classes of register event, by what they need from the machines around them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Written by the system about itself.
    Host,
    /// A halt of ACR use. Accepted in every system state in which anything runs.
    Halt,
    /// Adoption of ACR use, and administration.
    Direction,
    /// Material and observations: artifacts, notes, checks, inspections.
    Record,
    /// A step of a review: commissioning, claims, gates, objections, findings, closing, sessions.
    Review,
    /// The beginning or end of a task.
    Task,
}

/// Why a class of event is not accepted now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotPermitted {
    /// The system is in a state that does not accept this class of event; the state is given.
    System(System),
    /// A review step was offered while ACR is not in use.
    NotInUse,
}

/// The one coupling rule from the outside in: may an event of this class be accepted, given the state of
/// the system and of ACR use?
pub fn permits(system: System, in_use: Use, class: Class) -> Result<(), NotPermitted> {
    use System::*;
    let ok = match (system, class) {
        (Stopped, _) => false,
        (_, Class::Host) => true,
        (Start | Verify | Stopping, _) => false,
        (_, Class::Halt) => true,
        (Safe | Maintenance, _) => false,
        (Operational | Degraded, _) => true,
    };
    if !ok {
        return Err(NotPermitted::System(system));
    }
    if class == Class::Review && in_use != Use::InUse {
        return Err(NotPermitted::NotInUse);
    }
    Ok(())
}

/// What a halt of ACR use forces on a review in a given state: the event to apply, or nothing for a
/// session that is already sealed. Every task of a review that receives the event receives
/// [`TaskEvent::StopRequest`].
pub fn cascade_of_halt(review: Review) -> Option<ReviewEvent> {
    match review {
        Review::Active | Review::Paused => Some(ReviewEvent::Halt),
        _ => None,
    }
}

/// What a transition of the system forces on every task that is not in a final state. Reviews are never
/// touched by a system transition: they are in the record, and stay as they were until someone with the
/// standing to do so acts on them after the next verified start.
pub fn cascade_of_system(event: SystemEvent) -> Option<TaskEvent> {
    match event {
        SystemEvent::IntegrityFault | SystemEvent::VerifyFailed | SystemEvent::Shutdown | SystemEvent::MaintenanceBegin => Some(TaskEvent::StopRequest),
        _ => None,
    }
}

/// Write every machine's whole table as text, one line per state and event. The Python implementation
/// writes the same text from its own, separately kept statement of the tables; the two are compared byte
/// for byte.
pub fn export<W: core::fmt::Write>(w: &mut W) -> core::fmt::Result {
    writeln!(w, "# machine\tstate\tevent\tnext    ('-' = not accepted)    initial and final states are marked in the 'state' rows")?;
    for m in ALL.iter() {
        write!(w, "{}\tINITIAL\t{}\tFINAL\t", m.name, m.states[m.initial as usize])?;
        let mut first = true;
        for (i, s) in m.states.iter().enumerate() {
            if m.finals & (1 << i) != 0 {
                write!(w, "{}{}", if first { "" } else { "," }, s)?;
                first = false;
            }
        }
        writeln!(w, "{}", if first { "-" } else { "" })?;
        for (i, s) in m.states.iter().enumerate() {
            for (j, e) in m.events.iter().enumerate() {
                match m.step(i as u8, j as u8) {
                    Step::To(n) => writeln!(w, "{}\t{}\t{}\t{}", m.name, s, e, m.states[n as usize])?,
                    Step::No => writeln!(w, "{}\t{}\t{}\t-", m.name, s, e)?,
                }
            }
        }
    }
    Ok(())
}
