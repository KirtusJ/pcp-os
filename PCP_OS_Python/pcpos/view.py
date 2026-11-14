"""Views of the record: status, the State Brief, lists and the fidelity record. Reading changes nothing.

Output is compact by default: one fact per line, no decoration, so that it costs little to read into a context.
"""
from __future__ import annotations
from . import model, VERSION
from .base import canon


def _short(d):
    return d[:12]


def _clip(s, n):
    s = " ".join(s.split())
    return s if len(s) <= n else s[:n - 3] + "..."


def method_line(state):
    m = state.method
    if m is None:
        return "method: not pinned"
    how = "verified" if m["verified"].startswith("FILES") else "declared, not verified"
    pin = "required release" if m.get("matches_required") else "NOT the release this program requires"
    return "method: ACR release %s core %s (%s; %s)" % (_short(m["release_fingerprint"]), _short(m["core_descriptor_sha256"]), how, pin)


def review_lines(state, r, full=True, clip=100000):
    gates = r["required_gates"]
    nxt = next((g for g in gates if g not in r["passed_gates"]), None)
    out = ["review %s [%s: %s] %s session %d | committed: %s | next: %s" % (
        r["label"], r["procedure"], " ".join(gates), r["lifecycle"], r["session"], " ".join(r["passed_gates"]) or "none",
        ("%s %s" % (nxt, model.GATE_NAMES[nxt])) if nxt and r["lifecycle"] not in model.TERMINAL else "-")]
    t = r["target"]
    if t["t"] == "tree":
        tr = state.trees[t["name"]]
        out.append("  target: tree %s (%d files, manifest %s) extent %s" % (t["name"], tr["files"], _short(tr["manifest"]), r["extent"]))
    else:
        out.append("  target: artifact %s extent %s" % (_short(t["sha256"]), r["extent"]))
    if r.get("parent"):
        out.append("  parent: %s (as of event %d)" % (r["parent"], r["parent_basis"]["seq"]))
    if r["modules"]:
        out.append("  modules: " + ", ".join("%s %s" % (m["id"], m["version"]) for m in r["modules"]))
    for label in r["claim_order"]:
        c = r["claims"][label]
        sub = " substitute of %s" % c["substitute_of"] if c.get("substitute_of") else ""
        out.append("  claim %s [%s, %s%s]: %s" % (label, c["origin"], c["kind"], sub, _clip(c["text"], clip)))
        if c["criterion"]:
            out.append("    criterion: " + _clip(c["criterion"], clip))
        for f in model._current(state, r, label):
            out.append("    %s %s by %s | grounding %s | %s | %s%s" % (
                f["id"], f["disposition"], f["actor"], f["grounding"], f["execution"], f["relation"]["label"],
                " | conditions: %d" % len(f["conditions"]) if f["conditions"] else ""))
            if full:
                for cnd in f["conditions"]:
                    out.append("      depends on: " + cnd)
                if f["estimate"]:
                    out.append("      estimate (not a finding): " + _clip(f["estimate"], clip))
                if f["left_open"]:
                    out.append("      left open: " + _clip(f["left_open"], clip))
        cur = model._current(state, r, label)
        if len({f["disposition"] for f in cur}) > 1:
            out.append("    note: participants' findings on this claim differ; neither replaces the other (Core 5.2, DISPUTED)")
    for o in state.objections.values():
        if o["review"] == r["label"] and o["status"] in ("OPEN", "OPEN_RECORDED"):
            out.append("  objection %s %s%s: %s" % (o["id"], o["status"], " (claim %s)" % o["claim"] if o["claim"] else "", _clip(o["text"], clip)))
    return out


def status(state, events_n, head):
    out = ["PCP OS %s | events %d | head %s" % (VERSION, events_n, _short(head)), method_line(state),
           "ACR in use: %s" % ("YES" if state.adopted else "NO (ordinary work; nothing here asks or grants anything)")]
    for p in state.participants.values():
        out.append("participant %s: %s%s%s" % (p["label"], p["kind"], " principal" if p["principal"] else "",
                                             " | %s" % " / ".join(x for x in (p.get("name"), p.get("model"), p.get("lineage")) if x)
                                             if any(p.get(k) for k in ("name", "model", "lineage")) else ""))
    out.append("artifacts %d | trees %d | checks %d | findings %d | submissions %d | open obligations %d" % (
        len(state.artifacts), len(state.trees), len(state.checks), len(state.findings), len(state.submissions),
        sum(1 for o in state.obligations.values() if o["status"] == "OPEN")))
    for r in state.reviews.values():
        out += review_lines(state, r, full=False, clip=160)
    return "\n".join(out)


