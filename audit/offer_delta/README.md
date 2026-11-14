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

## A worker that submits each accepted offer

Both the full and delta previews now check the complete next event against the ledger's 65,536-byte limit. The limit includes the envelope, sequence number, and guard-computed fields. It excludes the terminating newline. The register encodes non-ASCII text as JSON escapes, so even a request well below 65,536 UTF-8 bytes can exceed the limit. The same Rust encoder counts the bytes for a preview and emits them for an append. Counting requires no encoded copy of the event. On append, even a refused event sends at most 65,536 bytes to the temporary buffer. Parsing and guarding the request still use memory proportional to the input.

Two other checks also apply at preview time. A register whose history fails guard replay accepts only a halt. During an open review, the host directs a caller who offers a supplied artifact by digest alone to `acr_artifact`, where the host can read the bytes. Submission checks the current state again before writing.

Run `python measure_workflow.py BASELINE_PCPOS CANDIDATE_PCPOS` with the same tokenizer setup. Use the compact-preview binary at `1a30a7d` as the baseline for this comparison. Each worker discovers the tools once, previews every proposal in delta mode and submits each acceptance. Each starts with a copy of the same initialized register. The harness checks both persisted notes and every refusal stage.

| Worker workload | RPC calls, before → after | Both directions, bytes | cl100k tokens, approx. | o200k tokens, approx. |
|---|---:|---:|---:|---:|
| Two valid notes | 5 → 5 | 16,495 → 16,495 | 3.1k → 3.1k | 3.2k → 3.2k |
| Same notes, two oversized notes and a forbidden system event | 10 → 8 | 223,934 → 120,468 | 58.8k → 31.0k | 47.8k → 25.5k |

The mixed case saves 103,466 bytes by declining the two oversized proposals before the worker retransmits them. One contains English text and the other contains 33,000 UTF-8 bytes of Japanese text. Both still incur the initial preview cost. The forbidden system event is refused at preview in both versions.

Wire counts include the newline on each JSON-RPC request and reply. The model projection counts the catalog once, then the tool name, compact arguments and decoded response text for each call. Token totals vary slightly with the digests in write receipts. The workload deliberately exercises refusals. It establishes neither a production refusal rate nor a provider's billed token count.

## Writing a prepared offer

A worker that needs a decision between preview and append can upload the body once. On the same native MCP connection:

1. Call `acr_offer` with `kind`, `body` and `"prepare":true`. Supply `expected_head` when the decision depends on a previously read state. Acceptance returns the delta above plus a string `ref`. Reconstruct and inspect the accepted body.
2. To write it, call `acr_commit` with just `{"ref":"<returned ref>"}`. The host locks the original register, checks the head again, reruns the submission checks and returns the usual receipt. Read the register before retrying if the receipt was lost.

One successful prepare replaces the previous one on that connection. A refusal, an ordinary preview or an unknown reference leaves the slot alone. A matching commit attempt consumes it, including on a stale-head refusal. Disconnecting drops it. The stored canonical body fits within the 65,536-byte event limit, plus separate path, kind and digest metadata. The reference identifies the resolved path, fixed actor, head, kind and accepted body by SHA-256. Identical preparations can return the same reference. A recipient needs the matching preparation on its own connection to use it. Direct `acr_submit` remains the one-call write path when no preview decision is needed.

Run `python measure_commit.py BASELINE CANDIDATE` with tiktoken 0.14.0. Use `53ca4e6` as the standalone baseline. The worker starts with the published FPHPM exercise register and a registered participant, discovers the tools, obtains a full head, checks it on submission or preparation, and reads status after each append. The harness checks the register against the intended writes. To test conditional status, build both binaries with the verified header updates from #66 and pass `--updates`.

One run with #66 and master at `f9bd594` gave these results. The two preview workflows made the same number of calls. The mixed case writes a short note, a 2,560-byte English note text and a 3,072-byte Japanese note text, and refuses a system event.

| Workload | Resend RPC bytes | Prepared RPC bytes | Resend → prepared cl100k | Resend → prepared o200k |
|---|---:|---:|---:|---:|
| Short note | 9,804 | 10,092 | 1,986 → 2,112 | 2,026 → 2,153 |
| Long English note | 14,910 | 12,645 | 2,771 → 2,517 | 2,807 → 2,555 |
| Mixed, 12 calls | 23,617 | 18,596 | 6,109 → 4,622 | 5,388 → 4,285 |

The direct-submit control used 17,311 bytes for the mixed case in nine calls. The standalone mixed preview comparison with full status reads used 31,104 → 25,577 bytes in thirteen calls. In the combined build the serialized tool list grew from 6,948 to 7,124 bytes, within its 7 KiB budget. A small single offer can cost more with a reference. Wire counts include every request and reply newline. Token counts project names, compact arguments, decoded results and one tool list. Digest values vary between runs. Provider framing, billing and live model decisions are outside this measurement.
