# Director 13 | Anti-bricking acceptance boundary (synthetic only)

**Date:** 16 November 2026, B.C. PCT (UTC−07:00).  
**Scope:** A test-only update-selector discriminator, not the PCP OS installed bootloader, GPT, UEFI firmware or a real update installer.  
**Source:** [PCP OS master 59f26ebe](https://github.com/KirtusJ/pcp-os/tree/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c).  
**Parent:** [Hardware and anti-bricking issue #250](https://github.com/KirtusJ/pcp-os/issues/250).

The daily-driver goal of not bricking an installation spans different risks: platform firmware/ROM; bootloader, EFI system partition and recovery image; persistent OS/application data; and peripheral firmware (USB, Wi-Fi, Bluetooth). The test here concerns **only a hypothetical software boot selector**. It must never be promoted as proof of hardware resilience.

## Previously observed source boundary

[Installed GPT integrity verifier](https://github.com/KirtusJ/pcp-os/blob/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c/os-docs/installed-acceptance/gpt_integrity.py) validates primary and backup GPT headers, CRCs, consistency, partition bounds, GUID identities and overlap. Its [seven mirror tests](https://github.com/KirtusJ/pcp-os/blob/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c/os-docs/installed-acceptance/test_gpt_integrity.py) reject corrupted and inconsistent metadata, and [entry semantics tests](https://github.com/KirtusJ/pcp-os/blob/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c/os-docs/installed-acceptance/test_gpt_semantics.py) reject partition overlaps and invalid identifiers. **Detection is not repair.** Failure of a strict verifier alone neither proves a machine cannot boot nor demonstrates it can recover.

The [installed QEMU acceptance](https://github.com/KirtusJ/pcp-os/blob/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c/os-docs/installed-acceptance/check.py) uses an OVMF variables **copy** and disk **snapshot**, verifying factory-image identity after the run. [Tailorbird stop/cold reboot](https://github.com/KirtusJ/pcp-os/blob/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c/os-docs/tailorbird-stop/README.md) uses a private sparse disk across QEMU processes and explicitly distinguishes emulator termination while the Linux host cache remains available from physical loss of power. Neither test validates a firmware capsule or physical recovery.

## What the new synthetic oracle establishes

The [selector_model.py](selector_model.py) invents two **unbootable byte-string images** and two separate **128-byte** selector copies. Each selector has a CRC32, two expected SHA-256 digests, a write-order sequence, a selected slot and a separate known-good slot. A valid candidate is offered first, followed by the independently verified known-good image.

All **129 byte-prefix cuts** of the first write, and **129 cuts** of the second, are checked. A cut must never erase the independently verified previous known-good choice. No assumption of atomic sector writes is imported. Each copy may be old, new or malformed. The tests also cover corrupt candidate, corrupt known-good, both selectors damaged, equal-generation conflicting copies, rollback against an **externally trusted sequence floor**, and a malicious selector forged with a recomputed CRC.

The forged-CRC case is a **negative witness**: self-consistent metadata with valid CRC and image digest is accepted by this deliberately unauthenticated design. This proves its **limitations**, not any integrity of an installed release. A counter/CRC alone is not a cryptographic signature; the model has no trusted public-key root and no independent publisher verification. The model sequence tracks writes, not a version or security anti-rollback policy. A corrupt known-good image causes **refusal**, not a fake safe-boot claim. Real bootability is **NOT TESTED**.

## Required follow-on engineering and audit

An actually trustworthy A/B installer needs independently authenticated release and image bytes, durable active and rollback slot identity, bounded attempts, truthful boot-health confirmation, and separately secured firmware/EFI boot state. Carefully distinguish **real power loss** from process termination, host SIGKILL or disk snapshots. Test actual lost power only on expendable hardware with independent restoration tools, before using a primary daily-driver device.

The update must consider power-cut windows during writes of the payload, metadata, GPT, EFI boot manager, NVRAM state and final confirmation, and show a separately bootable older image under a defined recovery policy. In particular, an externally trusted anti-rollback floor may require **explicit authorized recovery to an older known-good image**, not blind refusal of all older versions. User data and persistent application writes require their own transactions and rollback criteria, and firmware recovery needs OEM/hardware support.

For dongles, firmware qualification is hardware/vendor/class specific. A USB dongle removed during an update must not leave the OS falsely reporting the radio as connected; a physically damaged dongle is not the same as a bricked host motherboard. Neither USB firmware nor platform firmware is written by this test.

## Independent external reference points

- [NIST SP 800-193](https://csrc.nist.gov/pubs/sp/800/193/final) separates platform-firmware **protection, detection and recovery**.
- [UEFI Specification 2.11](https://uefi.org/specifications) defines the relevant EFI boot/variable/capsule interface at the target platform; this invented selector claims no UEFI conformance.
- [The Update Framework specification](https://theupdateframework.github.io/specification/latest/) gives external examples of authenticated root/targets/snapshot/timestamp roles, rollback/freeze/mix-and-match defenses and trusted persistent metadata. It is *not implemented here*.
- [NIST SP 800-218 SSDF](https://csrc.nist.gov/pubs/sp/800/218/final) and [Git release governance issue #251](https://github.com/KirtusJ/pcp-os/issues/251) inform externally authenticated build and media supply-chain acceptance, separate from the operating system's update protocol.

## Run model-only tests

From the repo root, using only the Python standard library:

~~~sh
python3 -m unittest discover -s audit/director-13/anti-brick -p 'test_*.py' -v
python3 audit/director-13/anti-brick/selector_model.py --output d13-anti-brick/selector.json
~~~

The isolated CI workflow archives the JSON of exactly what the model proved. **A green suite is a green synthetic model only.** It does not permit firmware updates, installation, tags, master promotion, an ISO release or claiming a device cannot brick. D7 retains the application store, D12 the hosted worker/restart context, and the bootloader/firmware/release owners retain real-hardware and installed image qualification.
