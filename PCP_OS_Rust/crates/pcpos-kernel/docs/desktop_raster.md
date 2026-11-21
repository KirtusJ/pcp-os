# Native raster desktop: first-launch review draft

Commission from Kirtus Justus, relayed verbatim in PCP OS issue #132,
comment 6905856615:

> "PCP-OS must load into a Rasterized desktop background fit to the users resolution automatically"

Director PT accepted it and assigned Session A the first-launch desktop
background and keyboard-accessible launcher in comment 6905884046.
Session B owns firmware framebuffer geometry, stride, channel layout and
absence reporting for the future 64-bit entry path. PT owns integrated
checks at two modes and the fallback. Base commit is
67e57f924ce0e2c86f9250c567796b745ed633c8.

## What this draft changes

* src/desktop.rs contains a deterministic, integer-only software raster
  background. Its geometry and colours are recomputed for every pixel
  of the currently selected display mode. No fixed-size image is
  stretched or loaded. Rendering uses **one allocated row**, emitting
  exactly width pixels for each of height rows to the existing
  dev::display_row path. The framebuffer writer handles actual pitch.
* Geometry admission refuses unavailable dimensions, widths/heights
  outside a bounded 320..16384 by 200..16384 interval, stride too
  small, non-4-byte stride and a visible row extent beyond the mapped
  device memory. Failure falls back to the existing solid background.
  This does not validate the underlying framebuffer's physical address
  or pixel-channel masks; those remain the driver/loader's job.
* session.rs rasterizes the wallpaper on first launcher display and
  on redraw, then paints opaque text panels and native controls. If
  screen width, height or supplied program count crowds the existing
  detailed menu, a compact keyboard-accessible launcher keeps program
  selection, Enter, register and power-off controls reachable.
* After an ordinary guest exits, the kernel requests the mode that
  was selected when the session started, rather than forcing a
  hard-coded 1024x768. A failed restoration is logged.
* No kernel capability policy, ACR guard, Qwen/model loading, guest
  backend, media installation or networking is changed.

## Actual scope boundary

The pinned Bochs/QEMU dev::display_init implementation itself selects
1024x768 first, then falls back to 800x600 or 640x480. The loader path
accepts certain existing 32-bit-per-pixel framebuffer descriptions.
**Neither proves it chooses the monitor's native or preferred mode**.
Session B's future 64-bit framebuffer handoff must provide trusted
width, height, pitch and pixel format. Once that interface is reviewed,
the raster remains the same and consumes whatever valid geometry the
driver exposes. It does not try to probe EDID, query UEFI GOP, write
firmware mode tables or override ownership.

This is the *desktop background and launcher layer*, not a desktop
window manager. The current kernel has no compositor, mouse or
concurrent windows. No LLM is needed for this code and the ordinary
headless/auto= path remains operational when no display is admitted.

## Verification plan (not an execution receipt)

From the kernel directory, on a host with a compatible Rust toolchain:

    rustc --edition=2021 --test tests/desktop_raster.rs \
      -o /tmp/pcpos-desktop-tests
    /tmp/pcpos-desktop-tests

The Rust tests cover six admitted modes; an exactly counted raster
at 320x200 and 800x600; nonzero padded stride without copying padding
as visible pixels; wrong dimensions, stride, alignment and mapped
extent; colour variation and deterministic output. These tests were
**authored, not run** in Session A's current environment, which has no
Rust toolchain. The tests do not establish that the kernel can compile
or reach the device.

Director PT's integrated checks should capture **actual** chosen
width, height, pitch and channel layout at two different modes;
a framebuffer screenshot at each mode, whether every visible row
matches that geometry, keyboard navigation/program selection, and
return to the exact original mode after a guest. Include negative
cases of rejected stride, undersized mapping, unsupported pixel
layout, no keyboard and no display. The established headless
program-launch path should still work. Real captures, exits and
kernel register readbacks are required before promoting claims.

A raster pixel in this source is encoded as 0xRRGGBB for the existing
PCP kernel device API. This does not attest that all firmware
framebuffers use that byte layout. If GOP reports PixelBltOnly,
there is no directly writable linear framebuffer; B must supply a
different drawing path or report an unsupported mode.

## Known performance/usability risks

The wallpaper is re-rasterized for every keyboard-driven redraw.
This is deliberately simple and low-allocation but may be slow at
4K/8K in QEMU or on real hardware. Instrument wall time and
painted-pixel counts before accepting interactive performance;
cache or damage-region rendering may be needed. The text font has
fixed cell dimensions and is not a high-DPI or screen-reader
solution. Keyboard access here is a bounded basic interaction,
not WCAG conformance or fully accessible desktop UI. Input
method, pointer, display switching, multi-monitor and protected
background asset storage remain future work.

## External primary references

- UEFI Specification 2.11, Graphics Output Protocol §12.9:
  HorizontalResolution, VerticalResolution, PixelFormat,
  PixelsPerScanLine, padded stride and PixelBltOnly semantics.
  https://uefi.org/specs/UEFI/2.11/12_Protocols_Console_Support.html
- QEMU Standard VGA hardware documentation, Bochs interface:
  https://github.com/qemu/qemu/blob/master/docs/specs/standard-vga.rst
- W3C WCAG 2.2, keyboard access, resize/reflow and status guidance:
  https://www.w3.org/TR/WCAG22/

These standards inform design and test selection; no compliance
certification, host-enforced ACR calibration, hardware boot, QEMU
launch or daily-driver readiness is claimed by this draft.
