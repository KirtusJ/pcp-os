#!/usr/bin/env python3
"""Native host contract: a previously verified method must not gain
new released-namespace modules after its manifest is removed.

Operates only on a copied method tree and an isolated temporary register.
No network access, live register, release archive or actual user data is used.
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


def run(binary, root, method, *args):
    env = dict(os.environ, PCPOS_ROOT=str(root), PCPOS_REGISTER=".pcp/register.jsonl",
               PCPOS_METHOD=str(method))
    return subprocess.run([str(binary), *map(str, args)], cwd=root, env=env,
                          capture_output=True, text=True, timeout=20)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pcpos", type=Path, required=True)
    ap.add_argument("--method", type=Path, required=True)
    ap.add_argument("--expect-fixed", action="store_true")
    args = ap.parse_args()
    binary, method_source = args.pcpos.resolve(), args.method.resolve()
    if not binary.is_file() or not (method_source / "MANIFEST.json").is_file():
        ap.error("supply the selected native executable and original method tree")
    if hashlib.sha256((method_source / "MANIFEST.json").read_bytes()).hexdigest() != PIN:
        ap.error("method directory is not the pinned build-22 release")
    with tempfile.TemporaryDirectory(prefix="pcpos-pin-recheck-") as tmp:
        root = Path(tmp)
        method = root / "method"
        shutil.copytree(method_source, method)
        reg = root / ".pcp/register.jsonl"
        reg.parent.mkdir()
        init = run(binary, root, method, "init", reg, "--principal", "principal")
        if init.returncode != 0:
            raise RuntimeError("initial register setup failed: " + init.stderr)
        events = [json.loads(line) for line in reg.read_text().splitlines()]
        pins = [e for e in events if e.get("kind") == "METHOD_PINNED"]
        verified = (len(pins) == 1 and
                    pins[0]["body"].get("verified") == "FILES_VERIFIED_AGAINST_OWN_MANIFEST")
        normal = run(binary, root, method, "workspace", reg, "--by", "principal",
                     "--method", method, "science")
        before = reg.read_bytes()
        (method / "MANIFEST.json").unlink()
        spec = method / "MODULES/08_Synthetic/SOURCE/Specification.md"
        spec.parent.mkdir(parents=True)
        spec.write_text("**Component:** " + chr(96) + "acr.synthetic" + chr(96) +
                        ", Version 1.0\n\n## 1. Test\n\nNot a released method module.\n")
        attempt = run(binary, root, method, "workspace", reg, "--by", "principal",
                      "--method", method, "synthetic")
        afterward = reg.read_bytes()
        event_list = [json.loads(line) for line in reg.read_text().splitlines()]
        synthetic_selected = any(e.get("body", {}).get("event") == "WORKSPACE_PROFILE" and
                                 any(m.get("id") == "acr.synthetic" for m in
                                     e["body"].get("modules", [])) for e in event_list)
        check = run(binary, root, method, "verify", reg)
        result = {
            "method_pin_verified": verified,
            "normal_science_selection_exit": normal.returncode,
            "new_synthetic_selection_exit": attempt.returncode,
            "synthetic_selected": synthetic_selected,
            "register_unchanged_after_refusal": before == afterward,
            "chain_verify_exit": check.returncode,
            "modified_only_disposable_method": True,
        }
    passed = (result["method_pin_verified"] and result["normal_science_selection_exit"] == 0
              and result["new_synthetic_selection_exit"] != 0
              and not result["synthetic_selected"]
              and result["register_unchanged_after_refusal"]
              and result["chain_verify_exit"] == 0)
    print(json.dumps({"pass_control": passed, "observations": result}, indent=2))
    if args.expect_fixed and not passed:
        print("REGRESSION_FAIL: stale verified method pin permitted unlisted module selection")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
