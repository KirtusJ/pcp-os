# Assemble the model-free x86-64 UEFI image

The model-free image boots the PCP OS desktop and offers Tailorbird and Surfbird in Programs. Tailorbird starts without a model service. Surfbird uses the bundled picture and sound clip and needs a supported audio device for playback. This recipe writes an ISO into a new regular file. It never writes a USB device.

## Prepare the components

Use Linux with Python 3.12 or later, Git, Cargo and rustc with rust-src and the x86_64-unknown-none target, ld.lld, GNU objcopy, xorriso, dosfstools, mtools and GRUB's x86_64-efi tools and modules. The two source repositories must contain the commits below, including the PCP OS submodule commit recorded by Tailorbird. Cargo needs the locked dependencies available locally or from its registry.

The example pins the integration kernel from #367, the application from KirtusJ/tailorbird#19, and the published PCP-OS.iso whose Surfbird, clips and notices are reused. The preparation step checks the donor digest, builds both ELF files from Git archives, records the resolved Cargo features, measures the kernel and binds the proposal grant to the compiled Tailorbird digest. GRUB reads the configuration on the volume selected by the firmware.

From the root of this checkout, set the three paths to your local inputs and run the following commands. The GRUB paths shown are for Debian.

```sh
OS_REPO=/path/to/pcp-os
TAILORBIRD_REPO=/path/to/tailorbird
ASSET_ISO=/path/to/PCP-OS.iso
python3 examples/release-media/basic.py \
  --os "$OS_REPO" --os-head d91fd184cdec7691248c3b57d0d39f4476f834fe \
  --tailorbird "$TAILORBIRD_REPO" --tailorbird-head 0fbf3826c159201bc583b9f09f8b6f8d1a1d4ac0 \
  --assets-iso "$ASSET_ISO" \
  --assets-sha256 93ed3b599ad7be0e1e88a64813b1f2b5299d51507df2edca691aaf6189d10d15 \
  --grub-modules /usr/lib/grub/x86_64-efi \
  --grub-license /usr/share/common-licenses/GPL-3 \
  basic-parts
```

Inspect `basic-parts/inputs.json` for the source commits, compiler, GRUB version, feature lists, kernel measurement and every file digest. Record its SHA-256 with the build evidence. For a repeat on the same builder, keep the toolchain, its installation path and the private Cargo build path fixed. Pass `--work /absolute/fixed-path` to choose the build path. It must be initially absent. Preparation removes it when finished, and requires a new parts directory for each run.

## Master and verify the ISO

Supply the digest from a trusted build record. If it differs, compare the complete receipts before accepting new bytes. The following digest is from the build of the pins above with rustc 1.100.0 and Debian GRUB 2.12-9+deb13u2.

```sh
python3 examples/release-media/build.py basic-parts \
  --inputs-sha256 cb81bf1d0bee18b33c404e0d42a34895037feaf66983145e2c6e77b1a6e000e8 \
  pcp-os-basic.iso
python3 examples/release-media/test_build.py
```

Mastering accepts the model-free inventory and the single kernel feature `proposals`. The fixture feature and its kernel banner each cause an independent refusal. The script compares every staged digest with the receipt and reads each file back from a fresh FAT32 image. The existing hybrid master then verifies the Joliet file tree against the FAT contents before publishing the finished file. The printed SHA-256 covers the entire ISO. Existing outputs are preserved.

Two clean builds on the same builder produced identical receipts and 70,389,760-byte images with SHA-256 `ea67434f1996cb81a3cb6e150c9b42b55127085a6e8501aac1e840cc256ce2c5`. OVMF booted that image as optical media, as raw USB media, and after copying its visible files to a fresh FAT32 volume. Each boot reached the 1920×1080 desktop. Physical UEFI and Rufus usage still need hardware checks.

The preloaded image requires a production loader with a published path and memory-handoff contract, a native model worker and an approved model digest. Its acceptance must include booting those components together. The model-free builder accepts only its own inventory.
