# R1 | A compact record form to replace JSON | Draft, not adopted

**Status:** first design step. The record profile in force is still the JSON form (ACR-OS-RECORD-1). Nothing below is final, and the question of what a person or a model should be shown is only opened here.

## 1. Where the bytes of the JSON form go

Measured on the register of the research-baseline exercise (13 events, 6,027 bytes), by walking every value:

| Share | Bytes | What |
| --- | --- | --- |
| 31.1% | 1,875 | Field names, quoted, repeated in every event |
| 28.5% | 1,716 | Digests written as 64 hexadecimal characters (each is 32 bytes of information) |
| 26.4% | 1,590 | All other values: the actual content |
| 9.3% | 560 | Braces, brackets, commas, colons, newlines |
| 4.7% | 286 | Timestamps as 20 characters of text |

About a quarter of the record is content. One register is a small sample; the shares will differ where events carry long text.

Size is the lesser cost. The greater ones:

- **A JSON parser is a large thing to trust.** Parsers disagree about duplicate keys, number ranges, escapes and trailing data; the Python implementation and the Rust core each had to add strictness that standard parsers lack. A kernel should not carry such a parser at all.
- **Canonical form is bolted on.** JSON has no single byte form; the hash chain depends on a convention (sorted keys, no spaces, ASCII escapes) that every implementation must reproduce exactly. The first cross-implementation test in this workspace failed on one escape rule.
- **No bytes.** Digests, and any binary content, must be text.

## 2. Requirements

1. One value, one byte form, by construction and not by convention.
2. A decoder that is small, single-pass, bounded in depth and memory, and refuses everything outside the profile.
3. Bytes as a first-class type.
4. Readable by existing tools, so that a record does not depend on this program to be inspected in twenty years.
5. A lossless mapping from and to the present JSON form, so that existing records can be carried over and checked.
6. A stated rule for change: a field number, once assigned, is never moved or reused.

## 3. Candidates against external practice

| Candidate | Canonical form | Decoder size | Existing tools | Assessment |
| --- | --- | --- | --- | --- |
| Deterministically encoded CBOR, restricted (RFC 8949 section 4.2) | Yes, specified | Small | Many, in every language | **Chosen for the draft.** A profile of a standard, not a new format |
| ASN.1 DER | Yes | Moderate; a long history of parser defects | Many | Heavier than needed |
| Protocol Buffers | No: serialization is explicitly not canonical | Moderate | Many | Fails requirement 1 |
| MessagePack | Not specified | Small | Many | Fails requirement 1 without a private convention |
| Bencode | Yes | Tiny | Few | No nulls or booleans; integers as text |
| A new format | Whatever is designed | Whatever is written | None | Fails requirement 4; every new format repeats old parser mistakes |

