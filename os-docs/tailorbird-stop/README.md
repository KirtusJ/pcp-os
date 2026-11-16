# Tailorbird Stop and cold readback

Run this regression when changing Tailorbird's task controls, its keyboard routing, or the application store. It exercises the explicit create, edit, and run commands in the native desktop with the Qwen worker present and its weights unavailable.

## Run

Use Python 3.11 or later, GNU objcopy, and QEMU's x86 system emulator with PC BIOS, TCG, standard VGA, and legacy virtio block support. Supply a desktop preview release manifest, its matching factory disk image, and the three matching component binaries from that build. The manifest must identify the disk GUID and SHA-256 digests for the image, kernel, Tailorbird, and Qwen worker. A trusted release manifest is an input to this test.

```sh
python3 os-docs/tailorbird-stop/check.py \
  --manifest /path/to/release.json \
  --disk /path/to/PCP-OS.img \
  --kernel /path/to/kernel.elf \
  --desktop /path/to/Tailorbird \
  --worker /path/to/qwen.elf \
  --output ./tailorbird-stop-run
```

Choose a new output directory for each attempt. Use `--qemu` or `--objcopy` to select an executable and `--firmware` to supply QEMU's firmware directory when necessary. The default deadline is 180 seconds per expected serial event. `--timeout` changes that deadline. By default the runner verifies the full factory image against the manifest. Hashing the image can take additional time on a busy host.

For repeat runs, pass `--fixture-sha256 DIGEST` with the fixture digest from a trusted prior full-image check or an independent read of the expected regions. This mode verifies every copied byte and its position. The digest covers the disk size as a little-endian unsigned 64-bit integer, followed by each range's byte offset and length in the same format and its contents. The range order is primary GPT, backup GPT, application partition. The report records `whole_image_verified: false` in this mode. Kernel, desktop, and worker digests are checked in both modes.

The runner opens the factory image read-only. It copies the GPT and application partition into a new sparse disk, then boots two successive 48 MiB guests against that private disk. It converts the measured kernel to an ELF32 Multiboot container for QEMU's direct loader. The serial transcript must show execution in long mode, a mounted store, a connected desktop, and worker exit code 3. This scope excludes installation, EFI boot, display appearance, program output on screen, and inference with loaded weights.

## What a pass establishes

1. Approving creation of `lark` commits exactly `print original`. The test reads the serialized application store and verifies its header and payload checksums.
2. Stop on a replacement with a queued run receives acceptance and confirmation. The runner sends one late approval after acceptance and another after confirmation. It stages a new proposal as a synchronization point, then cancels that proposal too. There must be no intervening edit confirmation or application read. A subsequent explicit run must produce one application-read receipt.
3. A second QEMU process opens the same disk, receives a stale approval, and reads the original application after an explicit run. The generation, decoded source, and hash of the store marker and both complete slots must match the approved checkpoint across Stop and the cold boot.
4. A new replacement with a queued run receives fresh approval. The runner requires an edit confirmation, a second application read on this boot, and a higher committed generation containing exactly `print replacement`.

Each boot must mount the store, launch the exact Tailorbird binary named by the manifest, and admit the model worker. The worker’s `Exited(3)` and the MCP connection can arrive in either order after admission. The runner checks the complete serial again after QEMU exits so a later relaunch cannot supply another process’s evidence.

A normal invocation writes `result.json`, serial transcripts, and QEMU diagnostics. The runner removes the temporary binaries and sparse disk on exit. A failed receipt, digest check, or deadline produces a nonzero exit. Inspect the first failing event and the matching transcript before rerunning. A harness-directed QEMU termination at a completed checkpoint appears separately in the result from the guest's worker exit.

The keyboard test uses the QEMU monitor's `sendkey` command. See the [QEMU monitor reference](https://www.qemu.org/docs/master/system/monitor.html).

## Visible program output on the fixed desktop preview

Add `--check-screen` to check what appears in the task output panel. This option is specific to the 960×540 preview whose layout and font the pixel test describes. It waits for QEMU to present a complete P6 frame at each application-read checkpoint. The complete `OUTPUT` tag and the full 84-character message field must match on the same row. Checking the full field distinguishes the program's result from its source, a longer string with the same prefix, or text in another control.

The expected visible results are `original` after Stop, `original` after cold reopen, and `replacement` after fresh approval. The runner saves the three accepted PPM files and records each full-file SHA-256 and row position in `result.json`. If presentation times out, inspect the most recent complete frame and serial transcript. A different layout needs independently derived pixel expectations. The [initial pixel criteria](https://github.com/KirtusJ/pcp-os/pull/157#issuecomment-6914807429) identify the reference field digests.

Run `python3 -m unittest discover -s os-docs/tailorbird-stop` from the repository to check the trace, disk and framebuffer comparisons. The screen tests independently render reference glyphs from the checked-in kernel font and exercise complete, truncated, oversized, and wrong-size images.


## Refusing a replacement with Alt+D

Pass `--decision deny` to test explicit refusal on the connected 960×540 preview. The runner stages a replacement with a queued run, presses Alt+D, and waits for the tool row to display the complete message `Request denied`. It saves that frame even when `--check-screen` is omitted. This gives the refusal its own visible acknowledgement before the runner sends a late Alt+A. The first guest then stages and cancels a separate proposal to show it has consumed further keyboard input. At that point the trace must contain only the original save and zero application reads.

The remaining checks use the same disk and a second guest. They require the original source and generation after refusal and cold reopen, followed by a fresh approval that commits the replacement. Add `--check-screen` to retain the application output frames as well as the denial receipt. The default `--decision stop` exercises Alt+S and checks its serial acceptance and confirmation as before.

## Restart with a decision still pending

On a POSIX host, pass `--decision interrupt` to stop the first QEMU process with SIGKILL after the guest stages the replacement and its queued run. The first transcript must end its control sequence with the pending proposal, and the store must still match the earlier approved generation. The result records the signal request and the process exit code.

On the same disk, a fresh guest receives a late Alt+A, then stages and cancels a separate edit before the first intentional application read. An extra confirmation, an early application read or a changed store fails this check. The explicit run must still use the original saved program. A later, explicitly approved replacement must commit and run. Add `--check-screen` to verify both cold-boot outputs. This exercises loss of the emulator process while the host and its disk cache remain available. Physical power loss has a different persistence boundary. [QEMU disk cache modes](https://www.qemu.org/docs/master/system/invocation.html)
