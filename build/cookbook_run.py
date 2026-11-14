"""Run the documented event bodies through the packaged program, as a host would. Every one must be written."""
import json, os, subprocess, sys, tempfile, pathlib
plugin, cookbook = pathlib.Path(sys.argv[1]).resolve(), json.load(open(sys.argv[2]))
root = pathlib.Path(tempfile.mkdtemp(prefix="pcp-cookbook-")).resolve()
env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
env.update(CLAUDE_PLUGIN_ROOT=str(plugin), CLAUDE_PROJECT_DIR=str(root))
exe = str(plugin / "servers/pcpos")
def hook(event, obj):
    return subprocess.run([exe, "hook", event], input=json.dumps(obj).encode(), env=env, cwd=root, capture_output=True).stdout.decode()
def tools(calls):
    lines = [json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}}), json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})]
    lines += [json.dumps({"jsonrpc": "2.0", "id": i + 2, "method": "tools/call", "params": {"name": n, "arguments": a}}) for i, (n, a) in enumerate(calls)]
    out = subprocess.run([exe, "mcp"], input=("\n".join(lines) + "\n").encode(), env=dict(env, PCPOS_ROOT=str(root)), cwd=root, capture_output=True).stdout.decode().splitlines()
    init, listed = json.loads(out[0]), json.loads(out[1])
    assert init["result"]["protocolVersion"] == "2025-11-25", init
    return [t["name"] for t in listed["result"]["tools"]], [json.loads(l)["result"]["content"][0]["text"] for l in out[2:]]
said = hook("prompt", {"hook_event_name": "UserPromptExpansion", "expansion_type": "slash_command", "command_name": "pcp-os:review", "command_args": "is the proof in proof.md complete", "prompt": "x"})
assert "adopted" in said, said
names, res = tools([("acr_submit", {"kind": k, "body": b}) for k, b in cookbook] + [("acr_status", {}), ("acr_verify", {})])
bad = [(cookbook[i][0], r) for i, r in enumerate(res[:len(cookbook)]) if not r.startswith("WRITTEN")]
assert not bad, bad
assert "guards replayed: 0 problem(s)" in res[-1] and "CLOSED_COMPLETE" in res[-2], res[-2:]
reg = [json.loads(l) for l in open(root / ".pcp/register.jsonl")]
pin = next(e for e in reg if e["kind"] == "METHOD_PINNED")["body"]
assert pin["verified"] == "FILES_VERIFIED_AGAINST_OWN_MANIFEST" and pin["presence"] == "NATIVE", pin
env_ev = next(e for e in reg if e["kind"] == "HOST_EVENT")["body"]
print("tools:", " ".join(names)); print("edition recorded:", env_ev["edition"]); print("method:", pin["verified"]); print(len(cookbook), "documented events written;", len(reg), "events in the register"); print(res[-2])
