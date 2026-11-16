# Director 11: exact no-feature diagnostic image identity experiment

This is **test-only** qualification of a statement about two pinned source trees, not product functionality. It does not build a release ISO, change master, or commission a review of an installed system.

**Current workflow source pins:** Director 10's [#233 `74c4d4a9934ad68e1b5be380971bf3377a464c6d`](https://github.com/KirtusJ/pcp-os/commit/74c4d4a9934ad68e1b5be380971bf3377a464c6d) and the diagnostic-disabled successor [#240 `f1b7430a9c1aaabc5587f19b16e0c1e76646ef81`](https://github.com/KirtusJ/pcp-os/commit/f1b7430a9c1aaabc5587f19b16e0c1e76646ef81). The workflow verifies the *actual* two checkout HEADs before issuing the final verdict. No other D10 revision is represented by the current result.

**Prior adverse lineage is retained.** Earlier D10 diagnostic source `b6d3027` and restored-function revision `9fa9819` gave negative exact measured-image comparisons, including [Actions 43964552058](https://github.com/KirtusJ/pcp-os/actions/runs/43964552058). The source-line shift was isolated through controlled blank/comment/cfg insertions and byte-level 31-word checks. D10 then preserved original source positions in `f1b7430a`. Results for those three revisions must not be conflated or described as retroactively overturned.

**Observed on exact current pins (as of 16 Nov 2026):** [Actions 43974384410](https://github.com/KirtusJ/pcp-os/actions/runs/43974384410) found repeat-build A/A whole-ELF identity; D10 feature-OFF versus #233 *whole ELF bytes differ* (`2c8a7058…` vs `91fed21c…`), while measured code+rodata, all allocated section contents, the entry point, all five PT_LOAD header contracts and their file-backed byte digests match. Changed nonallocated contents are `.symtab` and `.strtab`; `.shstrtab` shifts file offset with unchanged bytes. That job remains **RED by its predeclared strict whole-ELF predicate**, despite Q35 exit 33 and `ALL SELF-TESTS PASS`.

[Actions 43976036103](https://github.com/KirtusJ/pcp-os/actions/runs/43976036103) added a narrower GNU `objcopy --strip-all` *normalization experiment*: all three independently stripped `base_1`, `base_2`, `candidate_off` files had exactly 510,896 bytes and SHA-256 `8fa24fefaccde71bc53389757ef4f03f1e2195380b1bcba82c86dcfe98718dc8`. This is **stripped-copy byte equality under the one pinned runner**, not raw shipping-ELF identity. The actual QEMU test boots the untouched, **unstripped** no-feature ELF. The strict raw-ELF test rightly stays RED.

**Counterexample-driven correction of the stripped comparator:** an initial report hardcoded the raw whole-ELF inequality rather than reading its raw digest witnesses. Two new synthetic tests failed exactly as expected in [Actions 43976535496](https://github.com/KirtusJ/pcp-os/actions/runs/43976535496); the corrected script now requires all three original `.elf_sha256` files, rejects absent/malformed witnesses, and fail-closes if raw or stripped A/A disagrees. The full host-only controls then passed in [Actions 43976646713](https://github.com/KirtusJ/pcp-os/actions/runs/43976646713). The normalized identity observation is conditional on all controls; it cannot authorize any release.

## Discriminating controls

The isolated workflow builds, in sequence, using **one fixed absolute staging path** and a clean staging directory per compile:

1. `base_1`: #233 with **no feature**.
2. `base_2`: a second **independent clean #233 build** under exactly the same toolchain and staging path. This is the A/A positive control; if the pair differs, cross-source comparison cannot be interpreted as code drift without investigating the toolchain.
3. `candidate_off`: #240, with **no feature**.
4. `candidate_on`: the same #240 with **only `register-view-diagnostic` enabled**. This is the feature-separation control.

Report both SHA-256 of the entire *unstripped* QEMU loader ELF and the independent measurement algorithm's SHA-256 of its executable/read-only image sections; independently compare allocated-section and loadable-segment byte contracts, then optionally compare separately stripped disposable ELF copies. A result may distinguish reproducible ELF identity, unchanged measured code despite a packaging change, or changed measured code. Hash agreement demonstrates only the bytes checked on this runner; it does not prove equivalence on other toolchains, other machines or runtime paths.

The runner also boots the **no-feature candidate**, unattended in a disposable, model-free Q35 guest, using the separately calculated image measurement. It requires QEMU exit 33, an actual serial `ALL SELF-TESTS PASS`, and the measured-equal runtime report. Any failure retains the exact hashes and serial with a red CI verdict.

Only source-pinned hashes, model-free serial, section/load/stripping summaries, and the adjudication report are uploaded. No raw RAM, user records, installed Qwen, container worker or released image is part of the experiment.

## Disposition boundary

A result of `BYTE_IDENTICAL_IN_EXACT_BUILD_CONTRACT` would support that specific source/toolchain/feature contract. It would not make the diagnostic branch safe to merge or justify enabling serial export on real data. `DEFAULT_MEASURED_CODE_DIFFERS` would signal a concrete acceptance gap for source owners to investigate, not by itself a semantic failure of their code. A failed A/A control blocks all such cross-source conclusions.

The Principal's current coordination directive requires every coordinated push to master to accompany an updated ISO distribution; this branch must remain independent of master/release mutation absent all current approvals and acceptance criteria.
