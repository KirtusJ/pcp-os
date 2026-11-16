# Proposed native x86-64 handoff contract (Session B)

**Status: review-only Rust source, not a UEFI loader or a boot path.**

This is a self-contained `no_std`, no-dependency Rust crate proposed for the
firmware-to-kernel boundary, under PCP-OS [coordination #132](https://github.com/KirtusJ/pcp-os/issues/132).
It is additive: it changes neither the currently working Multiboot 1 assembly
trampoline nor the kernel's linker, `run.sh`, runtime, register or launch policy.

The common starting revision is `67e57f924ce0e2c86f9250c567796b745ed633c8`.
The design work in [PR #131](https://github.com/KirtusJ/pcp-os/pull/131)
remains separate and should be reconciled before an actual loader is wired.
Do not merge this as a completed boot feature.

## What this candidate actually offers

- Versioned, fixed-width, C-layout x86-64 **handoff values** with `u64`
  physical locations and sizes for the kernel, module images and names,
  memory-map table and command-line buffer. Raw untrusted discriminants are
  integers, not Rust enum instances.
- A pure validation function with no `unsafe`, pointer dereference, I/O,
  firmware services, global state, floating-point code or heap requirements.
  The caller supplies owned, bounded region/module **slices**. It is the
  caller's responsibility to demonstrate that the slices were copied from
  the declared physical locations. This library **cannot** do that.
- Framebuffer metadata with explicit `Absent`, `BltOnly` and `Linear`
  outcomes; width, height, **stride in bytes**, raw UEFI GOP format and
  channel masks. `Linear` means geometry checked, **not mapped or drawn**.
  Format `PixelBlueGreenRedReserved8BitPerColor` is 1 and matches the current
  native kernel's blue-low-byte rendering assumption; format 0 has red in
  byte zero and must NOT be misinterpreted as format 1. Format 2 carries
  8-bit nonoverlapping channel masks only in this first candidate.
- Range/overflow checks, descriptor ordering, a CPU-provided physical
  address limit, reserved-field rejection and basic non-overlap guards
  covering the kernel, handoff metadata, framebuffer, and module payloads.
  The header permits addresses above **4 GiB** without truncating them.

**No part is linked into the current kernel or used to enforce permissions.**

## Run these checks only in an execution environment with Rust installed

```sh
cargo test --manifest-path PCP_OS_Rust/crates/pcpos-kernel/handoff/Cargo.toml
cargo test --manifest-path PCP_OS_Rust/crates/pcpos-kernel/handoff/Cargo.toml --release
cargo check --manifest-path PCP_OS_Rust/crates/pcpos-kernel/handoff/Cargo.toml --target x86_64-unknown-none
```

The third command requires the bare-metal target installed; it checks
compilation, not UEFI startup or compatibility with the existing kernel.
Additional required integration tests: compile under
`x86_64-unknown-uefi` when a loader actually uses the crate; fixture tests
with firmware-reported modes at two resolutions, padded scanlines,
`PixelBltOnly`, invalid/overlapping masks, >4 GiB physical addresses,
malformed memory descriptors, and a fully headless path. Failures must be
retained, not silently normalized.

At authorship, **no Rust compiler or QEMU** is installed in this authoring
container. The 42 Rust tests under `tests/validation.rs` are authored
**but not executed or compiler-validated** by Session B. Director PT owns
executed real-image and OVMF/hardware receipts. A textual test suite is
not evidence of passing behavior.

## Advisory GOP mode-change witness classifier (source-only)

This crate also contains \`src/gop_witness.rs\`, which is a **separate, pure
function** accepting a recorder's report of *one* GOP handle before and after
a proposed \`SetMode\` call. It does not decode EDID, obtain any firmware pointer,
perform \`SetMode\`, invoke QEMU, map framebuffer bytes, or authenticate a
recorder. Its enumerated outcomes describe the **internal consistency of the
supplied report**, not physical occurrence or authority.

The smallest discriminating witness for **OS-owned mode selection** must
originate in instrumented OS-controlled code **before ExitBootServices**:
(1) a GOP handle/device-path identity and \`Mode.Mode=B\` observation,
(2) a sequenced call to that same handle's \`GOP.SetMode(A)\` with \`A != B\`,
the claimed initiator, and the actual return \`EFI_STATUS\`, and
(3) a sequenced \`Mode.Mode=A\` readback with reported width, height, pitch
(\`PixelsPerScanLine\`), pixel format and framebuffer base/size.
The recorder must demonstrate that it is exhaustive for this one GOP handle
over the interval, rather than merely declaring it. This last requirement
**cannot** be checked by the pure function. If no call was captured, a
matching initial/final resolution is at most *consistent with the already
active mode*, not an OS-selected mode.

The typed helper also refuses mixed-device handles, wrong stage/order,
unavailable mode indices, out-of-range geometry and native-driver calls
mistaken for UEFI Boot Services calls. Failed \`EFI_STATUS\` or contradictory
readback never yields \`ConsistentWithOsModeChange\`. Unknown actor or
incomplete capture stays unresolved, not silently promoted to PCP-OS credit.
A valid \`PixelBltOnly\` mode may be selectable but supplies no direct
writable framebuffer; the helper never authorizes drawing into it.

**EDID preference needs further independently checked evidence:** bytes,
checksum, active/discovered/override source, matching GOP device path,
preferred timing and a versioned selection rule mapping that timing to one
supported GOP mode. Matching width and height **alone** cannot prove the
preferred refresh/timing or that EDID caused a mode-selection decision.
No EDID preference conclusion is returned by this helper.

The \`tests/gop_witness.rs\` file adds **28 synthetic controls**, including
already-correct initial mode, firmware/GRUB/unknown actors, false success,
failed calls, late post-ExitBootServices readbacks, wrong handle, stale
sequences, padded pitch, and a BLT-only selection without framebuffer
permission. These are **not** real firmware events or kernel execution.
Test with the same \`cargo test --manifest-path\` commands above.
The existing handoff wire ABI and 42 original validation cases are unchanged.
This section does **not** mean PT's locally reported EFI preview has used
this witness or that a trusted recorder exists yet.


## Compatibility with PT's reported GRUB Multiboot2 EFI amd64 preview

Director PT [reports an OVMF/GRUB EFI amd64 Multiboot2 preview](https://github.com/KirtusJ/pcp-os/issues/132#issuecomment-6906026073). That is a **producer-reported emulator observation**, not a boot run or source-tree verification in this Session B. This contract is **not yet an adapter** for the preview's Multiboot2 information structure.

A crucial condition from the [GNU Multiboot2 Specification v2.0, §§3.1.8, 3.5 and 3.6.12](https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html):

- The **EFI amd64 entry** uses Multiboot2 header tag 9 together with the EFI boot-services tag 7. The entry receives the Multiboot2 magic and information-structure address under the specified machine state. The loader **must not load the kernel, modules or Multiboot2 information structure above 4 GiB - 1**. The proposed handoff contract's accepted >4-GiB **kernel/module** addresses describe **future loaders with a different placement contract**, not what this GRUB path currently certifies.
- In contrast, Multiboot2's **framebuffer information tag 8** contains a **64-bit** physical framebuffer address, separate pitch in bytes, width, height, bits per pixel and color-channel metadata. The bootloader is asked to place the framebuffer below 4 GiB if possible, but the address field remains 64-bit. Do not truncate it, or assume its pixel layout equals GOP's direct enum without decoding it.
- The EFI memory map can use variable descriptor sizes and may be absent in some boot-service-enabled combinations. A tag being present, a receipt claiming ExitBootServices, and a verified live memory map after that exit are different facts.

For integration, make a **separate, source-pinned Multiboot2 decoding adapter** with bounded 8-byte-aligned tag scanning, exactly one terminating tag, 64-bit framebuffer values, validated byte lengths, explicit color-channel conversion and proper boot-services/memory-map provenance. Do not cast arbitrary Multiboot2 bytes to `Handoff` or set `physical_limit` from untrusted input. Preserve negative fixtures for truncated tags, malformed entry states, overflow, channel aliasing, absent framebuffer, and a high framebuffer with low kernel/modules. This repo pin's Multiboot1 path is unchanged.

## Boundaries before using any handoff in a running OS

1. **Build a real `x86_64-unknown-uefi` loader.** UEFI apps use PE32+ and
   the `efiapi` calling convention, not the existing 32-bit Multiboot
   entry. Locate and verify the ELF64 kernel and material without trusting
   caller-supplied hashes. Reserve physical frames and track image identity.
   A converted ELF32 wrapper is not an EFI application.
2. **Normalize the firmware memory map.** UEFI's raw descriptor size may
   exceed a compiled structure size; obtain the descriptor-size value
   from firmware, validate every byte and map key, sort/copy the descriptors
   into the fixed v1 `MemoryRegion` array, and call
   `ExitBootServices` under its required retry protocol. A successful
   pure Rust value check cannot establish any of these firmware acts.
3. **Bind physical address lifetimes.** Every table, module name/image,
   command-line byte, framebuffer range and kernel page must be owned
   until the receiving kernel establishes its own mapping. Ensure the
   normalized descriptor array accurately describes the physical pages.
   The validation API does not prove reserved memory or descriptor origin.
4. **Explicitly switch to the kernel's new 64-bit entry** through a
   carefully reviewed ABI preserving the full pointer and all registers
   it requires. The pinned existing `arch::boot_info` consumes
   Multiboot 1 `u32` entry arguments and cannot read this record.
   Neither call into the old entry nor reinterpret UEFI pointers as
   Multiboot structs.
5. **Check and map the framebuffer.** GOP `PixelsPerScanLine` can exceed
   horizontal resolution, and a mode with `PixelBltOnly` supplies no
   directly writable framebuffer. The current `dev.rs` assumes BGRX
   and only maps a loader framebuffer with a **below-4-GiB** address,
   2-MiB alignment and 32-bit pixels. A valid handoff is not a valid
   kernel mapping. Mismatched pixel layout requires conversion or an
   explicit headless/non-graphical fallback; this first validation reports `UnsupportedPixelLayout` for otherwise
   well-formed 16-bit bitmask layouts, rather than calling the firmware mode invalid.
   A future loader/controller must handle this as a visible headless fallback,
   not silently stop the operating system.
6. **Keep startup independent of Qwen.** Kernel, storage, keyboard and
   desktop must not require a model. PT owns model data, service startup
   and effect authorization; Session A owns the rasterized desktop/UI.
   The separate proposed `PCPOS_DATA/models/qwen` media location is not
   an installed or mounted volume.
7. **Install and recovery are separate.** This crate does not build
   FAT media, sign any image, partition a disk, recover a register,
   stage a kernel slot, preserve installed state or arbitrate model
   updates. Existing unmerged IDE checkpoint [PR #119](https://github.com/KirtusJ/pcp-os/pull/119)
   and desktop [PR #120](https://github.com/KirtusJ/pcp-os/pull/120)
   should be considered without treating either as a complete installer.
   Retain the current Multiboot image as an independent test/recovery path.

## External, primary technical references

- [UEFI Specification 2.11, Graphics Output Protocol §12.9](https://uefi.org/specs/UEFI/2.11/12_Protocols_Console_Support.html):
  pixel formats, `PixelsPerScanLine`, `FrameBufferBase`, `FrameBufferSize`,
  and `PixelBltOnly`.
- [Rust platform support, `x86_64-unknown-uefi`](https://doc.rust-lang.org/rustc/platform-support/unknown-uefi.html):
  native PE32+, `efiapi`, linking, firmware execution restrictions.
- [UEFI 2.11 Boot Services](https://uefi.org/specs/UEFI/2.11/07_Services_Boot_Services.html):
  memory-map and `ExitBootServices` lifecycle.

**Provenance:** independent source inspection and code authorship; no kernel
execution, UEFI loader build, firmware call, hardware boot, or host-enforced
ACR calibration. This work is entirely in the PCP-OS product repository,
not the separate physics research repository.
