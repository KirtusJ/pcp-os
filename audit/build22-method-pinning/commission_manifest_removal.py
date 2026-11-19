#!/usr/bin/env python3
"""Check that an old verified METHOD_PINNED does not authorize new review
modules from a method whose manifest has since disappeared.

Both cases run on separate synthetic, disposable registers. No user data or
network access is used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

PIN = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707"


def run(binary, root, method, *args, input_text=None):
    env = dict(os.environ, PCPOS_ROOT=str(root), PCPOS_REGISTER=".pcp/register.jsonl",
               PCPOS_METHOD=str(method))
    return subprocess.run([str(binary), *map(str, args)], cwd=root, env=env, input=input_text,
                          capture_output=True, text=True, timeout=20)


def events(reg):
    return [json.loads(line) for line in reg.read_text().splitlines()]


def case(binary, parent, original, altered):
    root = parent / ("altered" if altered else "normal")
    root.mkdir()
    method = root / "method"
    shutil.copytree(original, method)
    reg = root / ".pcp/register.jsonl"
    reg.parent.mkdir()
    started = run(binary, root, method, "init", reg, "--principal", "principal")
    if started.returncode != 0:
        raise RuntimeError("cannot initialize verified method: " + started.stderr)
    pins = [e for e in events(reg) if e["kind"] == "METHOD_PINNED"]
    pin_verified = len(pins) == 1 and pins[0]["body"].get("verified") == "FILES_VERIFIED_AGAINST_OWN_MANIFEST"
    if not pin_verified:
        raise RuntimeError("initial method was not verified")
    target = root / "target.txt"
    target.write_bytes(b"target")
    module = "science"
    spec = method / "MODULES/03_Science/SOURCE/Specification.md"
    if altered:
        (method / "MANIFEST.json").unlink()
        module = "synthetic"
        spec = method / "MODULES/08_Synthetic/SOURCE/Specification.md"
        spec.parent.mkdir(parents=True)
        spec.write_text("**Component:** " + chr(96) + "acr.synthetic" + chr(96) +
                        ", Version 1.0\n\n## 1. Test\n\nUnlisted synthetic test module.\n")
    digest = hashlib.sha256(spec.read_bytes()).hexdigest()
    target_sha = hashlib.sha256(b"target").hexdigest()
    adopted = run(binary, root, method, "adopt", reg, "--by", "principal", "--basis", "use ACR")
    if adopted.returncode != 0:
        raise RuntimeError("cannot create adopted review fixture: " + adopted.stderr)
    submit = {"kind": "REVIEW_COMMISSIONED", "body": {
        "label": "R", "title": "Synthetic target",
        "target": {"t": "artifact", "sha256": target_sha},
        "extent": "TOTAL", "procedure": "FOCUSED", "director": "principal",
        "modules": [{"id": "acr." + module, "version": "1.0", "sha256": digest}],
        "claims": [{"label": "A", "text": "target", "origin": "COMMISSIONER"}]}}
    requests = [
        {"jsonrpc":"2.0", "id":0, "method":"initialize",
         "params":{"protocolVersion":"2025-06-18", "capabilities":{},
                   "clientInfo":{"name":"method-pin-contract","version":"1"}}},
        {"jsonrpc":"2.0", "method":"notifications/initialized"},
        {"jsonrpc":"2.0", "id":1, "method":"tools/call",
         "params":{"name":"acr_begin","arguments":{}}},
        {"jsonrpc":"2.0", "id":2, "method":"tools/call",
         "params":{"name":"acr_artifact","arguments":{"path":"target.txt"}}},
        {"jsonrpc":"2.0", "id":3, "method":"tools/call",
         "params":{"name":"acr_submit","arguments":submit}},
    ]
    child = run(binary, root, method, "mcp",
                input_text="\n".join(json.dumps(x) for x in requests) + "\n")
    if child.returncode != 0:
        raise RuntimeError("MCP transport failed: " + child.stderr)
    replies = [json.loads(line) for line in child.stdout.splitlines() if line.strip()]
    named = next((x for x in replies if x.get("id") == 3), None)
    if named is None:
        raise RuntimeError("no submission reply on the MCP channel")
    review_text = "\n".join(x.get("text", "") for x in named.get("result", {}).get("content", []))
    reviews = [e for e in events(reg) if e["kind"] == "REVIEW_COMMISSIONED"]
    verify = run(binary, root, method, "verify", reg)
    return {"mode":"altered" if altered else "normal",
            "initial_pin_verified":pin_verified,
            "review_written":bool(reviews),
            "returned_written_receipt":review_text.startswith("WRITTEN as event"),
            "returned_refusal":"REFUSE" in review_text,
            "chain_verify_exit":verify.returncode,
            "review_response_excerpt":review_text[:270]}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pcpos",type=Path,required=True)
    parser.add_argument("--method",type=Path,required=True)
    parser.add_argument("--expect-fixed",action="store_true")
    args=parser.parse_args()
    binary, method=args.pcpos.resolve(),args.method.resolve()
    if not binary.is_file() or not (method / "MANIFEST.json").is_file():
        parser.error("need an executable and the original build-22 method")
    if hashlib.sha256((method/"MANIFEST.json").read_bytes()).hexdigest()!=PIN:
        parser.error("method manifest is not the pinned build-22 release")
    with tempfile.TemporaryDirectory(prefix="pcpos-commission-pin-") as folder:
        parent=Path(folder)
        normal=case(binary,parent,method,False)
        altered=case(binary,parent,method,True)
    good=(normal["review_written"] and normal["returned_written_receipt"]
          and normal["chain_verify_exit"]==0)
    denied=(not altered["review_written"] and altered["returned_refusal"]
            and altered["chain_verify_exit"]==0)
    print(json.dumps({"pass_control":bool(good and denied),
                      "normal":normal,"altered":altered},indent=2))
    if args.expect_fixed and not (good and denied):
        print("REGRESSION_FAIL: stale method pin permitted synthetic review module")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
