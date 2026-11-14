import json, glob, sys, collections
d = sys.argv[1]; arms = sys.argv[2].split(",")
R = [json.load(open(f)) for f in sorted(glob.glob(d + "/*.json"))]
by = collections.defaultdict(list)
for r in R: by[(r["probe"], r["arm"])].append(r)
probes = sorted({r["probe"] for r in R})
print("probe  " + "  ".join(f"{a:>7}" for a in arms))
tot = collections.Counter(); n = collections.Counter(); cost = collections.Counter(); noreply = 0
for p in probes:
    row = []
    for a in arms:
        rs = by[(p, a)]; clear = sum(not r["flags"] for r in rs); row.append(f"{clear}/{len(rs)}"); tot[a] += clear; n[a] += len(rs)
        cost[a] += sum((m.get("cost") or 0) for r in rs for m in r["meta"][-1:]); noreply += sum(any(x is None for x in r["replies"]) for r in rs)
    print(p + "    " + "  ".join(f"{x:>7}" for x in row))
print("clear  " + "  ".join(f"{tot[a]:>3}/{n[a]:<3}" for a in arms)); print("cost$  " + "  ".join(f"{cost[a]:7.2f}" for a in arms)); print("conversations with a missing reply:", noreply)
os_ = [r for r in R if r["arm"] == "pcp-os"]
print("pcp-os: registers created", sum(r["register_kinds"] is not None for r in os_), "of", len(os_), "; all verify:", all("0 problem(s)" in " ".join(r["verify"] or ["0 problem(s)"]) for r in os_))
