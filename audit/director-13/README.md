# Director 13 | Process-boundary qualification

This test-only lane addresses the Principal's request to make PCP OS workers more independently supervised. Its immediate purpose is to **measure a boundary**, not to say that independent containment or always-running workers exist.

## Test target

The hosted Rust [Parrots all the way Down](../../store/apps/parrots-all-the-way-down/README.md) desk admits a node before starting its process. Its adapter monitors the **direct process** and the desk's cancellation state. Native Desk controls recursive admission budgets. Those facts do not establish a kernel-enforced container, a trusted process tree or persistent residency.

The Python [probe](containment_probe.py) operates only on a new temporary redb database and two test-only process trees:

1. **Positive control:** A directly supervised worker writes a heartbeat. Cancel its desk node and read back its termination; its heartbeat must stop.
2. **Adverse control:** A worker spawns one **unregistered child** into a separate Linux session. Cancel its desk node, wait for the directly supervised worker to end, then check whether the child's heartbeat still advances while the desk reports Cancelled. This is a deliberately constructed adversary with real process behavior, not a real model, Qwen guest or PCP OS kernel.
3. **Cleanup:** The test sends a release signal to the detached child and, if necessary, terminates only its test-created process group. It binds the process PID to the /proc start tick before signaling to avoid acting on a reused PID. An unclean or ambiguous run is not a pass.

The adverse test **currently expects an observed escaped child**. A green result therefore means **the containment gap was reproduced**, *not* that PCP OS is contained. If the adapter is later hardened, this expectation must be revised to require the descendant to terminate under the intended policy and to include negative cases for process-group escape, crash recovery and model service boundaries.

## Additional Linux process-group discriminator

The independent [group-boundary probe](group_boundary.py) starts **each** test adapter in a fresh Linux session so that its group contains only disposable test processes. It compares two unregistered descendants after the desk marks their parents cancelled and the adapters have exited:

- A descendant **still in the adapter's process group** must keep writing after desk cancellation, but **stop** when the separate test supervisor signals that specific group. This is a scoped *positive control* for a signal reaching a member of an already isolated process group.
- A descendant that creates a **new session** must keep writing even after that parent group is signaled. This is the deliberate *adverse control*: a process-group kill by itself does not enforce containment against a process able to call `setsid`.

Both groups are test-created, their PIDs are checked against `/proc` start ticks before use, and the detached child is independently cleaned up. Neither case uses a real PCP OS guest, device, model, installed host or service. A passing test **demonstrates the residual escape**, not an enforceable container. A selected cgroup/VM containment mechanism must later be tested separately with direct and detached descendants, crash and restart controls, and externally verified enforcement.

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
~~~

The workflow at .github/workflows/director-13-containment.yml preserves the JSON observation as an ephemeral Actions artifact. A local dry-run against a **synthetic Python adapter** checks the harness's structure only. It is not a run of the Rust implementation; only the exact GitHub Actions source/runner and archived evidence can support a Rust-host finding.

## Acceptance still missing

For an actual independent, continuously supervised runtime, require an authorized host enforcing a real containment boundary (for example, a correctly configured cgroup or VM), a trusted admission identity, bounded resource controls, verified process termination and restart handling, surviving evidence after parent death, and independent observation of the recovered supervisor and its model workers. Each control needs ordinary and adverse cases. A periodic ChatGPT call, successful daemon launch, hosted database row, GitHub Actions job or worker's self-report is not that evidence.

**Ownership:** This test-only lane is Director 13's, outside Director 10's register-display PR #233, Director 2's optical QEMU PR #235, Director 11's independent same-state evidence analysis and Director 12's host deployment/lifecycle contract. No installed VM, shipping image, product binary, method edition, research work or master revision is altered by this proposal.
