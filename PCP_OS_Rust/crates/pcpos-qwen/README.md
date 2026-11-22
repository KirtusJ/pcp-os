# Dense Qwen scalar execution

`pcpos-qwen` admits a bounded dense Qwen2 or Qwen3 GGUF index and runs a scalar
single-sequence forward pass for the native inference worker being built. It
does not contain a tokenizer, sampler, device integration or model Ready state.

`Model::admit(&index)` returns immutable references to every required tensor,
with its exact rank, dimensions and supported storage type checked. Qwen2 needs
Q/K/V biases; Qwen3 needs per-head Q/K normalization. The output projection may
be tied to the input embeddings. Missing or extra tensors refuse admission.
Only the explicitly supported architecture metadata is accepted; scaled RoPE,
sliding windows, experts, split GGUF containers and different architectures are
refused. Descriptive metadata is not interpreted as executable behavior.

Admission uses no allocation. The library forbids unsafe code and works without
`std`; forward state allocates its buffers fallibly. It reuses the bounded GGUF
reader, scalar tensor decoder and pinned Rust `libm` 0.2.16. The GGUF reader owns
the small index; the caller owns its prefix and must authenticate the same immutable model
bytes before executing any weights. Admission does not inspect floating scales
or token strings. The scalar row decoder remains responsible for finite weight
values, and a future tokenizer must validate its own vocabulary and merges.

Policy limits are 64 layers, 8192 embedding values, 65536 feed-forward values,
128 query/KV heads, 256 values per head, 262144 vocabulary entries and 262144
training-context positions. They bound this implementation, not the GGUF format.
Only even full-head RoPE and equal key/value head widths are supported. Norms
and biases require F32/F16/BF16; matrices also allow Q5_0/Q8_0/Q4_K/Q6_K, the
formats covered by `pcpos-model-tensor`.
Any quantized tensor requires `general.quantization_version` as an unsigned
32-bit integer equal to 2, the pinned GGML layout version. Missing, wrong-typed
or unknown versions refuse; all-floating models may omit the field.

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

Admission tests encode small GGUF files and use the real index reader: both architectures,
tied/untied output, reordered tensors, nonstandard Qwen3 head width, exact budget
and context boundaries, unsupported semantics, nonfinite metadata, and missing,
extra, transposed, wrong-rank or unsupported tensors. Host real-file checks also
admitted the 291-tensor Qwen2.5-Coder-0.5B reference and 399-tensor Qwen3-8B
candidate; the 8B one-byte-short KV budget and excessive context refused.

