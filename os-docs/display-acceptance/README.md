# Tailorbird display-mode acceptance

Run this check on Linux with Python 3.11 or newer, x86-64 QEMU and OVMF. Supply a raw PCP OS UEFI disk whose first GRUB menu entry launches the desktop and whose first launcher item is Tailorbird. The serial markers in the script identify this boot profile. The check uses TCG, disables networking and opens the input disk with a temporary write overlay. Each invocation requires a new output directory.

```sh
python3 check_display.py --image PCP-OS.img --image-sha256 "$DISK_SHA256" \
  --code /usr/share/OVMF/OVMF_CODE_4M.fd \
  --vars /usr/share/OVMF/OVMF_VARS_4M.fd \
  --mode 1920x1080 --output run-1920x1080
```

For the installed 1080p candidate, add `--check-title-1080p --require-qwen-ready` to the command above. The app capture then records the expected and observed SHA-256 of the title crop. A different native size leaves the observed digest empty and fails the title check. Other runs can still measure the three dimensions alone.

Use the digest from the intended release. `--require-qwen-ready` waits for the guest model-service readiness message before capturing Tailorbird. The default permits a desktop with no model loaded. To compare constrained and high-density modes, repeat with a fresh directory and a different `--mode`. `--vram` controls the emulated VGA memory in MiB. Screens requiring two 32-bit framebuffers need at least eight bytes of VRAM per pixel.

The result records QEMU’s reported device properties, the UUID, the firmware geometry emitted on serial, and the dimensions and file SHA-256 of three native PPM captures. Inspect each image to confirm the expected screen is visible. `three_samples_match_requested_mode` compares the dimensions at those three instants. A transient mode switch between captures can escape this check.

| Exit | Meaning |
| --- | --- |
| 0 | All three captured frames have the requested pixel dimensions and any requested title check passed. |
| 1 | The three captures completed and a requested dimension or title check failed. |
| 2 | Input validation or the observation failed. The error and available captures identify the last completed stage when a run directory exists. |

A dimension match can coexist with a stretched application bitmap. Inspect the glyph raster at the selected scale before making any claim about sharpness. Pointer targeting, physical monitors and a user’s preference require their own observations. [QEMU invocation](https://www.qemu.org/docs/master/system/invocation.html) · [Desktop DPI guidance](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)

## Public guest control

The [display workflow](../../.github/workflows/tailorbird-display-acceptance.yml) builds one desktop kernel from #120 and two public Tailorbird executables, the original from #134 and the candidate from #151. It boots each on the same Q35 configuration with fixed instruction-count timing, no network and a requested 1920×1080 desktop. Each run opens the app with Enter, samples the app, returns with Escape and powers off from the launcher. The expected original app size is 960×540. The candidate must present 1920×1080 and match the [precommitted title crop](https://github.com/KirtusJ/pcp-os/pull/148#issuecomment-6913956658). Both runs must restore the launcher header and selection. The workflow retains native PPMs, serial output and the result for each run. These public guests exercise a direct kernel boot with the model-free application. The installed UEFI disk and Qwen are covered by the separate command above.

The title digest uses the raw RGB bytes in rows 32 through 91, columns 44 through 403, in top-to-bottom, left-to-right order. It covers 21,600 pixels. [Netpbm PPM format](https://netpbm.sourceforge.net/doc/ppm.html)
