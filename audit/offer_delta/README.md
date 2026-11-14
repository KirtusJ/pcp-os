# Cost of an event preview

The native kernel takes numeric syscall arguments. The hosted Rust MCP server is a separate boundary where a worker asking for a preview already has the proposed event body. Passing `"delta":true` to `acr_offer` returns a JSON object in the MCP text content on acceptance. Apply `set` as whole top-level replacements and delete keys listed in `remove`. Omitted operations are empty. A null value in `set` is a value. The base is the exact body of that request, including any field the caller attempted to supply as provenance.

A second program or agent can use the patch if it retains that same body and correlates the reply by RPC ID. A recipient lacking the base needs the full accepted body. A preview refers to the state checked during that call. Submission runs the guards again. Omitting the option returns the existing human-readable reply. Refusals carry their existing reason and clause.

## Reproduction

Run `python measure.py BASELINE_PCPOS CANDIDATE_PCPOS` with tiktoken 0.14.0 and its cl100k_base and o200k_base vocabularies. Both registers start from the same published workspace event. The benchmark reconstructs all accepted bodies and compares them with the baseline, then compares the refusal verbatim.

| Case | Bidirectional RPC bytes, full → delta | cl100k tokens, full → delta | o200k tokens, full → delta |
|---|---:|---:|---:|
| Tool catalog | 9,808 → 10,035 | 2,065 → 2,111 | 2,098 → 2,145 |
| Short note | 271 → 246 | 24 → 20 | 25 → 21 |
| 2,624-byte note text | 5,515 → 2,868 | 791 → 404 | 792 → 404 |
| Guard adds participant ID and principal flag | 432 → 391 | 75 → 67 | 76 → 68 |
| Refused system event | 354 → 367 | 42 → 46 | 42 → 46 |

Bytes include both complete newline-terminated JSON-RPC messages. Offer token counts cover serialized arguments and decoded result text. Catalog tokens count the serialized tool list. Provider-specific chat framing, hidden prompts, cache discounts, retries and inference are outside these counts. One long note recovers the measured discovery overhead. A client that redelivers the catalog on every short call spends more overall. These fixtures measure exact transmission and decoding costs. They contain no live model decision trial.
