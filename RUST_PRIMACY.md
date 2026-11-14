# Rust holds primacy

**Permanent note for everyone who works on PCP OS, human or agent. Set by the rights holder, Kirtus Justus, 14 November 2026.**

PCP OS is primarily Rust. Rust always holds primacy, for security reasons.

What this means in this repository:

1. **The Rust implementation is the reference.** `PCP_OS_Rust/` (the core, the host program `pcpos`, the kernel, the published vectors) defines what PCP OS does. Where another implementation or a script disagrees with it, the Rust behaviour stands until the Rust source is changed by review.
2. **`PCP_OS_Python/` is the second implementation.** It is tested against the Rust vectors. It does not lead: a behaviour is not added, fixed or relaxed there first, and it is not the place where a security property is decided.
3. **Security-relevant behaviour is written in Rust.** Anything that touches the register, its hash chain, locks, repair, the object store, method verification, paths, links, permissions or what a participant may do is implemented and tested in Rust. A check that exists only in Python, in a shell script or in a plugin hook does not count as the protection.
4. **New instruments ("Store apps") that run inside the host are Rust**, registered in `PCP_OS_Rust/crates/pcpos-host/src/instruments.rs`. Tools kept outside the host in other languages are helpers for a particular workspace; they are not part of PCP OS and must not be described as such.
5. **A fault is reported against the Rust build**, with the exact revision, command, input and output. A fix lands in Rust first, with a test in Rust; the Python implementation follows so that the vectors still agree.
6. **Tests and audit harnesses** in other languages may drive the Rust program from outside. They do not replace Rust tests for the property they exercise.

If something here seems to conflict with a task you were given, stop and ask the rights holder; do not work around it.
