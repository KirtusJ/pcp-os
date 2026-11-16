# Director 11: read-only guest-memory candidate head oracle

**Experimental forensics, not a PCP OS runtime or acceptance certificate.** This is a standalone, host-side way to obtain an SHA-256 chain head *without using the displayed digest as the source of the expected answer*. It complements Director 10's register row and Director 2's optical/keyboard witness without modifying their files, production kernel, or shared installed VM.

## Scope and exact starting point

At Director 10's draft [#233 at `74c4d4a`](https://github.com/KirtusJ/pcp-os/pull/233), `session::register_view` calls `pcpos_core::ledger::verify(rec.ledger().bytes())`, which returns the fully checked event list and head. The kernel's live `Record` is held in RAM. `pcpos_core::ledger::verify_tail` hashes each **canonical event line (without the terminating `\n`)** to check the next `prev` and to produce the final head. This script instead scans a *guest-physical RAM dump* for candidate byte sequences and implements its own bounded JSON/canonical/prev-chain validation with Python's standard library. It never imports the native pixel checker nor selects a candidate based on the displayed head.

To make the framebuffer and RAM mutually comparable, capture both while the disposable **model-free QEMU guest is paused** at the register screen, before pressing Esc or P. In an existing authorized QMP session that has reached the stable register screen:

1. Send QMP `stop`, wait for the `STOP` event (and refuse the capture if stop is unconfirmed).
2. Send `screendump` to a fresh, trusted path and `pmemsave` (`val=0`, `size=134217728`, `filename=<trusted path>`) while still stopped. Confirm the guest reports 128 MiB RAM and that the command succeeds; the exact supported QEMU version and physical-memory layout matter. Record QMP request/reply and hashes for both captures.
3. Extract the **64 visible glyphs and the event count** from the P6 using the independent optical decoder. Then, separately, run this script on the RAM dump with the visible result, for example:

```sh
python3 register_ram_oracle.py paused-guest-ram.bin \
    --visible-head cc07e558fb7c0d3b3df55c816e7321f9cf6140108d2d20961e003228ab4fa10f \
    --visible-events 41
python3 -m unittest -v test_register_ram_oracle.py
```

The example head is the previously **pixel-decoded** observation from Director 10's report; no raw RAM evidence has been supplied for it here. This invocation is illustrative, **not** an executed guest result. For a new run, use its *own* event count and actual optically decoded head; never reuse an earlier boot's digest. Capture the QEMU kernel-image SHA-256 and measurement independently. Clean guest shutdown, if attempted after the capture, is a separate check.

## Interpretation and adversarial checks

- The candidate scan starts only at canonical event 0 (`WORKSPACE_CREATED`, `body.label = kernel`), enforces Rust's event field shape, string escaping, signed 64-bit numbers, bounded nesting, timestamps and authorizations, and hashes each line independently. It rejects broken predecessor links and ambiguous maximum-length complete chains, rather than choosing the chain whose head happens to match the pixels. It compares the unique longest candidate's count and head only **after** selecting it.
- A positive synthetic 41-event record, a stale shorter copy, and negatives for earlier-event mutation, last-event mutation, wrong visible last digit, wrong count, wrong workspace, and two conflicting longest valid candidates are in the test file. Their local execution is a host-only unit test, **not** native QEMU evidence.
- A successful scan would establish: canonical, chain-consistent candidate register bytes were present in memory captured while the framebuffer showed a matching head. It still would **not** establish the actual *live `Record` pointer*, authenticated event authors, immutable remote storage, or lack of another unobserved source. Heap/stale copies can exist. Do not describe this as the fully authenticated same-register-state promotion oracle without resolving pointer provenance and capture trust.
- An invalid-ledger native guest refusal is a **different** test. Neither this scanner nor changing the supplied visible digest injects damage into the active kernel register. A separately authorized test must corrupt the **live** pre-R view record (not a detached copy), observe `ledger::verify` returning an error and the actual `verification failed` screen, and preserve a valid-positive control in a separately booted disposable guest. Do not attempt a memory write against an installed/shared guest.
- A complete RAM dump can contain secrets, program contents, and historical copies. Do not publish, upload, or retain a full RAM dump from installed Qwen, a live workstation, or any non-disposable guest. Work only on a disposable model-free QEMU instance. For public archives retain the independent *summary*, QMP status/parameters, scan counts, and hashes. Treat even extracted raw records according to actual disclosure permissions.

No persistent container, independent PCP OS worker, installed LLM, runtime authorization, or current liveness is established by these files. They provide one bounded observation method for the already-open Director 10 acceptance question.


## RAM provenance negative controls (16 November 2026)

The phrase **unique candidate** means one *physical byte-range*, not merely one
distinct sequence of event bytes or one SHA-256 digest. A paused guest may contain
two byte-for-byte identical full copies of a valid chain at different addresses.
Neither copy is automatically the live Rust `Record`.

The bounded synthetic fixture `test_candidate_provenance.py` now distinguishes
those cases: equal-length, identical full copies **must fail closed** with
`multiple equally long RAM candidate locations`; different equal-length chains
fail as `ambiguous longest chains`. Shorter stale copies do not disqualify a
single, unique maximal 41-event candidate. A longer unrelated decoy fails the
visible-count comparison. Conversely, a *single fabricated chain in bytes that
contain no real kernel at all* can satisfy the tool's canonical/count/head
checks: this explicitly demonstrates that even a successful single
candidate is **not proof of live-record-pointer identity or authenticated
origin**. The tool does not promote it to one.

Before the change, identical maximal copies with the same content digest were
incorrectly collapsed into one "unique" candidate. Retain the test-first red
Actions log, not as a kernel failure but as an overstrong **host-side forensics
predicate**. The updated predicate explicitly refuses duplicate physical
locations regardless of their equal hashes.
