# Model tensor rows

`pcpos-model-tensor` decodes the weight formats present in the pinned Qwen3-8B
Q4_K_M candidate and Qwen2.5-Coder-0.5B reference: F32, Q5_0, Q8_0, Q4_K and Q6_K.
F16 and BF16 are also supported. Other GGUF kinds are explicitly refused.

`Row::new(kind, columns, bytes)` checks an exact row extent, block alignment,
arithmetic overflow and every floating value/scale for finiteness. It borrows the
original bytes. `decode` checks the output length before writing. `dot` decodes
one block at a time into 1 KiB of stack scratch, accumulates in f64 and returns a
finite f32 or an error; its caller can cancel between blocks of at most 256 values.
The library is `no_std`, allocation-free and forbids unsafe code. It adds only a
dependency on the existing GGUF index crate.

The caller must supply bytes from the verified immutable model, select the right
tensor/row and validate the model architecture. This crate is a scalar reference
component: it supplies no tokenizer, transformer, KV cache, sampler, device ABI,
model authentication or Ready state. It does not establish native inference
speed or Stop latency. No whole-model float copy is allocated.

## Validation and provenance

From the Rust workspace:

```sh
cargo test --locked -p pcpos-model-tensor
cargo test --locked --release -p pcpos-model-tensor
cargo build --locked --release --target x86_64-unknown-none -p pcpos-model-tensor --lib
```

The checked-in 27 golden rows come from the scalar GGML functions in the pinned
llama.cpp b12387 Windows CPU reference, plus direct IEEE bit identities for F32
and BF16. Tests compare every decoded bit and dot products against those decoded
reference values. Additional tests cover malformed extents, unsupported kinds,
nonfinite values/scales in later blocks, unchanged output after length refusal,
and cancellation. The fixture metadata pins the exact reference library SHA-256.

`tests/make_vectors.py /path/to/ggml-base.dll` regenerates the fixtures only with
that library hash. It is a host QA helper, not a native protection. The upstream
source revision is `a956a852c9c8bc1fb07206364838ddef299178c0`.
`THIRD_PARTY_NOTICES` retains the upstream MIT notice for the adapted layouts and
decoding formulas.

The host `decode` example accepts one bounded encoded row and writes raw
little-endian f32 output to a new file. It supports independent comparisons with
real model rows without loading the full model or executing any model output.
