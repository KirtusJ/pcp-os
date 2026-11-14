#!/usr/bin/env python3
"""Exercise the native ARM host's module-directory ancestry checks via QEMU.

External test driver only. Rust remains the implementation and reference.
All fixtures are disposable and read-only; no PCP register is written.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

MARKER = "EXTERNAL_MODULE_SOURCE_MARKER"
VALID = "LOCAL_MODULE_SOURCE_MARKER"


def invoke(runner: str, binary: Path, *args: str, root: Path, method: Path):
    environment = __import__("os").environ.copy()
    environment.update(PCPOS_ROOT=str(root), PCPOS_METHOD=str(method))
    return subprocess.run([runner, str(binary), *args], input=None,
                          text=True, capture_output=True, env=environment,
                          timeout=4.0, check=False)


def clause(runner: str, binary: Path, root: Path, method: Path):
    return invoke(runner, binary, "clause", str(method), "core", "1.1",
                  root=root, method=method)


def mcp(runner: str, binary: Path, root: Path, method: Path):
    environment = __import__("os").environ.copy()
    environment.update(PCPOS_ROOT=str(root), PCPOS_METHOD=str(method))
    request = {"jsonrpc": "2.0", "id": 1, "method": "tools/call",
               "params": {"name": "acr_clause",
                          "arguments": {"module": "core", "section": "1.1"}}}
    proc = subprocess.run([runner, str(binary), "mcp"], input=json.dumps(request) + "\n",
                          text=True, capture_output=True, timeout=4.0,
                          env=environment, check=False)
    if proc.returncode:
        raise AssertionError(f"MCP subprocess failed: {proc.returncode} {proc.stderr[:200]}")
    response = json.loads(proc.stdout.splitlines()[0])
    result = response.get("result", {})
    text = "".join(i.get("text", "") for i in result.get("content", []))
    return text, bool(result.get("isError"))


def contents(marker: str) -> bytes:
    return (
        "# Test\n\n**Component:** " + chr(96) + "acr.core" + chr(96) +
        ", Version 1.0  \n\n## 1. Scope\n\n### 1.1 Read\n\n" + marker + "\n"
    ).encode()


def check(binary: Path, runner: str):
    with tempfile.TemporaryDirectory(prefix="pcpos-arm-ancestor-") as temp:
        workspace = Path(temp)
        # Normal development method tree is still read-only and usable.
        good = workspace / "good"
        spec = good / "MODULES/01_Core/SOURCE/Specification.md"
        spec.parent.mkdir(parents=True)
        safe = contents(VALID)
        spec.write_bytes(safe)
        terminal = clause(runner, binary, workspace, good)
        expected_sha = hashlib.sha256(safe).hexdigest()
        assert terminal.returncode == 0 and VALID in terminal.stdout, terminal.stderr
        assert expected_sha in terminal.stdout
        text, is_error = mcp(runner, binary, workspace, good)
        assert not is_error and VALID in text and expected_sha in text

        for position in ("MODULES", "component", "SOURCE"):
            target = workspace / position / "method"
            outer_source = workspace / position / "outside/01_Core/SOURCE/Specification.md"
            outer_source.parent.mkdir(parents=True)
            outer_source.write_bytes(contents(MARKER))
            mod = target / "MODULES"
            if position == "MODULES":
                target.mkdir(parents=True)
                mod.symlink_to(outer_source.parent.parent.parent, target_is_directory=True)
            elif position == "component":
                mod.mkdir(parents=True)
                (mod / "01_Core").symlink_to(outer_source.parent.parent, target_is_directory=True)
            else:
                core = mod / "01_Core"
                core.mkdir(parents=True)
                (core / "SOURCE").symlink_to(outer_source.parent, target_is_directory=True)

            terminal = clause(runner, binary, workspace, target)
            assert terminal.returncode == 6, (position, terminal.returncode, terminal.stderr[:400])
            assert MARKER not in terminal.stdout, (position, "external bytes served on CLI")
            text, is_error = mcp(runner, binary, workspace, target)
            assert is_error and MARKER not in text, (position, "external bytes served over MCP")
            assert not (workspace / ".pcp/register.jsonl").exists()
            print(f"PASS {position}: CLI refused; MCP refused; no register")

        print("PASS ordinary module: CLI/MCP served exact marker and digest")
        print("PASS ARM module-ancestor suite: 4 fixtures")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--runner", default="qemu-aarch64")
    args = parser.parse_args()
    if not args.binary.is_file():
        parser.error("ARM binary missing")
    check(args.binary.resolve(), args.runner)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
