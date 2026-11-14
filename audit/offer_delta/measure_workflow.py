"""Measure one submit-on-accept worker over a real local MCP pipe."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import tiktoken

from measure import call, compact, exchange


def run(binary, root, encodings, include_oversized):
    expected = ["Review opened.", "The bound assumes independent launches. " * 64]
    proposals = [
        ("NOTE", {"text": expected[0]}),
        ("NOTE", {"text": expected[1]}),
    ]
    if include_oversized:
        proposals.extend([
            ("NOTE", {"text": "The terminal detector was calibrated before each trial. " * 1250}),
            ("NOTE", {"text": "測定" * 5500}),
            ("SYSTEM_STATE", {}),
        ])
    costs = [0] * (1 + len(encodings))
    calls = 0
    writes = 0
    refused_at = []
    env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
    env["PCPOS_ROOT"] = str(root)
    with subprocess.Popen([binary, "mcp"], cwd=root, env=env, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE) as child:
        def rpc(method, params):
            nonlocal calls
            calls += 1
            request = compact(dict(id=calls, jsonrpc="2.0", method=method, params=params)).encode() + b"\n"
            child.stdin.write(request)
            child.stdin.flush()
            reply = child.stdout.readline()
            message = json.loads(reply)
            if message["id"] != calls:
                raise ValueError("reply ID did not match")
            result = message["result"]
            costs[0] += len(request) + len(reply)
            if method == "tools/list":
                content = [compact(result["tools"])]
            else:
                # Include the tool name, serialized arguments and decoded text in the model projection.
                content = [params["name"], compact(params["arguments"]), result["content"][0]["text"]]
            for i, enc in enumerate(encodings, 1):
                costs[i] += sum(len(enc.encode_ordinary(s)) for s in content)
            return result

        rpc("tools/list", {})
        for kind, body in proposals:
            args = dict(kind=kind, body=body)
            result = rpc(*call("acr_offer", dict(args, delta=True)))
            text = result["content"][0]["text"]
            if result["isError"] or text.startswith("REFUSE"):
                refused_at.append("offer")
                continue
            if json.loads(text) != {"accept": True}:
                raise ValueError("unexpected change to a NOTE")
            result = rpc(*call("acr_submit", args))
            if result["isError"] or result["content"][0]["text"].startswith("REFUSE"):
                refused_at.append("submit")
            elif result["content"][0]["text"].startswith("WRITTEN"):
                writes += 1
            else:
                raise ValueError("unrecognized submission result")
        child.stdin.close()
        if child.wait():
            raise ValueError("server exited abnormally")
    events = [json.loads(line) for line in (root / ".pcp/register.jsonl").read_bytes().splitlines()]
    if writes != len(expected) or [e["body"]["text"] for e in events if e["kind"] == "NOTE"] != expected:
        raise ValueError("different accepted work")
    return dict(calls=calls, written=writes, refused_at=refused_at, rpc_bytes=costs[0],
                cl100k=costs[1], o200k=costs[2])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    encodings = [tiktoken.get_encoding(n) for n in ("cl100k_base", "o200k_base")]
    with tempfile.TemporaryDirectory() as tmp:
        seed = Path(tmp) / "seed"
        seed.mkdir()
        setup = exchange(args.baseline.resolve(), seed, [call("acr_begin", {})])
        if setup[0][2]["isError"]:
            raise ValueError("fixture setup failed")
        for profile in ("valid", "mixed"):
            for label, binary in [("baseline", args.baseline), ("candidate", args.candidate)]:
                root = Path(tmp) / (profile + "-" + label)
                shutil.copytree(seed, root)
                result = run(binary.resolve(), root, encodings, profile == "mixed")
                wanted = []
                if profile == "mixed":
                    wanted = ["submit", "submit", "offer"] if label == "baseline" else ["offer"] * 3
                if result["refused_at"] != wanted:
                    raise ValueError(result)
                print(profile, label, compact(result))


if __name__ == "__main__":
    main()
