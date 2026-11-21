# Director 14: optional dual-Qwen ISO file-inventory oracle

**TEST-ONLY; DOES NOT CONTAIN MODEL WEIGHTS OR AN ISO.** This independently
authored utility defines one narrow future acceptance check for [#246](https://github.com/KirtusJ/pcp-os/issues/246).
Do not interpret a synthetic green job as proof that either genuine Qwen model
exists in a release, that a license permits redistribution, or that Tailorbird
can select or execute a model.

## What it examines

The caller provides a local **already extracted or mounted, read-only candidate**
file root, a JSON inventory manifest, and the manifest's 64-hex SHA-256 from a
**separate trusted handoff**. `check_inventory.py` checks exactly two distinctly
labelled slots (8B and 12B), separate upstream model IDs, full 40-digit upstream
revision IDs, separate weight files and license files, plus a runtime and Tailorbird
binary, against streaming size and SHA-256 checks. It refuses duplicate paths,
file/symlink/hardlink aliasing, traversal, malformed/mismatched manifest,
missing/truncated/altered assets, and all aliases that collapse the two slots.
Checks are byte checks and **do not authenticate the claimed parameter count**.

### Candidate manifest shape

Top-level keys: `schema`, `source_revision`, `models` (exactly two entries),
`runtime`, and `tailorbird`. `schema` is
`pcpos.optional-dual-qwen-file-inventory.v1` and `source_revision` a full Git
40-hex commit. Each `models` entry contains `slot` (`qwen-8b` or `qwen-12b`),
`parameters_billions` (8 or 12, **declarative only**), `upstream_id`
(`organization/repository`), `revision` (40-hex), `quantization`, `weights`,
and `license`. Each of `weights`, `license`, `runtime` and `tailorbird` is
`{path,bytes,sha256}` with a canonical relative POSIX file path.

### Run

```sh
python3 audit/director-14/optional-dual-qwen/check_inventory.py \
  --root /readonly/factory/extracted --manifest /readonly/manifest.json \
  --expected-manifest-sha256 64_lowercase_hex_from_separate_handoff
python3 -m unittest discover -s audit/director-14/optional-dual-qwen -p 'test_*.py' -v
```

Unit tests create **tiny known-fake model data** whose bytes start with the letters
`GGUF`; they are neither valid GGUF weight sets nor redistributable models.
Run the utility on *actual* installed files separately. The present repo's existing
factory checker only validates one 0.5B model: this is a proposed **dual-slot
file inventory preflight** for an as-yet-undelivered optional edition, not a
change to that production checker or an installed-image validation.

## Unchecked and required separately

- The actual ISO byte digest, bootability, partition format, filesystem
  capacity, real GGUF semantic structure, 8B/12B true architecture and source
  provenance, model/license policy approval, full and exact source code build
  provenance, and site-specific hardware sizing.
- The actual offline clean boot, both Qwen inference paths, authenticated
  model-selection/readback in Tailorbird, restart/reconnect, cancelled jobs,
  memory release, and all operator/ACR destination permissions.
- A trusted manifest-signature chain. Hashing manifest bytes against a digest
  derived from the same manifest is *not* independent source authentication.
  Symlink checks plus pre-/post-hash descriptor metadata and pathname
  inode checks reject some observable input mutations. These are best-effort,
  not protection against adversarial mount rewrites, changed parent paths or
  rewrite-and-restore races. Consume an immutable, trusted factory snapshot.

The Principal's optional ISO model requirement and separate ISO-on-each-master-push
requirement remain open. No changes to D10's register, D2's raster, D11's
memory oracle, D12/D13's containment lanes, PT's Tailorbird source, installed
media or the default distribution are made by this test-only candidate.

## Future chunked-ISO byte-reconstruction preflight

A separate **test-only** script, \`check_chunk_manifest.py\`, can check an
ordered series of locally held part files against a separately hash-pinned
manifest. It streams each part once, checking part SHA-256 and byte length
while simultaneously computing the digest of **the concatenation in order**.
It does not build an ISO. The synthetic unit fixtures are short strings,
not bootable images or model weights.

\`\`\`sh
python3 audit/director-14/optional-dual-qwen/check_chunk_manifest.py \
  --root /readonly/parts --manifest /readonly/chunks.json \
  --expected-manifest-sha256 64_lowercase_hex_from_separate_handoff
\`\`\`

The JSON manifest has exactly four fields:
\`schema = "pcpos.optional-dual-qwen-chunk-inventory.v1"\`,
\`source_revision\` (full 40-hex commit), \`iso\`
(\`name\` ending in \`.iso\`, \`bytes\`, \`sha256\`), and
\`parts\` (an ordered nonempty list of objects with \`index\` starting at
zero, \`path\`, \`bytes\`, \`sha256\`).
The script refuses wrong indices/order, missing, changed, repeated, aliased
or symlinked chunks, path escapes and false assembled hashes. The
verified property is **the exact concatenated byte digest** of a
separately named candidate. Neither a claimed digest, a green
synthetic test, nor this checker authenticates a publisher or proves
those bytes contain a valid ISO filesystem, boots PCP OS or carry
either genuine Qwen model. The check is for an immutable,
trusted snapshot: concurrent replacement of parent directories and
model authorization remain separate concerns.

The new tests run in the same path-scoped CI as the original inventory
tests. No source for the installed factory checker, Tailorbird, Qwen
runtime, PCP OS kernel, ISO release or master branch is modified.
