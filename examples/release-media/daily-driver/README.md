# Boot the model-free x86-64 UEFI desktop

[Download the daily-driver ISO (ZIP)](pcp-os-x86_64-uefi-model-free.zip). The archive contains one ISO. It boots directly to the PCP OS desktop and offers Tailorbird and Surfbird in Programs. The image contains no Qwen weights, model manifest or model worker.

**Test environment.** Use a disposable x86-64 UEFI virtual machine. The EFI loader is unsigned. Use an unsigned OVMF profile in the VM. Physical hardware qualification is open.

This ISO reached the 1920×1080 desktop in separate optical and USB QEMU boots. In each run, opening Tailorbird produced its first frame and the message `tailorbird: no model service channel`. Both runs passed all 72 kernel checks and powered off.

## Boot from a checkout

Install Python 3, `qemu-system-x86_64` and OVMF. On Ubuntu 24.04 the packages are `python3`, `qemu-system-x86` and `ovmf`. Run this command from the repository root.

```sh
python3 examples/release-media/daily-driver/boot.py --check
```

The command verifies the ISO SHA-256, boots it read-only with no network or data drive, and exits successfully after the guest reports long mode, the expected kernel measurement and the desktop. Omit `--check` to use the desktop interactively. On Windows use `python` in place of `python3`. Use `--qemu PATH` for a portable QEMU installation and `--firmware DIR` for a separate OVMF installation.

Press **Enter** at the desktop to open Programs. Press **Enter** again to open Tailorbird. Tailorbird works offline and reports that its model service is unavailable. Use **Esc** to return, then **F10** and **Enter** to power off.

## Identify and verify the download

| Item | Value |
| --- | --- |
| ISO | `pcp-os-x86_64-uefi-model-free.iso` |
| Bytes | 70,440,960 |
| ISO SHA-256 | `2e76936c78dccb2c269c898651f839b18dd9319b33a3d02b21f501325a741ab3` |
| OS source | [`92623fa6`](https://github.com/KirtusJ/pcp-os/commit/92623fa6759bbb4e4f27ce5aa381de213a55ae0c) |
| Tailorbird source | [`645b4c26`](https://github.com/KirtusJ/tailorbird/commit/645b4c266390e51b6d5abb818dce96e6cf916d22) |

To verify the build identity, extract the ZIP into this directory and run both commands with GitHub CLI. The attestation covers the uncompressed ISO and its eleven-file component manifest. [GitHub Actions build and boot](https://github.com/KirtusJ/pcp-os/actions/runs/46084425686) · [Image receipt](image.json) · [Component manifest](inputs.json)

```sh
gh attestation verify pcp-os-x86_64-uefi-model-free.iso --repo KirtusJ/pcp-os --signer-workflow KirtusJ/pcp-os/.github/workflows/model-free-desktop.yml --source-digest de68560e2284f8c2efc4828e80b93ed18cd699e0 --bundle provenance.jsonl
gh attestation verify inputs.json --repo KirtusJ/pcp-os --signer-workflow KirtusJ/pcp-os/.github/workflows/model-free-desktop.yml --source-digest de68560e2284f8c2efc4828e80b93ed18cd699e0 --bundle provenance.jsonl
```

For offline verification, use the same bundle with a separately obtained trusted root and GitHub CLI's `--custom-trusted-root` option. [GitHub verification guide](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/verify-attestations-offline)

## Rebuild from source

Use the [build prerequisites](../README.md#prepare-the-components) with Rust 1.97.0, then run `bash examples/release-media/daily-driver/rebuild.sh NEW_OUTPUT_DIRECTORY` from the repository root. The script fetches the exact OS and Tailorbird commits, the guest ABI revision and the digest-pinned Enterprise CD used for Surfbird and its assets. It reads all eleven files from both the EFI partition and the optical filesystem before packaging the ISO. The manifest records the toolchain that produced the shipped bytes.

The build used Ubuntu GRUB 2.12-1ubuntu7.3. Both source packages, including upstream source and Ubuntu patches, are available alongside the project loader recipe. [GRUB tools source](https://launchpad.net/ubuntu/+source/grub2/2.12-1ubuntu7.3) · [GRUB EFI modules source](https://launchpad.net/ubuntu/+source/grub2-unsigned/2.12-1ubuntu7.3) · [Loader recipe](../basic.py)
