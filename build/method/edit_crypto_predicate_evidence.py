"""Apply the cryptography Section 4 revision to an extracted build-22 method packet."""

import argparse
import hashlib
from pathlib import Path


SPEC = Path("MODULES/05_Cryptography/SOURCE/Specification.md")
PREDECESSOR = "601502fa38d246c8c84a64ad5afd410023d3256f38ba56a1c3bc84683691efc7"
ANCHOR = b"Functional correctness, memory safety, timing behavior and physical leakage are distinct properties."
ADDITION = (
    "When several checks can reject an adverse input, evidence attributing rejection to a "
    "particular check MUST distinguish it from the other possible causes. A matched input "
    "pair, an observed execution path or a mutation that disables the check can supply "
    "that distinction. If the evidence shows only that the input was rejected, retain that "
    "narrower finding.\n\n"
    "**EXAMPLE:** FIPS 203 Section 7.2 checks an encapsulation key's size and coefficient "
    "encoding [C13]. For ML-KEM-512, keep a public test key at 800 bytes and change one "
    "packed 12-bit coefficient from 3328 to 3329, preserving its neighbor and the seed. "
    "The first key passes these input checks and the second fails the modulus check. "
    "Truncating the first key by one byte separately tests size. An implementation checking "
    "size alone would accept the same-length noncanonical key. Inspecting these test inputs "
    "establishes their coverage. A finding about a provider requires its observed results.\n\n"
).encode("utf-8")


def edit(packet):
    source = packet / SPEC
    original = source.read_bytes()
    if hashlib.sha256(original).hexdigest() != PREDECESSOR or original.count(ANCHOR) != 1:
        raise ValueError("Expected the build-22 cryptography specification (SHA-256 " + PREDECESSOR + ")")
    revised = original.replace(ANCHOR, ADDITION + ANCHOR, 1)
    source.write_bytes(revised)
    return hashlib.sha256(revised).hexdigest()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", type=Path)
    print(edit(parser.parse_args().packet))
