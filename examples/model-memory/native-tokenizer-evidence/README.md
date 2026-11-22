# Native Qwen tokenizer acceptance

The same native companion ran the tokenizer from PCP OS PR #381 at `0de2dd69fc441ae47336601f894e062669453b90` in two independent OVMF boots. One boot mapped the first 16 MiB of Qwen2.5-Coder-0.5B-Instruct Q4_K_M. The other mapped the first 16 MiB of Qwen3-8B Q4_K_M. The receipt identifies the exact Hugging Face revisions, original file sizes, prefix digests, guest and kernel digests, and frame counts. The EFI loader and the guest separately checked the prefix. The manifest authenticates only these 16 MiB. The original file size is supplied to the GGUF directory parser for validating tensor offsets.

Each boot reproduced the pinned llama.cpp b12387 vector digest over 541 inputs and 50,223 IDs. All 541 inputs streamed back to their original bytes. The second digest covers every normal ID from 0 through 151642, with its little-endian ID, byte count and raw piece. Both boots also passed literal control and user-defined delimiter checks, ordinary token 128247, table and encode budget checks, mid-sort cancellation, and cancellation on the final encode poll followed by an identical retry. The front received its acknowledgement and clean EOF. Each kernel passed 71 self-tests and returned all low and high frames. QEMU exited 33 both times.

The experiment uses a text-test companion. It measures tokenizer behavior with production vocabulary metadata. Model inference, the Tailorbird worker, physical performance, and human Stop latency require their own tests.

## Reproducing the inputs

Use the tokenizer reference script from the commit above. Run `python3 source/make_cases.py /path/to/tokenizer_reference.py cases.bin` and place the output beside `source/main.rs`. The script uses the reference input generator only. No expected token IDs are compiled into the guest. The two SHA-256 reference results in `source/pins.rs` are from the pinned upstream qualification. The guest recreates both hashes on the target.

Read bytes 0 through 16777215 inclusive from the two model-file URLs in the receipt. Check the HTTP Content-Range total and SHA-256 for each. Each prefix uses a one-shard PCPMDL01 manifest. Its digest is compiled independently into the EFI source and the companion.

The guest uses the repository native entry and page allocator, pcpos-model-media, pcpos-gguf, pcpos-qwen and no_std sha2 0.10.9. The source directory contains the guest, allocator, pins and exact per-family EFI inputs. The handoff assembly is the repository’s firmware-owned-uefi example. The kernel is #371 at `b65d7c45223724aa23519e12c1505981d91634c5`, built with `proposals,model-memory` and the 32 MiB link relocation. Each prefix was loaded across two model-owned physical extents with one guard page between them. The firmware and serial logs are retained verbatim.
