# Standalone GOP evidence probe (Session B, **not** PCP OS loader)

**Status: isolated source artifact for native UEFI x86-64 ABI review. It is not installed, launched, merged, or authorized as an OS boot component.**

Director PT's [coordinated firmware task](https://github.com/KirtusJ/pcp-os/issues/132) requires separating an already-active GOP mode from a mode explicitly selected by an OS-controlled `SetMode` invocation, and independently correlating EDID if claiming monitor preference. The local integrated EFI preview is not retrievable through the connected GitHub repository. This candidate provides a **standalone, opt-in experiment** without pretending to patch PT's private loader.

## Default behavior

The `pcpos-gop-probe` PE32+ application is an x86-64 `no_std`, `extern "efiapi"` program using fixed-width UEFI 2.11 tables and **no third-party dependencies**. It:

1. Checks the supplied SystemTable and BootServices signatures/table sizes.
2. Uses BootServices `LocateProtocol` to locate the **first GOP protocol instance**; reports the instance pointer as **ephemeral** evidence, not stable device identity.
3. Emits `BEFORE` GOP mode index, MaxMode, width/height, **PixelsPerScanLine**, pixel format and channel masks, full `u64` framebuffer base and extent to EFI `ConOut.OutputString`.
4. Queries up to 64 modes with `QueryMode`, prints returned status and geometry, and releases each firmware-allocated buffer with `FreePool`.
5. Emits `END READ_ONLY` and returns. It **never calls SetMode or ExitBootServices by default** and never dereferences the framebuffer.

No EDID discovery/override or active-mode association is performed. A `LocateProtocol` result alone does **not** establish that an EDID record belongs to the same video output device or that the selected mode is the monitor's preferred timing. A UEFI console print is only an application-observed report, not host-authenticated evidence of physical pixels, firmware correctness or PCP OS behavior.

## Copied-byte EDID preference decoding: evidence preparation, not mode selection

The separate pure `src/edid.rs` module now decodes a **bounded, complete
EDID 1.3 or 1.4 byte slice** supplied by a caller, without any UEFI pointer,
device access, memory allocation, or GOP mode call. The normal standalone
EFI executable is **unchanged** and does not read or use EDID. The library
also declares the native `EFI_EDID_ACTIVE_PROTOCOL` and
`EFI_EDID_DISCOVERED_PROTOCOL` GUIDs and C-layout structure for future
review; these are **not connected** to any GOP instance or dereferenced.

The parser uses the 128-byte EDID 1.x base header/checksum, advertised
extension block count, checksums of **all supplied complete blocks**, and
a conservative maximum of **16 extensions**. It does **not** decode CTA,
DisplayID or other extension timing, validate all monitor vendor semantics,
or infer that the byte source is trusted. A base-only EDID that advertises
missing extensions is **rejected as incomplete**, rather than silently
described as an entirely checked EDID.

The first 18-byte detailed timing descriptor at byte offset **54** encodes
the preferred mode. For EDID **1.3**, this profile requires byte 24 bit 1
to indicate preferred timing. For EDID **1.4**, the first DTD is required
regardless of this bit, which instead says whether the timing includes
the native pixel format/preferred refresh characteristic. Misusing the
1.3 interpretation for 1.4 would reject valid displays. The decoder
returns horizontal/vertical **active and blanking values**, pixel-clock
ticks of **10 kHz**, and the **interlaced flag** separately. It does not
promote the raw active vertical count to a progressive frame height,
conflate equal resolution with equal refresh/timing, calculate a validated
GPU refresh, or choose a GOP mode.

The `matches_dimensions_only()` helper explicitly compares **only**
horizontal and vertical active counts. It cannot establish the EDID
preferred timing was chosen: GOP `QueryMode` supplies resolution and
pixel layout but not a complete refresh/timing agreement, and EDID must
first be associated with the **same physical output(s)** and an actual
supported GOP mode. The UEFI spec says `EFI_EDID_DISCOVERED_PROTOCOL`
appears on individual video-output child handles; `EFI_EDID_ACTIVE_PROTOCOL`
reflects the selected/overridden active output(s). A `LocateProtocol`
result for the first GOP does **not** establish that an EDID protocol
on another handle belongs to that display, particularly with combined
outputs. No active/discovered/override association is claimed here.