Geometry was checked against the pinned llama.cpp b12387 implementations
([Qwen2](https://github.com/ggml-org/llama.cpp/blob/a956a852c9c8bc1fb07206364838ddef299178c0/src/models/qwen2.cpp),
[Qwen3](https://github.com/ggml-org/llama.cpp/blob/a956a852c9c8bc1fb07206364838ddef299178c0/src/models/qwen3.cpp))
and the actual candidate indexes, rather than inferred from marketing model size.

## Scalar forward pass

`forward::Decoder::new(model, bytes, context, budget, keep_going)` binds one
sequence to its admitted model and borrowed immutable weights. The bytes must
be the authenticated source of that index. The caller supplies the memory budget;
model metadata cannot choose it. `buffer_bytes` counts the payload of every f32
KV and scratch buffer before allocation. It excludes allocator overhead, the
decoder struct, stack, weights, index, tokenizer, page tables and OS memory.

`step(token, keep_going)` implements RMS normalization, Q/K/V projections and
Qwen2 biases or Qwen3 head norms, full-head NeoX RoPE, causal grouped-query
attention, SiLU gated feed-forward, residuals, output normalization and logits.
It decodes quantized rows on demand with f64 dot accumulation, retaining no
model-sized float copy. Finite checks reject invalid scales and arithmetic.
The host-independent scalar implementation is a correctness reference, not a
throughput-qualified native backend.

All buffers are allocated once with fallible reservations. Initialization polls
cancellation between chunks of 1024 floats; execution polls at row/block and
attention-position boundaries. Before a dot product, row validation may scan up
to 65536 weights without polling. Cancellation or an error returns no logits
and does not advance the position. Partial KV writes touch only the uncommitted
position; retry overwrites them. Tests compare retry and subsequent results to
uninterrupted execution. `reset` starts a new logical sequence; it makes no
secure-erasure promise for private memory. Kernel scheduling and whole-worker
Stop latency remain unmeasured by this library.

Four token steps through each tiny two-layer F32 Qwen fixture match the pinned
llama.cpp runtime within `2e-5 + 2e-5 * abs(reference)` for all 128 logits.
The fixtures are constructed in Rust and SHA-256 checked against the exact files
given to the independent runtime. Tests also cover cancellation at multiple
stages, retry, reset, exact budget, bad token/context limits and a nonfinite final
output weight. The library compiles for `x86_64-unknown-none`; this is not a boot.

The real 0.5B candidate was also run for token IDs 9707, 11, 279, 1495 (607744
logits). All four top tokens agreed with both references, but the optimized
llama.cpp quantized path differs materially: maximum logit errors by position
were 0.350, 7.846, 0.271 and 0.561. A second independently written NumPy graph,
using GGML-decoded weights and float64 matrix products without runtime activation
requantization, differed from Rust by at most 0.00011611. The optimized-path
discrepancy has not been traced layer by layer and remains qualification work;
top-token agreement is not quality parity. Exact measurements are in
`tests/data/real-0.5b-comparison.json`.

One shared-host pass took 14.14 seconds for the four Rust steps versus 0.49 seconds
for optimized llama.cpp; these are observations, not a controlled benchmark.
There is no 8B inference, native worker, UI chat, sampler or release-quality claim.

The `forward` host example accepts a model up to 1 GiB, a new output directory
and 1–32 token IDs, writing one little-endian f32 logit vector per completed step.
It does not tokenize or authenticate the file. The size ceiling deliberately
keeps the 8B file out of this whole-file diagnostic; the native worker must use
the verified immutable mapping.

For independent QA, export the tiny inputs with the ignored
`export_reference_models` test (set `QWEN_FIXTURE_DIR` to an existing directory).
`tests/llama_reference.py DLL_DIR MODEL NEW_OUTPUT_DIR TOKEN...` uses the pinned
Windows runtime; every DLL hash is checked against `reference-runtime.json`,
which was verified against the official b12387 archive. `tests/scalar_reference.py
DLL_DIR MODEL PROFILE_JSON NEW_OUTPUT_DIR TOKEN...` uses NumPy for the independent
Qwen2 graph; the profile supplies the directory and pinned model hash. These are
host QA tools, not native dependencies. See `THIRD_PARTY_NOTICES` for provenance.

## Prompt prefill and measured alternatives

`Decoder::prefill` accepts a nonempty token batch and computes vocabulary scores
only for its final token. All transformer and KV steps are retained. Empty input,
invalid IDs and insufficient remaining context refuse before mutation. A failure
or cancellation rolls the logical position back to its entry value; retained
context is untouched and retry overwrites only the uncommitted suffix. This is
logical rollback, not memory erasure. Tests compare batched and individual steps,
repeat resets/full contexts, and cut at early, late and seeded callback positions.

`Decoder::prefill_with_progress` lets a caller report how many input tokens
the model has processed for this reply. A `TokenProcessed` event counts one
complete token. The total includes the chat template and any conversation
history supplied by the caller. The final token includes the vocabulary
projection. A caller can stop on a regular `Poll` or on a progress event,
including the last one. Only a successful return makes the prompt available
for the next decoder step.

`Decoder::new` retains sequential f64 dot accumulation. `new_unrolled` explicitly
selects four-lane f64 reduction; model-level numerical/quality checks are required
because reassociation can differ materially on ill-conditioned inputs. Budgets,
weight validation, cancellation and retry ownership are unchanged. The separate
tensor tests retain an adverse numerical discriminator rather than promising
universal bit parity.

`cargo run --release -p pcpos-qwen --example speed -- MODEL NEW_DIR TOKENS_CSV MODE`
is a host-only paired diagnostic restricted to the checksum-pinned 0.5B reference.
Modes are `step`, `prefill`, `unrolled-step`, `unrolled-prefill` and `rows`. Use the
same binary/compiler/profile, identical token CSV and alternating run order. It
records 8 fixed greedy steps and every vocabulary logit; it does not stop on EOS
or apply chat sampling, and reports decoder time separately from loading/hash
checks. Shared-host timings are not native, target-laptop or release qualification.

The stock `x86_64-unknown-none` target uses software floating arithmetic. Enabling
SSE2 alone does not replace it. Disabling its `soft-float` feature violates the
target ABI and is not supported here. A hardware-float guest needs a consistent
target/core/alloc build and kernel register-state qualification; the kernel must
retain its own no-FP target. See [Rust target requirements](https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html).

## Qwen byte BPE tokenizer

`tokenizer::Tokenizer` admits only the GGUF `gpt2` / `qwen2` profile, with complete
byte coverage, distinct vocabulary strings and merge pairs, typed control IDs,
and existing normal-token operands/results for every merge. Unknown tokenizer
settings, normalization, automatic prefix space/BOS/EOS and unsupported kinds
are refused. The chat-template string is retained by GGUF but is not executed.

The ordered Qwen2 Unicode pre-tokenizer and byte alphabet follow pinned llama.cpp
`a956a852c9c8bc1fb07206364838ddef299178c0`. Unicode L/N/White_Space tables are
generated from its checksum-pinned `unicode-data.cpp`, independent of the host's
Unicode version. BPE merges use a bounded heap, ordered by rank then leftmost
position; they do not scan quadratically for the next merge. Normal vocabulary
strings borrow the prefix. Admission uses fallible allocations and in-place
heapsort with cancellation. The actual candidate tables request 6,676,400 bytes
on the 64-bit target; allocator page rounding, struct and stack are additional.

`encode` accepts at most 16 KiB of UTF-8, a caller token limit and an explicit
scratch/output budget. All vectors reserve fallibly before use. Scratch holds
at most one node and three heap entries per input byte; output at most one token
per byte. Cancellation is polled during pre-splitting, node/heap processing,
table validation and each heapsort sift. An error yields no partial token vector.
These operation boundaries do not establish native wall-clock Stop latency.

User input is literal. Added/control spellings in it are not interpreted as role,
tool or reasoning delimiters. A trusted template can obtain their IDs explicitly
with `special_id`. This intentionally differs from llama.cpp's automatic parsing
of USER_DEFINED token spellings even with `parse_special=false`. `piece` returns
raw normal/user-defined bytes and refuses controls/unused entries. `Utf8Stream`
retains at most three unfinished bytes, emits only whole UTF-8, and reports invalid
or unfinished sequences instead of silently replacing them. None of this grants
tool permission or constitutes a chat-template interpreter or a native service.

Host qualification on each pinned real 0.5B and 8B vocabulary: 541 cases and all
50,223 token IDs agree with b12387, including all 25 whitespace codepoints,
contractions, numbers, code, multilingual/combining/emoji text, literal control
delimiters and 400 seeded randomized inputs. All 151,643 normal-token raw pieces
agree byte-for-byte. No 8B weights are loaded: reference uses `vocab_only`; Rust
reads a 16 MiB prefix. Model-prefix, vector and piece hashes are asserted by the
ignored `actual_vocabulary_matches_reference_vectors_and_all_normal_pieces` test.
Generate its files with `tests/tokenizer_reference.py DLL_DIR MODEL NEW_DIR`, set
`QWEN_TOKENIZER_MODEL` / `QWEN_TOKENIZER_VECTORS`, and run that test explicitly.
The ordinary suite covers synthetic merge priorities, limits/refusals, cancellation,
literal delimiters, raw-byte round trips and UTF-8 token boundaries without models.

One reference-policy difference is preserved: b12387 guesses normal Qwen token
128247 (`</s>`) is a control and hides it when `token_to_piece(..., special=false)`
is called. Qwen's GGUF marks it normal; this decoder emits its four literal bytes.
The raw-piece comparison enables special display in the reference and records
the single override. It does not adopt the runtime's cross-model name heuristic
as a Qwen end-of-generation rule. Earlier failing comparison is retained locally.
