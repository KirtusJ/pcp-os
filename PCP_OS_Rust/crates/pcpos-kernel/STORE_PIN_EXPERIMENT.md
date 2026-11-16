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

A production **daily-driver** storage subsystem needs a separate, source-pinned acceptance contract: enumerate actual bus/port/device and stable path, fail closed if the chosen removable medium disappears, verify logical/physical sector size and 64-bit capacity, verify a selected partition against fully checked primary/backup GPT, bind installation permission to the selected device and operation, keep backup/recovery media independently bootable, and provide negative tests for wrong disk, duplicate GUID/ID, truncated capacity, interrupted install, both recovery slots invalid, and surprise removal. An **encrypted volume** adds a separate KDF/key lifecycle, authenticated encryption, wrong-key, corruption, relock and secrets-redaction obligations. A first-party ACP/PCP method text file does not enforce any of these device effects.

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

**Evidence status: authored sources only until the exact-head workflow finishes and its decoded log has been examined.** No actual source was merged, flashed, installed or granted a permission. No physical or private PCP-OS storage device was used.
