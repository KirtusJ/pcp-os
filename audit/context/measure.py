"""Measure PCP OS model-facing MCP payloads from two native host binaries.

Usage: python3 audit/context/measure.py OLD_PCPOS NEW_PCPOS BUILT_PLUGIN [--tokens]

The default register is the published scenario vector. Both programs read the
same private copy and use the same packaged events.md. Counts cover UTF-8 tool
result text or compact, key-sorted tools/list schema JSON. A separate poll row
adds encoded argument JSON and result text as independent fields. All rows omit
protocol wrappers, prompt caching, and other provider-side costs.
The crossover uses first-use order in the cookbook and one call per distinct kind.
--tokens adds the o200k_base and cl100k_base counts from an installed tiktoken
and its existing encoding cache (TIKTOKEN_CACHE_DIR is respected). No downloads.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


RECIPE_HEADING = re.compile(r"^## [0-9]+\. [^\r\n]+\r?\n\r?\n`([A-Z_]+)`\r?$", re.M)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def call(name, **arguments):
    return "tools/call", {"name": name, "arguments": arguments}


def session(binary, requests, env, root):
    # Each batch is one persistent MCP server. The first status primes its poll.
    batch = [("initialize", {"protocolVersion": "2025-06-18"}), *requests]
    wire = "".join(canonical({"jsonrpc": "2.0", "id": i, "method": method, "params": params}) + "\n"
                   for i, (method, params) in enumerate(batch))
    proc = subprocess.run([binary, "mcp"], input=wire, capture_output=True, encoding="utf-8",
                          cwd=root, env=env, check=True, timeout=60)
    replies = [json.loads(line) for line in proc.stdout.splitlines()]
    if len(replies) != len(batch):
        raise ValueError(f"{binary}: expected {len(batch)} replies, got {len(replies)}")
    results = []
    for i, ((method, params), reply) in enumerate(zip(batch, replies)):
        result = reply.get("result")
        if reply.get("id") != i or reply.get("jsonrpc") != "2.0" or not isinstance(result, dict) or "error" in reply:
            raise ValueError(f"{binary}: unexpected reply to {method}: {reply}")
        if method == "initialize":
            if not isinstance(result.get("protocolVersion"), str):
                raise ValueError(f"{binary}: initialization failed")
        elif method == "tools/list":
            entries = result.get("tools")
            if not isinstance(entries, list) or not entries or not all(isinstance(t, dict) and isinstance(t.get("name"), str) for t in entries):
                raise ValueError(f"{binary}: invalid tool catalogue")
            results.append(canonical(entries))
        else:
            blocks = result.get("content")
            if (result.get("isError") is not False or not isinstance(blocks, list) or len(blocks) != 1
                    or not isinstance(blocks[0], dict) or blocks[0].get("type") != "text" or not isinstance(blocks[0].get("text"), str)):
                raise ValueError(f"{binary}: invalid or failed {params['name']} call: {result}")
            results.append(blocks[0]["text"])
    return results


def measure(args):
    register = args.register.read_bytes()
    guide_bytes = (args.plugin / "skills/review/events.md").read_bytes()
    guide = guide_bytes.decode("utf-8")
    kind_sequence = RECIPE_HEADING.findall(guide)
    kinds = list(dict.fromkeys(kind_sequence))
    if not {"DELIVERY_STATE", "FINDING_RECORDED", "GATE_COMMITTED"}.issubset(kinds):
        raise ValueError("expected the packaged PCP OS event guide with numbered examples")
    with tempfile.TemporaryDirectory(prefix="pcpos-context-") as tmp:
        root = Path(tmp)
        path = root / ".pcp/register.jsonl"
        path.parent.mkdir()
        path.write_bytes(register)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
        env.update(PCPOS_ROOT=tmp, CLAUDE_PLUGIN_ROOT=str(args.plugin.resolve()))
        shared = [("tools/list", {}), call("acr_events"), call("acr_status")]
        old_tools, old_guide, old_status, old_poll, verify = session(
            args.baseline.resolve(), [*shared, call("acr_status"), call("acr_verify")], env, root)
        head = re.search(r"^chain intact: \d+ events, head ([a-f0-9]{64})$", verify, re.M)
        if not head or "guards replayed: 0 problem(s)" not in verify or old_status != old_poll:
            raise ValueError("baseline did not return a valid stable register with zero replay problems")
        new_tools, new_guide, new_status, new_poll, *recipes = session(args.candidate.resolve(), [
            *shared, call("acr_status", if_head=head[1]), *(call("acr_events", kind=k) for k in kinds)], env, root)
        if path.read_bytes() != register:
            raise ValueError("the register copy changed during context measurement")
    if old_guide != new_guide or old_guide.partition("\n\n")[2] != guide or hashlib.sha256(guide_bytes).hexdigest() not in old_guide.partition("\n")[0]:
        raise ValueError("the no-argument guides differ from one another or the complete packaged file")
    if head[1] not in new_status.splitlines()[0] or new_poll != "UNCHANGED":
        raise ValueError("candidate status failed to provide the full head and subsequently confirm it")
    for kind, recipe in zip(kinds, recipes):
        if RECIPE_HEADING.findall(recipe) != [kind] * kind_sequence.count(kind):
            raise ValueError(f"candidate returned an incomplete or overbroad result for {kind}")
    return [("tools/list", old_tools, new_tools), ("acr_events full", old_guide, new_guide),
            ("acr_status full", old_status, new_status), ("acr_status repeat", old_poll, new_poll)], kinds, recipes, head[1]


def counters(tokens):
    measures = {"bytes": lambda s: len(s.encode("utf-8"))}
    if tokens:
        from unittest.mock import patch
        import tiktoken
        # A cache miss or checksum failure in tiktoken would normally fetch a URL.
        with patch("tiktoken.load.read_file", side_effect=ValueError("tiktoken encoding cache missing or invalid; use a populated cache or omit --tokens")):
            for name in ("o200k_base", "cl100k_base"):
                encoder = tiktoken.get_encoding(name)
                measures[name] = lambda s, enc=encoder: len(enc.encode_ordinary(s))
    return measures


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("baseline", type=Path, help="original native pcpos executable")
    parser.add_argument("candidate", type=Path, help="updated native pcpos executable")
    parser.add_argument("plugin", type=Path, help="built plugin directory containing skills/review/events.md")
    parser.add_argument("--register", type=Path, default=Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors/register_scenario.jsonl")
    parser.add_argument("--tokens", action="store_true", help="include token counts using an installed offline cache")
    args = parser.parse_args()
    try:
        measures = counters(args.tokens)
        rows, kinds, recipes, head = measure(args)
    except (OSError, ValueError, ImportError, subprocess.SubprocessError) as error:
        parser.exit(1, f"measure: {error}\n")
    print("payload", *(f"{build}_{unit}" for unit in measures for build in ("baseline", "candidate")), sep="\t")
    guide = rows[1][1]
    for label, old, new in [*rows, *((k, guide, r) for k, r in zip(kinds, recipes))]:
        print(label, *(count(t) for count in measures.values() for t in (old, new)), sep="\t")
    _, old_poll, new_poll = rows[-1]
    print("acr_status repeat incl. arguments", *(count(params) + count(payload) for count in measures.values()
          for params, payload in [("{}", old_poll), (canonical({"if_head": head}), new_poll)]), sep="\t")
    print("all distinct kinds vs full once", *(v for count in measures.values() for v in (count(guide), sum(map(count, recipes)))), sep="\t")
    for unit, count in measures.items():
        total = 0
        for i, (kind, recipe) in enumerate(zip(kinds, recipes), 1):
            total += count(recipe)
            if total >= count(guide):
                print(f"# {unit}: cumulative scoped output reaches the full guide at {i}/{len(kinds)} distinct kinds ({kind})")
                break
        else:
            print(f"# {unit}: cumulative scoped output stays below the full guide through all {len(kinds)} distinct kinds")


if __name__ == "__main__":
    main()
