# Director 12: Docker-managed restart of a genuine PCP OS QEMU guest

**Purpose:** A deliberately small, reproducible experiment in process lifetime,
external supervision and fail-closed handoff, not a production service.
The Linux OCI container runs QEMU as a process. **PCP OS is the QEMU guest
kernel, never the Linux container's host kernel.** No Qwen model, ACR worker
or user task executes. The container and all guests end before CI finishes.

## Test architecture

The independent Linux Docker daemon restarts one named, unprivileged container
under `on-failure:2`. The container contains QEMU plus a test-only **static Rust**
launcher `runner.rs`. That launcher bootstraps the real current PCP OS ELF32
kernel handed in by a read-only bind mount. The current kernel's code/rodata
measurement is independently computed by its existing `tools/measure.py`, and
the kernel compares the externally named SHA-256 during each boot.

Only a disposable directory is writable by the container. For one synthetic
24-character request ID, each attempt writes a `START` journal marker before
QEMU creation. A successful QEMU ISA diagnostic exit 33 is followed by an
`END` marker; marker files are created with `create_new` and synced individually.
After attempts one and two, the wrapper sleeps over 10 seconds then exits with
**synthetic status 42**. Docker, not ChatGPT or the driver, restarts the container.
The third attempt exits normally. Host `docker inspect` must report
`RestartCount=2`, and the independent host test parses three distinct raw
kernel serials, each with native self-test success, current image equality
and a full replayed register head.

This is a **synthetic continuation of three independently booted guest
processes**, not semantic resumption of an unfinished review. A QEMU exit 33
means that particular kernel boot completed; the wrapper's status 42 is
deliberate test injection and **not** a reported kernel crash. On a later
container invocation with a different request ID, the wrapper refuses
the already occupied journal without booting anything.

## Failure window and stop boundary

A second, separate synthetic request forces the launcher to `abort`
**after** a durable `START` marker but **before** creating QEMU.
The Docker runtime restarts it once. On restart, the launcher observes an
incomplete prior assignment, records an explicit `INDETERMINATE` attention
marker and returns success **without retrying the task**, resetting budget
or fabricating a successful `END`. The external host oracle requires
`RestartCount=1`, one `START`, no `END`, and **zero guest serials**.
This is a test of a fail-closed control at a specified fault window, not
generalized worker recovery.

A **third test** opens a separate disposable QEMU process with its vCPUs
deliberately held at `-S`, after the same `START` marker. The host observes
the exact QEMU process by Docker's host-PID view and `/proc/<pid>/stat`
start clock, plus the host boot identifier. The test controller then
deliberately kills that container, checks that **the same original QEMU
process identity** is gone or terminal, and **manually** starts the same
container once. The re-entered Rust runner detects incomplete prior work,
writes `INDETERMINATE`, performs no second guest launch, and never invents
an `END`. This test's post-spawn guest was **paused before kernel boot**:
it is a process-start / termination / journal witness, not an additional
self-test or a claim of QEMU failure. The restart in this arm is explicitly
made by the controller, **not** by ChatGPT, PCP OS, or an autonomous service.

Other fault windows, including actual guest execution in progress,
external durable effects, host power loss, store corruption, cross-machine
identity and liveness during network partition, remain open.

The journal is *not* a cryptographically authenticated or crash-atomic
transactional register and supplies no publisher or permission binding.
Synchronizing file and parent-directory metadata is only a bounded durability
measure, not a complete atomic-commit or anti-tampering mechanism.
A hostile process with write access to the volume can forge entries.
The Docker/runtime policy and the test authority come from outside the
guest and wrapper. No formal §14.4 participant calibration is performed.

## Extra falsification controls for journal integrity

The test runner now checks a real native PCP OS x86_64 banner, an **exact**
kernel code/data measurement equality, `ALL SELF-TESTS PASS`, and one
well-formed 64-digit replayed register head before treating a QEMU exit
status 33 as sufficient for a synthetic `END`. It caps the retained serial
at 1 MiB and rejects a fake exit 33 paired only with a synthetic non-native
serial. A status code **alone** must never create an `END`. Before writing
it, the test runner synchronizes the observed serial and error streams,
then creates a new (never overwriting) END marker and syncs its directory
entry. These are scoped filesystem durability measures, **not** a
crash-atomic, tamper-resistant or signed ledger.

### Request/attempt replay and completed-journal receipts

The launch command now gives each real PCP OS QEMU guest an **externally
chosen synthetic** `d12_ticket=<24 lowercase hexadecimal characters>` and
`d12_attempt=<1, 2 or 3>` alongside the code/data image pin. The native
kernel prints that complete loader command line in its own serial output.
The controller compares exactly one printed line with the **host-requested**
ticket, attempt and measured image before creating its END marker. A
separate Linux host oracle checks the same original raw serial after CI.
This catches an otherwise-valid earlier transcript accidentally reused for a
different request or attempt. The exact older native transcript was used
byte-for-byte in a new journal as an adverse test and correctly refused.

