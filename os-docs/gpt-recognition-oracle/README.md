# GPT recognition is not two-copy GPT acceptance

Session B isolated **external QA** probe, 16 November 2026, B.C. Pacific Time
(PCT, UTC−07:00). This is not part of the PCP-OS Rust reference host,
kernel, firmware, installer, authorization broker, or cryptography.

Run:

    python3 os-docs/gpt-recognition-oracle/probe.py

The script creates **only** 8 synthetic 2 MiB **regular files** inside a
temporary directory (automatically destroyed), writes a specified GPT header,
entry array and protective MBR into each, and invokes the installed
util-linux **blkid -p -o export** on each temporary file. It has no CLI
parameters or path input, never opens /dev, and never mounts, partitions,
formats, enumerates or writes a device. No QEMU guest, model or PCP kernel
is invoked. The independent recognizer is external C/libblkid, not the pure
Python metadata model in [separate draft PR #145](https://github.com/KirtusJ/pcp-os/pull/145).

The input producer uses Python struct packing and zlib CRC32, with fixed
fields, a representable end-CHS for its 255-head/63-sector **synthetic**
geometry, 512-byte logical blocks and a 16 KiB primary/backup entry array.
It is not an independently certified GPT creator. This means the comparison
provides cross-tool evidence about recognition decisions on *these exact
bytes*, not certification of full UEFI compliance or of the private product.

The script asserts the intended positive and negative **fixture mutations**
(both headers' CRC status, disk GUID agreement, entry-array agreement, and
backup alternate-LBA agreement). It deliberately does **not** assert exact
blkid labels in CI because different util-linux releases may recognize
different damaged or ambiguous media. Every run prints its actual tool
version, each case's result and full image SHA-256. Do not silently convert
a recognition label, a status code or a matched digest into an installable
device, a valid GPT pair, a user approval or a durable write.

## Why this probe matters

A GPT header can carry a correct CRC32 while two independently checksummed
copies disagree on Disk GUID, partition-entry bytes or reciprocal endpoints.
A format recognizer may report PTTYPE=gpt even for these contradictory copies,
especially when the primary GPT remains readable. This is not evidence the
recognizer claims strict standards conformance. GPT must be separately checked
against [UEFI 2.11 §5](https://uefi.org/specs/UEFI/2.11/05_GUID_Partition_Table_Format.html),
including both headers and entry arrays; for destructive installer effects,
apply destination authorization and recovery checks as a **separate** gate.

Initial Session B local host readback from util-linux/libblkid **2.41.5**
showed PTTYPE=gpt for: valid first/fourth protective record, different Disk
GUIDs with both header CRCs valid, different partition arrays with both CRCs
valid, an invalid backup alternate-LBA pointer with CRC repaired, hybrid MBR
with intact GPT metadata, and a damaged primary header with valid backup.
When both header CRCs were damaged it reported PTTYPE=PMBR. These observations
apply only to the locally generated inputs and named version. An independent
CI run is required to report its own version and case results; do not assume
the CI host reproduces every local classification.

## Maintainer/review boundary

This is a **draft, external, source-only diagnostic**. It is intentionally
a **separate branch/PR**, so it does not change the exact source at already
reviewed [#145](https://github.com/KirtusJ/pcp-os/pull/145). The repository's
[Rust primacy policy](../../RUST_PRIMACY.md) remains governing: any real OS
security control or GPT acceptance rule must be implemented and tested in
the trusted Rust or other assigned production boundary, not in this Python
diagnostic. No claims of PCP true calibration, host-enforced authorization,
physical-disk identity, private installer execution, kernel boot or release
are made.
