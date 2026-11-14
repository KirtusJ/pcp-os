"""Measure an explicit preview/write decision over a persistent native MCP connection."""
import argparse
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path

import tiktoken

from measure import compact, exchange, call

FIXTURE = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors/register_fphpm_exercise.jsonl"


def run(binary, root, seed, mode, proposals, updates, encodings):
    register = root / ".pcp/register.jsonl"
    register.write_bytes(seed)
    totals = [0, 0, 0]
    calls = 0
    written = []
    with subprocess.Popen([binary, "mcp"], cwd=root, env={"PCPOS_ROOT": str(root)},
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL) as process:
        def rpc(method, params):
            nonlocal calls
            calls += 1
            request = compact(dict(id=calls, jsonrpc="2.0", method=method, params=params)).encode() + b"\n"
            process.stdin.write(request)
            process.stdin.flush()
            raw = process.stdout.readline()
            msg = json.loads(raw)
            if msg["id"] != calls:
                raise ValueError("reply ID differs")
            result = msg["result"]
            if method == "tools/list":
                parts = [compact(result["tools"])]
            else:
                if result["isError"]:
                    raise ValueError(result)
                parts = [params["name"], compact(params["arguments"]), result["content"][0]["text"]]
            totals[0] += len(request) + len(raw)
            for i, enc in enumerate(encodings, 1):
                totals[i] += sum(len(enc.encode_ordinary(s)) for s in parts)
            return parts[-1]

        rpc("tools/list", {})
        status = rpc(*call("acr_status", {"view": "update"} if updates else {}))
        initial = re.search(r"\bhead ([0-9a-f]{64})\b", status.splitlines()[0])
        if initial is None:
            # The original human view shows a prefix. Charge the call needed to get a usable full head.
            verified = rpc(*call("acr_verify", {}))
            initial = re.search(r"\bhead ([0-9a-f]{64})\b", verified.splitlines()[0])
        if initial is None:
            raise ValueError("initial register head unavailable")
        head = initial.group(1)
        for kind, body in proposals:
            args = dict(kind=kind, body=body)
            if mode == "direct":
                result = rpc(*call("acr_submit", dict(args, expected_head=head)))
            else:
                opts = {"prepare": True, "expected_head": head} if mode == "prepared" else {"delta": True}
                answer = rpc(*call("acr_offer", dict(args, **opts)))
                if answer.startswith("REFUSE"):
                    if kind != "SYSTEM_STATE":
                        raise ValueError(answer)
                    continue
                patch = json.loads(answer)
                if patch.get("accept") is not True or set(patch) - {"accept", "ref"}:
                    raise ValueError(patch)
                if mode == "prepared":
                    result = rpc(*call("acr_commit", {"ref": patch["ref"]}))
                else:
                    result = rpc(*call("acr_submit", dict(args, expected_head=head)))
            if kind == "SYSTEM_STATE":
                if not result.startswith("REFUSE"):
                    raise ValueError(result)
                continue
            if not result.startswith("WRITTEN"):
                raise ValueError(result)
            new_head = re.search(r"^head ([0-9a-f]{64})$", result, re.MULTILINE).group(1)
            status = rpc(*call("acr_status", {"view": "update", "if_head": head} if updates else {}))
            if updates:
                if json.loads(status) != [len(seed.splitlines()) + len(written) + 1, new_head]:
                    raise ValueError(status)
            elif new_head[:16] not in status.splitlines()[0]:
                raise ValueError(status)
            written.append(body)
            head = new_head
        process.stdin.close()
        if process.wait():
            raise ValueError("server exited abnormally")
    raw = register.read_bytes()
    if not raw.startswith(seed) or hashlib.sha256(raw.splitlines()[-1]).hexdigest() != head:
        raise ValueError("register does not match the last receipt")
    persisted = [json.loads(line) for line in raw[len(seed):].splitlines()]
    if [(e["kind"], e["body"]) for e in persisted] != [(k, b) for k, b in proposals if k == "NOTE"]:
        raise ValueError("persisted bodies differ")
    return [calls, len(written), *totals]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--updates", action="store_true", help="both binaries include the conditional status PR")
    args = parser.parse_args()
    encodings = [tiktoken.get_encoding(n) for n in ("cl100k_base", "o200k_base")]
    small = ("NOTE", {"text": "checked"})
    long = ("NOTE", {"text": "The bound assumes independent launches. " * 64})
    mixed = [small, long, ("SYSTEM_STATE", {}), ("NOTE", {"text": "測定値と位相差。" * 128})]
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / ".pcp").mkdir()
        register = root / ".pcp/register.jsonl"
        register.write_bytes(FIXTURE.read_bytes())
        setup = exchange(args.baseline.resolve(), root, [call("acr_begin", {})])
        if setup[0][2]["isError"]:
            raise ValueError("register setup failed")
        seed = register.read_bytes()
        print("workload mode calls writes rpc_bytes cl100k o200k")
        for label, proposals in [("short", [small]), ("long", [long]), ("mixed", mixed)]:
            for mode, binary in [("direct", args.baseline), ("resend", args.baseline), ("prepared", args.candidate)]:
                print(label, mode, *run(binary.resolve(), root, seed, mode, proposals, args.updates, encodings))


if __name__ == "__main__":
    main()
