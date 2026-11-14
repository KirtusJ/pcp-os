"""Time two release hosts on the same read-only MCP status workload.

Run on Linux with taskset. Each batch opens one stdio session, reads full status
once, then sends the chosen number of polls. Reports child user + system CPU,
including startup and all register reads. The Python driver is outside that time.
"""

import argparse
import hashlib
import os
from pathlib import Path
import statistics
import tempfile

from cpu import child_cpu
from measure import call, session


def benchmark(before, after, path, repeats, pairs, full):
    raw = path.read_bytes()
    rows = raw.splitlines()
    head = hashlib.sha256(rows[-1]).hexdigest() if rows else "0" * 64
    polls = [call("acr_status", **({} if full else {"if_head": head}))] * repeats
    requests = [call("acr_status"), *polls]
    samples = [[], []]
    with tempfile.TemporaryDirectory(prefix="pcpos-status-") as tmp:
        root = Path(tmp)
        private = root / ".pcp/register.jsonl"
        private.parent.mkdir()
        private.write_bytes(raw)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
        env["PCPOS_ROOT"] = tmp
        expected = None
        for pair in range(pairs + 2):
            for index in ((0, 1) if pair % 2 == 0 else (1, 0)):
                start = child_cpu()
                results = session((before, after)[index], requests, env, root)
                elapsed_ms = 1000 * (child_cpu() - start)
                if expected is None:
                    first = results[0]
                    if not first.startswith(f"REGISTER  .pcp/register.jsonl  |  {len(rows)} events  |  head {head}\n"):
                        raise ValueError("missing full status for expected register head")
                    expected = [first] + [first if full else "UNCHANGED"] * repeats
                if results != expected or private.read_bytes() != raw:
                    raise ValueError("status text or register bytes differ")
                if pair >= 2:
                    samples[index].append(elapsed_ms)
    old, new = (statistics.median(trials) for trials in samples)
    faster = sum(b < a for a, b in zip(*samples))
    print(f"{path.name}\t{len(rows)}\t{len(raw)}\t{repeats}\t{old:.3f}\t{new:.3f}\t{faster}/{pairs}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--register", type=Path, action="append", help="repeat to measure more than one file")
    parser.add_argument("--repeats", type=int, default=100)
    parser.add_argument("--pairs", type=int, default=9)
    parser.add_argument("--full", action="store_true", help="request a full view every time")
    args = parser.parse_args()
    if args.repeats < 1 or args.pairs < 1:
        parser.error("repeats and pairs must be positive")
    vectors = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors"
    paths = args.register or sorted(vectors.glob("register_*.jsonl"))
    print("register\tevents\tbytes\trepeats\tbefore_ms\tafter_ms\tfaster_pairs")
    for path in paths:
        benchmark(args.before.resolve(), args.after.resolve(), path, args.repeats, args.pairs, args.full)


if __name__ == "__main__":
    main()
