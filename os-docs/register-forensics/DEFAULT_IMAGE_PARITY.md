# Director 11: exact no-feature diagnostic image identity experiment

This is **test-only** qualification of a statement about two pinned source trees, not product functionality. It does not build a release ISO, change master, or commission a review of an installed system.

**Compared sources:** Director 10's draft [#233, `74c4d4a`](https://github.com/KirtusJ/pcp-os/commit/74c4d4a9934ad68e1b5be380971bf3377a464c6d) and its separately instrumented diagnostic draft [#240, `b6d3027`](https://github.com/KirtusJ/pcp-os/commit/9fa9819103a67b0f2b124991a2d0d88b4c64b755).

**This file documents the successor comparison** at D10 corrected commit `9fa9819`, after the original test-only comparison at `b6d3027` produced a negative image-identity result. The first red run and exact sha values remain preserved in [PR #242](https://github.com/KirtusJ/pcp-os/pull/242) and [Actions 43964552058](https://github.com/KirtusJ/pcp-os/actions/runs/43964552058); the present successor build is an independent attempt, not a retroactive reinterpretation of that result.

The diagnostic adds an opt-in `register-view-diagnostic` build feature. Earlier diagnostic commit `b6d3027` refactored the unconditional `register_view(rec)` into a raw-byte wrapper; that version **failed the measured-code identity check**. D10's corrected `9fa9819` restores the original #233 function body for the no-feature path and gates extra raw-buffer functions to diagnostic-only compilation. Exact resulting binary and measured identity remain subject to this new executed control.

## Discriminating controls

The isolated workflow builds, in sequence, using **one fixed absolute staging path** and a clean staging directory per compile:

1. `base_1`: #233 with **no feature**.
2. `base_2`: a second **independent clean #233 build** under exactly the same toolchain and staging path. This is the A/A positive control; if the pair differs, cross-source comparison cannot be interpreted as code drift without investigating the toolchain.
3. `candidate_off`: #240, with **no feature**.
4. `candidate_on`: the same #240 with **only `register-view-diagnostic` enabled**. This is the feature-separation control.

Report both SHA-256 of the entire QEMU loader ELF and the independent measurement algorithm's SHA-256 of its executable/read-only image sections. A result may distinguish reproducible ELF identity, unchanged measured code despite a packaging change, or changed measured code. Hash agreement demonstrates only the bytes checked on this runner; it does not prove equivalence on other toolchains, other machines or runtime paths.

The runner also boots the **no-feature candidate**, unattended in a disposable, model-free Q35 guest, using the separately calculated image measurement. It requires QEMU exit 33, an actual serial `ALL SELF-TESTS PASS`, and the measured-equal runtime report. Any failure retains the exact hashes and serial with a red CI verdict.

Only source-pinned hashes, model-free serial, and the adjudication report are uploaded. No raw RAM, user records, installed Qwen, container worker or released image is part of the experiment.

## Disposition boundary

A result of `BYTE_IDENTICAL_IN_EXACT_BUILD_CONTRACT` would support that specific source/toolchain/feature contract. It would not make the diagnostic branch safe to merge or justify enabling serial export on real data. `DEFAULT_MEASURED_CODE_DIFFERS` would signal a concrete acceptance gap for source owners to investigate, not by itself a semantic failure of their code. A failed A/A control blocks all such cross-source conclusions.

The Principal's current coordination directive requires every coordinated push to master to accompany an updated ISO distribution; this branch must remain independent of master/release mutation absent all current approvals and acceptance criteria.
