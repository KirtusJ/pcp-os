"""Compare baseline and candidate native pcpos binaries. Requires tiktoken for token counts."""
import argparse
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile

import tiktoken


def compact(obj):
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":"))


def exchange(binary, directory, messages):
    env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
    env["PCPOS_ROOT"] = str(directory)
    lines = [compact(dict(jsonrpc="2.0", id=i, method=method, params=params)) + "\n"
             for i, (method, params) in enumerate(messages)]
    output = subprocess.run([binary, "mcp"], input="".join(lines).encode(), capture_output=True,
                            cwd=directory, env=env, check=True).stdout.splitlines(keepends=True)
    if len(output) != len(messages):
        raise ValueError("missing response")
    return [(req, raw, json.loads(raw)["result"]) for req, raw in zip(lines, output)]


def call(name, args):
    return "tools/call", {"name": name, "arguments": args}


def trial(binary, root, delta, encodings):
    text = "The bound requires independent launches. " * 64
    cases = [
        ("short", "NOTE", {"text": "ok"}),
        ("long", "NOTE", {"text": text}),
        ("derived", "PARTICIPANT_ADDED", {"label": "peer", "kind": "AUTOMATED"}),
        ("refusal", "SYSTEM_STATE", {}),
    ]
    args = [{"kind": kind, "body": body, **({"delta": True} if delta else {})}
            for _, kind, body in cases]
    messages = [("tools/list", {}), call("acr_begin", {})] + [call("acr_offer", a) for a in args]
    catalog, begun, *offers = exchange(binary, root, messages)
    if begun[2]["isError"]:
        raise ValueError("setup failed")
    counts = {}
    results = []
    def row(wire, content):
        return [wire, *(len(enc.encode_ordinary(content)) for enc in encodings)]
    counts["catalog"] = row(len(catalog[0].encode()) + len(catalog[1]), compact(catalog[2]["tools"]))
    for (label, _, body), a, (request, raw, response) in zip(cases, args, offers):
        answer = response["content"][0]["text"]
        if response["isError"]:
            raise ValueError(answer)
        counts[label] = [len(request.encode()) + len(raw), *(
            len(enc.encode_ordinary(compact(a))) + len(enc.encode_ordinary(answer)) for enc in encodings)]
        if label == "refusal":
            if not answer.startswith("REFUSE"):
                raise ValueError(answer)
            results.append(answer)
        elif delta:
            patch = json.loads(answer)
            if patch.get("accept") is not True:
                raise ValueError(answer)
            restored = {k: v for k, v in body.items() if k not in patch.get("remove", [])}
            results.append({**restored, **patch.get("set", {})})
        else:
            results.append(json.loads(answer.removeprefix("ACCEPT\nbody that would be written: ")))
    return counts, results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    encodings = [tiktoken.get_encoding(name) for name in ("cl100k_base", "o200k_base")]
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        old, new = root / "old", root / "new"
        old.mkdir()
        fixture = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors/register_scenario.jsonl"
        # A pinned WORKSPACE_CREATED event makes the generated participant IDs reproducible.
        (old / ".pcp").mkdir()
        (old / ".pcp/register.jsonl").write_bytes(fixture.read_bytes().splitlines(keepends=True)[0])
        shutil.copytree(old, new)
        full, expected = trial(args.baseline.resolve(), old, False, encodings)
        delta, observed = trial(args.candidate.resolve(), new, True, encodings)
    if observed != expected:
        raise ValueError("reconstructed bodies or refusals differ")
    print("case      full [RPC bytes, cl100k, o200k]   delta [RPC bytes, cl100k, o200k]")
    for label in full:
        print(f"{label:8}  {full[label]}  {delta[label]}")


if __name__ == "__main__":
    main()
