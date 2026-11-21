# Director 11 | Offline GPT mirror / UEFI protective MBR acceptance boundary

**Experimental, test-only audit. No production GPT implementation changes.**

## Exact scope

The Principal asked the active directors to audit PCP OS daily-driver
wireless, USB dongles, Bluetooth/Bluetooth app, bricking risk and release
version/tag controls. The owning **device/anti-bricking audit** is
[issue #250](https://github.com/KirtusJ/pcp-os/issues/250), maintained by
Director 13; version/tag governance is
[issue #251](https://github.com/KirtusJ/pcp-os/issues/251).
Director 11 here independently exercises *one narrow validation boundary*,
without modifying either owner's implementation.

- Pinned source: [pcp-os master commit 59f26ebe](https://github.com/KirtusJ/pcp-os/blob/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c/os-docs/installed-acceptance/gpt_integrity.py)
- Expected exact Git source blob: `620c82a7d686230d1b2ed82421b349da7a72f6be`.
- Function: `require_gpt_mirror(fd, disk_size)`, which documents its purpose as
  checking both GPT metadata copies, not entire UEFI boot compliance.
- External comparator: [UEFI 2.11, ``5.2.3 and 5.3.2](https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html).
  For a *bootable GPT disk* the protective MBR at LBA0 is required and
  contains an MBR signature `55 aa`, a protective partition type `0xEE`,
  and the prescribed LBA/size fields. The current GPT header revision
  value for version 1.0 is `0x00010000`.

## Bounded controls

The CI runner verifies both the **exact Git checkout commit** and
**exact Git blob SHA** of the checker it imports, then creates a disposable
sparse 16-MiB **512-byte-sector** GPT image in a temporary directory for
each of these conditions:

1. Valid primary/backup GPT, protective MBR and valid revision: expected
   acceptance positive control.
2. Valid mirrored GPT but **entire LBA0 missing**: check whether the narrow
   checker notices an absent protective MBR.
3. Mirrored GPT with PMBR type `0x07` in place of `0xEE`.
4. Mirrored GPT with missing mandatory `55 aa` MBR signature.
5. Mirrored valid GPT CRCs with header revision `0x00010001` on both copies
   rather than the version 1.0 spec's `0x00010000`.
6. Corrupted primary GPT signature: expected rejection control.
7. Corrupted backup GPT entry array: expected rejection control.

The source module's `os.pread` calls are also counted for the valid fixture:
their read offsets should not include LBA0, explaining why PMBR changes
cannot affect this function's result. The test records actual accept/reject
results rather than reclassifying source acceptance as a UEFI/firmware pass.
If a future source change closes the gap, the pinned source check fails and
the test must be reassessed under a new exact revision.

## Important distinction

A **green experiment** means the *narrow scope and counterexamples* were
reproduced. It **does not mean** a PMBR-invalid disk is UEFI-compliant,
bootable, update-safe or proven to brick a machine. The existing
`gpt_integrity.py` is intentionally a mirror-integrity function, not an
independent bootability/firmware-recovery oracle. A release-level disk/ISO
acceptance checker must treat PMBR, EFI loader, boot manager, NVRAM,
rollback, user data and real offline recovery as **separate obligations**.

Neither this branch nor its runner flashes firmware, writes to real disks,
modifies an installed virtual machine, loads a Qwen model, alters boot media,
retags a repository or changes release / master. No raw synthetic disk is
published; only a bounded JSON summary is archived.
