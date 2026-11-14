"""Compare Linux process CPU for two PCP OS release binaries, using only the standard library.

Run `taskset -c CPU python3 audit/context/cpu.py OLD_PCPOS NEW_PCPOS` on an available CPU.
Each batch starts one stdio MCP server. CPU includes child user and system time, startup,
parsing, dispatch and output. The Python driver and its result checks are outside the measure.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import statistics
import subprocess
import tempfile


def requests(count, body):
    return b"".join((json.dumps({"jsonrpc": "2.0", "id": i, **body}, separators=(",", ":")) + "\n").encode()
                    for i in range(1, count + 1))


def child_cpu():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def validate(output, count, expected_text):
    lines = output.splitlines()
    if len(lines) != count:
        raise ValueError(f"expected {count} replies, got {len(lines)}")
    for i, line in enumerate(lines, 1):
        reply = json.loads(line)
        result = reply["result"]
        if reply["jsonrpc"] != "2.0" or reply["id"] != i:
            raise ValueError(f"unexpected response ID or version at request {i}")
        if expected_text is None:
            if result != {}:
                raise ValueError(f"unexpected ping result at request {i}")
        elif result.get("isError") or not result["content"][0]["text"].startswith(expected_text):
            raise ValueError(f"unexpected artifact digest at request {i}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--pairs", type=int, default=12, help="measured base/candidate pairs after two warmups")
    args = parser.parse_args()
    if args.pairs < 1:
        parser.error("--pairs must be positive")
    binaries = [args.before.resolve(), args.after.resolve()]
    payload = ("Measured file content, line 123.\n" * 132)[:4096]
    digest = hashlib.sha256(payload.encode()).hexdigest()
    artifact = {"name": "acr_artifact", "arguments": {"text": payload, "record": False}}
    batches = [
        ("ping", 48_000, requests(48_000, {"method": "ping"}), None),
        ("digest_4KiB", 4_096, requests(4_096, {"method": "tools/call", "params": artifact}),
         f"sha256 {digest}\nsize 4096\n"),
    ]
    samples = {name: [[], []] for name, *_ in batches}
    reference = {}
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("CLAUDE_", "PCPOS_"))}
        env["PCPOS_ROOT"] = tmp
        for pair in range(args.pairs + 2):
            # Alternate both workload order and which binary goes first within each pair.
            for name, count, data, expected_text in (batches if pair % 2 == 0 else reversed(batches)):
                for index in ((0, 1) if pair % 2 == 0 else (1, 0)):
                    start = child_cpu()
                    out = subprocess.run([binaries[index], "mcp"], input=data, stdout=subprocess.PIPE,
                                         env=env, cwd=root, check=True).stdout
                    elapsed = child_cpu() - start
                    if any(root.iterdir()):
                        raise ValueError(f"{name} wrote to its empty workspace")
                    if name not in reference:
                        validate(out, count, expected_text)
                        reference[name] = hashlib.sha256(out).digest()
                    elif hashlib.sha256(out).digest() != reference[name]:
                        raise ValueError(f"{name} response bytes differ from the baseline")
                    if pair >= 2:
                        samples[name][index].append(elapsed * 1_000_000 / count)

    print("Linux child user + system CPU, microseconds/request; affinity", sorted(os.sched_getaffinity(0)))
    for name, count, *_ in batches:
        before, after = samples[name]
        faster = sum(new < old for old, new in zip(before, after))
        print(f"{name}: {count} requests/batch; median {statistics.median(before):.3f} -> "
              f"{statistics.median(after):.3f}; candidate faster in {faster}/{args.pairs} pairs")


if __name__ == "__main__":
    main()
