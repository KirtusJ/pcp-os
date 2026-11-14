#!/usr/bin/env python3
"""Do all implementations agree on what the modules are? Identifier, version and digest of each, from five places.

    python3 check_modules.py <method dir> <pcpos program> <PCP OS Python dir> <machine form json> <a built plugin dir>
"""
import csv, hashlib, json, re, subprocess, sys
from pathlib import Path
METHOD, PROGRAM, PY, MACHINE, PLUGIN = (Path(a) for a in sys.argv[1:6])
views = {}
# 1. The native host program: discovered from each Specification's header.
out = subprocess.run([str(PROGRAM), "modules", str(METHOD)], capture_output=True, text=True).stdout
views["PCP OS host program (Rust)"] = {f[1]: (f[2], f[5]) for f in (l.split() for l in out.splitlines()) if len(f) >= 6 and f[1].startswith("acr.")}
# 2. The method packet's own lookup tool.
sys.path.insert(0, str(METHOD / "TOOLS")); import acr
views["method packet tool (Python)"] = {acr.identity(k, METHOD)[0]: acr.identity(k, METHOD)[1:] for k in acr.COMPONENTS}
# 3. The second implementation of PCP OS.
sys.path.insert(0, str(PY)); from pcpos import method as pm
m = pm.Method(METHOD, pm.verify_dir(METHOD))
views["PCP OS second implementation (Python)"] = {x["id"]: (x["version"], x["sha256"]) for x in m.modules()}
# 4. The machine form.
views["machine form"] = {x["id"]: (x["version"], x["specification"]["sha256"]) for x in json.loads(MACHINE.read_text())["modules"]}
# 5. The catalogue in the method packet (identifier and version only) and the plugin's module table.
cat = {r["component_id"]: r["component_version"] for r in csv.DictReader((METHOD / "REFERENCE/Module_Catalog.csv").open())}
table = dict(re.findall(r"`(acr\.[a-z_]+)` ([0-9.]+)", (PLUGIN / "skills/modules/SKILL.md").read_text()))
ref = views["PCP OS host program (Rust)"]
bad = []
for name, v in views.items():
    if v != ref:
        bad.append((name, sorted(set(v.items()) ^ set(ref.items()))[:4]))
if cat != {k: v[0] for k, v in ref.items()}: bad.append(("Module_Catalog.csv", cat))
if table != {k: v[0] for k, v in ref.items()}: bad.append(("plugin modules skill", table))
# Each file is the one the packet's manifest lists.
listed = {f["path"]: f["sha256"] for f in json.loads((METHOD / "MANIFEST.json").read_text())["files"]}
for x in json.loads(MACHINE.read_text())["modules"]:
    if listed.get(x["specification"]["path"]) != x["specification"]["sha256"]: bad.append(("manifest", x["id"]))
print("%d modules; %d sources agree on identifier, version and digest: %s" % (len(ref), len(views) + 2, ", ".join(list(views) + ["Module_Catalog.csv (identifier, version)", "plugin modules skill (identifier, version)"])))
for k, (v, d) in ref.items(): print("  %-24s %s  %s" % (k, v, d))
if bad:
    print("DISAGREE:", bad); sys.exit(1)
