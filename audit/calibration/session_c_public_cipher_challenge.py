#!/usr/bin/env python3
"""Session C: independent known-plaintext key search against a PUBLIC toy 16-bit SPN.

No real keys or protected inputs. Python 3, standard library only.
Source convention: SBOX string index n is the output nibble for nibble n;
P maps source bit (4*i+j) to destination bit (4*j+i).
Rounds 0..r-1 apply P(S(state XOR round_key(i))); finally XOR round_key(r).
"""
from __future__ import annotations

import hashlib
import json
import time
from array import array

MASK = 0xFFFF
SBOX = tuple(int(c, 16) for c in "E4D12FB83A6C5907")
INV_SBOX = tuple(SBOX.index(i) for i in range(16))
PLAINTEXTS = (0x0000, 0x1234, 0xABCD, 0xFFFF)
PUBLIC_FIXTURE_KEY = 0xB7E1
ROUND_COUNTS = (1, 2, 4, 8)
# Fixed public ciphertexts, computed once by the reference oracle, independently
# checked before any attack. The recovery loop receives ONLY these pairs.
KNOWN_CIPHERTEXTS = {
    1: (0xE83A, 0x5CED, 0x0E3D, 0x23FE),
    2: (0xC555, 0x588E, 0x5C80, 0x4E2D),
    4: (0x907F, 0xC530, 0x437A, 0xBF9A),
    8: (0x7127, 0x7537, 0x1C9F, 0x8CE8),
}


def rotl16(x: int, shift: int) -> int:
    n = shift % 16
    return ((x << n) | (x >> ((16 - n) % 16))) & MASK if n else x & MASK


def round_key(key: int, index: int) -> int:
    return rotl16(key, 3 * index) ^ ((index * 0x9E37) & MASK)


def substitute_reference(x: int, table: tuple[int, ...] = SBOX) -> int:
    return sum(table[(x >> (4 * i)) & 15] << (4 * i) for i in range(4))


def permute_reference(x: int) -> int:
    result = 0
    for i in range(4):
        for j in range(4):
            bit = (x >> (4 * i + j)) & 1
            result |= bit << (4 * j + i)
    return result


def encrypt_reference(plaintext: int, key: int, rounds: int) -> int:
    state = plaintext
    for i in range(rounds):
        state = permute_reference(substitute_reference(state ^ round_key(key, i)))
    return state ^ round_key(key, rounds)


def decrypt_reference(ciphertext: int, key: int, rounds: int) -> int:
    state = ciphertext ^ round_key(key, rounds)
    for i in range(rounds - 1, -1, -1):
        state = substitute_reference(permute_reference(state), INV_SBOX) ^ round_key(key, i)
    return state


# Attack path intentionally uses a *different implementation* from the KAT oracle:
# precompute each nibble's SP contribution, then make a full 65536-entry SP table.
def build_sp_table() -> array:
    nibble_contrib = [[0] * 16 for _ in range(4)]
    for i in range(4):
        for nibble in range(16):
            val = SBOX[nibble]
            for j in range(4):
                if (val >> j) & 1:
                    nibble_contrib[i][nibble] |= 1 << (4 * j + i)
    return array("H", (
        nibble_contrib[0][x & 15]
        | nibble_contrib[1][(x >> 4) & 15]
        | nibble_contrib[2][(x >> 8) & 15]
        | nibble_contrib[3][(x >> 12) & 15]
        for x in range(1 << 16)
    ))


def encrypt_attack_path(plaintext: int, schedule: tuple[int, ...], sp: array) -> int:
    state = plaintext
    for k in schedule[:-1]:
        state = sp[state ^ k]
    return state ^ schedule[-1]


def check_against_reference(sp: array) -> int:
    assert len(SBOX) == len(set(SBOX)) == 16
    assert tuple(INV_SBOX[v] for v in SBOX) == tuple(range(16))
    # The transpose-permutation is its own inverse.
    for x in range(1 << 16):
        assert permute_reference(permute_reference(x)) == x
        assert sp[x] == permute_reference(substitute_reference(x))
    checked = 0
    sample_keys = (0x0000, 0x0001, 0x1234, PUBLIC_FIXTURE_KEY, 0xFFFF)
    sample_plaintexts = (0x0000, 0x1234, 0xABCD, 0xFFFF, 0x5A5A)
    for rounds in ROUND_COUNTS:
        for key in sample_keys:
            schedule = tuple(round_key(key, i) for i in range(rounds + 1))
            for pt in sample_plaintexts:
                c1 = encrypt_reference(pt, key, rounds)
                c2 = encrypt_attack_path(pt, schedule, sp)
                assert c1 == c2, (rounds, key, pt, c1, c2)
                assert decrypt_reference(c1, key, rounds) == pt
                checked += 1
    return checked


def recover_keys(pairs: tuple[tuple[int, int], ...], rounds: int, sp: array) -> dict:
    survivors: list[int] = []
    cumulative = [0] * len(pairs)
    # Each of 65536 keys is genuinely attempted; early rejection saves computations.
    evaluations = 0
    start = time.perf_counter()
    for key in range(1 << 16):
        schedule = tuple(round_key(key, i) for i in range(rounds + 1))
        match = True
        for j, (pt, ct) in enumerate(pairs):
            evaluations += 1
            if encrypt_attack_path(pt, schedule, sp) != ct:
                match = False
                break
            cumulative[j] += 1
        if match:
            survivors.append(key)
    return {
        "rounds": rounds,
        "known_pairs": [[f"{pt:04X}", f"{ct:04X}"] for pt, ct in pairs],
        "surviving_keys": [f"{key:04X}" for key in survivors],
        "surviving_counts_after_1_to_4_pairs": cumulative,
        "candidate_keys_examined": 1 << 16,
        "encryptions_evaluated": evaluations,
        "elapsed_seconds_local": round(time.perf_counter() - start, 6),
    }


def main() -> None:
    sp = build_sp_table()
    controls = check_against_reference(sp)
    results = []
    for r in ROUND_COUNTS:
        # Oracle check is a separate fixture validation, NOT the attack input.
        for pt, ciphertext in zip(PLAINTEXTS, KNOWN_CIPHERTEXTS[r]):
            assert encrypt_reference(pt, PUBLIC_FIXTURE_KEY, r) == ciphertext
            assert decrypt_reference(ciphertext, PUBLIC_FIXTURE_KEY, r) == pt
        pairs = tuple(zip(PLAINTEXTS, KNOWN_CIPHERTEXTS[r]))
        result = recover_keys(pairs, r, sp)
        assert f"{PUBLIC_FIXTURE_KEY:04X}" in result["surviving_keys"]
        assert result["surviving_counts_after_1_to_4_pairs"][-1] == len(result["surviving_keys"])
        assert all(a >= b for a, b in zip(result["surviving_counts_after_1_to_4_pairs"], result["surviving_counts_after_1_to_4_pairs"][1:]))
        results.append(result)
    report = {
        "cipher_spec": "S=E4D12FB83A6C5907 low-to-high nibbles; P bit 4*i+j to 4*j+i; ki=rotl16(key,3*i) XOR ((i*0x9E37) mod 65536); round P(S(x XOR ki)); final XOR k_rounds",
        "plaintexts": [f"{p:04X}" for p in PLAINTEXTS],
        "public_fixture_key": f"{PUBLIC_FIXTURE_KEY:04X}",
        "reference_vs_attack_cross_checks": controls,
        "full_state_sp_table_checks": len(sp),
        "results": results,
    }
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
