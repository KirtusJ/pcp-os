"""Inspect two public NIST ACVP fixtures and isolate the FIPS 203 Section 7.2 checks.

Supply local copies of the two pinned internalProjection.json files documented in
crypto_predicate_evidence.md. An explicit --openssl path also runs a provider check.
"""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


FIXTURE_DIGESTS = (
    "8fe8b0ade31d8f5754e909599574eb9f2d60932d95a39bc479b2b48e38f65916",
    "a556952ce869bb89c3a3196a701dad89647c193a34c86eafb61a9d710d5b810f",
)


def coefficients(key):
    for start in range(0, 768, 3):
        a, b, c = key[start:start + 3]
        yield a | ((b & 15) << 8)
        yield (b >> 4) | (c << 4)


def accepts(key, check_modulus):
    if len(key) != 800:
        return False
    return not check_modulus or all(v < 3329 for v in coefficients(key))


def inspect(path, expected_hash, expected_shape):
    payload = path.read_bytes()
    if hashlib.sha256(payload).hexdigest() != expected_hash:
        raise ValueError("Unexpected fixture digest: " + str(path))
    group, = (g for g in json.loads(payload)["testGroups"]
              if g.get("function") == "encapsulationKeyCheck" and g["parameterSet"] == "ML-KEM-512")
    negative = [bytes.fromhex(t["ek"]) for t in group["tests"] if not t["testPassed"]]
    valid = next(bytes.fromhex(t["ek"]) for t in group["tests"] if t["testPassed"])
    observations = [
        (len(key), sum(v >= 3329 for v in coefficients(key)), accepts(key, False))
        for key in negative
    ]
    if observations != [expected_shape] * 5 or not accepts(valid, True):
        raise AssertionError((path, observations))
    print(path.name, "negative cases:", len(negative), "shape:", expected_shape)
    return valid


def check_pair(valid):
    def change_first(value):
        return bytes((value & 255, (valid[1] & 0xF0) | (value >> 8))) + valid[2:]

    inside, outside = change_first(3328), change_first(3329)
    if list(coefficients(inside))[1:] != list(coefficients(outside))[1:] or inside[768:] != outside[768:]:
        raise AssertionError("Unintended coefficient or seed change")
    cases = (inside, outside, inside[:-1])
    full = [accepts(key, True) for key in cases]
    size_only = [accepts(key, False) for key in cases]
    if full != [True, False, False] or size_only != [True, True, False]:
        raise AssertionError((full, size_only))
    print("Fixture predicate:", full, "size-only mutation:", size_only)
    return cases


def der(tag, data):
    size = len(data)
    if size < 128:
        length = bytes((size,))
    else:
        octets = size.to_bytes((size.bit_length() + 7) // 8, "big")
        length = bytes((128 + len(octets),)) + octets
    return bytes((tag,)) + length + data


def provider_check(openssl, original, cases):
    command = [str(openssl), "pkey", "-provider", "default", "-pubin", "-inform", "DER", "-pubcheck", "-noout"]
    version = subprocess.check_output([str(openssl), "version"], text=True).strip()
    print(version, "default provider, public-key input check")
    # RFC 9935 ML-KEM-512 AlgorithmIdentifier, with absent parameters.
    algorithm = bytes.fromhex("300b0609608648016503040401")
    statuses = []
    for key in (original, *cases):
        spki = der(0x30, algorithm + der(0x03, b"\x00" + key))
        result = subprocess.run(command, input=spki, capture_output=True)
        statuses.append(result.returncode)
    print("Provider status (published valid, 3328, 3329, truncated):", statuses)
    if [code == 0 for code in statuses] != [True, True, False, False]:
        raise AssertionError(statuses)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("historical", type=Path)
    parser.add_argument("corrected", type=Path)
    parser.add_argument("--openssl", type=Path, help="also exercise this OpenSSL 3.5+ executable")
    args = parser.parse_args()
    inspect(args.historical, FIXTURE_DIGESTS[0], (1216, 0, False))
    original = inspect(args.corrected, FIXTURE_DIGESTS[1], (800, 1, True))
    cases = check_pair(original)
    if args.openssl:
        provider_check(args.openssl, original, cases)
