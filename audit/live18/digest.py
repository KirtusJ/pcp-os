"""Reduce each stream-json transcript to what was asked, which tools were called, what each was told, and the reply."""
import json, sys, glob, os
for f in sorted(glob.glob(os.path.join(os.path.dirname(os.path.abspath(__file__)), "*.jsonl"))):
    calls = {}; out = [f"== {os.path.basename(f)}"]
    for l in open(f, errors="replace"):
        try: j = json.loads(l)
        except Exception: continue
        if j.get("type") == "system" and j.get("subtype") == "init":
            out.append(f"model {j.get('model')}; plugins {[p.get('name') for p in j.get('plugins', [])]}; skills {len(j.get('skills', []))}; tools {len(j.get('tools', []))}")
        if j.get("type") == "assistant":
            for c in j["message"]["content"]:
                if c.get("type") == "tool_use": calls[c["id"]] = c["name"].split("__")[-1]; out.append("CALL " + calls[c["id"]] + " " + json.dumps(c["input"])[:260])
        if j.get("type") == "user" and isinstance(j["message"]["content"], list):
            for c in j["message"]["content"]:
                if isinstance(c, dict) and c.get("type") == "tool_result":
                    x = c.get("content"); x = x if isinstance(x, str) else json.dumps(x)
                    out.append("  -> " + x[:260].replace("\\n", " / "))
        if j.get("type") == "result": out.append("REPLY " + (j.get("result") or "")[:1500]); out.append(f"cost ${j.get('total_cost_usd')}, turns {j.get('num_turns')}")
    open(f[:-6] + ".txt", "w").write("\n".join(out) + "\n")
