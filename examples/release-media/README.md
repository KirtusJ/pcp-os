# Assemble PCP OS x86-64 UEFI release components

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


## Build the model worker for a pinned manifest

The worker can be compiled before the production loader is ready. Use the Tailorbird and PCP OS repositories above with Rust 1.97.0 and its `x86_64-unknown-none` target. Tailorbird #20 at the commit below pins PCP OS `0de2dd69fc441ae47336601f894e062669453b90`. The worker uses that commit for its guest ABI, tokenizer, decoder and allocator. The boot kernel is a separate component.

This example uses the Qwen2.5-Coder-0.5B-Instruct qualification reference. Supply the actual packed manifest and the independent digest recorded for that model.

```sh
python3 examples/release-media/worker.py \
  --os "$OS_REPO" --tailorbird "$TAILORBIRD_REPO" \
  --tailorbird-head ffffc081dfd545fa2c65a94dc9a01cd144b14c34 \
  --manifest /path/to/coder-packed/model.manifest \
  --manifest-sha256 bbdaa8cd263d9454159f12b55ec32613d18a4b7b4d75d284ad0cadc932bc1f4b \
  coder-worker
```

`coder-worker/model-worker.elf` and `coder-worker/inputs.json` contain the worker and its build record. The receipt records the application commit and tree, PCP OS gitlink, target, compiler, Cargo, resolved feature list, manifest pin and ELF SHA-256. The script prints the receipt digest. For a repeat on a known builder, add `--expect-elf-sha256` with the expected worker digest.

For compile-only work, omit `--manifest` and supply the independently trusted `--manifest-sha256`. The Qwen3-8B candidate has its own manifest pin, `2c02ff6f00a2ee802ce09648dfbc56dda38a64b009635f0aeb383f09e1af67dc`. Building a worker for a pin establishes a binary identity. The loader and worker still have to verify the manifest and model bytes on the selected media. [Candidate package record](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7074823392)

On one Linux builder the Coder worker was `5761a734b2a3a9ef15bbe68e8e7e0e5972c2dc751fa05125fbdd27556a08255d` and the 8B worker was `45b992964cc324229a56d6532d85d462d2a1cc833c7ab3fb4dcb26c22bf4da50`. Their 189,152-byte ELF files differed only in the embedded manifest pin. The Coder worker matched an independent build using Tailorbird's own command on this host. Rust also embeds paths from the local dependency cache. Use the ELF digest of the specific build the loader will start.