This is a **correlation and replay-detection control**, not a signature:
the ticket is visible in the command line and mutable fixture files. A party
who controls both the serial output and retained journal can still forge
a plausible-looking receipt. No host-enforced PCP/ACR operation authority,
trusted signer, nonrepudiation or durable register protection is established.

The end-of-budget path has the same obligation. Even if **all three** START
and END markers are present, a subsequent controller invocation must inspect
all three original native serial receipts again before returning a successful
closed result. The retained red negative omitted real attempt 2's serial
while keeping all its START/END markers; the earlier controller silently
returned success. The corrected controller creates an explicit
`INVALID_JOURNAL` attention record and does not launch another QEMU.
A complementary test substitutes the actual successful *attempt 1* serial
for the same request's *attempt 2*, retaining all three END markers. The
expected result is another fail-closed hold due to the wrong attempt ticket.

On a restart, the Rust `journal.rs` module examines the **whole bounded**
three-attempt namespace. Valid consecutive completed attempts advance, but
a sparse future START/END, orphan output, missing START/END, malformed
identity, unexpected reserved member, oversized record or an existing
attention marker must **hold**. A prior completed attempt must also have a
retained native serial passing the same exact measurement/self-test/register
receipt check; a plausible-looking `END` plus a fake or absent serial does
not permit another guest launch. Rust unit tests cover positive and
adversarial journal inputs in the CI runner.

The host test repeats selected adversaries **inside real disposable Docker
containers**: a fake code 33 with no native kernel serial, a future
attempt 3 without attempts 1/2, and a syntactically valid prior END
whose recorded serial was never produced by a kernel. Each must leave a
clear attention receipt, no extra guest and no synthetic END. These are
deliberate falsifications of the fixture's inference logic, not observations
of malicious real-world actors.

## Concurrent controller and live-worker ambiguity

One further failure window is **not** an actual crash. Two independently
started Linux OCI controllers can see the same shared journal while the
first controller and its deliberately paused QEMU process remain OS-live.
Without a controller lease, a later controller may mistake the first's
`START` without `END` as abandoned, write `INDETERMINATE`, and
prematurely quarantine a healthy ongoing attempt.

The diagnostic Rust controller now acquires a **nonblocking Linux advisory
`flock`** on `controller.lock` before reading or changing any request
state. The file descriptor is retained while the controller owns its
attempt; an OS process exit releases the kernel lock. Another controller
that observes this **same-host busy lease** must stop without launching a
QEMU guest, overwriting the request, or writing `attention.txt`. It emits
a distinctly labeled `D12_BUSY` stderr line so that a clean wrapper
exit cannot silently be described as task completion.

The external Docker oracle runs the *actual original QEMU process with
vCPUs held at `-S`* and checks its host PID/start/boot identity is
OS-live throughout. It then tests both a simultaneous **same-ticket**
controller and a **different-ticket** controller using the same shared
volume. Both must emit observable BUSY, neither may cause a retry or
`INDETERMINATE`, and the original single START and QEMU process must
remain unchanged. Original contender logs are included in the artifact's
synthetic-file allowlist.

This is **cooperative same-host process coordination only**. Advisory
`flock` is neither a mandatory OS security boundary nor proof against
another writer holding the same file permissions, symlink/inode
replacement, a remote host, a noncooperating actor, or a lost host. Nor
does an exit code of zero signify work completion: BUSY must be handled
as a separate outcome by any future orchestrator. A real PCP OS/Qwen
service needs an independently enforceable actor identity, exclusive
service ownership, durable operator authority and external process
reconciliation.

## Runtime and release limits

The ephemeral OCI test container requests a read-only root, no network,
drop all capabilities, no-new-privileges, unprivileged UID/GID 65534, 64 PIDs,
1 GiB memory, 2 CPUs and a bounded temporary filesystem. QEMU uses software
emulation only. Requested settings are read from `docker inspect`; this test
does not certify escape resistance or all resource controls under attack.
Ubuntu 24.04 and its installed QEMU packages are not a cryptographically
pinned release source. The test reports the actual OCI image ID and
runtime version.

Every state file and QEMU serial is synthetic, created under a disposable
runner directory and cleaned up. The artifact retains the **original raw
serials** for all observed guest boots, plus an explicit allowlist of
test-only START/END/attention journal bytes, and a JSON summary recording
the SHA-256 digest and length of each retained member. The host-side
verification is not the sole oracle: the archived guest serials can be
reparsed without relying on the test harness's findings. No kernel ELF,
private task payload or persistent register is uploaded. The test still
does **not** establish an autonomous PCP OS deployment, an
independently resident director after the CI run, an installed model, trusted
operator capabilities, durable audit/event reconciliation, or an ISO build.

The Principal requires an updated ISO on **every coordinated master push**.
This draft does not modify master, the released ISO, any installed VM, or
the other directors' assigned branches. Its evidence is a step toward a
properly installed persistent host, not permission to claim it.