def brief(state, events_n, head, actor=None, review=None, max_bytes=None):
    """The State Brief (Core 8.1): the smallest projection sufficient for the next act, bound to its cutoff.

    Controlling conditions and claims are carried in their original words and are never shortened (Core 15.5).
    """
    head_lines = ["STATE BRIEF | cutoff: event %d, head %s | projection: open reviews, conditions, open objections and obligations" % (events_n - 1, head),
                  method_line(state), "ACR in use: %s" % ("YES" if state.adopted else "NO")]
    if actor and actor in state.participants:
        p = state.participants[actor]
        head_lines.append("acting: %s (%s%s)" % (actor, p["kind"], ", " + p["lineage"] if p.get("lineage") else ""))
    fixed = []
    for c in state.conditions.values():
        if review is None or c.get("review") in (None, review):
            fixed.append("condition %s [from %s]%s: %s" % (c["id"], c["origin"], " (review %s)" % c["review"] if c.get("review") else "", c["text"]))
    reviews = [r for r in state.reviews.values() if (review is None and r["lifecycle"] not in model.TERMINAL) or r["label"] == review]
    body = []
    for r in reviews:
        body += review_lines(state, r, full=True)
    tail = []
    for o in state.obligations.values():
        if o["status"] == "OPEN" and (review is None or o.get("review") in (None, review)):
            tail.append("obligation %s: %s" % (o["id"], o["text"]))
    if not reviews:
        tail.append("no open review" if review is None else "no review labelled %s" % review)
    text = "\n".join(head_lines + fixed + body + tail)
    if max_bytes and len(text.encode("utf-8")) > max_bytes:
        # shorten only what is not controlling: drop estimates and left-open notes, then per-finding lines
        keep = [l for l in body if not l.lstrip().startswith(("estimate", "left open"))]
        text = "\n".join(head_lines + fixed + keep + tail)
        if len(text.encode("utf-8")) > max_bytes:
            keep = [l for l in keep if l.startswith(("review", "  claim", "    criterion", "  objection", "  target")) or "depends on" in l]
            text = "\n".join(head_lines + fixed + keep + tail)
        text += "\n[shortened to fit %d bytes: findings detail omitted; conditions, claims, criteria and objections are complete. Full: pcpos brief]" % max_bytes
    return text


def listing(state, what):
    out = []
    if what == "artifacts":
        for d, a in state.artifacts.items():
            out.append("%s %8d %s %s%s%s" % (_short(d), a["size"], a["origin"], ",".join(a["names"]) or "-",
                                             "" if a["stored"] else " (reference only)",
                                             " <%s>" % a["source"]["url"] if a.get("source") else ""))
    elif what == "trees":
        for n, t in state.trees.items():
            out.append("%s %d files %d bytes manifest %s %s%s" % (n, t["files"], t["bytes"], _short(t["manifest"]), t["container"],
                                                                " stored" if t.get("stored") else " (reference only)"))
    elif what == "checks":
        for k in state.checks.values():
            out.append("%s exit %s (expected %s)%s %dms %s | %s" % (k["id"], k["exit"], k["expect_exit"], " TIMEOUT" if k["timeout"] else "",
                                                                 k["duration_ms"], k["name"] or "-", " ".join(k["argv"])[:120]))
    elif what == "findings":
        for f in state.findings.values():
            out.append("%s %s/%s %s by %s | %s | %s | %s%s" % (f["id"], f["review"], f["claim"], f["disposition"], f["actor"], f["grounding"],
                                                              f["execution"], f["relation"]["label"], " | supersedes %s" % f["supersedes"] if f["supersedes"] else ""))
    elif what == "objections":
        for o in state.objections.values():
            out.append("%s %s %s: %s" % (o["id"], o["review"], o["status"], _clip(o["text"], 160)))
    elif what == "conditions":
        for c in state.conditions.values():
            out.append("%s [from %s]: %s" % (c["id"], c["origin"], c["text"]))
    elif what == "obligations":
        for o in state.obligations.values():
            out.append("%s %s: %s" % (o["id"], o["status"], _clip(o["text"], 200)))
    elif what == "submissions":
        for s in state.submissions.values():
            out.append("%s events %d head %s %s objects %d" % (s["label"], s["events"], _short(s["head"]), s["compatibility"], s["objects"]))
    elif what == "participants":
        for p in state.participants.values():
            out.append("%s %s%s %s" % (p["label"], p["kind"], " principal" if p["principal"] else "", p.get("lineage") or "-"))
    elif what == "runs":
        for r in state.runs.values():
            out.append("%s %s %s exposure %d item(s) trace %s output %s" % (r["id"], r.get("provider"), r.get("model"), len(r.get("exposure", [])),
                                                                         r.get("trace"), _short(r["output"]) if r.get("output") else "-"))
    return "\n".join(out) if out else "(none)"


def fidelity(state, review=None, delivery="TOOL_ASSISTED", enforced=None):
    """A Core 15.8 record written from the register. Values the record cannot establish are UNKNOWN."""
    m = state.method
    extent = []
    if m:
        extent.append("acr.core 1.0 core-descriptor %s release %s (%s)" % (m["core_descriptor_sha256"], m["release_fingerprint"],
                                                                         "verified" if m["verified"].startswith("FILES") else "declared"))
    fs = [f for f in state.findings.values() if review is None or f["review"] == review]
    mods = {}
    for r in state.reviews.values():
        if review is None or r["label"] == review:
            for x in r["modules"]:
                mods[x["id"]] = "%s %s %s" % (x["id"], x["version"], x["sha256"])
    extent += sorted(mods.values())
    order = model.GROUNDING
    grounding = order[min(order.index(f["grounding"]) for f in fs)] if fs else "UNKNOWN"
    rec = {"delivery": delivery, "extent": extent or ["NONE_PINNED"],
           "grounding": grounding, "execution": ("EXECUTED" if fs and all(f["execution"] == "EXECUTED" for f in fs) else
                                                 "REASONED" if fs else "UNKNOWN"),
           "observability": "BLACK_BOX", "reasoning_trace": "UNKNOWN",
           "claims": [{"id": f["id"], "review": f["review"], "claim": f["claim"], "disposition": f["disposition"], "grounding": f["grounding"],
                       "execution": f["execution"], "relation": f["relation"]["label"],
                       "kind": state.reviews[f["review"]]["claims"][f["claim"]]["kind"]} for f in fs],
           "written_from": "the register, by PCP OS %s; grounding is the weakest among the listed findings; a value the record cannot establish is UNKNOWN" % VERSION}
    if enforced:
        rec["enforced_controls"] = enforced
    return rec
