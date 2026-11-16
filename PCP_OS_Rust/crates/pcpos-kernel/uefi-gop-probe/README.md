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

**Retained negative CI result:** The first hosted job [run 43804687581](https://github.com/KirtusJ/pcp-os/actions/runs/43804687581) failed **before UEFI compilation** because an integration-test invocation also attempted to link the non-Linux `efi_main` binary as a Linux executable. It produced a missing-`main` linker error. This is a source/build gating mistake, not evidence of a UEFI failure; the feature gate above is its direct repair, pending a fresh run.

## Source and safety boundaries

The minimal unsafe region is confined to firmware-entry code: table pointer reads, function-pointer conversion, and calls to firmware interfaces. The source is intentionally **not linked** to the kernel or the existing Multiboot1/Multiboot2 path. UEFI returns references and dynamically allocated buffers that must remain valid until `FreePool`; the program operates only while Boot Services are active.

The EFI ABI and GOP/EDID constraints are taken from [UEFI 2.11 EFI System Table and Boot Services](https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html), [UEFI 2.11 GOP §12.9.2](https://uefi.org/specs/UEFI/2.11/12_Protocols_Console_Support.html), and [Rust's x86_64-unknown-uefi platform guide](https://doc.rust-lang.org/rustc/platform-support/unknown-uefi.html). The UEFI spec requires the true firmware-returned QueryMode structure byte count and FreePool; PixelBltOnly does not expose a linear framebuffer. Native UEFI `efiapi` is not the usual x86-64 SysV ABI.

**Unresolved:** matching a GOP instance to discovered/active/overridden EDID, a mode-selection policy preserving refresh timing, real firmware pre/post record, a safe actual-device test, and source access to PT's private preview. Until those are available, the principal's automatic native-resolution startup requirement remains **unverified**.

**Provenance:** authored in the absorption-only Session B context. No true calibration, PCP OS boot, runtime authorization, hardware observation, merged change, or release claim.
