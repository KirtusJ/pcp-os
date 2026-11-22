# Session C: independent public 16-bit SPN known-plaintext challenge

**Scope:** Read-only educational attack; no real secrets, production cipher, PCP OS runtime, formal independent ACR calibration, or certified security claim. The challenge input was supplied publicly. All hexadecimal words are 16-bit. Python 3.13.5, standard library only, local container. This helper is **not a PCP OS Rust instrument**.

## Fixture contract

- S-box `E4D12FB83A6C5907` maps input nibble `n` to hex digit at index `n`; apply on four nibbles, least significant first.
- Bit permutation: source `4*i+j` maps to destination `4*j+i`, for `i,j=0..3`.
- Round key `k_i = rotl16(master_key, 3*i) XOR ((i*0x9e37) & 0xffff)`.
- Rounds `i = 0..r-1`: `x <- P(S(x XOR k_i))`.
- Final whitening: `x <- x XOR k_r`.
- Public fixture master key `B7E1`; rounds `1,2,4,8`; plaintexts `0000,1234,ABCD,FFFF`.

## Known answers (ciphertexts in plaintext column order)

| Rounds | `0000` | `1234` | `ABCD` | `FFFF` |
| ---: | --- | --- | --- | --- |
| 1 | `E83A` | `5CED` | `0E3D` | `23FE` |
| 2 | `C555` | `588E` | `5C80` | `4E2D` |
| 4 | `907F` | `C530` | `437A` | `BF9A` |
| 8 | `7127` | `7537` | `1C9F` | `8CE8` |

## Key search using **only** the four known pairs

Each round count was tested against **all** 65,536 master keys. The enumerator uses a precomputed SP table, different from the independently coded reference encryptor and decryptor. Candidate checks stop after the first failing pair, without omitting any master key.

| Rounds | Keys matching `0000` pair | Keys matching first two pairs | Keys matching all four |
| ---: | --- | --- | --- |
| 1 | `7221, B7E1, DA6C` | `B7E1` | `B7E1` |
| 2 | `B7E1` | `B7E1` | `B7E1` |
| 4 | `5553, B7E1, B8D6` | `B7E1` | `B7E1` |
| 8 | `B7E1` | `B7E1` | `B7E1` |

All four pairs were considered for every round count, even when an earlier pair already gave a unique result. Exact counts after pairs 1, 2, 3, 4 are respectively `[3,1,1,1]`, `[1,1,1,1]`, `[3,1,1,1]`, and `[1,1,1,1]`. The attack actually examined 262,144 candidate keys across these four runs, requiring 262,160 pair-encryptions with early rejection.

## Execution receipt and limits

Command:

```sh
python3 -B audit/calibration/session_c_public_cipher_challenge.py
```

Actual results: full SP-table comparison over **65,536/65,536 states** and **100/100** reference-vs-attack cipher/decryptor comparisons passed. The challenge produced the ciphertext and survivor counts above. The Python file on this PR has Git blob SHA `725e4ae815eeaec26c73878b7c29eabd7ab8d123` and local SHA-256 `4552b6318b3981b3c663a75483f11711424f50c7840d95099a500c50e96062ed`. The SHA is a file-identity check, not a security seal or external authorship attestation.

**Finding:** Within the stated computational budget and these four public pairs, increasing from 1 to 8 rounds did **not** prevent unique recovery of the public fixture key. Eight rounds cost more work **per candidate**, but the master-key search space remains 2^16. A different true key or key-collision pattern might give a different number of compatible keys for a chosen set of four pairs. This finite search does not establish resistance to differential cryptanalysis, chosen-plaintext attacks, related keys, implementation side channels, authenticated encryption, or any production security property. The known public fixture key must not be confused with a protected key.

The canonical production comparison is a standardized **authenticated** construction, such as AES-GCM in [NIST SP 800-38D](https://csrc.nist.gov/pubs/sp/800/38/d/final), with security-strength guidance in [NIST SP 800-57 Part 1 Rev. 5](https://csrc.nist.gov/pubs/sp/800/57/pt1/r5/final). Their existence does not validate an implementation or this toy cipher.
