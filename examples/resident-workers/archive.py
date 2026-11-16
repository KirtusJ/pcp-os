#!/usr/bin/env python3
"""Save verifiable serial evidence through a completed resident-worker round."""
import argparse
import json
import os
from pathlib import Path
import re
import sys
from tempfile import TemporaryDirectory
import zipfile

from supervise import require
from verify import integer, verify


def write_ledger(run, archive, cycle):
    # Only complete event lines are committed by the supervisor. Stop at the
    # selected event, even if the writer has already advanced by another round.
    lengths = {}
    with (run / "events.jsonl").open("rb") as events, archive.open("cohort/events.jsonl", "w") as ledger:
        for line in events:
            if not line.endswith(b"\n"):
                break
            event = json.loads(line)
            ledger.write(line)
            if event["event"] != "readback":
                continue
            record = event.get("record")
            require(isinstance(record, dict), "full historical round record unavailable")
            for row in record["nodes"]:
                name = row["serial_file"]
                require(isinstance(name, str) and re.fullmatch(r"[1-9][0-9]*-[1-9][0-9]*\.serial", name),
                        "invalid per-attempt serial filename")
                length = integer(row["serial_bytes"], "serial length", 1)
                require(length > lengths.get(name, 0), f"{name}: serial prefix did not advance")
                lengths[name] = length
            if event["cycle"] == cycle:
                return lengths, record
    raise ValueError(f"completed round {cycle} unavailable")


def freeze(run, oracle, cycle, output):
    integer(cycle, "cycle", 1)
    with TemporaryDirectory(dir=output.parent) as staging:
        candidate = Path(staging) / "readback.zip"
        with zipfile.ZipFile(candidate, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            lengths, record = write_ledger(run, archive, cycle)
            with archive.open("cohort/readback.json", "w") as snapshot:
                snapshot.write((json.dumps(record, indent=2) + "\n").encode())
            with archive.open("fixture/oracle.json", "w") as reference:
                reference.write(oracle.read_bytes())
            for name, length in sorted(lengths.items()):
                with (run / name).open("rb") as source, archive.open("cohort/" + name, "w", force_zip64=True) as dest:
                    remaining = length
                    while remaining:
                        chunk = source.read(min(remaining, 1024 * 1024))
                        require(chunk, f"{name}: truncated serial prefix")
                        dest.write(chunk)
                        remaining -= len(chunk)
        # Replay the packed bytes, including rows for guests retired since an
        # earlier round. Publish only when the complete check succeeds.
        with zipfile.ZipFile(candidate) as archive:
            root = zipfile.Path(archive)
            result = verify(root / "cohort", root / "fixture" / "oracle.json", cycle, check_history=True)
        os.link(candidate, output)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--oracle", type=Path, required=True)
    parser.add_argument("--cycle", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = freeze(args.run, args.oracle, args.cycle, args.output)
    except (OSError, ValueError, TypeError, KeyError, RuntimeError, zipfile.BadZipFile) as error:
        print(f"archive failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
