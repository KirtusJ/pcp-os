# GPT preflight for a future virtio-blk installer

**Session B, isolated review candidate only.** This is a read-only parser and test harness, **not** a virtio-blk driver, GPT repair tool, encrypted filesystem, installation program, bootloader, ISO generator, device-identity authenticator, human approval mechanism or host-enforced PCP. It does not access firmware or real disk devices.

## Scope of the contract

Director PT assigned legacy **virtio-blk PCI with GPT** as Session B's first current-product storage transport in [coordination issue #132](https://github.com/KirtusJ/pcp-os/issues/132#issuecomment-6909248700). PT's private version-8 dual-identical-disk refusal is a producer-reported emulator result. This public candidate is **separate**, based on the pinned public master and does not possess PT's private driver or installer bytes.

The dependency-free `no_std` Rust library defines a **read-only 512-byte sector reader** supplied by a future driver. It interprets the capacity in **512-byte protocol units**, as required by [OASIS virtio-blk 1.2 §5.2](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html). The driver must separately negotiate `VIRTIO_BLK_F_RO`, `VIRTIO_BLK_F_FLUSH`, sector/cache features and discover the device using a valid PCI binding. This parser **does not do that**. Its read-only transport trait deliberately has **no write method**.

The source applies a deliberately *narrow profile* of the [UEFI 2.11 GPT rules](https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html):

- The expected 512-byte protective MBR record, primary header at LBA 1, backup header at the actual final LBA, valid signatures, v1 revision, checksums and reciprocal LBAs.
- GPT header **exactly 92 bytes** and **128 partition entries × 128 bytes**, occupying 32 protocol sectors in each copy. The specification permits other valid header/entry shapes. This implementation returns `UnsupportedProfile` for those instead of pretending that they are malformed.
- Exact equality of primary/backup **entry bytes**, independently verified CRC32 for the header and partition arrays, consistent disk GUID, usable LBA bounds, no duplicate partition GUIDs or overlapping partition extents, and an explicit selected partition unique GUID **and type GUID**.
- Full 64-bit LBA and checked capacity bounds. A corrupt primary **or** backup is a refusal for installer-write purposes. UEFI permits recovery under explicit policy, but this source **never repairs either GPT** and cannot waive user approval.
- A reported read-only virtio device can be **inspected**, but its `still_writable` *planning check* refuses writes. Capacity/generation changes likewise refuse. An untrusted caller can lie about generation; this is an expectation for a future trusted device API, not proof of device identity.

A complete device inventory may be checked for duplicate selected GUIDs through `choose_unique`. The `inventory_complete` flag is **not authenticated**, and the function cannot discover hidden or removed devices. **Cloning a GPT yields cloneable GUIDs and CRCs**. The [UEFI spec warns that copied disks require regenerated disk and partition GUIDs](https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html); none of this authenticates a physical drive or grants a destructive operation.

## Tests and deliberate adverse examples

`tools/make_fixtures.py` synthesizes an entire 512-sector, 512-byte-per-sector GPT image using Python's **independent `zlib.crc32`** implementation. It also generates malformed versions: primary and backup CRCs, mismatched headers, damaged protective MBR, disagreement or CRC mutation of partition arrays, overlap, duplicate partition ID, unsupported entry count, wrong type/disk identity, 64-bit out-of-range LBAs, truncated capacity, and an **exact clone**. No fixtures are created on a real disk.

`tests/gpt.rs` supplies host-side `ReadSector` from those temporary bytes and checks the library's error classifications, a known CRC32 test vector, explicit read-only/generation-change write-planning refusals, and duplicate-GUID ambiguity. The CI independently runs `sgdisk --verify` against the positive GPT image as a second source of structural evidence before running debug/release Rust tests. A generated test image is a *data fixture*, not a bootable or installed operating system.

To execute locally on a normal host (with Python 3, Cargo and `sgdisk`):

```sh
python3 PCP_OS_Rust/crates/pcpos-kernel/gpt-preflight/tools/make_fixtures.py --out /tmp/pcpos-gpt-test-only
sgdisk --verify /tmp/pcpos-gpt-test-only/valid.raw
PCPOS_GPT_FIXTURES=/tmp/pcpos-gpt-test-only cargo test --offline --manifest-path PCP_OS_Rust/crates/pcpos-kernel/gpt-preflight/Cargo.toml
```

All files in this workflow live under the GitHub runner's disposable work/temporary directories. **No QEMU kernel boot or physical-media hotplug is claimed** by this particular parser test.

## Outstanding product acceptance blockers

This code cannot itself establish a **real** legacy virtio-blk PCI binding, negotiated feature set, capacity, queue or DMA safety, device serial/path uniqueness, ongoing medium presence, I/O cancellation, flush durability, filesystem access, data encryption, key lifecycle, authenticated update signing, partition installation, multi-disk fallback, human consent or boot recovery. A real driver must check the device generation at each I/O and refuse when it is unplugged or a different device claims the same address. A GUI response does not prove a write or its recovery was permitted.

A future end-to-end acceptance exercise must attach two separate disposable QEMU virtio-blk controllers with matching GPT identities, remove one during pending operations, corrupt each metadata copy and both, shorten device capacity, toggle `VIRTIO_BLK_F_RO`, inject flush failures, cut power between phases and compare **both full disk-image hashes**. It must then reproduce on physical supported hardware with installer/recovery media before daily-driver use. Keep an actual bootable ISO, whole-system source generation and rollback distinct.

**Status until CI execution:** authored source and independent test specifications only. Do not infer passing Rust tests or correct virtio/GPT hardware behavior until a job with exact source SHA has executed and its logs are reviewed. No merged product or true calibration is claimed.
