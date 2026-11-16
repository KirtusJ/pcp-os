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

The host-side [verify.py](verify.py) invokes the **same QEMU binary** in three
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

The OCI runtime uses no network, a read-only root filesystem, unprivileged
UID/GID 65534, no new privileges, zero Linux capabilities, an isolated
temporary filesystem, PID and memory limits and TCG (not host KVM). It mounts
only the single test kernel image read-only. A pass is limited to this host
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
