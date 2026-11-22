# PCP OS release catalog

Use this page to choose an image, find its source and reproduce its checks. The source baseline for this edition of the catalog is [`eb47a850`](https://github.com/KirtusJ/pcp-os/commit/eb47a850b563d9cc04ff5a87050bdb6ec93c1f0b), 22 November 2026. A pull request can advance independently of a tested image. Use the commit and digest in that image’s receipt when repeating a result.

## Which image do I have?

| Image | How to identify it | Where to start |
| --- | --- | --- |
| Published PCP OS 0.3.1 live CD | `PCP-OS.iso`, 1,859,584 bytes, SHA-256 `93ed3b599ad7be0e1e88a64813b1f2b5299d51507df2edca691aaf6189d10d15`. The Enterprise repository holds this historical BIOS image. | First boot in a disposable x86-64 BIOS VM with only the read-only CD attached. The guide identifies the matching kernel, what survives a reboot and the observed disk write. [First-boot guide](https://github.com/KirtusJ/pcp/blob/03d68793f754a6057d6fcde87c210a1931e53ee2/audit/public-live-cd/README.md) |
| BIOS tester built from source | The build records the checkout, whole-ISO digest and kernel measurement. CI artifacts have a retention limit. | Use the [builder](../examples/tester-cd/README.md), then qualify the same artifact with the [three-host VM procedure](../examples/desktop-source-cd/README.md). |
| Plain UEFI candidate | Separate model-free assembly with a desktop, Tailorbird and Surfbird. A3 recorded optical, raw-USB and file-copy VM boots for the nominated kernel. | Use the [pinned build instructions](https://github.com/KirtusJ/pcp-os/blob/9da57f693d1efcbca159fd1cc6635d1fa2ba0ef4/examples/release-media/README.md) with the revisions in the [boot receipt](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7077892711). |
| Preloaded UEFI candidate | The owner requires bundled models, Tailorbird and an automatic local connection on first boot. The 0.5B Coder image exercises the integration. The optional 8B/12B artifact and physical trial remain open. | Follow the [model and worker staging](https://github.com/KirtusJ/pcp-os/pull/383), [firmware loader](https://github.com/KirtusJ/pcp-os/pull/385), [C and Rust image receipts and USB approval screens](https://github.com/KirtusJ/pcp-os/tree/052d3f0799560de7dbf5cf0a2a840ac4dab3d611/examples/release-media/observations/coder-first-boot) and [dual-model requirement](https://github.com/KirtusJ/pcp-os/issues/246). |

## Build and test from this repository

| Target | Source and entry point | What to verify |
| --- | --- | --- |
| Rust host and integrity core | [Rust workspace](../PCP_OS_Rust/README.md) and [desktop-platform workflow](../.github/workflows/rust-host.yml) | From `PCP_OS_Rust/`, run `cargo run --locked --release --bin pcpos-vectors -- vectors`, `git diff --exit-code -- vectors`, then `cargo test --locked --release --workspace`. The [vector inventory](../PCP_OS_Rust/vectors/README.md) supplies the expected digest for the generated guard cases. |
| Python implementation | [Python setup and test discovery](../PCP_OS_Python/README.md) | Generate the Rust vectors first. Record skipped tests and the exact method packet supplied. |
| Native kernel, desktop and boot media | [Kernel build and QEMU instructions](../PCP_OS_Rust/crates/pcpos-kernel/README.md), [firmware handoff](../examples/firmware-owned-uefi/README.md) and [optical disk observations](../examples/optical-disk-check/README.md) | Record the ELF digest, kernel measurement, feature set, firmware, devices and exit result. The [installed-disk acceptance procedure](installed-acceptance/README.md) takes a separately identified installed disk. |
| Editions and plugins | [Release builder](../build/release.sh), [method inputs](../build/method/) and [source-to-package reconciliation](../RECONCILIATION.md) | Identify the edition, build and method-manifest digest. The build requires the implementation packet. The public Enterprise build 19 and the Commercial build 22 adapters have different method pins. |
| Applications and review modules | [Store and package index](../store/README.md), [hosted Surfbird import and native guest](../apps/surfbird/README.md), [method modules](../README.md#method-modules) | Use the application’s own manifest, runtime and tests. The hosted browser, Windows window and native guest have distinct acceptance records. |

The [repository tree map](https://github.com/KirtusJ/pcp-os/blob/7324d627935075064591c927574c46d56ba86f41/CATALOG.md) describes individual directories and their release status. Z3 maintains the inventory and its [consistency check](https://github.com/KirtusJ/pcp-os/pull/387). Proposed moves and compatibility requirements are tracked in the [tidying plan](https://github.com/KirtusJ/pcp-os/issues/368#issuecomment-7078265625).

## Repository ownership

The [dated ecosystem audit](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7078182236) records exact commits, executed tests, package digests and exceptions for all nine repositories below.

| Work | Source of record |
| --- | --- |
| OS, kernel, host, Store and native Surfbird guest | [pcp-os](../README.md) |
| Enterprise edition and historical live CD | [pcp](https://github.com/KirtusJ/pcp) |
| Lightweight Claude adapter and native-host Claude adapter | [pcp-claude](https://github.com/KirtusJ/pcp-claude) · [pcp-os-claude](https://github.com/KirtusJ/pcp-os-claude) |
| Experimental audit method, pinned references and reproducible ZIP | [pcp-fidelity-lab](https://github.com/KirtusJ/pcp-fidelity-lab) |
| Tailorbird UI and model worker candidates | [tailorbird](https://github.com/KirtusJ/tailorbird) |
| Hosted Surfbird reader and Windows window | [surfbird](https://github.com/KirtusJ/surfbird) |
| Android companion for the research director exchange | [android-helpdesk](https://github.com/KirtusJ/android-helpdesk) |
| Downstream research edition and register | [From Prephysical Hypotheses to Physical Models](https://github.com/KirtusJ/From-Prephysical-Hypotheses-to-Physical-Models) |

## Open release decisions

| Owner or tracker | Evidence needed for the next decision |
| --- | --- |
| Z3, kernel | A3 cleared the nominated kernel with the C reference loader in optical and raw-USB VM boots. The USB run also verified file refusal, creation, reopen and frame recovery. Carry the kernel pin into each new loader/worker trial. [Kernel nomination](https://github.com/KirtusJ/pcp-os/pull/371#issuecomment-7078205268) · [A3’s integration run](https://github.com/KirtusJ/pcp-os/pull/371#issuecomment-7078307097) |
| B3, firmware | C3 cleared the Rust verification and handoff source. A3 booted the `87c627b8` EFI with Tailorbird #22 and verified file refusal, approval and reopen. B3 reproduced a later EFI across checkout paths and booted it from emulated USB. Match the EFI digest in the chosen trial before repeating it. [C3’s source review](https://github.com/KirtusJ/pcp-os/pull/385#issuecomment-7078451850) · [A3’s file trial](https://github.com/KirtusJ/pcp-os/pull/385#issuecomment-7078580990) · [B3’s build and boot](https://github.com/KirtusJ/pcp-os/pull/385#issuecomment-7078501536) |
| C3, worker, and A3, assembly | The [sequential prefill library](https://github.com/KirtusJ/pcp-os/pull/386) landed. The [worker candidate](https://github.com/KirtusJ/tailorbird/pull/22) passed 216 host tests and an [independent ELF build](https://github.com/KirtusJ/tailorbird/pull/22#issuecomment-7078355602). Native first-text timing and completion must be measured on the exact image. Native hardware floating point still needs ABI and state-switch qualification. Boot the written medium offline, exchange a normal message and verify a separately approved file task without a download or endpoint setup. [Acceptance instruction](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7078209559) |
| Release and hardware owners | Identify the release commit and signing authority, nominate both model assets, and record physical-device and recovery results. The 22 November inventory found recovery tags and zero GitHub Releases across the audited repositories. [Provenance](https://github.com/KirtusJ/pcp-os/issues/251) · [Device and recovery tracking](https://github.com/KirtusJ/pcp-os/issues/250) |

## Historical evidence and handoff

The [design documents](PCP_OS_Specification.md) describe their dated Build 22 baseline. The [audit directory](../audit/), [director task records](../audit/director-z3-tasks/README.md) and [source reconciliation](../RECONCILIATION.md) preserve earlier trials and their limitations. For a new trial, record the source commit, command, artifact digest, environment, outcome and open failures. Use the [coordination record](../COORDINATION.md) for standing agreements and the [director exchange](https://github.com/KirtusJ/pcp-os/pull/276) for subsequent dispositions.
