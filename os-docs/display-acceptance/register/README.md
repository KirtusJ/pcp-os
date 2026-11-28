# Register head: independent native-frame acceptance

Director 2's **read-only** verifier for a PCP OS kernel register screen, alongside the [existing Tailorbird display acceptance](../README.md). This checker does not boot a guest, derive or verify a ledger head, authorize a write, or certify continuous service. An actual native QEMU P6 `screendump` and a separately authenticated head **from the same boot** are mandatory for release acceptance. Synthetic host-backed frames are only test controls.

The current [merged register UI in #226](https://github.com/KirtusJ/pcp-os/pull/226) puts the heading and all 64 hexadecimal digits on one 117-cell line. It clips at 800×600 and 1024×768. Until the display owner corrects that layout and a native frame exists, **installed/guest register-head acceptance remains OPEN**.

After that correction, for a candidate which paints `HEAD ` followed by all 64 lowercase hexadecimal digits at grid column 2 and row 2 in kernel TEAL on GROUND, run:

```sh
python3 os-docs/display-acceptance/register/check_register.py \
  --image /path/to/native-register.ppm \
  --head "$(cat /path/to/verified-head.txt)"
python3 -m unittest discover -s os-docs/display-acceptance/register -p 'test_*.py'
```

`--font` selects the exact kernel font for the measured image. `--prefix`, `--col`, `--row`, `--fg` and `--bg` allow the implementation owner to specify a different *documented* drawing contract. Do not change these just to make a failing capture pass. The default full row occupies x=20..709, y=40..59 and fits 800×600. Every RGB pixel of the expected 69-cell text, including the last hex character, is compared to the native lossless P6 frame. The checker exits 0 only on a complete match, 1 for mismatched pixels and 2 for a malformed request, unsupported geometry or unrepresentable/clipped label. The JSON result records frame geometry, coordinates, mismatch count and the first mismatching location. It must be paired with independent ledger integrity verification and real guest navigation: correct visible verified/failed status, page Up/Down restoration, and Esc return.

The seven source-only tests include matching 800×600, 1024×768 and 1920×1080 synthetic frames; changed-tail negative controls; last-cell pixel corruption; malformed hashes/PPM; and rejection of the old 117-cell layout at small widths. None of those cases counts as a guest run. To avoid accidental claims based solely on an uploaded still, retain the exact native guest serial, framebuffer size and stride, kernel image/ELF SHA-256, app/ledger pins, the complete QEMU invocation, the unedited P6 file and its SHA-256, and the output from this checker.
