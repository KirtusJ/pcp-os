# Dense Qwen model geometry

`pcpos-qwen` admits a bounded dense Qwen2 or Qwen3 GGUF index for the native
inference worker being built. It currently validates geometry only: there is no
transformer, tokenizer, sampler, device integration or model Ready state here.

`Model::admit(&index)` returns immutable references to every required tensor,
with its exact rank, dimensions and supported storage type checked. Qwen2 needs
Q/K/V biases; Qwen3 needs per-head Q/K normalization. The output projection may
be tied to the input embeddings. Missing or extra tensors refuse admission.
Only the explicitly supported architecture metadata is accepted; scaled RoPE,
sliding windows, experts, split GGUF containers and different architectures are
refused. Descriptive metadata is not interpreted as executable behavior.

The library uses no allocation or unsafe code and works without `std`. Its sole
dependency is the existing bounded GGUF reader. That reader owns the small
index; the caller owns its prefix and must authenticate the same immutable model
bytes before executing any weights. Admission does not inspect floating scales
or token strings. The scalar row decoder remains responsible for finite weight
values, and a future tokenizer must validate its own vocabulary and merges.

Policy limits are 64 layers, 8192 embedding values, 65536 feed-forward values,
128 query/KV heads, 256 values per head, 262144 vocabulary entries and 262144
training-context positions. They bound this implementation, not the GGUF format.
Only even full-head RoPE and equal key/value head widths are supported. Norms
and biases require F32/F16/BF16; matrices also allow Q5_0/Q8_0/Q4_K/Q6_K, the
formats covered by `pcpos-model-tensor`.

`kv_bytes(context, budget)` computes exact f32 key and value storage and refuses
a context beyond the model's training length or the caller's independent byte
budget before any allocation. It excludes weights, scratch, tokenizer, index,
page tables and the OS: it is not a total-memory or minimum-system-RAM claim.
At 2048 positions the pinned 0.5B reference requires 50,331,648 KV bytes; the 8B
candidate requires 603,979,776 KV bytes. Neither number establishes throughput.

## Checks

```sh
cargo test --locked -p pcpos-qwen
cargo test --locked --release -p pcpos-qwen
cargo build --locked --release --target x86_64-unknown-none -p pcpos-qwen --lib
cargo run --locked --release -p pcpos-qwen --example admit -- MODEL.gguf FILE_BYTES 2048 KV_BUDGET_BYTES
```

The diagnostic reads at most 32 MiB, and accepts a separate declared whole-file
size so a downloaded prefix can be inspected without loading weights. Its output
explicitly says that weights are unverified and no inference has run.

Tests encode small GGUF files and use the real index reader: both architectures,
tied/untied output, reordered tensors, nonstandard Qwen3 head width, exact budget
and context boundaries, unsupported semantics, nonfinite metadata, and missing,
extra, transposed, wrong-rank or unsupported tensors. Host real-file checks also
admitted the 291-tensor Qwen2.5-Coder-0.5B reference and 399-tensor Qwen3-8B
candidate; the 8B one-byte-short KV budget and excessive context refused.

Geometry was checked against the pinned llama.cpp b12387 implementations
([Qwen2](https://github.com/ggml-org/llama.cpp/blob/a956a852c9c8bc1fb07206364838ddef299178c0/src/models/qwen2.cpp),
[Qwen3](https://github.com/ggml-org/llama.cpp/blob/a956a852c9c8bc1fb07206364838ddef299178c0/src/models/qwen3.cpp))
and the actual candidate indexes, rather than inferred from marketing model size.