Designing a wholly new encoding was considered and set aside: the gain over a restricted CBOR profile would be a few bytes, and the cost is requirement 4. *[CBOR and its deterministic encoding are cited from recall of RFC 8949 and were not re-inspected this session; the profile's output was decoded by an independent CBOR implementation, `cbor2`, which reads it.]*

## 4. The draft profile

- Definite lengths only; integer heads in shortest form; no floating point; no tags; map keys in bytewise order of their encodings; text is UTF-8.
- A digest is 32 bytes, never text. Text of 64 lower-case hexadecimal characters is not in the profile, so a digest has one form.
- A field name in the table (`r1::KEYS`, 71 entries so far) is its number, never text. A name in the table sent as text is not in the profile.
- An event is a positional array `[seq, prev, seconds, actor, authority, kind, body]`; the time is an integer of seconds. Positional export requires a real proleptic Gregorian date in the four-digit UTC spelling with seconds 00–59. The JSON register admits the fixed timestamp spelling even for some dates without a calendar interpretation. Such a register can be verified and encoded as an R1 map. A positional export reports the first unrepresentable event and leaves the destination untouched.

`crates/pcpos-core/src/r1.rs` implements the encoder and a decoder that refuses anything outside the profile. Both are exercised in the test suite and inside the kernel at start.

## 5. First measurements

| Register | JSON form | R1, values as maps | R1, events positional |
| --- | --- | --- | --- |
| Reference vector, 4 events | 891 bytes | 552 (61%) | 340 (38%) |
| Research-baseline exercise, 13 events | 6,027 bytes | 3,651 (60%) | 2,962 (49%) |

Every event of both registers decodes to exactly the value encoded.

### Transmitting an existing register

A sender can run `pcpos-verify register.jsonl --r1 transfer.r1` and give the recipient the full canonical register head through an authenticated or independently trusted channel. The compact file is a sequence of self-delimiting positional events. A receiver can restore the canonical JSONL bytes with the Rust host:

```sh
pcpos-verify --from-r1 transfer.r1 "$expected_register_head" > received.jsonl
```

The receiver decodes the positional fields, checks each original JSON predecessor and sequence number, and compares the final JSON head with the supplied head before emitting any bytes. It returns 6 for a bad frame, broken chain or head mismatch and 2 for an invalid head argument or I/O error. Shell redirection opens the destination before the program runs. Use a fresh destination. This command verifies record integrity. It does not replay ACR guards or authenticate the source of the supplied head.

The 82-event scenario travels in 12,787 bytes and recovers all 28,651 JSONL bytes, including the final newline. A full lowercase-hex head adds 64 bytes if sent separately, before transport framing. With this transport the sender and receiver can pass compact bytes between programs without supplying the register to a model. This measures wire bytes only. A model-facing adapter still determines which fields become tokens.

### Updating a receiver that already holds a snapshot

The receiver gives the sender the full head of its existing canonical JSONL snapshot. The sender verifies the source and exports only the R1 events following that head. The receiver verifies its local snapshot, extends it with the received frames and checks the independently obtained final head before writing the complete JSONL to stdout. Use a fresh output path because shell redirection opens it before verification.

```sh
# Sender, using the checkpoint reported by the receiver
pcpos-verify source.jsonl --r1 changes.r1 --after "$base_head"
# Receiver, using its existing snapshot and the expected final head
pcpos-verify --from-r1 changes.r1 "$expected_register_head" --base local.jsonl > next.jsonl
```

A head missing from the source cannot identify a suffix. A receiver at the final head needs zero event frames. A zero-event transfer still requires the receiver to verify its local snapshot and match the expected final head. The 64-zero genesis head requests the full stream. Only transmitted events need a timestamp representable in positional R1.

| Events already held in the 82-event scenario | R1 event bytes sent | SHA-256 head bytes if both heads travel as lowercase hex |
| ---: | ---: | ---: |
| 41 | 6,036 | 128 |
| 81 | 105 | 128 |
| 82 | 0 | 128 |

For comparison, sending the full stream and one final head costs 12,851 bytes before transport framing. These counts describe the transfer between programs. The sender still verifies the full source, and the receiver reads the local prefix and outputs the full reconstructed register. To see whether the ACR guards accept that history, use `pcpos verify next.jsonl` separately.

## 6. Open, in order of importance

1. **Two forms, one record.** What is hashed has to be the compact form. What a person or a model reads should be a rendering generated from it, never a second source. The rendering for models is a separate design problem: its cost is counted in tokens, not bytes, and no tokenizer was measured here.
2. **The chain changes.** Hashing R1 bytes gives different head digests. Carrying existing registers over needs a recorded bridge event that binds the old head to the new chain.
3. **The key table is a schema, and a schema needs governance:** who assigns numbers, how a reader treats an unknown number, how the table's own identity is pinned in the register.
4. **Kind and enumerated values** (dispositions, gates, origins) are still text. Making them numbers saves more and risks ambiguity where the same field can hold free text.
5. **Whether `kind`-specific bodies should be positional** too. That is where most of the remaining weight is.
6. **Independent review of the decoder**, and fuzzing, before it is trusted with anything.
