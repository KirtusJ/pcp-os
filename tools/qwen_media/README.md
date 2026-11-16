# Offline model payload staging and exact-byte checks for PCP OS

**Scope:** host-side, review-only tooling that refuses wrong or ambiguous local model bytes. It does not install Qwen into the PCP OS kernel, implement inference, authenticate a publisher, or enable any Tailorbird tool. The OS remains capable of ordinary manual use when Qwen is missing. Ownership: Director PT, PCP OS #132. Coexists with Session A Tailorbird UI PR #134 and Session B boot-image audit PR #133.

## Intended generated media layout

Stage model weights on a **separate data volume**, not the UEFI FAT ESP:

`target/pcpos-media/data/models/qwen/Qwen3-0.6B/<40-hex-upstream-commit>/Qwen3-0.6B-Q4_K_M.gguf`

The directory and model bytes are generated locally; neither enters Git. Exact model ID, revision and GGUF quantization are packaging choices, not an automatically trusted "latest." The reference candidate is the official [Qwen3-0.6B-GGUF](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF) model, with [Qwen's GGUF and llama.cpp instructions](https://github.com/QwenLM/Qwen3/blob/main/docs/source/run_locally/llama.cpp.md). Select the precise file, source commit and SHA-256 through an independent release review. Nothing in this patch downloads anything, particularly during an offline boot. Retain license, provenance and attribution in the eventual installer.

## Read-only verification and optional manifest construction

From a clean model directory, after placing real GGUF bytes obtained by the operator and independently establishing their upstream revision and SHA-256:

```
python3 tools/qwen_media/model_package.py build <model-dir> \
  --file Qwen3-0.6B-Q4_K_M.gguf --model-id Qwen3-0.6B \
  --revision <40-hex-commit> --source Qwen/Qwen3-0.6B-GGUF \
  --expected-sha256 <trusted-model-file-sha256>
```

The command writes a canonical `MODEL_MANIFEST.json` **without replacing an existing manifest**, and prints its SHA-256. This is a locally generated consistency statement, not publisher authentication. Sign/pin the manifest digest in an independently trusted **OS release record** before any model-activation decision. To verify offline:

```
python3 tools/qwen_media/model_package.py verify <model-dir> \
  --expected-manifest-sha256 <digest-pinned-by-trusted-OS-release>
```

The verifier refuses missing or malformed pins, duplicate or extra JSON fields, noncanonical encodings, symlinked directory components/files, hard-linked files, device/FIFO inputs, truncated/unsupported GGUF headers, oversized data, and model/manifest mismatch. It streams model reads within bounded memory. Tests use tiny *synthetic* GGUF-header fixtures and do not instantiate a model or run inference:

```
cd tools/qwen_media
python3 -m unittest -v test_model_package.py
python3 -O -m unittest -v test_model_package.py
```

## Security and installation boundary

A model digest compares bytes only. A manifest's source field does not prove source identity. This tool does not establish a trusted boot, signed OS image, trusted partition, verified root mount, immutable file handles, malicious concurrent-edit resistance, race-proof ancestor confinement, or authenticated model execution. Actual GPT/LLM inference requires a separately tested Qwen-compatible engine and tokenizer/chat template, hardware-specific resource limits and a native PCP OS storage/runtime. Neither a successful verification nor an AI-generated explanation grants a tool permission. A trusted OS controller must bind the exact task, scoped one-use action, immutable input source and cancellation fence before any Tailorbird effect.

The external MCP **HTTP** OAuth authorization scheme is not a default authorization scheme for local `stdio` tools. For the local service, retain OS-process identity, least-privilege capability dispatch and explicit human confirmation for mutations; a model must never be able to approve its own edit. See the [MCP authorization specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization) for the transport distinction. **Offline** must be a testable absence of network capability, not merely an inactive network UI.

Existing PCP OS master presently has no data-volume mount/filesystem or Qwen inferencer. This source change does **not** turn the proposal into an installed or preloaded model. No kernel, user program, application or installed release is modified here.
