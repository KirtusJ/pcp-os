//! Tasks: what is running, for whom, under what limits, and what is known about how it ended.
//!
//! The table is the data behind the task manager. It is deliberately small:
//!
//! * fixed capacity, no heap: a full table refuses a new task, it never grows;
//! * every change of a task's state goes through the [`crate::machine::TASK`] table;
//! * a task is never `DONE` on its own word: it reports a result digest when it exits, and it is `DONE`
//!   only when the caller hands in the digest actually read back and the two are equal;
//! * a task that is not heard from within its quiet limit becomes `LOST` at the next sweep;
//! * spending past a budget stops the task.
//!
//! Time is a count of ticks supplied by the caller. The table does not read a clock.
use crate::machine::{Refusal, Task, TaskEvent};

/// Longest task label, in bytes.
pub const LABEL_BYTES: usize = 32;

/// What kind of worker runs the task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Worker {
    /// A program whose behaviour is fixed by its code and inputs.
    Program,
    /// A process driven by a language model.
    Model,
}

/// One task.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// The task's number. Never reused within one table.
    pub id: u32,
    label: [u8; LABEL_BYTES],
    label_len: u8,
    /// The kind of worker.
    pub worker: Worker,
    /// The review the task belongs to, as a number chosen by the caller; 0 for none.
    pub review: u32,
    /// The task's state.
    pub state: Task,
    /// Tick at which the task was admitted.
    pub admitted: u64,
    /// Tick at which the task was last heard from.
    pub heard: u64,
    /// Longest silence allowed, in ticks.
    pub quiet_limit: u64,
    /// Units the task may spend (model tokens, seconds, calls: the caller's unit).
    pub budget: u64,
    /// Units spent so far.
    pub spent: u64,
    /// True if the task was stopped because it spent past its budget.
    pub over_budget: bool,
    claimed: Option<[u8; 32]>,
}

impl Row {
    /// The label.
    pub fn label(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len as usize]).unwrap_or("?")
    }

    /// The result digest the task reported when it exited, if it has exited.
    pub fn claimed_result(&self) -> Option<&[u8; 32]> {
        self.claimed.as_ref()
    }

    /// Whether this task needs a person's response, and how urgently. `None` is the normal case.
    pub fn attention(&self) -> Option<Priority> {
        match self.state {
            Task::Lost => Some(Priority::High),
            Task::Stopping => Some(Priority::High),
            Task::Failed => Some(Priority::Medium),
            Task::Stopped if self.over_budget => Some(Priority::Low),
            _ => None,
        }
    }
}

/// Urgency of something that needs a response. Three levels and no more.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Respond when convenient.
    Low,
    /// Respond in the course of the work.
    Medium,
    /// Respond now: something may still be acting, or the state of something is unknown.
    High,
}

/// Why a task operation is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskError {
    /// The table is full.
    Full,
    /// The table is sealed: its set of tasks is fixed.
    Sealed,
    /// No task has this number.
    Unknown,
    /// The label is empty, too long, or not printable ASCII.
    BadLabel,
    /// The task's machine does not accept the event in its present state.
    Refused(Refusal),
    /// The task is not in a final state, so its row cannot be released.
    NotFinal,
}

/// A table of at most `N` tasks.
pub struct TaskTable<const N: usize> {
    rows: [Option<Row>; N],
    next_id: u32,
    sealed: bool,
}

impl<const N: usize> Default for TaskTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> TaskTable<N> {
    /// An empty table.
    pub const fn new() -> Self {
        TaskTable { rows: [None; N], next_id: 1, sealed: false }
    }

    /// Fix the set of tasks: from now on no task is admitted. For installations whose tasks are all known
    /// when the system is built.
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Admit a task. It starts `CREATED`.
    pub fn admit(&mut self, label: &str, worker: Worker, review: u32, now: u64, quiet_limit: u64, budget: u64) -> Result<u32, TaskError> {
        if self.sealed {
            return Err(TaskError::Sealed);
        }
        let b = label.as_bytes();
        if b.is_empty() || b.len() > LABEL_BYTES || !b.iter().all(|c| (0x21..0x7f).contains(c)) {
            return Err(TaskError::BadLabel);
        }
        let slot = self.rows.iter().position(|r| r.is_none()).ok_or(TaskError::Full)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or(TaskError::Full)?;
        let mut l = [0u8; LABEL_BYTES];
        l[..b.len()].copy_from_slice(b);
        self.rows[slot] = Some(Row { id, label: l, label_len: b.len() as u8, worker, review, state: Task::Created, admitted: now, heard: now,
                                     quiet_limit, budget, spent: 0, over_budget: false, claimed: None });
        Ok(id)
    }

    fn row_mut(&mut self, id: u32) -> Result<&mut Row, TaskError> {
        self.rows.iter_mut().flatten().find(|r| r.id == id).ok_or(TaskError::Unknown)
    }

    /// The task with this number.
    pub fn get(&self, id: u32) -> Option<&Row> {
        self.rows.iter().flatten().find(|r| r.id == id)
    }

