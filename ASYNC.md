# Asynchronous work in PCP OS

**Standing note for everyone who works on PCP OS. Set by the rights holder, Kirtus Justus, 14 November 2026: asynchronous work is core to PCP and is considered at all times. Rust's `async` and the Tokio runtime have a long and well-known history of hazards; that history is to be considered deeply.** Read with `RUST_PRIMACY.md`.

This file is the standing navigation. It states the architecture as observed, the history to hold in mind, the invariants intended, and where the concrete tests and notes are. It is not a ban: a path that needs `async` or a runtime makes its own engineering case (see "Bringing asynchronous Rust in").

## The architecture as observed

Checked on `master` at `6254a71`. This is a description, not a rule.

- No `async fn`, no `.await`, no futures, no Tokio and no other runtime in the Rust workspace. One external dependency, `sha2`, in `pcpos-core`.
- `pcpos-core` is shared with the kernel and builds without the standard library.
- The **host** (`pcpos-host`) is a synchronous program. Concurrency between hosts meets at the register file: a lock, an append, and a repair that must respect the lock.
- The **kernel** (`pcpos-kernel`) does its own asynchronous work without a runtime: a 1 kHz timer, preemption, a process table scanned in turn, level-triggered wake-ups, interrupts off whenever the kernel runs.
- In the hosted path (`acr_submit`) both `TASK_STARTED` and `TASK_ENDED` are written by the host from what it observes; a participant cannot submit either. Other actors at the level of `pcpos-core` (the kernel's own register, for one) have their own rules.

Faults on the asynchronous side have been found here **without** Tokio (see the record at the end). The absence of a runtime is not the absence of the problem.

## The history to hold in mind

Stated per operation, because the hazards are specific. Sources are linked; where a line is general knowledge and not checked against a source for this note, it says so.

| Matter | What is the case | What it bears on here |
|---|---|---|
| Dropping a future | Polling stops and the synchronous drop glue of what it owns runs. Code after the suspension point does not run. Whether that is safe depends on the operation: Tokio documents which of its operations are cancellation safe in `select!` ([cancellation safety](https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety)). An external write can have had a partial effect. | A register append, a lock release or a task end placed after a suspension point. |
| Dropping a `JoinHandle` | The spawned task is detached and keeps running; its panic is then not observed by anyone ([JoinHandle](https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html)). `JoinSet` and explicit join or drain protocols supply supervision. | "Every task has an end that is recorded" needs someone to hold the handle. |
| Blocking work | A blocking call on an executor thread stalls the tasks on it. `spawn_blocking` moves it to a pool, and a closure that has started there generally cannot be aborted ([blocking work](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)). | The host's work is file input and output under a lock. |
| Cooperative scheduling | A task that never yields starves others; Tokio added a per-task budget for this ([history](https://tokio.rs/blog/2020-04-preemption)). | The kernel preempts; a hosted executor would not. |
| Locks and suspension | A standard mutex held across a suspension point can block the executor or deadlock. Tokio's mutex is made to be held across one; it has no poison flag, so a panic while holding it does not mark the data. General knowledge. | Register integrity depends on the lock and on noticing a writer that died. |
| Clean-up that must wait | There is no asynchronous drop. A clean-up that itself has to wait needs an explicit protocol. General knowledge. | Stop asked, stop confirmed. |
| Queues | A channel without a bound turns a slow reader into memory growth. A bound needs a stated behaviour when it is reached. | A kernel with a fixed frame pool; a host that refuses and says why. |
| Runtimes inside runtimes | Calling `block_on` inside a running runtime, and constructing or dropping a runtime inside one, behave differently and some forms panic. General knowledge. | The host is also used as a library (`pcpos mcp`). |
| `no_std` | The language's futures live in `core`. A particular executor may require the standard library. | The kernel is bare metal; `pcpos-core` is shared with it. |
| Dependencies | A runtime brings a tree of crates. | One dependency today; supply-chain exposure is a security matter. |

## Invariants intended

These are intended invariants, each under an explicit model of delivery and recovery. They are not claims that the source meets them everywhere today.

1. **One end per task.** A task's end is recorded once, whatever ends it: completion, a fault, a stop, a kill while blocked. Where the end cannot be observed (the parent died, an exit was never delivered) the status is *unknown* until reconciliation, and is recorded as such, not as an end.
2. **One task per start.** A start delivered twice is one task. Identity of an attempt (stop, retry, duplicate delivery), serialization by the lock, and durability of the acknowledgement are three separate problems; the lock solves only the second.
3. **Stopping is two recorded steps.** Asked, then confirmed. Nothing is treated as stopped because it was asked to stop.
4. **Waiting is bounded where a bound is needed.** Where load or a shutdown protocol makes a bound necessary, the wait has one (a timeout, a budget, a quiet limit), with its source stated and the behaviour at the bound defined. A long-lived listener may wait for its next event.
5. **Queues are bounded where load makes it necessary**, with the recovery at the bound defined.
6. **A fault is shown by a test** that fails before the fix and passes after, in Rust; for the kernel, a self-test in the emulator, with no claim about hardware. A test of a refusal asserts the refusal and carries the nearest accepted case as its control.

## Bringing asynchronous Rust in

Nothing here forbids it. A change that introduces `async`, an executor or a runtime on some path states, for that path: why the present means (separate processes and the register lock; threads) do not serve; for each row of the table that applies, how the matter is handled and which test shows it; and what it does to `pcpos-core`'s use from the kernel and to the dependency list. Both directors review it.

## Where the concrete work is

All of the following is on `master`.

- Kernel, asynchronous side: issues #31 and #32, fixed by pull request #33 (a stopped program left RUNNING in the task table; a refused program's memory not returned), reproduced by both directors on two emulated CPU models. Pull request #37 makes the test of the refusal assert the refusal and carry the nearest accepted case as its control. Issue #35, fixed by pull request #38: when a later member of a group of built-in programs is refused, the earlier members' frames are returned, and a built-in address space gives its frames back at every allocation cutoff (`test=frame-cutoffs`).
- Host, task lifecycle: pull request #40. A start delivered twice made two tasks, and completions of agents whose identifiers share their first 200 characters were assigned to the wrong task; hooks are now matched on the digest of session and agent identifier together, and an old row that could match several tasks is left unresolved. Director G's note with the history and the sources is [`os-docs/Async_execution.md`](os-docs/Async_execution.md).
- Still open: a stop that arrives under a different session identifier than its start leaves the task unended, with nothing yet that reconciles it (the "unknown" status of invariant 1); issue #42 (a subagent transcript that is a named pipe can block every register writer).
