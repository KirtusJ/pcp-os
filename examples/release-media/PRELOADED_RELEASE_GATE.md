# Preloaded candidate media: exact-inventory gate

This is a **candidate assembly** entry point, not permission to distribute an image. It implements the repeatable path/byte gate requested in the [22 November release provenance review](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7079446982), separately from the model-free recipe.

Before running it, an independent reviewer must approve (1) the complete raw-byte SHA-256 of an exact expected-files.json inventory and (2) the publisher's whole-model digest. The inventory maps each path to **only** its bytes and sha256 fields. It must contain EFI/BOOT/BOOTX64.EFI, PCPOS/KERNEL.ELF, PCPOS/TAILOR.ELF, PCPOS/WORKER.ELF, PCPOS/model.manifest and contiguous PCPOS/shard-00000.bin onward. Do not silently generate the approved inventory pin from the same unchecked staging input.

Place only those approved files under payload-tree/EFI/BOOT/ and payload-tree/PCPOS/. The source tree may be a reused staging directory: this checker still rejects **every missing, unlisted, modified, symlink or special file and unexpected directory**, compares size and digest of every file, validates manifest shape and per-shard hashes, reconstructs the model, and checks its complete digest. It never trusts an existing FAT volume. It freshly formats a private, new FAT32 volume and reopens its file tree; it then uses the existing hybrid builder and independently checks every Joliet file against the same approved inventory **before publishing** the result. Existing output paths are refused, never replaced.

From the repository root, after approving the independent pins:

    python3 examples/release-media/preloaded_gate.py payload-tree \
      --inventory expected-files.json \
      --inventory-sha256 "$APPROVED_INVENTORY_SHA256" \
      --model-sha256 "$APPROVED_PUBLISHER_MODEL_SHA256" \
      --fat-bytes 5253365760 candidate.iso

The 5,253,365,760-byte FAT size is the previously measured **Qwen3-8B candidate** size, not a default for all models. Python 3.12+, mkfs.fat, mmd, mcopy, and xorriso must be available. Both expected-files.json and candidate.iso are regular files, not devices. The command never writes to a block device. Run python3 examples/release-media/test_preloaded_gate.py for offline negative controls without build tools or model weights.

The measured 8B candidate with ISO SHA-256 a0298480bf5e8773206369fda4b0e6a00d4ce31e8c83367db71c9541a9fa9f88 had an exact 80-path inventory, 75 shards and model SHA-256 d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785. [Immutable inventory](https://github.com/KirtusJ/pcp-os/blob/56345939c7345bcc49c1c6171e46cc5d59d9da5a/examples/firmware-model-loader/observations-qwen3-8b/expected-files.json) and [independent image readback](https://github.com/KirtusJ/pcp-os/blob/bd9c067b7fa3e13cd17ac62691113e4efc8b27d8/examples/release-media/observations/qwen3-8b/independent-image-readback.json) are **historical evidence**. This recipe has not been shown to reproduce that exact image, nor independently run against the real 8B bytes in this PR.

## Separate public-distribution and hardware gates

The existing 80-path inventory contains **no publisher license or notice file**. The current EFI has **no embedded Authenticode signature**. An approved model publisher revision, authentic license/notice texts, exact notice bytes, notice digests and final-media readback are outstanding. The release owner must approve model distribution terms and choose release contents; a new distributable image cannot simply inherit the historic 80-path pin. The resulting image needs an independently trusted maintainer signing/attestation record binding its **whole ISO digest** to exact source, toolchain and component records. A signed Git source commit does not sign a finished ISO. Attestation availability has not been exhaustively verified.

Model readiness, full native generation on the exact shipping worker/UI, Secure Boot, physical cold boot, temperature/fan observations, shutdown and recovery remain **separate acceptance gates**. The checker cannot prove absence of malicious hidden FAT entries, disk-write safety on a physical USB device or firmware compatibility. Preserve the original candidate and independent evidence. Nothing in this gate authorizes production, physical-device writes or public distribution.
