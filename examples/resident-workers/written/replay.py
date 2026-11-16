"""Replay a fixed indexed round and the following written frame from each VM."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from supervise import require
from verify import verify


def vocabulary(path):
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() ==
            "037cb335abb25d1fa9e8ecae30ed2a3a8ace9302862ebcdc05d51a6bbb10c312", "vocabulary digest")
    offset = 4
    pieces = []
    for _ in range(512):
        _, size = struct.unpack_from("<fI", data, offset)
        offset += 8
        pieces.append(data[offset:offset + size])
        offset += size
    require(offset == len(data), "vocabulary extent")
    return pieces


def replay(run, fixture):
    verified = verify(run, fixture / "oracle.json", 1)
    indexed = json.loads((run / "readback.json").read_bytes())
    require(hashlib.sha256((run / "readback.json").read_bytes()).hexdigest() == verified["sha256"],
            "indexed snapshot mismatch")
    trial = json.loads((run / "written-round.json").read_bytes())
    cases = json.loads((fixture / "written.json").read_bytes())
    prompts = (HERE / "prompts.txt").read_text().splitlines()
    require([c["prompt"] for c in cases] == prompts and len(prompts) == 4, "prompt mismatch")
    require(trial["cycle"] == 1 and len(trial["nodes"]) == 24, "incomplete written cohort")
    pieces = vocabulary(fixture / "tok512.bin")
    participants = []
    for node, (admission, row) in enumerate(zip(indexed["nodes"], trial["nodes"], strict=True), 1):
        require((node, admission["uuid"], admission["serial_file"], admission["serial_bytes"]) ==
                (row["node"], row["uuid"], row["serial_file"], row["start"]), "written identity or offset mismatch")
        with (run / admission["serial_file"]).open("rb") as stream:
            raw = stream.read(row["end"])
        require(len(raw) == row["end"] and row["start"] < row["end"] and
                hashlib.sha256(raw).hexdigest() == row["sha256"], "written prefix mismatch")
        frame = raw[row["start"]:].replace(b"\r\n", b"\n").decode("ascii").split("\n")
        require(len(frame) == 6 and frame[-2:] == [f"READY {node}", ""], "written frame extent")
        case = cases[0 if node == 1 else 1 if node <= 4 else 2 if node <= 12 else 3]
        require(all(row[k] == v for k, v in case.items()) and frame[0] == "EXCERPT " + case["prompt"],
                "wrong written case")
        require(re.fullmatch(r"INPUT(?: [0-9]+)+", frame[1]) and re.fullmatch(r"OUTPUT(?: [0-9]+)+", frame[2]),
                "invalid token record")
        inp = [int(i) for i in frame[1].split()[1:]]
        out = [int(i) for i in frame[2].split()[1:]]
        require(inp == case["input_ids"] and out == case["output_ids"] and frame[3] == "STOP " + case["stop"],
                "guest tokens differ from the upstream reference")
        require(0 < len(out) <= 48 and 2 not in out[:-1] and
                ((out[-1] == 2 and frame[3] == "STOP EOS") or
                 (out[-1] != 2 and len(out) == 48 and frame[3] == "STOP CAP")), "invalid stopping condition")
        previous = inp[-1]
        answer = bytearray()
        for token in out:
            if token == 2:
                break
            piece = pieces[token]
            if previous == 1:
                piece = piece.removeprefix(b" ")
            if re.fullmatch(rb"<0x[0-9A-Fa-f]{2}>", piece):
                piece = bytes([int(piece[3:5], 16)])
            answer.extend(piece)
            previous = token
        require(answer.hex() == case["decoded_hex"], "independent vocabulary decode mismatch")
        participants.append(dict(node=node, uuid=row["uuid"], prompt=case["prompt"],
                                 response=answer.decode("utf-8", "backslashreplace"), stop=case["stop"]))
    return dict(indexed=verified, participants=participants)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cohort", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(replay(args.cohort, args.fixture)))
