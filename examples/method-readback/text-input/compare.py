"""Compare a successful text-input guest transcript with raw upstream oracle stdout."""
import argparse
import math
from pathlib import Path
import re
import struct


SENTENCES = (b"Any participant MAY halt its own participation.",
             b"A Review Session is a bounded assessment cycle.")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def scores(lines):
    require(len(lines) == 32 and all(re.fullmatch(r"LOGITS(?: [0-9a-f]{8}){16}", line) for line in lines),
            "expected all 512 hexadecimal float32 scores")
    bits = "".join("".join(line.split()[1:]) for line in lines)
    values = struct.unpack(">512f", bytes.fromhex(bits))
    require(all(math.isfinite(value) for value in values), "nonfinite next-token score")
    return values, bits


def maximum_error(a, b):
    return max(abs(x - y) for x, y in zip(a, b, strict=True))


def compare(serial, reference, excerpt):
    lines = serial.splitlines()
    launches = [i for i, line in enumerate(lines) if line.startswith("starting text.elf ")]
    ends = [i for i, line in enumerate(lines) if line.startswith("text.elf ended ")]
    require(len(launches) == len(ends) == 1 and launches[0] < ends[0], "expected one guest process")
    require(re.match(r"^text\.elf ended DONE: exit 0\b", lines[ends[0]]), "guest did not exit successfully")
    body = lines[launches[0] + 1:ends[0]]
    oracle = reference.splitlines()
    require(len(body) == 35, "incomplete guest output")
    require(body[0] == "EXCERPT " + excerpt, "guest excerpt differs from the assigned passage")
    require(len(oracle) == 34, "incomplete oracle output")
    require(body[1:3] == oracle[:2], "input or greedy output IDs differ")
    require(re.fullmatch(r"INPUT 1(?: [0-9]+){1,93}", oracle[0]) and
            re.fullmatch(r"OUTPUT(?: [0-9]+){3}", oracle[1]), "invalid token record")
    ids = [int(value) for line in oracle[:2] for value in line.split()[1:]]
    require(all(token < 512 for token in ids), "token outside vocabulary")
    guest, _ = scores(body[3:])
    host, _ = scores(oracle[2:])
    error = maximum_error(guest, host)
    require(error <= 0.001, f"maximum absolute score error {error} exceeds 0.001")
    return error


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("case", choices=("original", "alternate", "same-offset-control"))
    parser.add_argument("serial", type=Path)
    parser.add_argument("oracle", type=Path)
    args = parser.parse_args()
    excerpt = SENTENCES[0 if args.case == "original" else 1].decode("ascii")
    error = compare(args.serial.read_text(encoding="ascii"), args.oracle.read_text(encoding="ascii"), excerpt)
    print(f"Input and greedy IDs match. Maximum absolute error across 512 scores: {error:.9g}")