    /// Every task, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().flatten()
    }

    /// Offer an event to a task. An accepted event counts as hearing from the task, unless it is one the
    /// system itself produces (`DEADLINE`, `STOP_REQUEST`, `RESULT_*`).
    pub fn event(&mut self, id: u32, event: TaskEvent, now: u64) -> Result<Task, TaskError> {
        let r = self.row_mut(id)?;
        let next = r.state.step(event).map_err(TaskError::Refused)?;
        r.state = next;
        if !matches!(event, TaskEvent::Deadline | TaskEvent::StopRequest | TaskEvent::ResultVerified | TaskEvent::ResultMissing) {
            r.heard = now;
        }
        Ok(next)
    }

    /// The task reports that it has ended successfully and names the digest of its result.
    pub fn exit_ok(&mut self, id: u32, result: [u8; 32], now: u64) -> Result<Task, TaskError> {
        let next = self.event(id, TaskEvent::ExitOk, now)?;
        if next == Task::Exited {
            self.row_mut(id)?.claimed = Some(result);
        }
        Ok(next)
    }

    /// Settle an exited task against what was actually read back: the digest found, or `None` if nothing
    /// was found. Equal digests make the task `DONE`; anything else makes it `FAILED`.
    pub fn settle(&mut self, id: u32, found: Option<[u8; 32]>) -> Result<Task, TaskError> {
        let claimed = self.row_mut(id)?.claimed;
        let ev = match (claimed, found) {
            (Some(c), Some(f)) if c == f => TaskEvent::ResultVerified,
            _ => TaskEvent::ResultMissing,
        };
        self.event(id, ev, 0)
    }

    /// Record spending. Spending past the budget asks the task to stop and says so.
    pub fn charge(&mut self, id: u32, units: u64, now: u64) -> Result<bool, TaskError> {
        let r = self.row_mut(id)?;
        if r.state.is_final() {
            return Err(TaskError::Refused(Refusal { machine: "TASK", state: r.state.name(), event: "CHARGE" }));
        }
        r.spent = r.spent.saturating_add(units);
        r.heard = now;
        if r.spent > r.budget {
            r.over_budget = true;
            let _ = self.event(id, TaskEvent::StopRequest, now)?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Apply `DEADLINE` to every task that has been silent longer than its quiet limit. Returns how many
    /// tasks changed state. This is the watchdog; call it from the timer.
    pub fn sweep(&mut self, now: u64) -> usize {
        let mut n = 0;
        for r in self.rows.iter_mut().flatten() {
            if !r.state.is_final() && now.saturating_sub(r.heard) > r.quiet_limit {
                if let Ok(next) = r.state.step(TaskEvent::Deadline) {
                    r.state = next;
                    n += 1;
                }
            }
        }
        n
    }

    /// Ask every task that is not in a final state to stop; with `review` non-zero, only that review's
    /// tasks. Returns how many were asked.
    pub fn stop_all(&mut self, review: u32) -> usize {
        let mut n = 0;
        for r in self.rows.iter_mut().flatten() {
            if !r.state.is_final() && (review == 0 || r.review == review) {
                if let Ok(next) = r.state.step(TaskEvent::StopRequest) {
                    r.state = next;
                    n += 1;
                }
            }
        }
        n
    }

    /// How many tasks a [`TaskTable::stop_all`] with the same argument would ask to stop. The task manager
    /// shows this before a halt is confirmed.
    pub fn would_stop(&self, review: u32) -> usize {
        self.iter().filter(|r| !r.state.is_final() && (review == 0 || r.review == review)).count()
    }

    /// Release the row of a task that is in a final state.
    pub fn release(&mut self, id: u32) -> Result<(), TaskError> {
        for slot in self.rows.iter_mut() {
            if let Some(r) = slot {
                if r.id == id {
                    if !r.state.is_final() {
                        return Err(TaskError::NotFinal);
                    }
                    *slot = None;
                    return Ok(());
                }
            }
        }
        Err(TaskError::Unknown)
    }

    /// Counts of tasks needing a response, by priority: (high, medium, low).
    pub fn attention(&self) -> (usize, usize, usize) {
        let mut c = (0, 0, 0);
        for r in self.iter() {
            match r.attention() {
                Some(Priority::High) => c.0 += 1,
                Some(Priority::Medium) => c.1 += 1,
                Some(Priority::Low) => c.2 += 1,
                None => {}
            }
        }
        c
    }
}

/// Register-wide limits for a tree of admitted tasks. A root is at depth zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecursionLimits {
    /// Greatest admitted distance from a root.
    pub max_depth: u16,
    /// Maximum number of simultaneously outstanding admissions.
    pub max_live: u16,
    /// Maximum cumulative admissions, including finished or cancelled tasks.
    pub max_total: u32,
}

impl RecursionLimits {
    /// Check the configured limits. A depth of zero allows roots only.
    pub fn validate(self) -> Result<(), &'static str> {
        if self.max_live == 0 || self.max_total == 0 {
            return Err("live and total limits must be positive");
        }
        Ok(())
    }

    /// Check one proposed admission against the counts immediately before it.
    pub fn admit(self, depth: u32, live: u32, total: u32) -> Result<(), &'static str> {
        self.validate()?;
        if total >= self.max_total {
            return Err("total node limit reached");
        }
        if live >= u32::from(self.max_live) {
            return Err("live node limit reached");
        }
        if depth > u32::from(self.max_depth) {
            return Err("depth limit reached");
        }
        Ok(())
    }
}
