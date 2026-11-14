# Reconciliation of this source with the build in use

14 November 2026. Written by the Claude session acting as Director Z on the research program "From Prephysical Hypotheses to Physical Models", at the direction of the rights holder, Kirtus Justus. Proprietary; all rights reserved; see `README.md`.

**What was pushed.** The contents of `PCP_1.0_Source.zip` as supplied by the rights holder (SHA-256 `bbee36c4ee05f70f5062e396a8b033a80e3b3465da176c0efed3e5e5fc4f040b`, 146 entries), unchanged, plus this file and a `.gitignore`. No file of the archive was edited. The build output (`PCP_OS_Rust/target/`) is not included.

**The question.** The rights holder said the archive might be older than the version the research program runs. It is not: both are build 22.

| Check | Result |
|---|---|
| Version in `PCP_OS_Rust/Cargo.toml` | 1.0.22 |
| Version the binary in use reports | pcpos 1.0.22 (the packaged `servers/linux-x86_64/pcpos` of `pcp-os-claude`, SHA-256 `c2c6795d…`) |
| Archive digest | equal to the digest cited by the audit branches of `pcp` (`audit/build22-…`) |
| `cargo build --release --bin pcpos` from this source | exit 0 in 17 s (rustc as installed in the session) |
| Built binary against the binary in use, same input, byte comparison of output and exit code | identical on all six commands tried: `verify` and `status` on a copy of the research register (582 events), `instrument list`, `machine`, `modules <method>`, `clause <method> core 15.5` |
| Binary bytes | differ, as expected: the packaged binary is a static build, the one built here is dynamically linked (SHA-256 `c0425b0d…`); no claim of a reproducible build |

**Not done.** `cargo test` and the published vectors were not run. The Python implementation was not exercised (its entry point did not start with `python3 -I` here; not investigated). The kernel crate was not built. The audit branches' regression suites were run earlier against the packaged binary only (results on `pcp#18`, `pcp-os-claude#2`, `#3`), not against a binary built from this tree.

**Branch.** Pushed to `claude/cool-newton-bqk60b`, the session's branch; nothing was pushed to any other branch.

## Later the same day: `master` is ahead of the build in use

Written by Director Z after an audit of `master` at `6254a71` in a fresh clone. The section above describes the archive as pushed (`bbe8996`) and stays as written; this section says what has changed since, because one sentence above ("both are build 22") is no longer the whole truth.

**The source on `master` is no longer the source of the binary in use.** The binary the research program runs is still the packaged build 22 (`servers/linux-x86_64/pcpos` of `pcp-os-claude`, SHA-256 `c2c6795d…`). Since `bbe8996`, ten files in the Rust tree have changed on `master` (1,025 lines added, 33 removed; nine are source or tests, one is the kernel's README):

| Change | Pull request | In the binary in use? |
|---|---|---|
| `mathematics.exact`: signed and decimal boundaries | #20 | no |
| New instrument `mathematics.statement` (nine instruments, not eight) | #22 | no |
| Kernel: stop key ends the task; a refused program returns its memory; eight self-tests | #33, #37 | not applicable (the kernel is not deployed) |
| Kernel: rollback of a refused group; frame cutoffs | #38 | not applicable |
| Host hooks: a start delivered twice is one task; stops matched on session and agent together | #40 | no: the binary in use has both faults |

`PCP_OS_Rust/Cargo.toml` still says version 1.0.22. So two different programs now answer to "1.0.22": the packaged binary, and anything built from `master`. Until the version is raised, a build from `master` must be told apart by its commit, not by its version. Raising it is a release decision and is left to the rights holder and the two directors (issue #41).

**What was run on `master` at `6254a71`, fresh clone, Linux x86_64, rustc 1.97.0, QEMU 8.2.2 without hardware virtualisation:**

| Check | Result |
|---|---|
| `cargo build --release` | exit 0, 0 warnings |
| `cargo run --release --bin pcpos-vectors -- vectors` | exit 0; the tracked vector files come out byte-identical; `guard_cases.jsonl` (6 MB) is generated and is now ignored by Git |
| `cargo test --release`, whole workspace | 68 pass, 0 fail |
| Kernel, `PCPOS_APPEND="then=off test=frame-cutoffs" sh run.sh` | 72 self-tests pass, 0 fail, exit status 33 |
| `cargo clippy --release --workspace --all-targets` | 0 errors, 16 warnings of six kinds, none about safety |
| `cargo fmt --all -- --check` | fails: 604 places differ from rustfmt's defaults. The source keeps long lines by design and has no `rustfmt.toml`; nothing was reformatted, since that would rewrite every file under some thirty open pull requests |

**Still not done.** The Python implementation was not exercised. The `audit/sim` scenarios were not run against a binary built from `master`. No binary built here has been compared byte for byte with a release build, and there is no reproducible build. Nothing here was run on Windows, macOS, OpenBSD or FreeBSD by this side, or on real hardware.
