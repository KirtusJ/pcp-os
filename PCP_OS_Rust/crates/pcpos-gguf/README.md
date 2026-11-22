# Read a model index before loading weights

`pcpos-gguf` reads little-endian GGUF v3 metadata and tensor locations from a
bounded file prefix. It is a Rust `no_std` library with no dependencies or unsafe
code. The caller retains the input bytes. Only the metadata and tensor index
allocate; reservations can fail without panicking. Token strings and arrays
borrow their original bytes, with no lossy text conversion.

This is the first model-loading component for the Qwen-preloaded PCP OS image.
It does not run a model, load weights, tokenize a prompt, authenticate a file,
grant a capability, or make Tailorbird ready to answer. The existing kernel is
not modified. A successful parse is not permission to execute model output.

Call `Index::parse(prefix, file_bytes)` with the prefix and length of the **same
immutable file**. `NeedMore { minimum }` means the file is large enough but the
caller must provide more of its prefix. `Truncated` means even the declared file
cannot hold the index read. A tensor outside that file returns `TensorRange`.
Offsets and lengths remain `u64`; they are not pointers into the prefix. The
caller must still bind the index and later tensor reads to the same verified
artifact. Do not reopen an untrusted path and assume it still names those bytes.

The reader rejects duplicate metadata keys and tensor names, overlapping tensor
ranges, zero or overflowing dimensions, partial quantization blocks within a
row, unsupported type codes, bad UTF-8, non-boolean Boolean bytes, invalid
alignment and nonzero index padding. It supports F32/F16/BF16 and the conventional
Q4/Q5/Q8 and K-quant block layouts used by the current Qwen candidates. This says
where encoded blocks live; it does not implement their numerical operations.

Policy limits are 32 MiB of prefix, 256 metadata entries, 1,024 tensors, 16 MiB
per string, one million elements per array, two million value visits in total,
and eight array levels. Alignment is a power of two up to 4,096. These are
explicit reader limits, not claims that larger or other GGUF files are invalid.
Parsing has bounded work but is synchronous; a native caller should run it in
the cancellable model worker, not in the desktop's input loop.

From `PCP_OS_Rust`, run:

```sh
cargo test --locked -p pcpos-gguf
cargo test --locked --release -p pcpos-gguf
cargo run --locked --release -p pcpos-gguf --example inspect -- /path/to/model.gguf
```

The host-only inspector reads at most 32 MiB, prints the architecture and index,
and never reads all the weights. It does not download models. Tests include
synthetic ranges above 4 GiB without creating a multi-gigabyte file, truncation
at every byte of an index, overflow, wrong block boundaries and forged metadata.
Workspace CI runs the same library on Linux, macOS and Windows.

For a separately retained prefix, the diagnostic accepts
`--prefix-file-bytes WHOLE_FILE_SIZE` after the path. Its output explicitly says
that the size is declared and the weights are unverified. This is useful for
testing publisher range responses; it does not turn that prefix into a complete
model or certify the declared size.

The layout follows the [upstream GGUF specification](https://github.com/ggml-org/ggml/blob/d9fe7bff3d2bf4d077e25c654486d02c321734aa/docs/gguf.md)
and the [upstream quantization structures](https://github.com/ggml-org/ggml/blob/d9fe7bff3d2bf4d077e25c654486d02c321734aa/src/ggml-common.h).
The [Candle reader](https://github.com/huggingface/candle/blob/1a49dc27c1da7671e0ee12279ddf4ba619412856/candle-core/src/quantized/gguf_file.rs)
was inspected as an existing Rust implementation. Its standard-library I/O and
tensor allocations cannot be used unchanged inside the present native kernel.
This small index reader does not substitute for an inference engine; reuse of
existing numerical implementations remains separate work.
