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
