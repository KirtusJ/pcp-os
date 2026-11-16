# Experimental v2 IDE volume identifier, not a disk authentication mechanism

**Session B, absorption-only source experiment.** This is **not** a PCP-OS production installer, encrypted volume, removable-device driver, recovery boot path or authorization grant. The current experiment is stacked on [draft #142](https://github.com/KirtusJ/pcp-os/pull/142), which in turn is based on historical unmerged [#119](https://github.com/KirtusJ/pcp-os/pull/119). There is no read/write integration with Director PT's private release.

## Motivation and exact source boundary

Historical `src/dev/store_disk.rs` hard-codes the IDE primary master at I/O base `0x1f0` and `0xe0`, without a device enumeration or hardware serial/WWN/size check. The original v1 store accepts the same public `PCPOS USER STORE 1\n` marker from every formatted store, permitting a wrong-volume write if the wrong primary device happens to carry an identical static marker. [Exact QEMU role witness](https://github.com/KirtusJ/pcp-os/actions/runs/43815025040) reproduced that outcome on temporary QEMU disk files.

This candidate adds an **off-by-default** Cargo feature `store-id-gate`, not a new storage driver. The legacy default **does not change**. When built with the feature:

- Boot must name `store=ide0` and exactly one `store_id=` followed by **32 canonical lowercase hex digits**, not zero.
- The complete, validated loader command line must be available. Duplicate/malformed selector tokens fail before a file is opened for writing. No legacy fallback is compiled.
- Sector zero must match **exactly 512 bytes**: `PCPOS USER STORE 2\n` at offset zero; bytes through offset 31 reserved/zero; **16-byte ID at offsets 32..47**; all remaining bytes zero. The pinned value must equal the supplied boot ID. A legacy v1 marker is not accepted in this feature mode.
- Existing historical two-slot checksums and fixed-position writes are otherwise unchanged. A matching ID grants *no* independent authority; it is an **accidental device-swap discriminator** in this single-device fixture only.

The QEMU harness uses deliberately public reproducible IDs. There is **no secure provisioner, trusted random source, identity ownership record or user consent UI**. New IDs in a real installation would have to be generated safely, tied to a discovered/authorized target and stored in an accountable installation manifest. Even then an on-media ID is copyable.

## Adversarial decision table

| Emulated arrangement | Selector | Expected |
| --- | --- | --- |
| Correct v2 store on IDE0, non-store sentinel on IDE1 | Matching unique ID | Write, cold-read, sentinel unchanged |
| Wrong v2 ID on IDE0, intended store on IDE1 | Intended ID | **Refuse**, no disk writes |
| Correct v2 store on IDE0 | Incorrect/missing/duplicate/uppercase/zero ID | **Refuse**, no writes |
| Legacy v1 media on IDE0 | Supplied v2 ID | **Refuse**, no transparent downgrade |
| IDE0 absent, correct store on IDE1 | Intended ID | **Refuse**, no implicit remapping |
| **Different disk with an exact clone of ID and store header on IDE0** | Matching ID | **EXPECTED ADVERSE:** still writes the wrong disk |

The final case is important: this addition **cannot protect against deliberate cloning, spoofed hardware, an attacker who edits boot parameters, duplicate device IDs, replacement of a storage bus, a disk that lies about flushes, or physical unplug mid-I/O**. No cryptographic authentication, encryption, partition boundaries or actual ATA `IDENTIFY DEVICE` capacity checking are implemented. There is no anti-rollback or anti-bricking mechanism.

## External source checks and next gates

The [UEFI 2.11 GPT specification](https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html) distinguishes **64-bit LBAs**, primary/backup GPT headers, CRC32, and per-partition GUIDs, and explains that copied GPT disks need new Disk GUID and UniquePartitionGUID values to avoid ambiguous identification. Those *on-media* GUIDs aid device management; neither a GUID nor a CRC authenticates the physical disk or a permission grant. The ATA T13 `IDENTIFY DEVICE` format distinguishes device capacity and logical sector sizes; the legacy primary-master driver currently does not use it. Neither source should be presented as a certificate that this special 259-sector raw IDE store is GPT compliant.

A production **daily-driver** storage subsystem needs a separate, source-pinned acceptance contract: enumerate actual bus/port/device and stable path, fail closed if the chosen removable medium disappears, verify logical/physical sector size and 64-bit capacity, verify a selected partition against fully checked primary/backup GPT, bind installation permission to the selected device and operation, keep backup/recovery media independently bootable, and provide negative tests for wrong disk, duplicate GUID/ID, truncated capacity, interrupted install, both recovery slots invalid, and surprise removal. An **encrypted volume** adds a separate KDF/key lifecycle, authenticated encryption, wrong-key, corruption, relock and secrets-redaction obligations. A PCP method text file does not enforce any of these device effects.

A fully bootable and installable ISO plus Tailorbird release-source generation, independent audits and 64-bit app-wide migration remain **outside this experiment** and are not presumed complete by a green host/kernel test.

## Test entry points

```sh
# Pure host-value controls in debug and optimized profiles
rustc --edition=2021 --test PCP_OS_Rust/crates/pcpos-kernel/tests/store_identity.rs -o /tmp/store-id-test
/tmp/store-id-test

# Product-private/sensitive disk paths are never used. With QEMU only:
PCPOS_KERNEL_FEATURES=store-id-gate sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
python3 PCP_OS_Rust/crates/pcpos-kernel/tools/store_bound_probe.py \
  --kernel PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32
```

The accompanying `.github/workflows/ide-volume-pin.yml` builds and runs the historical default path **separately** before the feature-gated candidate, using only CI-created temporary disks. The host may install QEMU/compiler packages using network access; QEMU guests run with no NIC.

## Independently inspected executable receipts

The original test submission [run 43817842255](https://github.com/KirtusJ/pcp-os/actions/runs/43817842255)
failed **before the kernel ran**: a test fixture's alternate volume ID
had 31 rather than 32 hexadecimal digits; the parser rejected it and
a positive fixture then attempted to unwrap the negative result. This
was a **test-data defect**, not evidence that a genuine 128-bit disk ID
was rejected. The 32-digit fixture was corrected in both test sources.
The older historical role test was also left separate.

The [successful CI run 43817995284](https://github.com/KirtusJ/pcp-os/actions/runs/43817995284)
at corrected head `0e61cddcbf5c336b35951a20c2e11e4b6b8cf359`
ran all **18 host Rust ID tests in ordinary and optimized profiles**,
built the historical/default kernel **separately** from the optional
`store-id-gate` kernel, and executed both QEMU matrices.

Then the [expanded exact-source run 43818444515](https://github.com/KirtusJ/pcp-os/actions/runs/43818444515),
[job 133135726540](https://github.com/KirtusJ/pcp-os/actions/runs/43818444515/job/133135726540),
tested `906e80fb3e85fd15232220850b062e29188cd299` with
**18/18 ID tests in both profiles**, and source-bound actual
QEMU `-kernel` test guests. The same run observed:

- Historical **v1** fixture: `primary-marked-store` write/cold-read
  success, safe refusal for an unmarked GPT-like primary and a missing
  IDE0, **and the original wrong-primary cloned-marker write**.
- Feature-gated **v2**: exact ID allows creation and cold-read;
  different-ID primary, wrong selected ID, missing, duplicate, uppercase,
  short or zero ID, ambiguous `store=` tokens and legacy v1 marker
  **refuse without a file write**.
- Two additionally tested selectors, an ID alone with **no `store=`**
  and an ID with unsupported `store=another`, **refused with an explicit
  message** instead of launching a user program on an unsuspected
  volatile-RAM backend.
- **Exact-cloned ID/sector on the wrong IDE0 remains accepted** and
  receives the write. This is still **EXPECTED ADVERSE**. The original
  intended store at IDE1 remained unchanged.

The host-inspected kernel ELF32 SHA-256 identities in that run were
`60fcf96ad5fffc5481b0283f948d76df5a5f80bca4da2d6f11a121c089db5026`
for the **historical default** and
`8e7934629683ddfc9bb3e77813e12d53683f41c250cc0efb4fa5fd97d261cdfe`
for the **opt-in** kernel. The tests are executed on *disposable*
QEMU primary-master IDE disk image files only, with real host-file
byte/hash readback. The displayed `PASS` from the harness means the
expected **negative clone outcome was retained**, not that device
identity or safe disk installation passed.

**After this result, documentation/CI trigger edits are separately
recorded as revisions. The result above is tied to the exact executed
source SHA, not silently to any newer untested commit.** No
source was merged, flashed, installed or granted a permission.
No real hardware, encrypted volume, physical boot disk, 64-bit-LBA
transport, removable hotplug or PT-private release was tested.



## Current executable receipt and separate evidence-source categories

A subsequent **12 inline Rust test cases** in `src/store_identity.rs` were added
to the 18 existing host cases. Some intentionally overlap: this is **30
executed Rust tests**, not 30 independent assessors or physical devices.

[GitHub Actions run 43820682770](https://github.com/KirtusJ/pcp-os/actions/runs/43820682770)
at exact code head `e1c3867bb638a36ba8e5bcce7a77622e5fac1cc0`
executed **30/30 host debug tests** and **30/30 optimized tests**,
compiled the historic default and opt-in 64-bit kernel separately, and
completed the two QEMU disk-role matrices. The feature-gated candidate
refused mismatched/malformed/missing IDs, unsafe selector combinations,
v1 downgrades and absent IDE0 without a host-observed disk mutation.
**An exact-cloned ID on the wrong IDE0 still wrote to the wrong volume.**
This is an expected *negative* security/disambiguation finding, not
approval of safe physical disk selection.

Compiled historical kernel SHA-256: `60fcf96ad5fffc5481b0283f948d76df5a5f80bca4da2d6f11a121c089db5026`.
Opt-in ID-gated kernel SHA-256: `e783e4413c9876faf96298f21ddd7ec52fe3f50a448f06a9ab0448381882761b`.
Do not conflate those feature-set identities.

**Externally referenced:** UEFI/GPT source material describes partition
identifiers and structures. It does not prove this raw IDE store
correctly identifies or authorizes a target, nor does it prove a
physical disk implementation exists.

**Internally source-bound and executed:** the exact public Git commit,
CI job/toolchain, QEMU console output and independently compared
disposable disk-file bytes. These are observable outcomes of a
particular emulator build, not independent inspection of PT's private
installer or real device.

**No checked source or evidence available:** physical device path,
actual GPT/ESP table provenance and 64-bit LBA capacity, authenticated
volume identity, encryption/KDF/secret recovery, surprise-unplug,
USB/NVMe/AHCI support, hardware bricking/recovery and the current
private shipping artifact. Each remains unqualified, never
automatically passed due to offline or missing source access.

**Separation of permissions:** a syntactically valid `store_id=`
selects cloneable metadata only. It is neither user consent nor an
authorization capability. Optional PCP/ACR review-stage selection
cannot grant storage writes or replace a privileged installer gate.
These statements are acceptance guidance, not host enforcement.
