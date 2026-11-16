# Session B: virtio-blk/GPT selected-target safety, source-only acceptance draft

**16 November 2026, B.C. PCT (UTC−07:00).** **DRAFT; NOT AN INTEGRATION.** Maintainer allocation: B owns selected-target/device-boundary tests; PT owns private installer, trusted authority, native drivers and release; A owns presentation/onboarding; PCP steward owns method meaning; release/QA owns independent acceptance.

## Exact evidence boundary

Public `master` inspected at `67e57f924ce0e2c86f9250c567796b745ed633c8`. Its virtio code is an experimental virtio-net path, **not** the private virtio-blk/GPT installer. PT *reported* a private UEFI optical image at `ae72a4e57346a3bbbe129a742f82f2ac0166c403` in [issue #132](https://github.com/KirtusJ/pcp-os/issues/132#issuecomment-6909706885). That private source and ISO were not independently fetched or executed in this review. Historic opt-in PR [#143](https://github.com/KirtusJ/pcp-os/pull/143), SHA `0e648ca36ef19aa53c5025100ae026a5695fb6de`, contains a **retained wrong-disk write for an exact clone of the marker and ID**. No success of that legacy IDE experiment authenticates a device or certifies this different private transport.

Externally retrieved:
* [UEFI 2.11 GPT, §5](https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html): 64-bit LBA, primary and end-of-device backup headers and entry arrays, header/array CRCs, usable bounds and repair guidance. **Coverage limit:** search-index source text was inspected, but a subsequent direct page fetch returned HTTP 403; the complete HTML was not directly inspected.
* [OASIS Virtio 1.3, §§2.4, 2.5, 3.1, 5.2](https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.html): block `sector` units always **512 bytes**, regardless of `blk_size` optimal size hint; configuration consistency and device reset, capacity bounds, read-only feature and flush operations.
* [NIST SP 800-193](https://csrc.nist.gov/pubs/sp/800/193/final) for firmware protection, detection and recovery as design context, **not proof of implementation**.

## Proposed product contract, not a PCP method amendment

1. **Actual bus enumeration:** the trusted OS supplies current device handle, PCI path/function, controller/reset epoch, device generation, capacity in 512-byte protocol sectors, a separately validated GPT LBA size, read-only/flush capability and present/in-flight state. A controller path, GPT Disk GUID, static marker or virtio ID is *not* authenticated physical drive identity. For the legacy virtio configuration without generation field, follow Virtio's consistent configuration-read requirement, and separately audit change/reset handling.
2. **Selection versus effect authority:** a written principal instruction identifies current target, contents, operation, region and selected image digest. The **host** captures authentic actor/authorization and binds it to the enumerated handle/generation/epoch, exact scope, expiry/revocation, transaction and destination. The model, the interface, a checkbox, a `store_id=` token or a procedural override can never grant a disk write. Re-check authority at the destination before irreversible effects.
3. **Typed geometry:** keep firmware optical 2048-byte records, virtio protocol 512-byte sectors, and GPT logical LBAs as separate units. Check arithmetic overflow, capacity changes and partition boundaries. On fresh whole-disk factory copying require the target's exact capacity to match the factory image; a larger target needs separately proved GPT backup relocation/expansion. No silent partial image copy.
4. **Media condition:** a fully blank claim requires an actual full-media check or an explicitly weaker label. A valid GPT is *populated*, not blank. Corrupt/contradictory primary or backup GPT, stale backup after resize, unknown contents, truncated image, mismatched digest or unverifiable volume identity are not automatic permission to erase. Validate primary+backup GPT headers, CRCs, arrays, reciprocal locations, partition ranges and nonoverlap; use a separately authorized repair route.
5. **In-flight fencing and durability:** on unplug, controller reset, changed geometry, stale selection, revoked grant or cancellation, do not submit further writes and explicitly observe/drain/fence outstanding requests before returning completion or reusing DMA memory. Distinguish completed, possible and unknown effects. Do not label installation durable without supported successful flush and independently observed readback. Checkpoint and cancel behavior after a power cut requires a new approval.
6. **Recovery:** independently accessible recovery with the installed disk absent, both GPT copies damaged, keys unavailable, both system slots invalid and update interruption at each stage must have named outcomes. Physical-device testing remains separate from disposable QEMU fixture evidence. Keep encrypted volume tests independent from plaintext disk identity.
7. **Evidence-source mode:** local source inspected offline, externally inspected material, available-but-unread, and unavailable source must be separately reported. Core §15.8's actual `Grounding/Delivery/Execution/Observability` axes remain governing; this fixture's source labels are **proposed** descriptive metadata, not canonical PCP enum extensions. Optional external research being unavailable does not authorize bypassing high-consequence installer checks.

## Fixture contract

`target_preflight.py` is a *pure model* taking caller-supplied facts; no device, user, permission mechanism, source file, cryptographic signer or OS is accessed. It can return only `HOLD` or **`ELIGIBLE_FOR_HOST_RECHECK`**, never `AUTHORIZED` or `SAFE_TO_WRITE`. A virgin blank disk need not contain a pre-existing media ID; the mock plan may therefore match an empty ID without asserting uniqueness. This initial restrictive profile holds when there are two writable block devices **even if one is on an unsupported bus**, because unsupported is not invisible. An externally sourced cache that is accessible offline but unread is `NOT_INSPECTED`, not `UNAVAILABLE`. `test_target_preflight.py` exercises expected consistency failures and **preserves an indistinguishability counterexample** where a same-port full clone can pass the pure model. Do not treat a green model test as disk safety.

Run the tests with `python3 -m unittest -v test_target_preflight.py` in this directory.

## Concrete PT-side falsification requests (not performed here)

| Arm | Exact product evidence needed |
|---|---|
| Correct fresh target | Optical source identity, trusted device enumeration and typed consent; independent whole-media before/after digest and separate installed-media readback |
| Wrong/cloned disk | Two devices, swapped controllers, copied GPT GUID and ID, one surviving clone on original path; report which ambiguity is detectable and show *no writes* for all denied cases |
| Geometry | 512-byte virtio protocol vs 2048-byte optical media vs 512/4096 GPT LBAs, undersize/oversize, high-LBA arithmetic and last-LBA backup GPT |
| GPT failures | Corrupt primary, corrupt backup, conflicting arrays, both invalid, stale alternate after expansion; show repair/refusal without silent formatting |
| Interruptions | Cut before any write, after payload, before/between/after each GPT metadata copy, in-flight device removal; confirm actual outstanding I/O fate and independently usable recovery |
| Authorization | Invalid actor, stale approval, changed image, cancel before admission, revocation during I/O; prove the effect-side decision, not a UI checkbox or assistant report |
| Offline triplet | Local source present and actually read; no local source; unavailable external source with networking disabled; show exactly `inspected`, `unavailable`, `not-inspected`, or `skipped` |
| Separate release gate | Tailorbird source proposal → pinned dependencies → independent clean whole-system rebuild → bootable/installable ISO → destructive target permission → recovery and signed promotion |

**Negative boundary:** the fixture does not assert an install, actual media writes, QEMU result, host-enforced ACR/PCP calibration, kernel start, protection of physical hardware, or acceptance of an ISO. It does not change current production code or the physics/cryptography projects.