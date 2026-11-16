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
