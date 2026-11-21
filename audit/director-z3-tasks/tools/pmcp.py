#!/usr/bin/env python3
"""pmcp.py <tool> [key=value ...]: one call of a PCP OS host tool over the host's own MCP server, on the
register in this folder (audit/director-z3-tasks/.pcp/register.jsonl), as the participant DirectorZ3.

The host program is PCPOS_BIN if set, else PCP_OS_Rust/target/release/pcpos in this repository.
"""
import json, os, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
BIN = os.environ.get("PCPOS_BIN") or str(HERE.parents[1] / "PCP_OS_Rust" / "target" / "release" / "pcpos")
env = dict(os.environ, PCPOS_PRINCIPAL="KirtusJustus", PCPOS_ACTOR="DirectorZ3")
p = subprocess.Popen([BIN, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env, cwd=HERE)


def call(i, method, params):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": method, "params": params}) + "\n")
    p.stdin.flush()
    return json.loads(p.stdout.readline())


call(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "z3-tasks", "version": "0"}})
p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
p.stdin.flush()
r = call(2, "tools/call", {"name": sys.argv[1], "arguments": dict(a.split("=", 1) for a in sys.argv[2:])})
p.stdin.close()
res = r.get("result", r)
print("\n".join(c.get("text", "") for c in res.get("content", [])) if isinstance(res, dict) and "content" in res else json.dumps(r)[:600])
sys.exit(1 if isinstance(res, dict) and res.get("isError") else 0)
