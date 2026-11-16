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
generalized worker recovery. Additional windows (post-spawn, external effects,
host crash, store corruption and liveness during partition) remain open.

The journal is *not* a cryptographically authenticated, fsync-directory
transactional register and supplies no publisher or permission binding.
A hostile process with write access to the volume can forge entries.
The Docker/runtime policy and the test authority come from outside the
guest and wrapper. No formal §14.4 participant calibration is performed.

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
