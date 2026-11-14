# Cost of reading register status

`acr_status` now accepts `{"view":"attention"}` for questions such as “What needs my response?” The result includes the register label, event count and full head, followed by every attention item, priority and prescribed response. The host verifies the register and replays the guards before rendering this view. Use the default full status for a review, a task inventory, the scope of a halt, or after losing a previously held full view.

For a repeat check, a caller holding a full view can pass its full digest as `if_head`. The server reopens the named register under its shared read lock on every poll. If the label and caller's digest match the last full view in the same MCP session, the server compares the current bytes with the verified copy. An exact match returns `UNCHANGED`. A changed copy goes through verification and guard replay. A new session or a caller omitting the digest receives a full view. An attention request leaves the full-view basis intact and takes no `if_head`.

One register per session can retain up to 1 MiB of verified bytes. Larger registers undergo full verification and replay on each poll. The bound covers the retained snapshot. The live read, parsed events and replay state require additional transient memory.

## Model-facing payloads

Both release hosts read the same three published registers. The baseline is the integrated host at `3c0e596ec4d5ade272d2910d89435109b66d6c6a`. Each pair below compares the baseline full status with the candidate attention view for the same question. Counts include compact argument JSON and the complete returned text as separately encoded fields.

| Published register | Attention items | UTF-8 bytes | o200k_base tokens | cl100k_base tokens | o200k_base with one discovery | cl100k_base with one discovery |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Reference, 4 events | 1, covering two guard problems | 1,068 → 349 | 278 → 109 | 279 → 109 | 1,608 → 1,465 | 1,574 → 1,429 |
| FPHPM exercise, 13 events | 0 | 1,440 → 173 | 367 → 67 | 368 → 67 | 1,697 → 1,423 | 1,663 → 1,387 |
| Scenario, 82 events | 2 | 2,881 → 427 | 738 → 126 | 739 → 127 | 2,068 → 1,482 | 2,034 → 1,447 |

Discovery adds one complete tool catalog for the corresponding build. The new selector adds 123 bytes, 26 o200k_base tokens or 25 cl100k_base tokens to that catalog, including for callers who never request the view. Counts exclude JSON-RPC envelopes, instructions loaded by the client, provider formatting, caching and model output. No model was invoked. The audit checks that the default full response matches the baseline and that the projected header, warnings and responses match the corresponding full text byte for byte.

## Host CPU

Each trial starts a native Linux release host, initializes one MCP session, requests full status once, then sends 100 matching-head polls. Times are median child user plus system CPU in milliseconds for the entire batch. The files were warm in the OS cache. Both binaries were pinned to the same CPU with two warm-up pairs and twelve measured pairs, alternating order. All replies were checked against the expected text.

| Published register | JSONL bytes | Baseline ms | Candidate ms | Candidate faster |
| --- | ---: | ---: | ---: | ---: |
| Reference | 891 | 5.049 | 2.719 | 12/12 pairs |
| FPHPM exercise | 6,027 | 25.198 | 3.115 | 12/12 pairs |
| Scenario | 28,651 | 107.923 | 4.916 | 12/12 pairs |

A control requesting the full 82-event status every time took 110.137 ms on the baseline and 113.479 ms on the candidate. The repeat-poll gain applies when the caller keeps its previous full view and the register bytes match. Every poll still reads the file in full.

## Reproduction

Build the baseline and candidate in separate checkouts with the same Rust toolchain. The run above used Rust 1.97.0 on Linux x86-64. From each checkout run `cargo build --locked --release --manifest-path PCP_OS_Rust/Cargo.toml --bin pcpos`. From the candidate root, use the two absolute executable paths and a CPU allowed by the host.

```sh
python3 audit/context/status_payload.py "$BASELINE_PCPOS" "$CANDIDATE_PCPOS" --tokens
taskset -c "$CPU" python3 audit/context/status_cpu.py "$BASELINE_PCPOS" "$CANDIDATE_PCPOS" --pairs 12
taskset -c "$CPU" python3 audit/context/status_cpu.py "$BASELINE_PCPOS" "$CANDIDATE_PCPOS" --pairs 12 --full
```

Token counts used tiktoken 0.14.0 with locally cached o200k_base and cl100k_base encodings. Omit `--tokens` to reproduce the byte counts with the Python standard library. Use `--register PATH` with the CPU script to measure a different workload.

## Reading a register that keeps changing

A worker that retains a full status can poll `acr_status` with `{"view":"update","if_head":"<full SHA-256>"}`. Three replies are possible.

| Reply in the MCP text content | Client action |
|---|---|
| `UNCHANGED` | Keep the complete view and its head. |
| JSON pair `[count,"new head"]` | Replace the event count and head in the register header. Keep every byte of the previously rendered status body. Use the new head on the next poll. |
| Full status | Replace the complete view, including its head. |

The pair describes the displayed status. New ledger events may contain evidence or notes that the status does not display. A consumer of those events still needs the register. A client that lost its view requests a full status without `if_head`.

Each changed register goes through chain verification and guard replay. The server hashes the complete rendered body after the header. It sends the pair only when the caller supplies the basis held for that register in the same session, the new chain extends that basis at its original event count, and the body digest matches. A missed reply, fork, rollback, register switch or changed body yields a full status. A broken chain produces an error. The session retains one 32-byte body digest and one event count in addition to the existing cache. The default full response and attention view retain their existing formats.

### Measured on published registers

Run `python3 audit/context/status_updates.py BASELINE_PCPOS CANDIDATE_PCPOS --tokens` against the release host at `54b5903` and this revision. Omit `--tokens` to measure bytes without a tokenizer. Each persistent client discovers all tools once and reads the same file after each append. The harness reconstructs every candidate status and compares it byte-for-byte with the full response of the parent binary. The six-read control begins at event 77 of the scenario and changes the displayed system state on each subsequent read.

| Register and read count | Header updates | Bidirectional JSON-RPC bytes, parent → candidate | o200k_base tokens | cl100k_base tokens |
|---|---:|---:|---:|---:|
| Published scenario, 82 | 19 | 181,294 → 159,277 | 44,411 → 39,098 | 44,416 → 39,084 |
| FPHPM exercise, 13 | 5 | 24,602 → 20,527 | 5,639 → 4,672 | 5,618 → 4,646 |
| Reference register, 4 | 1 | 11,866 → 10,969 | 2,563 → 2,351 | 2,531 → 2,318 |
| Scenario events 77–82, 6 | 0 | 25,217 → 25,356 | 5,897 → 5,933 | 5,865 → 5,901 |

Wire totals include request and response framing and line terminators. Token totals count the tool catalog once and the tool name, compact arguments and decoded response text separately for each call. They exclude provider chat framing, hidden prompts, billing and inference. The selector costs bytes on every call. Its savings depend on how often append operations leave the displayed body intact.

If the host also loads the generated Enterprise review instructions once, add 36 tokens to the candidate relative to the parent under either tokenizer. Those instructions grew by 151 UTF-8 bytes. On the six-read control, discovery, the selector and that one instruction load together cost 72 extra tokens.

This change aims at transmission. Changed registers still undergo the existing full read, verification, replay and rendering. A SHA-256 of the rendered body is computed when a full status basis is established or refreshed. A matching poll that reuses verified bytes skips this work. This benchmark measures neither model inference nor a CPU speedup.
