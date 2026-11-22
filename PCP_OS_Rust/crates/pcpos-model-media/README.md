# Verify model bytes before exposing them to an engine

`pcpos-model-media` is a `no_std` Rust library for the worker side of offline
model loading. It uses the workspace's SHA-256 implementation. It does not
implement an inference engine, a native disk driver, an ISO builder or a kernel
permission. No Tailorbird or kernel ABI changes are included.

The trusted image/launch contract supplies a manifest digest. The manifest
cannot nominate its own trust: copying its hash from the same untrusted medium
does not authenticate it. `Manifest::parse` first checks that independent digest,
then the canonical record shape. V1 is little-endian:

| Byte offset | Field |
| --- | --- |
| 0 | Eight-byte magic `PCPMDL01` |
| 8 | Whole-model byte length, u64 |
| 16 | Shard size, u32 |
| 20 | Number of shards, u32 |
| 24 | Whole-model SHA-256, 32 bytes |
| 56 | Ordered shard SHA-256 values, 32 bytes each |

All shards except the last have exactly the declared shard size. The last has
the remaining bytes. Shard size is a power of two from 1 KiB through 64 MiB;
there are at most 4,096 shards and at most 64 GiB total. No trailing manifest
bytes are allowed. These are version-one policy limits. Names, paths, sector
addresses and permission grants are deliberately outside this record; the
kernel's nominated read-only device supplies a logical `ReadAt` source.

`VerifiedModel::load` reads in at-most-64-KiB requests, supports short reads,
verifies every shard and the complete ordered model, and checks actual EOF.
It uses fallible allocations. On cancellation, read failure, incorrect length
or digest mismatch, all partially retained buffers are dropped and no model is
returned. The returned model owns its bytes and only exposes immutable reads,
so later changes to the source cannot replace verified weights. A successful
load establishes identity, not that the model architecture is supported or that
the inference engine can answer.

The progress callback reports actual bytes read and verified shards. Returning
false cancels loading. It must not show service Ready until the loader returns
success and the engine finishes its separate initialization. Cancellation
cannot interrupt a blocked device read: the native adapter must bound each
read's wait. Peak memory includes all model bytes plus index/allocator overhead;
this loader does not make a 5 GB model fit the current sub-1-GiB native frame pool.

From `PCP_OS_Rust`:

```sh
cargo test --locked -p pcpos-model-media
cargo test --locked --release -p pcpos-model-media
cargo run --locked --release -p pcpos-model-media --example pack -- MODEL.gguf EXPECTED_SHA256 NEW_DIRECTORY
cargo run --locked --release -p pcpos-model-media --example verify -- MODEL.gguf MANIFEST TRUSTED_MANIFEST_SHA256
```

The host packer copies bytes into numbered 64 MiB files, verifies the whole
model against the supplied publisher digest, and writes `model.manifest` last.
It never overwrites an existing destination. A failure can leave partial shard
files, with no usable manifest; such a directory is not a release. The printed
manifest digest is an input to the later image-pinning step, not a signature or
proof of publisher identity. Packaging a model does not boot it.

The host verifier exercises the library against a raw model file and a separately
pinned manifest. It retains all verified bytes, so use a model that fits available
host memory. It is a reader reference, not native boot or inference evidence.

Tests cover wrong pins, reordered or changed shards, inconsistent whole-model
digests, short/extra bytes, source failures, cancellation, cross-shard reads,
post-load source mutation and lengths above 4 GiB without giant fixture files.
The generic source allows a native device adapter to be added without changing
the identity rules. The current source/shard layout proposal is tracked in
[the directors' exchange](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7074224633).
