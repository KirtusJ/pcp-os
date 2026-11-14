"""Compare full MCP status with view="attention" on the three published registers.

Usage: python3 audit/context/status_payload.py BASELINE_PCPOS CANDIDATE_PCPOS [--tokens]

Each arm reads a fresh private copy. Costs add compact argument JSON and tool
result text as separately encoded fields. Discovery rows additionally count
that binary's complete tools/list catalog once. Sizes exclude JSON-RPC
envelopes, provider message formatting, caching and model-generated output.
--tokens requires tiktoken with both encodings already cached locally.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from measure import call, canonical, counters, session


FIXTURES = (
    ("register_scenario", 2, 0),
    ("register_fphpm_exercise", 0, 0),
    ("register_reference", 1, 2),
)
ATTENTION = {"view": "attention"}


def read_status(binary, raw, candidate):
    with tempfile.TemporaryDirectory(prefix="pcpos-status-") as tmp:
        root = Path(tmp)
        path = root / ".pcp/register.jsonl"
        path.parent.mkdir()
        path.write_bytes(raw)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
        env["PCPOS_ROOT"] = tmp
        requests = [("tools/list", {})]
        if candidate:
            requests.append(call("acr_status", **ATTENTION))
        requests.extend([call("acr_status"), call("acr_verify")])
        result = session(binary, requests, env, root)
        if path.read_bytes() != raw:
            raise ValueError(f"{binary}: status measurement changed the register")
        return result


def measure_status(baseline, candidate, fixtures):
    for name, items, problems in FIXTURES:
        raw = (fixtures / f"{name}.jsonl").read_bytes()
        lines = raw.splitlines()
        head = hashlib.sha256(lines[-1]).hexdigest()
        old_catalog, old_full, old_verify = read_status(baseline, raw, False)
        catalog, scoped, full, verify = read_status(candidate, raw, True)
        if (old_full, old_verify) != (full, verify):
            raise ValueError(f"{name}: default status or verification changed")
        verified = f"chain intact: {len(lines)} events, head {head}\nguards replayed: {problems} problem(s)\n"
        if not verify.startswith(verified):
            raise ValueError(f"{name}: full verification differs from fixture")
        header, _, body = full.partition("\n")
        prefix, start, rest = body.partition("ATTENTION ")
        attention, end, _ = rest.partition("\nREVIEWS ")
        if not header.endswith(f"  |  head {head}") or not prefix.endswith("\n") or not start or not end:
            raise ValueError(f"{name}: missing register head or attention section in full status")
        expected = "nothing needs a response" if items == 0 else f"{items} item(s)"
        if attention.partition("\n")[0] != expected or attention.count("\n         -> ") != items:
            raise ValueError(f"{name}: attention responses differ from fixture")
        if problems and f"{problems} recorded event(s) would not pass the guards" not in attention:
            raise ValueError(f"{name}: guard warning missing")
        if scoped != f"{header}\nATTENTION {attention}\n":
            raise ValueError(f"{name}: scoped status differs from the full register header and attention section")
        status_tool = next((tool for tool in json.loads(catalog) if tool["name"] == "acr_status"), {})
        if "view" not in status_tool.get("inputSchema", {}).get("properties", {}):
            raise ValueError("candidate does not advertise the status view")
        yield name, old_catalog, catalog, old_full, scoped


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--tokens", action="store_true")
    args = parser.parse_args()
    vectors = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors"
    try:
        measures = counters(args.tokens)
        rows = list(measure_status(args.baseline.resolve(), args.candidate.resolve(), vectors))
    except (OSError, ValueError, ImportError, subprocess.SubprocessError) as error:
        parser.exit(1, f"status_payload: {error}\n")
    print("fixture", "cost", *(f"{build}_{unit}" for unit in measures for build in ("full", "attention")), sep="\t")
    for name, old_catalog, catalog, full, scoped in rows:
        for discovery in (False, True):
            label = "arguments + result" + (" + discovery" if discovery else "")
            print(name, label, *(count(canonical(arguments)) + count(text) + (count(tools) if discovery else 0)
                  for count in measures.values()
                  for arguments, text, tools in [({}, full, old_catalog), (ATTENTION, scoped, catalog)]), sep="\t")


if __name__ == "__main__":
    main()
