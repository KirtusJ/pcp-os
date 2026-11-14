#!/usr/bin/env python3
"""Twelfth round: the edition texts that the reading copies are made from.

    python3 edit_editions_round12.py <build dir holding edition_texts.py>
"""
import re, sys
from pathlib import Path
p = Path(sys.argv[1]) / "edition_texts.py"; t = p.read_text(encoding="utf-8")
def rep(a, b, n=1):
    global t
    assert t.count(a) == n, (a[:80], t.count(a)); t = t.replace(a, b)
rep("Without it, the assistant is required to ask what you mean (Core Section 15.6).",
    "Without it, the name has no ACR meaning to the assistant, which reads it as ordinary text and asks only if that would change what it does next (Core Section 15.6).")
rep("An assistant that holds only the Implementation packet does not know the product name and is required to ask what was meant.",
    "An assistant that holds only the Implementation packet does not know the product name: it gives the name no ACR meaning and asks only if that would change what it does next.")
n = t.count("It is named ACR throughout and is the only part an AI assistant should be given.")
rep("It is named ACR throughout and is the only part an AI assistant should be given.", "It is named ACR throughout and is the part an AI assistant is given as text.", n)
lines = t.split("\n"); out = []; added = 0
for line in lines:
    out.append(line)
    if line.startswith("- `PCP_1.0_Readers.zip`"):
        out.append("- `PCP_1.0_Plugins/`, `PCP_1.0_Machine/` and `PCP_1.0_OS/`: the two plugins for Claude, the release as data with the design profiles, and the PCP OS documents. `README.md` describes each. With Claude Code, a plugin is the shorter path than this guide.")
        added += 1
assert added == 3, added
p.write_text("\n".join(out), encoding="utf-8")
print("edition texts edited:", n, "guides;", added, "lists")
