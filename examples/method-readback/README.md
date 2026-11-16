# Read the selected Core Specification in a PCP OS guest

From the repository root, run the following on Linux x86-64 with Python 3, a C compiler, GNU binutils, QEMU and a bootable PCP OS multiboot kernel. Supply the expected method manifest digest from the release you selected.

```sh
python3 examples/method-readback/check.py \
  --method /path/to/method \
  --manifest-sha256 7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707 \
  --kernel /path/to/worker-kernel.elf32
```

The example digest selects the 58-file Enterprise method. Its Core Specification is 138,793 bytes and has SHA-256 `649211ff2d765e06b5d5eb31dc80af6f97aedea242f2dcb5b77b3fa270ef34d6`. The runner accepts another release when its manifest matches the supplied digest and identifies exactly one Core source at the expected path. The example exercises that one source.

CI repeats the six cases against two public releases using the same kernel from the published ISO. The bundled build 19 and standalone build 22 have different manifests and Core bytes. Each run uses its own release digest. [Bundled method](https://github.com/KirtusJ/pcp/tree/e23784155622311cfff697468beb0ba8bbe83194/PCP_1.0_Plugins/plugins/pcp-os/method) · [Standalone method](https://github.com/KirtusJ/pcp-os-claude/tree/c0fbde739b4cc3bc409878d901de0974702c506e/method)

The host verifies the manifest and Core bytes, then embeds a reference copy in a small ELF using GNU objcopy. It starts an offline VM for each case. A separate boot module supplies the file the guest process opens. The process compares bytes through end of file. The runner checks the kernel measurement, the executable SHA-256, the guest’s serial result and process exit, and the emulator’s final status. [GNU objcopy binary input](https://sourceware.org/binutils/docs/binutils/objcopy.html)

| Guest input | Required guest result |
| --- | --- |
| Exact reference | MATCH |
| Last byte changed, last byte missing, or a byte appended | DIFFERENT for each input |
| File omitted | MISSING |
| Exact file supplied with file access denied | DENIED |

All cases reuse the same executable. They measure whether this process read the identified bytes in these boots. A language model receiving the source as input, retrieving a clause or applying it to a task requires separate observation in that model’s configuration.

## Run the public resident worker with Core

The public worker check repeats a complete 24-guest challenge using the original fixture, then links the byte reader into a second copy of the same Rust worker. Both builds use the pinned upstream checkpoint. The original fixture compares its 48 predicted token IDs with an independently compiled C reference. The second executable starts at `core_entry`, reads the separate boot module through EOF and passes the original process stack to the unchanged Rust entry point on a match. A same-length substitution and an absent file must produce different failed process exits before any model output. The same executable then completes a full challenge with the exact Core module attached. The supervisor and its wire protocol are unchanged.

Use the Linux dependencies and two digest-checked inputs in the public fixture instructions. CI builds the worker, supervisor and kernel from the checkout GitHub tests. For a pull request, the tested commit includes its proposed merge with the base branch. The Enterprise adapter and model inputs have independent pins. The archived `inputs.json` records the actual checkout revision, measured kernel, method manifest and hashes of the worker, control, oracle and Core.

For a local run, check out the desired worker revision in `fixture` and the pinned Enterprise method in `adapter`. Pass the expected full worker commit, obtained from the revision you intend to qualify. To reproduce the original #167 configuration, omit `--fixture-revision` and use `f1d1ec07ccc30dec4a8c54a6b454efec30cc0f6a`.

```sh
sh fixture/PCP_OS_Rust/crates/pcpos-kernel/run.sh build
python3 examples/method-readback/public-worker.py \
  --fixture fixture --fixture-revision "$expected_worker_commit" \
  --kernel fixture/PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 \
  --model inputs/checkpoint.bin --reference inputs/run.c \
  --method adapter/method --out receipts
```

The output directory must be new. The traces identify each guest and link its fresh response to the independent oracle. The test verifies process-level access to Core. The transformer still receives indexed prompts. [Public fixture and input pins](https://github.com/KirtusJ/pcp-os/tree/f1d1ec07ccc30dec4a8c54a6b454efec30cc0f6a/examples/resident-workers) · [GNU ld entry points](https://sourceware.org/binutils/docs/ld/Entry-Point.html)
