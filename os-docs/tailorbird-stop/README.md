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

Choose a new output directory for each attempt. Use `--qemu` or `--objcopy` to select an executable and `--firmware` to supply QEMU's firmware directory when necessary. The default deadline is 180 seconds per expected serial event. `--timeout` changes that deadline. Hashing the full factory image can take additional time on a busy host.

The runner opens the factory image read-only. It copies the GPT and application partition into a new sparse disk, then boots two successive 48 MiB guests against that private disk. It converts the measured kernel to an ELF32 Multiboot container for QEMU's direct loader. The serial transcript must show execution in long mode, a mounted store, a connected desktop, and worker exit code 3. This scope excludes installation, EFI boot, display appearance, program output on screen, and inference with loaded weights.

## What a pass establishes

1. Approving creation of `lark` commits exactly `print original`. The test reads the serialized application store and verifies its header and payload checksums.
2. Stop on a replacement with a queued run receives acceptance and confirmation. The runner sends one late approval after acceptance and another after confirmation. It stages a new proposal as a synchronization point, then cancels that proposal too. There must be no intervening edit confirmation or application read. A subsequent explicit run must produce one application-read receipt.
3. A second QEMU process opens the same disk, receives a stale approval, and reads the original application after an explicit run. The generation, decoded source, and hash of the store marker and both complete slots must match the approved checkpoint across Stop and the cold boot.
4. A new replacement with a queued run receives fresh approval. The runner requires an edit confirmation, a second application read on this boot, and a higher committed generation containing exactly `print replacement`.

A normal invocation writes `result.json`, serial transcripts, and QEMU diagnostics. The runner removes the temporary binaries and sparse disk on exit. A failed receipt, digest check, or deadline produces a nonzero exit. Inspect the first failing event and the matching transcript before rerunning. A harness-directed QEMU termination at a completed checkpoint appears separately in the result from the guest's worker exit.

The keyboard test uses the QEMU monitor's `sendkey` command. See the [QEMU monitor reference](https://www.qemu.org/docs/master/system/monitor.html).
