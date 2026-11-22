# Preloaded x86-64 EFI loader

The application at `EFI/BOOT/BOOTX64.EFI` opens the firmware volume from which it was launched. It reads `PCPOS/KERNEL.ELF`, `PCPOS/TAILOR.ELF`, `PCPOS/WORKER.ELF`, `PCPOS/model.manifest` and the numbered `PCPOS/shard-00000.bin` files. The firmware console reports verified bytes and errors. The loader opens files read-only and requires no serial input, emulator port, network or internal disk.

## Build interface

Build the normal kernel with `proposals,model-memory` and record its resolved features (model-memory enables companion). For the OVMF trials at kernel commits `b65d7c45223724aa23519e12c1505981d91634c5` and `1af35d14ab5d37aaee3963be20380fff65d53a12`, use a fresh kernel checkout and apply the repository's 32 MiB link relocation before invoking the normal build:

```sh
git apply -C0 examples/firmware-owned-uefi/load-base.patch
PCPOS_KERNEL_FEATURES=proposals,model-memory sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
```

Stage the resulting ELF32 kernel as `KERNEL.ELF`, both ELF64 guest binaries and the packer's binary manifest under their prescribed names. Build the worker with the manifest SHA-256 fixed at compilation. Use those build records to supply a UTF-8 JSON receipt of this form. Substitute the actual lowercase, 64-digit SHA-256 values:

```json
{
  "format": "pcpos-preloaded-inputs/1",
  "kernel_features": ["proposals", "companion", "model-memory"],
  "worker_manifest_sha256": "<worker build's manifest pin>",
  "files": {
    "KERNEL.ELF": "<kernel SHA-256>",
    "TAILOR.ELF": "<foreground SHA-256>",
    "WORKER.ELF": "<worker SHA-256>",
    "model.manifest": "<manifest SHA-256>"
  }
}
```

Supply the receipt digest through the independent assembly record. From the repository root, run:

```sh
python3 examples/firmware-model-loader/build.py parts/PCPOS preloaded-inputs.json \
  --receipt-sha256 "$APPROVED_RECEIPT_SHA256" parts/EFI/BOOT/BOOTX64.EFI
```

The reference implementation lives in [pcpos-efi-loader](../../PCP_OS_Rust/crates/pcpos-efi-loader). Its Rust host program binds the assembly receipt and inputs. Its freestanding Rust library calls GNU-EFI through the read-only ABI adapter in `firmware.c`. The existing assembly transition enters the Multiboot kernel.

The command needs Python 3, Cargo with the locked crates available, the Rust `x86_64-unknown-none` target, GNU cc/binutils and x86-64 GNU-EFI headers, CRT and libraries. `--gnu-efi /prefix` selects an installation outside `/usr`. The output must be a new file. The builder checks the receipt and four inputs, the kernel features and fixture banner, the worker's supplied and embedded manifest binding, and the bounded GGUF-media manifest shape. It derives load segments and the kernel's runtime measurement from the checked kernel ELF. It compiles the pins into the EFI binary. Copy the numbered shards from the pinned pack into the same PCPOS directory before mastering.

## Firmware handoff

Before opening payload files the loader reserves the linked kernel footprint and low boot/module storage and reserves the model above 4 GiB. It chooses up to eight conventional ranges below 68 GiB, as required by the kernel. A capacity refusal reports the required and representable high bytes. All allocations are tracked for release on error. Usable low pages near the kernel leave space for its low frame pool. The temporary kernel-file buffer is released after the ELF segments have been copied.

For model backing, low allocations and the kernel's free-page map, the loader requires conventional memory with the write-back capability. It reserves descriptors marked runtime, specific-purpose, nonvolatile, read-protect, write-protect cacheable, read-only or execute-protect. The final map checks the same attributes on loader-owned model pages. Firmware reports capabilities in this descriptor, so this conservative rule can reduce available capacity. [UEFI 2.11, GetMemoryMap](https://uefi.org/specs/UEFI/2.11/07_Services_Boot_Services.html#efi-boot-services-getmemorymap)

The manifest must match the embedded digest before its ordered shard digests are used. The loader checks the directory inventory and lengths, reads each shard directly into its final model pages, and hashes both each shard and the concatenation. It clears the last page's unused bytes. The worker subsequently verifies the same data from its sealed mapping. Only those reserved model pages receive memory type `0x80000001` in the final Multiboot map. The kernel starts Tailorbird automatically and returns to its desktop when the foreground ends. The handoff includes the three named modules, compatible active GOP framebuffer when available, kernel measurement, foreground/worker digests, model-memory extents and manifest pin. It refreshes the memory map and retries an invalid ExitBootServices key.

The EFI image is unsigned. Its embedded digests cover damaged or mismatched payload files when the loader itself is trusted. Replacing both the EFI image and its payloads is outside that integrity guarantee. The x86-64 to Multiboot transition must reside below 4 GiB and the kernel's linked physical range must be available. The loader reports either placement refusal before opening files. Physical firmware, Secure Boot, physical media writers and target-laptop performance require separate qualification.

## Checks

```sh
python3 examples/firmware-model-loader/check_inputs.py parts/PCPOS preloaded-inputs.json
python3 examples/firmware-model-loader/test_loader.py
python3 examples/firmware-model-loader/check_media.py prepared.fat \
  --code /usr/share/OVMF/OVMF_CODE_4M.fd --vars /usr/share/OVMF/OVMF_VARS_4M.fd --logs refusal-logs
```

The Rust policy tests exercise restricted firmware descriptors above and below 4 GiB, fragmented allocation, partial and full rollback, memory refusal before reads, short successful reads, read errors, EOF, actual bytes across a physical split, zero final-page padding, surrounding sentinels and a stale firmware exit key. The media checks use QEMU, OVMF and mtools. They keep the supplied FAT volume read-only and change private copies for the file controls. The order control requires at least two full-size shards. The Rust placement tests and the booted EFI image both use SHA-256 and the shared PCP OS manifest parser. The default `pcpos-model-media` feature retains the owned model API. The firmware builds it without heap allocation.
