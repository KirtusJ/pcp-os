# PCP OS | Rust workspace | early development

PCP OS is part of Parrot Council Protocol (PCP), the product. The method it carries is Adversarial Claim Review (ACR). The host program `pcpos` is version 1.0.22: PCP 1.0, build 22. The kernel is early and keeps its own version, 0.3.1. Record identifiers that begin `acr:` and the record form `ACR-OS-RECORD-1` are wire names of the method's record and are kept so that earlier registers still verify.

PCP OS is an operating system built around one idea: the record of what was examined, by whom and on what evidence is kept by a mechanism that the examined party cannot rewrite. ACR is part of the system itself: its mechanically checkable rules are native code in the same core the kernel runs. This workspace is the beginning of the implementation. It is not a usable operating system.

**Rust is the authority.** Where implementations differ, the Rust one is right by definition and the other is corrected. The Python implementation is tested against files this workspace publishes (`vectors/`).

## What exists, and what was checked

For the names, inputs and invocation of built-in instruments, see the [Store instrument catalog](../store/instruments/README.md). Run `pcpos instrument list` for the catalog in your binary.

| Part | What it is | Checked how |
| --- | --- | --- |
| `crates/pcpos-core` | The core, `no_std`, no `unsafe`, one dependency (SHA-256). Record values and the register; capabilities, within one holder and across holders; **state machines** as complete tables with their composition rule; the **task table**; the **authority model** (identities, roles, groups, editions, standards); **ACR native** (guards, derived state, the record), with the identity of the method release it implements | 30 tests. Tables checked exhaustively. 400 seeded random walks through the record keep it consistent: the guards return their own output unchanged, a refused event changes nothing, and replaying a register gives the state reached step by step. Completion, stopping and adoption have their own tests: a task is DONE only on the supervisor's read-back of bytes produced after it started, one observation settles one attempt, a halt asks a review's tasks to stop and a late completion is not DONE, and where a person must adopt, the person acts as themselves. The guards agree with the Python implementation on 17,113 published cases: verdict, exit code, clause, and the exact body written (where a field has the wrong type, on verdict only). `docs/Guard_Clause_Trace.tsv` resolves every clause the guards cite against the pinned Core Specification |
| `crates/pcpos-kernel` | A new kernel for x86_64. Protects its memory and proves it by attempting the forbidden accesses; handles every exception, a double fault on its own stack; read-only interrupt table; 1 kHz timer and watchdog; runs the native guards; **loads programs** (static, position-independent ELF files handed over by the loader) and checks each before running it; an address space each from a pool of frames that is returned at exit; floating-point and vector state per process; **the Linux system-call conventions** for the subset a single-threaded C program needs; **a capability in front of every device**; **drivers** for an emulated display adapter (two pages), PS/2 keyboard, AC'97 sampled sound and OPL2 music; blocking and idling; **a launcher**, a register view and a stop key of its own; keeps a register of its own start, verification, processes and stop | Started in QEMU 8.2.2 without hardware virtualisation: 58 self-tests pass on a processor with every optional protection and on one with none (`docs/boot_log_qemu.txt`). `tests/run_tests.py` starts it 24 times for 17 tests: Doom is played by keys sent to the emulator with pictures taken at each step, its demos are run with and without the limiter, sound is captured to a file and examined, seven malformed programs are each refused, and a probe program is run with and without grants. Results, and what they do not show, are in the crate's README. Since build 22 the image is the same file from any folder, it compares its measurement of itself with a value named on the loader's command line (`tools/measure.py` computes that value from the file), and a kernel that fails verification starts no program. A deliberate fault and a deliberately exhausted stack are each reported and stop it (`docs/fault_demo_qemu.txt`, `docs/overflow_demo_qemu.txt`). Not started on hardware or in another virtual machine |
| [`crates/pcpos-pkg`](crates/pcpos-pkg/src/lib.rs) | Checks names, versions, paths, ranked sources and supplied package bytes. Its [transaction model](crates/pcpos-pkg/src/txn.rs) plans and commits changes to in-memory generations. A distributable catalog, persistent installation, signature verification and register integration remain separate work. | Crate tests cover immutable verified entries, pins across withdrawal, ambiguous paths, ranked sources, stale plans, request retries and restore. Run `cargo test -p pcpos-pkg`. |
| `crates/pcpos-host` | `pcpos` (status, verify, machine tables, method check, a person's commands at a terminal, a tool server for models, and hooks that record what a host observes), `pcpos-verify`, `pcpos-vectors` (writes the published vectors) | 25 tests start the built program and speak to it as a host would: a register is never reached through a link or outside the served directory; the caller does not choose the actor and cannot write as a person or as the system; adoption is the person's when the host saw their words and is marked relayed when it did not; a typed halt is enforced by the guards; a write names the head it extends and many writers leave one intact chain; a register holding an event the guards refuse accepts a halt and nothing else; a subagent is a task whose end the host observes; a context reset is recorded by the host; a person adds a module of their own beside the register under a `local.` identifier, and it never displaces a module of the method; an identifier two folders claim is served from neither; text given only in a conversation is recorded as an artifact whose digest the server computed; a report of a command that does not say, as true or false, whether it was interrupted is not kept; a file is material a review was given only if its last write is shown to be earlier than the commissioning; an instrument is run by the program and recorded as a check it ran, and a finding citing it is recorded as executed. `audit/sim/sim.py` drives the packaged program through 799 constructed scenarios with the Python implementation as a second oracle |
| `editions/` | Design manifests for the three editions. No image is built from them | Not applicable |

`unsafe` appears in two files of the kernel: `crates/pcpos-kernel/src/arch.rs`, the processor and memory (57 blocks, 20 unsafe implementations or functions), and `crates/pcpos-kernel/src/dev.rs`, device registers and device memory (6 blocks). Each use states its reason. Every other file, among them the loader, the system calls, the process table and the session, refuses it at compile time.

## Checking a signed column in protected bins

`mathematics.protected_bins` is built into the Rust host. Both columns use integer numerators over the same square-root denominator. With `scale_exponent` equal to 4 the denominator is 4 and each column's integer squared norm must be 16. Bins correspond by index. They may have different numbers of coordinates, including zero. For each bin the instrument compares the squared norm and the odd part of the gcd of its entries. The zero bin has gcd zero.

```sh
pcpos instrument mathematics.protected_bins '{"scale_exponent":4,"source":[[3,1],[1,1,1,1,1,1]],"target":[[2,2,1,1],[1,1,1,1,1,1]]}'
```

The example reports squared norms 10 and 6 and odd gcd 1 in both bins. Protecting the amplitude 3 separately changes the test. Its squared norm agrees with that of `[2,2,1]`, but their odd gcds are 3 and 1. These arithmetic checks concern the supplied columns. Physical routing, persistent records and measured frequencies require separate evidence.

The CLI exits with 0 for agreement, 1 for a computed mismatch and 2 for invalid or out-of-range input. From MCP, call `acr_instrument` with the same instrument name and input object. Its usual review and recording rules apply. Supply 1–64 bins and at most 4,096 signed 64-bit JSON integers per column. The exponent ranges from 0 through 127. Every bin sum and column sum must fit an unsigned 128-bit integer. The instrument refuses overflow. It ships with a host built from this source.

## Bounding a binary read with missing outcomes

`science.incident_bounds` answers whether every possible completion of the submitted finite table lies within a closed band. Give it the incident denominator, observed hits and misses, and every unread binary flag. The three counts must exhaust the stated denominator. Give the band as two numerators with a common positive denominator. In this example the possible hit counts are 66, 67, 68 and 69.

```sh
pcpos instrument science.incident_bounds '{"incidents":128,"hits":66,"misses":59,"unread":3,"lower_numerator":66,"upper_numerator":69,"denominator":128}'
```

MCP workers call `acr_instrument` with the same instrument name and input object. A narrower upper bound of 68 gives a computed failure. A missing category, inconsistent total, invalid band or extra input field gives a refusal. Zero unread is valid. Each count and band component must fit a nonnegative signed 64-bit integer. Every integer hit count between the endpoints is attainable. Making an inference about the incident population additionally requires a sampling model and a justified denominator. A paper reporting only a rounded mean cannot supply this input. For the statistical background see [Horowitz and Manski on experiments with missing outcomes](https://doi.org/10.1080/01621459.2000.10473902).

## What does not exist

Storage that survives a stop, a file system on a disk, a network stack, a mouse, threads, signals, a second processor, messages between processes, a shell, a driver for any real device, a graphics processor driver, start from UEFI. The kernel's files are the ones the loader handed it plus what a program writes while it runs, kept in memory and gone at the stop. The system-call interface is the subset that the programs run so far needed: a program that needs more is told the call does not exist. Two programs have been run on it, Doom and a probe written for the tests. The authority model beyond device capabilities is still data structures with tests.

The [Doom Store package](../store/games/doom/) supplies the user program and its source. Players supply their own IWAD. The kernel build script in `crates/pcpos-kernel/programs/doom/build.sh` takes two source trees (doomgeneric at commit e257a095 and Chocolate Doom at commit a2dfd51f, both GPL-2.0) and compiles a separate program with six PCP OS platform files. The kernel loads the resulting executable in its own address space.

## Building

Linux, with a Rust toolchain that has its own sources installed (1.97.0 was used for the build of 13 November 2026), `ld.lld`, `objcopy` and `qemu-system-x86_64`:

```
cargo test                               # the core, on the host
cargo build --release                    # pcpos, pcpos-verify, pcpos-vectors
cargo build --release --bin pcpos --target x86_64-unknown-linux-musl     # the program as supplied: one static file
cargo run --release --bin pcpos-vectors -- vectors      # rewrite the published vectors
crates/pcpos-kernel/run.sh               # builds the kernel and starts it in QEMU
```

The kernel build sets `RUSTC_BOOTSTRAP=1` to compile `core` for the bare-metal target with a stable compiler. A release build would use a pinned toolchain that ships that target.

## Rights and status

Proprietary; see `NOTICE.md`. Untested and unreviewed beyond the table above. Nothing here is certified, and nothing here is fit for use where a failure can harm people.
