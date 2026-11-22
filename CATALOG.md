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
| `examples/large-memory` | Check: memory above four gibibytes. | no |
| `examples/loader-line` | Check: what the kernel accepts on the loader's line. | no |
| `examples/model-memory` | Check: memory a loader filled with a model is kept out of the pool and given read-only to the companion alone (kernel feature model-memory). | no |
| `examples/method-readback` | CI check: a guest reads the selected Core Specification. | no |
| `examples/model-media-iso` | Mastering a UEFI model-media ISO. | no |
| `examples/optical-disk-check` | CI check: booting the CD does not write to an attached disk. | no |
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
| `PCP_OS_Rust/crates/pcpos-model-tensor` | Decodes model tensor rows. | not determined |
| `PCP_OS_Rust/crates/pcpos-pkg` | Package reading and checking. | not determined |
| `PCP_OS_Rust/crates/pcpos-qwen` | Scalar execution of a dense Qwen model. | not determined |

## Session context

A tracked file must not carry a session's address or a session's scratch path. The check lists every file that does. Today those are the records under `audit/director-z3-tasks/records`, named in the check as the one place still allowed until they leave master (issue 368); nothing else may.
