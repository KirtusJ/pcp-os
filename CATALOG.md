# Catalogue of this repository

Every entry at the top of the repository, and every folder one level down in the folders that hold many things, with one line on what it is. `build/check_catalog.py` fails when a tracked path of that kind is missing here or a path listed here no longer exists, so this page cannot fall behind the tree.

"In the release" says whether the path's content is part of what a person boots or installs. "not determined" means nobody has yet established it; it is not a quiet yes.

| Path | What it is | In the release |
|---|---|---|
| `.gitattributes` | Line endings and binary marking. | no |
| `.github` | Continuous integration workflows. | no |
| `.gitignore` | Paths git ignores. | no |
| `AGENTS.md` | Routes an agent from its task to the executable, package or record it needs. | no |
| `apps` | Applications kept in this repository. | not determined |
| `ASYNC.md` | How asynchronous tasks are run and recorded. Proposed to move under os-docs (issue 368). | no |
| `audit` | Records of audits and of sessions' runs. Most of it is proposed to leave master for a named branch (issue 368). | no |
| `build` | Scripts that assemble a release, its documents and the plugins. | not determined |
| `CATALOG.md` | This page. | no |
| `CLAUDE.md` | Standing rules for Claude sessions working here. | no |
| `COORDINATION.md` | Agreements of the baseline work, each with where it was made. Proposed to move under os-docs (issue 368). | no |
| `examples` | Checks and experiments, each in its own folder; several are run by CI. | no |
| `os-docs` | The OS documents and the acceptance checks for installed systems. | not determined |
| `PCP_OS_Python` | The second implementation, in Python, with its tests. Used to compare with the Rust one: its tests run the published guard cases against both (A3's audit, 22 November 2026). It is not what boots. | no |
| `PCP_OS_Rust` | The Rust workspace: the reference implementation. | not determined |
| `README.md` | What PCP OS is and how to start. | not determined |
| `RECONCILIATION.md` | Which host build a live register depends on. Proposed to move under os-docs (issue 368). | no |
| `RUST_PRIMACY.md` | Permanent rule: Rust is the reference implementation. | not determined |
| `store` | The Store: packaged applications, games, engines and their index. | not determined |
| `apps/surfbird` | Surfbird: the native page reader and its host side. | not determined |
| `audit/calibration` | Calibration records of an earlier session. | no |
| `audit/director-z3-tasks` | Director Z3's recorded-task tool and records. The records are proposed to stay only on branch z3/task-records. | no |
| `audit/games` | Game builds and recorded play, 14 November 2026. | no |
| `audit/live` | Record of an earlier live session. | no |
| `audit/live17` | Record of an earlier live session. | no |
| `audit/live18` | Record of an earlier live session. | no |
| `audit/live19` | Record of an earlier live session. | no |
| `audit/live20` | Record of an earlier live session. | no |
| `audit/sim` | Record of an earlier simulation. | no |
| `examples/companion` | Check: two programs side by side exchanging bytes (kernel feature companion). | no |
| `examples/desktop-source-cd` | Record: the source-built tester CD booted on three desktop hosts. | no |
| `examples/firmware-model-loader` | Builds and tests the UEFI loader for preloaded model media. | no |
| `examples/firmware-owned-uefi` | Experiment and CI check: a UEFI loader handing the full-size kernel over under OVMF. | no |
| `examples/guarded-uefi` | Experiment and CI check: the kernel under Q35/OVMF, with the frame-map tests. | no |
| `examples/heat` | Host tests of the kernel's reading of how many degrees a processor reports below its highest junction temperature (kernel feature `heat`, off by default). | no |
| `examples/large-memory` | Check: memory above four gibibytes. | no |
| `examples/loader-line` | Check: what the kernel accepts on the loader's line. | no |
| `examples/model-memory` | Check: memory a loader filled with a model is kept out of the pool and given read-only to the companion alone (kernel feature model-memory). | no |
| `examples/method-readback` | CI check: a guest reads the selected Core Specification. | no |
| `examples/model-media-iso` | Mastering a UEFI model-media ISO. | no |
| `examples/optical-disk-check` | CI check: booting the CD does not write to an attached disk. | no |
| `examples/processors` | Check: the kernel reads from the firmware's tables how many processors the machine has (issue 396, stage 1). | no |
| `examples/proposals` | Checks: a program proposes a file, the kernel asks, records and shows it (kernel feature proposals). | no |
| `examples/qwen` | Example: a local model reading a PCP register through llama.cpp on the host. | no |
| `examples/release-media` | Tools for assembling and verifying release media. | no |
| `examples/resident-workers` | CI check: 24 guests with resident numerical workers. | no |
| `examples/tester-cd` | CI: builds the tester CD from source. | no |
| `examples/vector-state` | Check: saved vector register states are what return. | no |
| `examples/vm-store` | CI check: files kept across cold boots in a QEMU PC (kernel feature vm-store). | no |
| `os-docs/app-persistence` | Notes on application persistence. | not determined |
| `os-docs/display-acceptance` | CI check: Tailorbird display-mode acceptance. | not determined |
| `os-docs/install-source` | Installing from source. | not determined |
| `os-docs/installed-acceptance` | CI check: installed recovery and desktop acceptance. | not determined |
| `os-docs/review-modules` | Review modules. | not determined |
| `os-docs/tailorbird-stop` | Regression: Tailorbird Stop and cold readback. | not determined |
| `os-docs/working` | Working records of the tidying (issue 368): which branches were removed, with their tips. | no |
| `store/apps` | Store applications. | not determined |
| `store/art` | Pictures used by the Store pages. | not determined |
| `store/engines` | Store engines (Heliograph). | not determined |
| `store/games` | Store games with their builds and evidence. | not determined |
| `store/index` | The Store's package index. | not determined |
| `store/instruments` | Notes on instruments that ship in the Rust host. | not determined |
| `store/native` | The native Store launcher. | not determined |
| `PCP_OS_Rust/crates/pcpos-core` | The register and its rules, shared by host and kernel. | yes |
| `PCP_OS_Rust/crates/pcpos-efi-loader` | Verifies the kernel, guests and model shards and hands their measured memory to the kernel under UEFI. | not determined |
| `PCP_OS_Rust/crates/pcpos-gguf` | Reads a model index before weights are loaded. | not determined |
| `PCP_OS_Rust/crates/pcpos-host` | The host program and tool server. | yes |
| `PCP_OS_Rust/crates/pcpos-kernel` | The kernel, its built-in programs and its emulator checks. | yes |
| `PCP_OS_Rust/crates/pcpos-model-media` | Verifies model bytes before an engine sees them. | not determined |
| `PCP_OS_Rust/crates/pcpos-model-simd` | Checks CPU and OS vector support and computes ordered or ggml-compatible packed Q4_K and Q6_K products. | not determined |
| `PCP_OS_Rust/crates/pcpos-model-tensor` | Decodes tensor rows and reuses packed activations across quantized matrix rows. | not determined |
| `PCP_OS_Rust/crates/pcpos-pkg` | Package reading and checking. | not determined |
| `PCP_OS_Rust/crates/pcpos-qwen` | Runs dense Qwen inference with bounded causal prompt batches and a scalar fallback. | not determined |

## The other repositories

This table is the index to the other repositories the baseline work covers: what each is, and where its own catalogue is. It is written by hand and is not checked by `build/check_catalog.py`, which reads only this repository's tree; the commits named are where each was read on 22 November 2026.

| Repository | What it is | Its catalogue |
|---|---|---|
| [pcp](https://github.com/KirtusJ/pcp) | The published edition: the Enterprise Edition of PCP 1.0, build 19, with its packets, the ISO and the first-boot guide. Verified by its own `verify_edition.py`. | None of its own; its `MANIFEST.json` and `EDITION_GUIDE.md` say what is in the edition. |
| [pcp-os-claude](https://github.com/KirtusJ/pcp-os-claude/tree/c0fbde739b4cc3bc409878d901de0974702c506e) | The released Build 22 OS adapter: the plugin that delivers the method over the `pcpos` host. Pinned by commit wherever it is used. | None is added, on purpose: it is a release, and a new file would make a new one. Its [`MANIFEST.json`](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/MANIFEST.json) lists all 84 payload files with lengths and SHA-256 (A3 verified each, 22 November 2026). |
| [pcp-claude](https://github.com/KirtusJ/pcp-claude/tree/3187dc226cd9f51b81a940f7d986bc124b26bc12) | The released Build 22 text adapter: the method as skills, subagents and pinned text, without the host. | None is added, for the same reason. Its [`MANIFEST.json`](https://github.com/KirtusJ/pcp-claude/blob/3187dc226cd9f51b81a940f7d986bc124b26bc12/MANIFEST.json) lists all 74 payload files (A3 verified each, 22 November 2026). |
| [pcp-fidelity-lab](https://github.com/KirtusJ/pcp-fidelity-lab) | An experimental plugin for auditing how well a PCP analysis follows its sources. Kept apart from PCP OS; not in any release. | [`CATALOG.md`](https://github.com/KirtusJ/pcp-fidelity-lab/blob/453bbaf97c68d6581e73fbd1467623781c0a781b/CATALOG.md), checked in its CI by this repository's check at a pinned commit. |
| [surfbird](https://github.com/KirtusJ/surfbird) | Surfbird's hosted reader and optional Windows window. The native guest is here, under `apps/surfbird`. | [`CATALOG.md`](https://github.com/KirtusJ/surfbird/blob/906658eb5b71ceb0eac59df3be4eff93e1001637/CATALOG.md), checked the same way. |
| [tailorbird](https://github.com/KirtusJ/tailorbird) | Tailorbird, the program for working with a local model worker, where a person talks to Taylor. It pins this repository as a submodule. | [`CATALOG.md`](https://github.com/KirtusJ/tailorbird/blob/a5aa8524262efc126e4095d86c7eb79f4cc14ff3/CATALOG.md), checked by this repository's check from that submodule. |

Not indexed here: the research program's repository, which is separate from the product, and [Director HelpDesk](https://github.com/KirtusJ/android-helpdesk), which the README links and which was not read for this table.

## Session context

A tracked file must not carry a session's address or a session's scratch path. The check lists every file that does. Today those are the records under `audit/director-z3-tasks/records`, named in the check as the one place still allowed until they leave master (issue 368); nothing else may.
