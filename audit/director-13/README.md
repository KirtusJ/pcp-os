# Director 13 | Process-boundary qualification

This test-only lane addresses the Principal's request to make PCP OS workers more independently supervised. Its immediate purpose is to **measure a boundary**, not to say that independent containment or always-running workers exist.

## Test target

The hosted Rust [Parrots all the way Down](../../store/apps/parrots-all-the-way-down/README.md) desk admits a node before starting its process. Its adapter monitors the **direct process** and the desk's cancellation state. Native Desk controls recursive admission budgets. Those facts do not establish a kernel-enforced container, a trusted process tree or persistent residency.

The Python [probe](containment_probe.py) operates only on a new temporary redb database and two test-only process trees:

1. **Positive control:** A directly supervised worker writes a heartbeat. Cancel its desk node; check its original PID and Linux start tick for actual process exit, then check the heartbeat stopped. A *separate silent-but-live process* is a negative oracle control: lack of heartbeat alone is not evidence of termination.
2. **Adverse control:** A worker spawns one **unregistered child** into a separate Linux session. Cancel its desk node, wait for the directly supervised worker to end, then check whether the child's heartbeat still advances while the desk reports Cancelled. This is a deliberately constructed adversary with real process behavior, not a real model, Qwen guest or PCP OS kernel.
3. **Cleanup:** The test verifies the registered actor's PID and Linux start tick even if its adapter already exited, and terminates an unexpected live actor. It releases the detached child and, where necessary, signals only its verified test-created process group. An unclean or ambiguous run is not a pass.

The adverse test **currently expects an observed escaped child**. A green result therefore means **the containment gap was reproduced**, *not* that PCP OS is contained. If the adapter is later hardened, this expectation must be revised to require the descendant to terminate under the intended policy and to include negative cases for process-group escape, crash recovery and model service boundaries.

## Additional Linux process-group discriminator

The independent [group-boundary probe](group_boundary.py) starts **each** test adapter in a fresh Linux session so that its group contains only disposable test processes. It compares two unregistered descendants after the desk marks their parents cancelled and the adapters have exited:

- A descendant **still in the adapter's process group** must keep writing after desk cancellation, but **stop** when the separate test supervisor signals that specific group. This is a scoped *positive control* for a signal reaching a member of an already isolated process group.
- A descendant that creates a **new session** must keep writing even after that parent group is signaled. This is the deliberate *adverse control*: a process-group kill by itself does not enforce containment against a process able to call `setsid`.

Both groups are test-created, their PIDs are checked against `/proc` start ticks before use, and the detached child is independently cleaned up. Neither case uses a real PCP OS guest, device, model, installed host or service. A passing test **demonstrates the residual escape**, not an enforceable container. A selected cgroup/VM containment mechanism must later be tested separately with direct and detached descendants, crash and restart controls, and externally verified enforcement.

## Logical task budget versus real OS process count

The bounded [budget-bypass probe](budget_escape.py) tests a distinct consequence of untrusted descendants. It configures a disposable native Rust Parrots desk with **depth 0, one live admitted task, ten cumulative admissions**. A correctly admitted root spawns **three** OS children into separate Unix sessions without using the admission service. While the registered root is active, a second legitimate adapter request must fail with the **live node limit** and must not execute its requested program. The same readback must show **one** registered task, but the original worker and three unregistered children are independently observed alive using their /proc PID/start identities.

After the registered root is cancelled, its original worker must be observed exited. The desk must contain one cancelled node and no other admitted task, while each of the three unregistered children remains alive and writes a fresh heartbeat. Every child is bounded to 22 seconds, has a test-only isolated process group and exit marker, and is explicitly cleaned up by PID/start-checked disposal. This is **not a bypass of the native register-admission check**: the legitimate second task is correctly refused. It is proof that a bound on *admitted task records* is not a bound on *operating system processes* when a registered program can spawn unregistered descendants. The eventual enforcement candidate must use an actual OS process/cgroup limit and negative cases for both same-group and detached descendants.

## Read-only cgroup feasibility check

The separate [cgroup preflight](cgroup_preflight.py) does **not** create cgroups, move processes, modify a mount, issue a privilege escalation or establish an enforcement mechanism. It reads the transient Linux worker's cgroup v2 mount and namespace-relative membership, controller names, available control files, observed read-only mount flags, write-access hints, and whether a pidfd for the current test process can be opened. It omits the actual host cgroup path. A successful preflight merely identifies the observed API surface and hints about delegation; it cannot qualify a cgroup parent, usable enforcement, process confinement, crash recovery or resident service. A future confinement test must use an explicitly authorized, isolated resource boundary and an independent before/during/after process check. 

## Run on a disposable Linux host

From the repository root, with a suitable current Rust toolchain:

~~~sh
cargo build --locked --release --manifest-path store/apps/parrots-all-the-way-down/host/Cargo.toml --bins
cargo test --locked --release --manifest-path store/apps/parrots-all-the-way-down/host/Cargo.toml --test dispatch -- --test-threads=1
python3 audit/director-13/containment_probe.py \
  --cli store/apps/parrots-all-the-way-down/host/target/release/parrots-all-the-way-down \
  --delegate store/apps/parrots-all-the-way-down/host/target/release/pcp-delegate \
  --output d13-evidence/process_boundary.json
python3 audit/director-13/group_boundary.py \
  --cli store/apps/parrots-all-the-way-down/host/target/release/parrots-all-the-way-down \
  --delegate store/apps/parrots-all-the-way-down/host/target/release/pcp-delegate \
  --output d13-evidence/process_group_boundary.json
python3 audit/director-13/budget_escape.py \
  --cli store/apps/parrots-all-the-way-down/host/target/release/parrots-all-the-way-down \
  --delegate store/apps/parrots-all-the-way-down/host/target/release/pcp-delegate \
  --output d13-evidence/budget_escape.json
python3 audit/director-13/cgroup_preflight.py \
  --output d13-evidence/cgroup_preflight.json
~~~

The workflow at .github/workflows/director-13-containment.yml builds and executes the **actual Rust adapter** on a transient Linux Actions runner and retains all three process-test JSON observations and cgroup preflight result as one ephemeral artifact. A fake-adapter rehearsal, if separately performed, would check only harness structure, not Rust behavior. The actual code revision, job outcome and raw saved artifact must be inspected for any native-host finding. Neither this test nor a green Actions job establishes a resident runtime.

## Acceptance still missing

For an actual independent, continuously supervised runtime, require an authorized host enforcing a real containment boundary (for example, a correctly configured cgroup or VM), a trusted admission identity, bounded resource controls, verified process termination and restart handling, surviving evidence after parent death, and independent observation of the recovered supervisor and its model workers. Each control needs ordinary and adverse cases. A periodic ChatGPT call, successful daemon launch, hosted database row, GitHub Actions job or worker's self-report is not that evidence.

**Ownership:** This test-only lane is Director 13's, outside Director 10's register-display PR #233, Director 2's optical QEMU PR #235, Director 11's independent same-state evidence analysis and Director 12's host deployment/lifecycle contract. No installed VM, shipping image, product binary, method edition, research work or master revision is altered by this proposal.
