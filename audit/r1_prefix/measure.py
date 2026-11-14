"""Linux benchmark. Supply baseline and candidate pcpos-verify release binaries."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path
import subprocess
import tempfile


def head(line):
    return hashlib.sha256(line).hexdigest()


def make_register(directory, count):
    source = directory / "source.jsonl"
    base = directory / "prefix.jsonl"
    current = "0" * 64
    with source.open("wb") as full, base.open("wb") as prefix:
        for seq in range(count + 1):
            event = {"seq": seq, "prev": current, "actor": "worker", "auth": "HOST", "kind": "NOTE",
                     "ts": "2026-11-13T00:00:00Z", "body": {"sample": seq, "flags": [True, False]}}
            line = json.dumps(event, sort_keys=True, separators=(",", ":")).encode()
            full.write(line + b"\n")
            if seq < count:
                prefix.write(line + b"\n")
            else:
                anchor = current
            current = head(line)
    return source, base, anchor, current


def measure(binary, wire, target, base, expected_file):
    # The receiver verifies everything before writing. Keep it alive on a pipe
    # while reading its own kernel-reported high-water mark after the exec.
    output = base.with_suffix(".result")
    with output.open("wb") as dest, subprocess.Popen(
        [binary, "--from-r1", wire, target, "--base", base], stdout=subprocess.PIPE, pipesize=4096
    ) as proc:
        first = proc.stdout.read(1)
        if not first:
            raise ValueError("receiver produced no register")
        status = Path(f"/proc/{proc.pid}/status").read_text()
        peak = int(next(row for row in status.splitlines() if row.startswith("VmHWM:")).split()[1])
        dest.write(first)
        shutil.copyfileobj(proc.stdout, dest)
        if proc.wait():
            raise RuntimeError(f"receiver exited {proc.returncode}")
    with output.open("rb") as a, expected_file.open("rb") as b:
        if hashlib.file_digest(a, "sha256").digest() != hashlib.file_digest(b, "sha256").digest():
            raise ValueError("restored register differs")
    return peak


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--counts", type=int, nargs="+", default=[1000, 10000, 50000])
    args = parser.parse_args()
    if min(args.counts) < 100:
        parser.error("use at least 100 events so the verified output fills the measurement pipe")
    binaries = [args.baseline.resolve(), args.candidate.resolve()]
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        for count in args.counts:
            source, base, anchor, target = make_register(folder, count)
            wire = folder / "one.r1"
            subprocess.run([binaries[0], source, "--r1", wire, "--after", anchor],
                           stdout=subprocess.DEVNULL, check=True)
            samples = [[], []]
            for order in [(0, 1), (1, 0), (0, 1)]:
                for index in order:
                    samples[index].append(measure(binaries[index], wire, target, base, source))
            print(json.dumps(dict(prefix_events=count, prefix_bytes=base.stat().st_size,
                                  wire_bytes=wire.stat().st_size, baseline_peak_KiB=samples[0],
                                  candidate_peak_KiB=samples[1])), flush=True)


if __name__ == "__main__":
    main()
