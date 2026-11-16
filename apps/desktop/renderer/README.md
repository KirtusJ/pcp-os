# PCP OS native raster desktop: first-launch draft

**Principal requirement (verbatim):** “PCP-OS must load into a Rasterized desktop background fit to the users resolution automatically”

Source basis: `master` at `67e57f924ce0e2c86f9250c567796b745ed633c8`, requested by Kirtus Justus in [coordination issue #132](https://github.com/KirtusJ/pcp-os/issues/132#issuecomment-6905856615). Director PT specifically assigned the *first-launch background and keyboard-accessible launcher* to Session A in [this comment](https://github.com/KirtusJ/pcp-os/issues/132#issuecomment-6905884046). Session B owns the 64-bit firmware handoff with measured video mode/pixel format/pitch; PT owns integrated boot/QEMU evidence. This is a separate draft from Tailorbird [PR #134](https://github.com/KirtusJ/pcp-os/pull/134).

## What this draft actually changes

- Adds a pure, `no_std` Rust rasterizer under `apps/desktop/renderer/`, with bounded geometry, per-pixel gradient and normalized accent. The logical output is exactly `width × height`; each row contains `width` 32-bit RGB pixels. It never assumes a 1024 × 768 bitmap, fixed 16:9 ratio or zero row padding.
- Connects the rasterizer to the existing **kernel-owned first-launch menu** through its safe `dev::display_row` abstraction. The display driver determines the actual width, height and stride. If the reported geometry is invalid at startup, the graphical menu is not entered and the kernel retains its serial/no-display fallback. A flat background is used only when a still-valid framebuffer cannot be rasterized.
- Replaces rigid desktop menu placement with a responsive panel and an explicit selected-item highlight. Up/Down, Enter, F (full supplied-file identities), R (kernel register), P / Esc and kernel Ctrl+Alt+Delete remain keyboard reachable. Long supplied-program lists scroll within the visible viewport. No LLM is needed to draw or use the launcher.
- Restores the original startup display mode after a guest exits, rather than forcing the prior fixed 1024 × 768 constant.

**Important limitation:** This is *rasterization at the selected/reported display mode*, not proof the kernel can automatically **discover the user's monitor's native resolution**. At the pinned source the Bochs driver deliberately tries 1024 × 768, then 800 × 600, then 640 × 480; a loader-set 32-bit framebuffer can use other modes. Automatic firmware/GOP/EDID selection, reported pixel-format validation, firmware display stride, headless operation, multi-display behavior, hot-plug and real persistence remain unresolved. B and PT must validate the handoff. This is the first-launch kernel menu, **not a full window manager or composited desktop**, and neither Qwen nor Tailorbird is launched.

## Test plan: source and execution are different evidence

**Host Rust library tests (not executed by Session A authoring runtime):**

```sh
cargo test --manifest-path apps/desktop/renderer/Cargo.toml --lib
```

These Rust tests cover row-sized output at 640×480, 1024×768, 1366×768 with a padded stride, 1920×1080 and 7680×4320; rejection of zero or oversized dimensions, malformed stride, too-small mapped memory, wrong row length and out-of-bounds Y; a minimum usable 40×12-character grid; selected-item visibility in a long list. These are authored assertions, **not** passed tests or display readbacks until someone executes them.

**Director PT's compiled kernel and display checks:**

1. Build against the exact draft head. Save `rustc --version`, target, command, stderr/stdout, exit status and kernel/loader image hashes. Execute the existing kernel self-tests and negative controls without changing their test expectations.
2. Present **at least two modes**, e.g. 800×600 and 1024×768 on Bochs, then preferably a loader-supplied mode with a nontrivial pitch such as 1366×768 and 1408 pixels per scanline. Capture the driver-reported width, height, stride, pixel layout, physical buffer length, displayed background and selected-item visibility. Do not substitute a command-line QEMU window size for the guest's actual mode.
3. Confirm every logical row has exactly `width` RGB pixels and that the driver uses `pitch_bytes` to advance framebuffer rows. Test a guarded/padded scanline independently; no writes to row padding or unmapped memory. Test rejection of malformed or unsupported geometry and a display-absent boot; retain the serial log and no-hang evidence.
4. Supply 0, 1, and more than one page of executable loader modules; navigate with Up/Down, Enter, F, R and Esc; confirm a visible selection and that SAFE mode blocks program start. Confirm existing digest/program reporting survives the new layout.
5. Run and stop one guest that changes its display mode, then verify the desktop redraws at the **initial selected dimensions**, not 1024×768. Observe Ctrl+Alt+Delete kernel stop independently from any Tailorbird button.
6. Remove any optional Qwen model and Tailorbird binary. Confirm the background and launcher still appear, accept keyboard input, and can start a non-LLM guest. Record a genuine failure if the loader, keyboard or framebuffer is missing.

**No boot, QEMU, kernel compile, hardware test, monitor-native autodetection, security authorization or true PCP calibration has occurred in this absorption session.** The local authoring environment lacks Rust and QEMU. The branch is for Director PT's review and verified execution, not for unapproved merge/deployment.

## Provenance and design boundaries

- [UEFI 2.11 Graphics Output Protocol](https://uefi.org/specs/UEFI/2.11/12_Protocols_Console_Support.html) separates horizontal/vertical pixel dimensions, PixelsPerScanLine and pixel format; not every graphics mode provides an addressable RGB framebuffer. The kernel's current Multiboot driver does not yet establish the full GOP contract.
- [W3C WCAG 2.2](https://www.w3.org/TR/WCAG22/) motivates the keyboard-operable foreground with explicit focus. This draft does not claim accessibility compliance.
- `PCP_OS_Rust/crates/pcpos-kernel/src/dev.rs` remains the privileged framebuffer writer. The new pure renderer cannot set modes, touch raw memory, authorize tasks or run model-generated commands.

**Acceptance gate:** PT and B must independently verify detected/selected mode provenance, real framebuffer geometry, rendered pixels, negative fallback, and keyboard navigation at multiple live modes. Source code or authored test names alone do not satisfy the principal's whole requirement.
