# Twin Rook v1 · candidate review

**Status.** Educational candidate. The claim that composing the two stages requires enumerating the full key-pair space was refuted on the fixture below. Preserve it as a regression case for future proposals.

**Construction.** Split a 16-bit block into two bytes, most significant first. In each of four rounds numbered 0 through 3, set `k = rotl8(stage_key,i) XOR ((73*i) mod 256)`. Set `f(r) = (rotl8(((r XOR k)*157) mod 256,3) + k) mod 256`. Replace `(L,R)` by `(R,L XOR f(R))`. Encrypt the block under stage key A and then under independent stage key B. Each key ranges from zero through `2^bits-1`. The fixture generator is deterministic; disclosing its seed gives a solver an additional shortcut.

**Adversarial question.** Can a worker given three public pairs recover the unique 8-bit key pair using fewer stage evaluations than exhaustive search? Success requires exact recovery, a unique surviving pair and an independently reproducible work count under the stated accounting rule.

| Check | Public pairs | Survivors | Stage evaluations |
| --- | --- | ---: | ---: |
| Exhaustive | ee0e:6346, fc3a:8e9a, 45c3:0338 | 1 | 131082 |
| Meet-in-the-middle | same three | 1 | 522 |
| Meet-in-the-middle, contradictory control | ee0e:6346, ee0e:6347, 45c3:0338 | 0 | 520 |

The two attacks recovered decimal keys 57 and 194. A forward table of 256 entries reduces stage evaluations in exchange for memory and lookup work. The cipher has no authentication operation. These measurements answer one known-plaintext experiment. They support no claim about protecting real messages.

**Refinement.** Keep this definition and its counterexample with their release. Give each changed construction a new revision and identify its parent, changed assumption, attacker's knowledge, exact rejection criterion and prior distinguishing fixtures. Repeat the positive, adverse and contradictory cases. Preserve failures and ambiguous results. Advance a candidate only on its measured claims. Record the independent challenger and register evidence when available. This report supplies evidence for ACR; it does not assert an independently closed register review.