**Tests:** `tests/edid.rs` adds **23 host-only synthetic cases** for EDID
1.3 versus 1.4 bit interpretation; a known 1920×1080 detailed-timing
encoding; checksum, missing/extra/invalid extensions, unsupported EDID
revisions, invalid/absent first DTD, interlacing, and the explicit
dimensions-only matching limit. One additional `tests/abi.rs` case
checks both UEFI EDID protocol GUIDs and the x86-64 C struct layout,
giving **13 ABI tests**. The tests do not generate firmware records or
prove native monitor selection. **Executed, source-bound result:** [GitHub Actions run 43807258854](https://github.com/KirtusJ/pcp-os/actions/runs/43807258854)
checked out exact head `5ec6b56f0f9e46ed8d9cd3c3fa1ea70eb7dafac8`
on Ubuntu 24.04 with Rust/Cargo 1.99.1. The **23 EDID synthetic
fixtures passed in debug and release** (0 failed); the **13 UEFI ABI
tests passed in debug and release** (0 failed). The same job cross-compiled
the default read-only `x86_64-unknown-uefi` PE32+ EFI_APPLICATION and
opt-in fixture **without launching either**, and confirmed two full-clean
default builds were byte-identical (8,704 bytes, SHA-256
`55b32462189ad6ac1ed9e296def3f0f5e300c6e0832f5c7c724a22eabde3d0d2`,
COFF TimeDateStamp 0). The unchanged default EFI image digest is
consistent with the new pure EDID parser remaining unused at runtime.
This receipt is host synthetic/ABI validation and EFI file compilation only,
not a firmware EDID or GOP capture.

Source distinctions: [UEFI 2.11 GOP and EDID protocols](https://uefi.org/specs/UEFI/2.11/12_Protocols_Console_Support.html),
[Linux `drm_edid.c` detailed timing conversion](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/drm_edid.c),
and [open-source `edid-decode` preferred-timing version rules](https://android.googlesource.com/platform/external/edid-decode/+/refs/heads/android15-qpr1-s5-release/parse-base-block.cpp).
The parser is deliberately narrower than a full EDID/E-EDID verifier.

## Explicit optional emulated mode-switch fixture

Compiling with `--features exercise-mode-switch` enables a deliberately **non-default** branch that selects the first distinct, QueryMode-supported index, prints an `STANDALONE_FIXTURE_SET_MODE` event, invokes GOP `SetMode`, records raw return status and post-mode metadata, then attempts to restore the original mode and reports that status. It is only for an isolated disposable emulator with an explicit testing decision; **do not run this binary on personal hardware, production boot media, or before an existing installation**. The attempt to restore could fail. A standalone EFI application performing a mode change does **not** prove PCP-OS performed it.

Even this optional branch does **not** authenticate an OS-controlled caller, EDID, actual framebuffer mapping, or the completeness of the event sequence. Reproduction in the real product requires an implementation-owned probe inside PT's pre-`ExitBootServices` path, binding GOP device handle, device path, EDID source and exact callsite/result/readback at the same revision. The app **does not** try to transition the PCP-OS kernel and cannot be wired to the running kernel by interpreting its output.

## Build and validation commands

```sh
cargo test --offline --lib --manifest-path PCP_OS_Rust/crates/pcpos-kernel/uefi-gop-probe/Cargo.toml
cargo test --offline --test abi --manifest-path PCP_OS_Rust/crates/pcpos-kernel/uefi-gop-probe/Cargo.toml
rustup target add x86_64-unknown-uefi
cargo build --offline --release --features uefi-app --target x86_64-unknown-uefi --manifest-path PCP_OS_Rust/crates/pcpos-kernel/uefi-gop-probe/Cargo.toml
# Only cross-COMPILE the optional path, do not execute:
cargo build --offline --release --features exercise-mode-switch --target x86_64-unknown-uefi --manifest-path PCP_OS_Rust/crates/pcpos-kernel/uefi-gop-probe/Cargo.toml
```

The associated read-only GitHub Actions workflow records the triggering exact source SHA, compiler and Cargo versions, tests, PE32+ header fields and SHA-256 of generated EFI executables. **Building EFI bytes is not booting them.** Nothing in this proposal launches QEMU or a real UEFI application.

**Cross-compilation feature gate:** The EFI-only binary uses `efi_main` and has no Linux `main`. It is intentionally excluded from ordinary `cargo test --test abi` using `required-features = ["uefi-app"]`. The explicitly enabled `uefi-app` build is the read-only firmware application. The `exercise-mode-switch` feature requires `uefi-app` and compiles a different optional program. Do not enable either target during ordinary Linux host tests.

**Retained negative CI result:** The first hosted job [run 43804687581](https://github.com/KirtusJ/pcp-os/actions/runs/43804687581) failed **before UEFI compilation** because an integration-test invocation also attempted to link the non-Linux `efi_main` binary as a Linux executable. It produced a missing-`main` linker error. This is a source/build gating mistake, not evidence of a UEFI failure. The feature gate above corrected it. The subsequent [exact-head GitHub Actions run 43804946236](https://github.com/KirtusJ/pcp-os/actions/runs/43804946236) at `b39beaa23a34f36387a4884cdb5b84786717dcb2` **completed successfully** on Ubuntu 24.04 / Rust 1.99.1: 12/12 host ABI and pure-value tests passed; the read-only `x86_64-unknown-uefi` PE32+ EFI_APPLICATION binary compiled and was structurally inspected; a separate explicitly opt-in mode-switch build also compiled. Neither EFI image was executed.

**Executable identities from that successful CI build (not boot receipts):**
- Read-only PE32+ EFI_APPLICATION: SHA-256 `b3212814608893e07d7d072df793caa5baf0b88be776713368bed77e7afd3369` (9216-byte binary).
- Explicit opt-in mode-switch fixture (not run): SHA-256 `8892c9417a3ce53632fdf1a523e8c8224086ccde703da9bc760428a39d32e3f0`.
- The binary identities are tied to the exact build and compiler reported above, not to PT's separate private preview or any installed disk.

## PE/COFF reproducibility audit (source-bound, not an executed EFI program)

The source compiled, but **reproducibility was not automatic**: hashes of EFI
binaries differed across earlier runs even when the Rust application source
did not change. Those differing hashes must not be substituted for one
another. An actual *full clean* same-commit test gave adverse evidence:

- [Run 43805379370](https://github.com/KirtusJ/pcp-os/actions/runs/43805379370)
  initially reported an identical rebuild after a partial `cargo clean
  --release --target`; examination showed it removed only three files and
  the rebuild completed in zero seconds. This is **insufficient evidence** of
  reproducibility. The comparison was corrected to `cargo clean
  --manifest-path ...` and verified the prior EFI binary was absent before
  rebuilding.
- [Run 43805503682](https://github.com/KirtusJ/pcp-os/actions/runs/43805503682)
  then confirmed **10 changed bytes** in two actual clean same-source PE
  builds. Their COFF TimeDateStamp values differed. The job was expected
  to fail and did.
- [Run 43805648957](https://github.com/KirtusJ/pcp-os/actions/runs/43805648957)
  using `/Brepro` still showed differences in generated PE metadata. This
  does **not** prove that all linker `/Brepro` usages are ineffective.
- [Run 43805807021](https://github.com/KirtusJ/pcp-os/actions/runs/43805807021)
  set `/timestamp:0`. It fixed both COFF timestamps to zero but **eight
  remaining bytes** changed in the PE debug-related region. An exact
  explanation of those bytes is not established by the diff alone.
- [Run 43805950794](https://github.com/KirtusJ/pcp-os/actions/runs/43805950794)
  combined `/timestamp:0` with `/DEBUG:NONE` for the EFI target. Two
  **full clean** builds on the same Ubuntu 24.04 runner, from the same
  source and Rust/Cargo 1.99.1, produced **bit-identical 8,704-byte** PE32+
  EFI_APPLICATION files. The SHA-256 of each was
  `55b32462189ad6ac1ed9e296def3f0f5e300c6e0832f5c7c724a22eabde3d0d2`;
  COFF TimeDateStamp was **0** and difference count was **0**.
  The 12 host tests and both EFI variant compilations also passed.

For a **locally equivalent** release-mode build using the same Rust target,
apply the same *target-scoped* linker options:

```sh
rustup target add x86_64-unknown-uefi
export CARGO_TARGET_X86_64_UNKNOWN_UEFI_RUSTFLAGS='-C link-arg=/timestamp:0 -C link-arg=/DEBUG:NONE'
cargo build --offline --release --features uefi-app --target x86_64-unknown-uefi --manifest-path PCP_OS_Rust/crates/pcpos-kernel/uefi-gop-probe/Cargo.toml
```

The two-build CI receipt demonstrates **only same-runner, same-compiler,
same-source bit identity** under these flags. It does not certify
cross-machine reproducibility, supply-chain authenticity, publisher
signatures, firmware compatibility, or reproducibility of PT's private
PCP-OS binary. `/DEBUG:NONE` deliberately omits PE debug/CodeView
information; it is appropriate here only for the isolated release probe,
not a blanket debugging or security recommendation. The next meaningful
verification is an independently built, source-bound comparison across
runners or toolchains, still separate from a UEFI runtime test.

## First actual standalone Q35/OVMF execution (read-only, not PCP OS)

[Exact-head GitHub Actions run 43808788133](https://github.com/KirtusJ/pcp-os/actions/runs/43808788133),
[job 133105553642](https://github.com/KirtusJ/pcp-os/actions/runs/43808788133/job/133105553642)
**completed successfully** at commit
`2e22cb376e845198173ec4aa4f4466af16127894`.
This is the first **executed UEFI probe** in Session B's own independently
inspectable work. The step ran a disposable Q35/OVMF guest using
QEMU **8.2.2**, Ubuntu 24.04 OVMF **2024.02-2ubuntu0.10**, a private copy
of OVMF variables, **no networking**, and a QEMU virtual FAT device
constructed only from this CI branch's EFI app. The firmware code image
SHA-256 was `ad23261d43116abc3747d59a7837fbf578c9586a8993f4eeba064cd07eccb083`;
the vars template was `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.

The special **`ci-debugcon`** compile-time feature mirrors UEFI ConOut
text to the QEMU-only I/O debug port 0xE9. It is **not** enabled in either
the normal default read-only binary or its normal release CI hash. It
must not be run on physical PCs. The observed emulator-only EFI binary
SHA-256 was `40b2ac5476b8e9286a48d41a7f1ea50a9f74c8c003be69f06726f11ac97f9d59`.

The **actual decoded QEMU debug-output trace** contained:
- `PCPOS_EFI_ENTRY_V1`, followed by
  `PCPOS_GOP_PROBE_V1 STANDALONE ... no_edid_association`
- An initial `BEFORE mode=0 max=30`, framebuffer physical base
  `0x80000000`, framebuffer size **4,096,000 bytes**.
- Initial mode **1280 × 800**, `PixelsPerScanLine=1280`,
  GOP pixel format **1 (BGRX)**, mode info version 0.
- Exactly **30 GOP QueryMode successes**, indices 0 through 29, including
  **1920×1080 at index 22** and **2560×1440 at index 28**.
- `PCPOS_GOP_PROBE_V1 END READ_ONLY` and
  `PCPOS_EFI_RETURN_V1 0x0000000000000000` (EFI_SUCCESS).
- The app made **no SetMode or ExitBootServices call**. The virtual
  machine continued to firmware boot management and was deliberately
  stopped after a **25-second test timeout**; QEMU timeout exit 124
  is not a probe failure because the explicit EFI completion and return
  status were observed.

This strengthens **only** the execution finding for the **separate
standalone EFI diagnostic** and actual emulated firmware GOP query.
It is **not a PCP OS kernel boot**, does not authenticate a monitor or
selected EDID, does not show PCP OS invoked a SetMode, does not verify
physical hardware and does not establish host-enforced PCP calibration.

**Retained emulator setup failures:** [run 43808168346](https://github.com/KirtusJ/pcp-os/actions/runs/43808168346)
mounted a 32 MiB partitionless FAT32 virtual disk and yielded **zero**
probe bytes by timeout. [run 43808422257](https://github.com/KirtusJ/pcp-os/actions/runs/43808422257)
added an early EFI entry marker but still yielded **zero** bytes.
Neither establishes that EFI entry executed. [run 43808598412](https://github.com/KirtusJ/pcp-os/actions/runs/43808598412)
changed to QEMU virtual FAT but failed at QEMU launch
(`Block node is read-only`) because the drive was opened writable;
corrected with `readonly=on`. Only then did the actual probe execute.
Keep these distinct from faults in UEFI pointer layout or GOP logic.

## Source and safety boundaries

The minimal unsafe region is confined to firmware-entry code: table pointer reads, function-pointer conversion, and calls to firmware interfaces. The source is intentionally **not linked** to the kernel or the existing Multiboot1/Multiboot2 path. UEFI returns references and dynamically allocated buffers that must remain valid until `FreePool`; the program operates only while Boot Services are active.

The EFI ABI and GOP/EDID constraints are taken from [UEFI 2.11 EFI System Table and Boot Services](https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html), [UEFI 2.11 GOP §12.9.2](https://uefi.org/specs/UEFI/2.11/12_Protocols_Console_Support.html), and [Rust's x86_64-unknown-uefi platform guide](https://doc.rust-lang.org/rustc/platform-support/unknown-uefi.html). The UEFI spec requires the true firmware-returned QueryMode structure byte count and FreePool; PixelBltOnly does not expose a linear framebuffer. Native UEFI `efiapi` is not the usual x86-64 SysV ABI.

**Unresolved:** matching a GOP instance to discovered/active/overridden EDID, a mode-selection policy preserving refresh timing, real firmware pre/post record, a safe actual-device test, and source access to PT's private preview. Until those are available, the principal's automatic native-resolution startup requirement remains **unverified**.

**Provenance:** authored in the absorption-only Session B context. No true calibration, PCP OS boot, runtime authorization, hardware observation, merged change, or release claim.
