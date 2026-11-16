# Director 12 | PCP kernel booted under an OCI-hosted emulator

This is **not** a PCP OS host, a container that uses the PCP OS kernel as its
own kernel, an installed model, or a production service. A Linux container
shares its Linux host's kernel. This fixture deliberately starts QEMU *inside*
a Linux OCI container and boots the separately built x86_64 PCP OS kernel as
a QEMU **guest**.

## Goal and exact boundary

This test isolates a useful step toward the Principal's ongoing-execution
objective: can a nonprivileged, network-off, resource-bounded Linux container
host an actual PCP OS *guest* process with the expected image measurement and
native kernel-register self-tests? The test does **not** establish indefinite
residence, independent control, application continuity, a Qwen service, a
Tailorbird application, or migration of an existing ChatGPT conversation.

The container is diagnostic only. The Dockerfile installs packaged QEMU and
SeaBIOS from Ubuntu 24.04 without a fixed apt snapshot or digest pin.
Retain the observed image ID and QEMU version; do not describe dependency
reproducibility or publisher authentication that this path did not establish.
The QEMU guest and kernel file are taken from this exact Git checkout.
The workflow uses a separate stage and does not update any issued ISO.

## Run and independent checks

The [CI workflow](../../../.github/workflows/director-12-kernel-in-oci.yml)
builds `PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32` using
`run.sh build` and records the complete file SHA-256. The kernel's expected
code and read-only-data measurement comes from the kernel's released
`tools/measure.py`, applied to that file rather than a model's assertion.

The host-side [verify.py](verify.py) invokes the **same QEMU binary** in four
separate, short-lived containers with exact configuration. These controls
preserve original serial output and exit codes, not just generated findings.

- **Correct external image value:** demand exit status 33, native kernel
  banner, matching kernel-measured hash, `ALL SELF-TESTS PASS`, and one full
  64-digit kernel-register head.
- **Deliberately wrong external image value:** demand that the kernel report
  the mismatch, fail one or more self-tests, refrain from running its built-in
  programs, and exit with a status other than 33.
- **Omitted image value:** demand that the native kernel explain the missing
  comparison. Self-tests may pass, but that provides no external image pin,
  illustrating why successful boot alone cannot satisfy the image criterion.
- **Externally loaded no-grant program:** compile the *existing* kernel test
  `programs/probe/probe.c` to a static PIE ELF64 on the Linux CI builder.
  QEMU hands the file to the PCP OS loader as `probe.elf`; the program is
  selected using `auto=probe.elf grant=none` (exact module name). Demand a
  native run of the program, `probe: 0 failure(s)`, verified image pin,
  `ALL SELF-TESTS PASS`, and a full native kernel register head. This
  tests one real external application under its restricted capabilities,
  *not* a Qwen service or a generally complete Linux ABI.

The OCI runtime uses no network, a read-only root filesystem, unprivileged
UID/GID 65534, no new privileges, zero Linux capabilities, an isolated
temporary filesystem, PID and memory limits and TCG (not host KVM). It mounts
only the test kernel image and a compiled optional probe ELF as read-only
inputs. A pass is limited to this host
kernel, Docker image, QEMU version, builder, selected flags and time window.
A resource-limit configuration string is not by itself proof of exhaustive
enforcement against arbitrary adversarial code.

No command in this test installs a persistent task, updates `master`, builds
or publishes a refreshed ISO, modifies the governing ACR method, handles
private Qwen data, changes any running installed VM, or starts a worker after
the CI executor disappears. The owner-specified condition for a **master**
push includes coordinated updated ISO distribution, and a tested OCI guest
is not a reason to bypass that condition.

## Known gaps and next discriminator

The kernel runs inside an emulated machine and does not become the
container's actual host kernel. The container itself remains a Linux
process. To claim a self-maintaining, ongoing PCP OS director, additionally
establish a real independently controlled deployment, authenticated
operator/grants, persistent model/checkpoint/store, separated enforcement,
bounded resource accounting, restart/stop reconciliation, user-visible
control, valid and invalid model interactions, and an independent
*post-ChatGPT* observer. The legacy resident 24-guest demonstrations and
periodic ChatGPT automations do not provide those facts for a new runtime.
The proposed next step must preserve actual adverse observations and never
treat a local self-test or successful QEMU exit as release authority.

## Evidence history (16 November 2026, B.C. PCT)

The initial three-arm [Actions 43965271297](https://github.com/KirtusJ/pcp-os/actions/runs/43965271297)
passed with observed native kernel boot/refusal behavior and a seven-file
raw-serial artifact. A later **red** [Actions 43966709674](https://github.com/KirtusJ/pcp-os/actions/runs/43966709674)
added an external probe but mistakenly requested `auto=probe` although the
actual native serial reported `file from the loader: probe.elf`. The kernel
properly refused to run a nonexistent name and stopped with a nonzero status;
this was a **harness selection defect**, not an observed fault in the native
loader. The failed run and its nine-file serial artifact are retained.

After switching to the exact observed `auto=probe.elf` selection, the
[four-arm Actions 43967270609](https://github.com/KirtusJ/pcp-os/actions/runs/43967270609)
passed on draft head `bd4fb9e5ba473d47dfd196fbf3056ec311c9d6dc`.
Its external no-grant guest printed `probe: 0 failure(s)`, the kernel verified
its own image, reported 47 register events and exited through the diagnostic
device with code 33. The original valid, bad and missing-pin controls also
ran and retained their own native serials. This exact evidence belongs to its
recorded CI checkout and measured image, not to an arbitrary rebuild or
production deployment.
