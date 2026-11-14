# Memory used to receive one more R1 event

PR #49 adds transfers after a verified checkpoint. Its receiver reads the local canonical prefix, builds a vector containing every parsed event, copies the raw prefix into its ledger and then processes the received frames. The change measured here discards each parsed prefix event after checking it and transfers ownership of the input byte buffer to the ledger. It still reads and hashes the whole local prefix on every call and retains the full restored JSONL for output.

Run `python3.11 measure.py BASELINE_PCPOS_VERIFY CANDIDATE_PCPOS_VERIFY` on Linux, with release builds from `7e6442c` and this branch. The benchmark generates canonical synthetic NOTE events, sends one further event per register and checks the digest of every restored file. It measures the receiver's own Linux VmHWM after verification, while the output pipe holds it open. Each figure is the median of three processes per build, run in alternating order.

| Saved events | Local JSONL bytes | R1 update bytes | Baseline peak KiB | Changed peak KiB |
|---:|---:|---:|---:|---:|
| 1,000 | 201,780 | 76 | 3,352 | 1,256 |
| 10,000 | 2,037,780 | 76 | 24,088 | 3,044 |
| 50,000 | 10,277,780 | 76 | 116,380 | 11,096 |

Wire bytes count the positional frames. Exchanging the base and target heads adds 128 bytes if both are sent as lowercase hexadecimal. Linux process memory depends on the allocator, record contents and platform. These measurements cover local verification and restoration. The procedure involves no model call.

## Sending one more event

An incremental sender can verify the source in a single forward pass. It keeps the current line and the positional frames after the checkpoint, discarding earlier decoded events. Memory therefore depends on the largest event and the outgoing update. A sender that must send most of the history can still accumulate a large update. The destination is opened after the entire source passes verification.

Run `python3.11 measure_sender.py BASELINE_PCPOS_VERIFY CANDIDATE_PCPOS_VERIFY` on Linux with release binaries from `8d4b5af` and this revision. The control uses the same source and checkpoint. The benchmark checks byte equality of the outgoing frames and reconstructs each source with the receiver. For each binary it measures Linux VmHWM through the first verified-head status line, before writing the destination. A filled pipe keeps the process alive at that point. The measurement requires access to the child's /proc status and wait channel.

| Events already held | Source bytes including one new event | R1 update bytes | Previous sender KiB | Streaming sender KiB |
|---:|---:|---:|---:|---:|
| 1,000 | 201,984 | 76 | 3,164 | 1,060 |
| 10,000 | 2,037,986 | 76 | 22,112 | 1,064 |
| 50,000 | 10,277,986 | 76 | 106,412 | 1,064 |

Values are medians of three runs per binary. The CPU still reads and checks every source line. As in the receiver benchmark, the two hexadecimal heads add 128 bytes if exchanged. No model receives the prefix in this experiment.
