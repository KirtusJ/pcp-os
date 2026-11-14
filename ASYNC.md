# Asynchronous work in PCP OS

**Standing note for everyone who works on PCP OS. Set by the rights holder, Kirtus Justus, 14 November 2026: asynchronous work is core to PCP and is considered at all times. Rust's `async` and the Tokio runtime have a long and well-known history of hazards; that history is to be considered deeply before any of it is brought in.** Read with `RUST_PRIMACY.md`.

## What is true of this source today

Checked on `master` at `0e068ee`:

- No `async fn`, no `.await`, no futures, no Tokio and no other asynchronous runtime anywhere in the Rust workspace. The only external dependency is `sha2`, in `pcpos-core`.
- The **host** (`pcpos-host`) is a synchronous program. Concurrency between hosts is handled at the register file: a lock, an append, and a repair that must respect the lock.
- The **kernel** (`pcpos-kernel`) does its own asynchronous work without a runtime: a 1 kHz timer, preemption, a process table scanned in turn, level-triggered wake-ups, and interrupts that are off whenever the kernel runs.
- A **task** in PCP is a recorded thing: `TASK_STARTED` by a participant, `TASK_ENDED` by the host only, with stop asked and stop confirmed as separate states. Asking is not stopping.

So PCP OS does not have "the Tokio problem" today, because it does not use Tokio. The note below is about keeping it that way on purpose and not by accident.

## The hazards to hold in mind

Each of these is a known failure of asynchronous Rust in practice. Each maps to something PCP's own design already forbids or records; a change that brings one of them in must say how the mapping is kept.

| Hazard in `async` Rust and Tokio | What it would break here |
|---|---|
| **Cancellation by drop.** A future dropped at an `.await` simply stops; code after that point never runs. `select!` and timeouts do this silently to the branches that lose. | A task's end must be a recorded fact. A register append, a lock release or a `TASK_ENDED` that sits after an await can be skipped without any error. |
| **Blocking inside a task.** A synchronous file call, a lock or a long computation on an executor thread stalls every other task on it. | The host's work is file I/O under a lock. Put on an executor it would either block the runtime or be moved to a blocking pool whose tasks cannot be cancelled. |
| **A lock held across an await.** A standard mutex held over a suspension point deadlocks or blocks the executor; the asynchronous mutex has no poisoning and hides panics. | Register integrity depends on the lock and on noticing a writer that died. |
| **Detached tasks.** A spawned task outlives its caller; a panic in it is lost unless its handle is awaited. There is no structured scope. | "Every task ends exactly once and the end is recorded" cannot be shown for a task nobody holds. |
| **No asynchronous drop.** Clean-up that itself needs to wait cannot run when a future is dropped. | Stop asked, stop confirmed: the confirmation is exactly such a clean-up. |
| **Unbounded queues.** Channels without a bound turn a slow reader into memory growth. | A kernel with a fixed frame pool; a host that must refuse and say why, not grow. |
| **Runtime lock-in and a runtime inside a runtime.** Libraries bind to one runtime; starting one inside another panics. | The host is also a library for other programs (`pcpos mcp`, the kernel shares `pcpos-core`). |
| **`'static + Send` bounds and pinning.** They push shared state into reference-counted, lock-wrapped structures and make lifetimes hard to read. | The crate's rule that every `unsafe` and every shared structure states why it is sound. |
| **A large dependency tree.** A runtime brings many crates. | One dependency today; supply-chain exposure is a security matter, and Rust holds primacy here for security reasons. |
| **No `no_std` story for the common runtimes.** | The kernel is bare metal. |

## Rules

1. No asynchronous runtime, and no `async`/`.await`, enters `pcpos-core` or `pcpos-kernel`. `pcpos-core` stays usable from bare metal.
2. If the host ever needs concurrency inside one process, the first choice is what it has: separate processes and the register lock. The second is threads with a bounded queue. An asynchronous runtime is a last choice, and a proposal for it states, for every row of the table above, how the hazard is excluded and which test shows it.
3. Every task, in the host and in the kernel, has an end that is recorded exactly once, whatever ends it: completion, a fault, a stop, a kill while blocked, the death of its parent. A test shows it for each way of ending.
4. Stopping is two steps and both are recorded. Nothing is treated as stopped because it was asked to stop.
5. Anything that waits has a bound: a timeout, a budget or a quiet limit, with the bound's source stated.
6. A fault on the asynchronous side is shown by a test that fails before the fix and passes after, in Rust; for the kernel, a self-test in the emulator, with no claim about hardware.

## Record

- 14 November 2026: eight kernel self-tests of the asynchronous side added and two faults fixed (a stopped program left RUNNING in the task table; a refused program's memory not returned): issues #31 and #32, pull request #33.
