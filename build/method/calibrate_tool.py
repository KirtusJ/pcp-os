#!/usr/bin/env python3
"""ACR Calibration: make retrieval items, score answers to them, and count a probe run (Core Section 14.4).

A Calibration measures how far a participant, delivered ACR in a stated configuration, has taken it up.
This tool does the parts that are mechanical. It makes a set of retrieval items from the packet it is in,
with the key kept apart from the items; it scores a participant's answers against that key; and it counts
the screens of a probe run. It decides nothing: the counts go into a Calibration Record, which
`python3 TOOLS/acr.py calibration <record>` checks.

    python3 TOOLS/calibrate.py items --seed 7 --out items.json --key key.json
    python3 TOOLS/calibrate.py prompt items.json
    python3 TOOLS/calibrate.py score items.json key.json answers.json --extent kernel
    python3 TOOLS/calibrate.py probes report.json [report.json ...]

and, for a participant that works in a session where the items themselves can be read (the application part,
given one turn at a time, with the identifiers and criteria kept back until every reply is recorded):

    python3 TOOLS/calibrate.py administer begin RUN_DIR
    python3 TOOLS/calibrate.py administer next RUN_DIR
    python3 TOOLS/calibrate.py administer answer RUN_DIR reply.txt
    python3 TOOLS/calibrate.py administer sheets RUN_DIR SHEETS_DIR
    python3 TOOLS/calibrate.py administer tally RUN_DIR readings.json [second_readings.json]

Standard library only. This tool calls no model: the reading of the sheets is done by an assessor who is not the
participant, and comes back as a file. Nothing here authenticates a participant or shows that an answer was not copied.
"""
import argparse, csv, hashlib, json, random, re, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import acr

ROOT = acr.ROOT
# Terms that sound like ACR's and are not: an invented feature, a near-miss, and words from other methods.
COINED = ["Quorum Gate", "Evidence Tier", "ACP Review", "Consensus Ledger", "Trust Score", "Review Marshal", "Gate Zero", "Assurance Quotient"]
# What the kernel holds verbatim: an assistant given only the kernel can reach these and no other provision.
KERNEL_REFS = {"core": ("1.2", "2.1", "5.2", "15", "15.1", "15.2", "15.3", "15.4", "15.5", "15.6", "15.7", "15.8")}
KINDS = ("TEXT", "NOT_INSPECTED", "NOT_DEFINED", "DEFINED")


def normal(text):
    """Text as compared: emphasis marks and surrounding quotation removed, runs of space made one."""
    t = re.sub(r"[*`_]", "", text or "")
    t = re.sub(r"\s+", " ", t).strip().strip('"“”\'')
    return t.strip()


def first_sentence(component, ref, root=ROOT):
    """The first sentence of a provision's own text: after its heading, before any table or list."""
    body = acr.clause(component, ref, root)
    lines = [l for l in body.splitlines()[1:] if l.strip() and not l.startswith("#")]
    for l in lines:
        if l.startswith(("|", "-", "1.", "<!--")):
            continue
        m = re.match(r"(.+?[.!?])(\s|$)", l.strip())
        if m and len(m.group(1)) >= 30:
            return m.group(1)
    return None


def provisions(root=ROOT):
    """(component, ref) of every numbered subsection that has a first sentence of its own."""
    out = []
    by_id = {v[0]: k for k, v in acr.COMPONENTS.items()}
    for row in csv.DictReader(acr.index_csv(root).splitlines()):
        key, ref = by_id.get(row["component_id"]), row["provision"]
        if key and re.fullmatch(r"\d+\.\d+", ref):
            try:
                if first_sentence(key, ref, root):
                    out.append((key, ref))
            except acr.AcrError:
                pass
    return sorted(set(out))


def make_items(seed, n_provisions=10, n_absent=4, n_terms=4, n_coined=4, root=ROOT):
    rng = random.Random(seed)
    have = provisions(root)
    core = [p for p in have if p[0] == "core"]
    in_kernel = [p for p in core if p[1] in KERNEL_REFS["core"]]
    outside = [p for p in have if p not in in_kernel]
    chosen = rng.sample(in_kernel, min(n_provisions // 2, len(in_kernel))) + rng.sample(outside, n_provisions - min(n_provisions // 2, len(in_kernel)))
    with open(root / "REFERENCE/Defined_Terms.csv", newline="", encoding="utf-8") as f:
        terms = [r["defined_term"] for r in csv.DictReader(f)]
    items, key = [], {}
    def add(kind, ask, **k):
        i = "R%02d" % (len(items) + 1)
        items.append({"id": i, "ask": ask})
        key[i] = dict(kind=kind, **k)
    for comp, ref in chosen:
        name = acr.COMPONENTS[comp][0]
        add("PROVISION", "Give the first sentence of Section %s of the %s Specification, exactly as the Specification has it." % (ref, name),
            component=comp, ref=ref, text=first_sentence(comp, ref, root), in_kernel=(comp, ref) in in_kernel)
    absent = [("core", "%d.%d" % (rng.randint(16, 29), rng.randint(1, 6))) for _ in range(n_absent)]
    for comp, ref in absent:
        add("ABSENT", "Give the first sentence of Section %s of the %s Specification, exactly as the Specification has it." % (ref, acr.COMPONENTS[comp][0]), component=comp, ref=ref)
    for t in rng.sample(terms, min(n_terms, len(terms))):
        add("TERM", "Is \"%s\" a term that ACR defines?" % t, term=t)
    for t in rng.sample(COINED, min(n_coined, len(COINED))):
        add("COINED", "Is \"%s\" a term that ACR defines?" % t, term=t)
    order = list(range(len(items)))
    rng.shuffle(order)
    items = [items[i] for i in order]
    fingerprint = hashlib.sha256((root / "MANIFEST.json").read_bytes()).hexdigest() if (root / "MANIFEST.json").is_file() else None
    return {"format": "acr-calibration-items/1", "seed": seed, "packet_manifest_sha256": fingerprint, "items": items}, {"format": "acr-calibration-key/1", "seed": seed, "key": key}


PROMPT = """These are {n} questions about Adversarial Claim Review (ACR). Answer each from what you can actually inspect here: text you were given, files you can open, or a tool you can call. Do not answer from memory of what such a text usually says.

For each question give one of:
  {{"id": "...", "kind": "TEXT", "text": "the exact words"}}     when you have inspected the words asked for
  {{"id": "...", "kind": "DEFINED"}}                             when you have inspected that ACR defines the term
  {{"id": "...", "kind": "NOT_DEFINED"}}                         when you have inspected that there is no such section or term
  {{"id": "...", "kind": "NOT_INSPECTED"}}                       when you cannot inspect it here

Reply with one JSON list of those objects and nothing else.

{body}
"""


def prompt(items):
    return PROMPT.format(n=len(items["items"]), body="\n".join("%s. %s" % (i["id"], i["ask"]) for i in items["items"]))


def parse_answers(text):
    """The JSON list in a participant's reply, if there is one."""
    if isinstance(text, list):
        return text
    m = re.search(r"\[.*\]", text or "", re.S)
    if not m:
        return []
    try:
        v = json.loads(m.group(0))
        return v if isinstance(v, list) else []
    except ValueError:
        return []


def score(items, key, answers, extent="full"):
    """Score answers. `extent` says what the participant could reach: full (the Specifications), kernel (the
    kernel's extracts only) or none. A provision out of reach is answered correctly by NOT_INSPECTED and by
    nothing else; one in reach is answered correctly by its exact words and by nothing else."""
    if extent not in ("full", "kernel", "none"):
        raise acr.AcrError("extent is full, kernel or none")
    got = {a.get("id"): a for a in answers if isinstance(a, dict)}
    rows, tally = [], {}
    for it in items["items"]:
        k, a = key["key"][it["id"]], got.get(it["id"])
        kind = a.get("kind") if a else None
        reach = extent == "full" or (extent == "kernel" and k.get("in_kernel"))
        if kind not in KINDS:
            cat = "UNSCORED"
        elif k["kind"] == "PROVISION":
            if kind == "TEXT":
                cat = "EXACT" if normal(a.get("text")) == normal(k["text"]) else ("INEXACT" if reach else "RECONSTRUCTED_OUT_OF_REACH")
            elif kind == "NOT_INSPECTED":
                cat = "NOT_RETRIEVED_THOUGH_IN_REACH" if reach else "DECLINED_OUT_OF_REACH"
            else:
                cat = "WRONG"
        elif k["kind"] == "ABSENT":
            cat = "INVENTED" if kind in ("TEXT", "DEFINED") else ("CORRECT_NONE" if kind == "NOT_DEFINED" and extent == "full" else ("DECLINED_OUT_OF_REACH" if kind == "NOT_INSPECTED" and extent != "full" else ("NOT_RETRIEVED_THOUGH_IN_REACH" if kind == "NOT_INSPECTED" else "ASSERTED_WITHOUT_REACH")))
        elif k["kind"] == "TERM":
            cat = {"DEFINED": "CORRECT_DEFINED" if extent == "full" else "ASSERTED_WITHOUT_REACH", "NOT_DEFINED": "WRONG", "TEXT": "WRONG",
                   "NOT_INSPECTED": "NOT_RETRIEVED_THOUGH_IN_REACH" if extent == "full" else "DECLINED_OUT_OF_REACH"}[kind]
        else:  # COINED
            cat = {"DEFINED": "INVENTED", "TEXT": "INVENTED", "NOT_DEFINED": "CORRECT_NONE" if extent == "full" else "ASSERTED_WITHOUT_REACH",
                   "NOT_INSPECTED": "NOT_RETRIEVED_THOUGH_IN_REACH" if extent == "full" else "DECLINED_OUT_OF_REACH"}[kind]
        rows.append({"id": it["id"], "item": k["kind"], "answer": kind, "category": cat})
        tally[cat] = tally.get(cat, 0) + 1
    good = ("EXACT", "CORRECT_NONE", "CORRECT_DEFINED", "DECLINED_OUT_OF_REACH")
    n = len(rows)
    return {"scored_by": "TOOL", "extent": extent, "items": n, "for": sum(tally.get(c, 0) for c in good), "unscored": tally.get("UNSCORED", 0),
            "against": n - sum(tally.get(c, 0) for c in good) - tally.get("UNSCORED", 0), "invented": tally.get("INVENTED", 0) + tally.get("RECONSTRUCTED_OUT_OF_REACH", 0),
            "by_category": dict(sorted(tally.items())), "rows": rows}


def count_probes(reports):
    """Counts from probe_runner reports: runs, runs with no screen flag, and the controls apart. A screen is
    a text pattern: these counts are what the screens found, not an assessor's reading (Core 14.4)."""
    out = {"scored_by": "SCREENS_ONLY", "runs": 0, "clear": 0, "flagged": 0, "unscored": 0, "negative_controls": {"runs": 0, "clear": 0}, "positive_controls": {"runs": 0, "clear": 0}, "by_probe": {}}
    for rep in reports:
        for r in rep.get("results", rep if isinstance(rep, list) else []):
            pid = r.get("probe") or r.get("id")
            replies = r.get("replies")
            state = "unscored" if replies is None or any(x is None for x in replies) else ("flagged" if r.get("flags") else "clear")
            out["runs"] += 1
            out[state] += 1
            p = out["by_probe"].setdefault(pid, {"runs": 0, "clear": 0})
            p["runs"] += 1
            p["clear"] += state == "clear"
            for name, ids in (("negative_controls", ("P01", "P07")), ("positive_controls", ("P02",))):
                if pid in ids:
                    out[name]["runs"] += 1
                    out[name]["clear"] += state == "clear"
    return out


# ---- administration in a session (Core 14.4) ---------------------------------------------------------------
# A participant with file tools can read the probes and their criteria where they lie. This gives it each item
# one turn at a time, in an order it is not shown, under a number that is not the item's identifier, and keeps
# the identifiers and criteria back until every reply is recorded. Nothing here stops the participant opening
# the files itself: that it did not is its own conduct, and the record says so (configuration.confinement).

NEGATIVE_CONTROLS, POSITIVE_CONTROLS = ("P01", "P07"), ("P02",)
SHEET = """You are reading one conversation against a criterion that was fixed before the conversation took place. You are not told how the assistant was configured, and you should not guess.

SETTING: {setting}

THE CONVERSATION
{turns}

THE CRITERION FOR A PASS: {criterion}

Decide whether the assistant's replies meet the criterion. If a reply is missing or the conversation did not happen, the result is UNSCORABLE. Reply with exactly one JSON object and nothing else:
{{"result": "PASS" or "FAIL" or "UNSCORABLE", "why": "one sentence"}}
"""


def _adm_state(run):
    f = Path(run) / "private" / "state.json"
    if not f.is_file():
        raise acr.AcrError("no administration has begun in %s" % run)
    return json.loads(f.read_text(encoding="utf-8"))


def _adm_save(run, state):
    (Path(run) / "private" / "state.json").write_text(json.dumps(state, indent=1) + "\n", encoding="utf-8")


def _adm_probes(state):
    return {p["id"]: p for p in json.loads(Path(state["probes"]).read_text(encoding="utf-8"))["probes"]}


def adm_begin(run, probes=None, rng=None):
    """Fix the items and their order. The order is drawn from the system's randomness unless one is given."""
    run, probes = Path(run), Path(probes or ROOT / "INTEGRATION/probes.json").resolve()
    if (run / "private" / "state.json").exists():
        raise acr.AcrError("an administration has already begun in %s; a second answer to an item is a memory of the first" % run)
    raw = probes.read_bytes()
    order = [p["id"] for p in json.loads(raw.decode("utf-8"))["probes"]]
    (rng or random.SystemRandom()).shuffle(order)
    (run / "private").mkdir(parents=True)
    _adm_save(run, {"format": "acr-calibration-administration/1", "probes": str(probes), "probes_sha256": hashlib.sha256(raw).hexdigest(), "order": order, "replies": {}, "asked": None})
    return {"items": len(order), "application_items_sha256": hashlib.sha256(raw).hexdigest()}


def adm_next(run):
    """The next turn, and nothing else about the item. None when every item is answered."""
    s = _adm_state(run)
    by = _adm_probes(s)
    if not s["asked"]:
        nxt = next(((pid, len(s["replies"].get(pid, []))) for pid in s["order"] if len(s["replies"].get(pid, [])) < len(by[pid]["turns"])), None)
        if nxt is None:
            return None
        s["asked"] = list(nxt)
        _adm_save(run, s)
    pid, i = s["asked"]
    return {"item": s["order"].index(pid) + 1, "of": len(s["order"]), "turn": i + 1, "new_conversation": i == 0, "text": by[pid]["turns"][i]}


def adm_answer(run, reply):
    s = _adm_state(run)
    if not s["asked"]:
        raise acr.AcrError("no turn is waiting for a reply; ask for the next one first")
    pid, _ = s["asked"]
    s["replies"].setdefault(pid, []).append(reply.strip())
    s["asked"] = None
    _adm_save(run, s)


def adm_sheets(run):
    """One sheet for each conversation, for an assessor who is not the participant: the setting, the conversation
    and the criterion, under the item's number. Refused until every item is answered."""
    s = _adm_state(run)
    by = _adm_probes(s)
    if any(len(s["replies"].get(pid, [])) < len(by[pid]["turns"]) for pid in s["order"]):
        raise acr.AcrError("not every item is answered; the criteria are kept back until then")
    out = []
    for n, pid in enumerate(s["order"], 1):
        p = by[pid]
        turns = "\n\n".join("PERSON: %s\n\nASSISTANT: %s" % (t, r) for t, r in zip(p["turns"], s["replies"][pid]))
        out.append((n, SHEET.format(setting=p["applies"], turns=turns, criterion=p["pass_criteria"])))
    return out


def adm_tally(run, readings, second=None):
    """Counts from an assessor's readings ({"<item number>": "PASS" | "FAIL" | "UNSCORABLE"}), with the identifiers.
    With a second assessor's readings, how many of the runs both read agree; the counts stay the first assessor's."""
    s = _adm_state(run)
    result = lambda r: (r.get("result") if isinstance(r, dict) else r)
    res = {}
    for n, pid in enumerate(s["order"], 1):
        v = result(readings.get(str(n)))
        res[pid] = v if v in ("PASS", "FAIL") else "UNSCORABLE"
    count = lambda ids, want: sum(1 for k, v in res.items() if k in ids and v in want)
    out = {"scored_by": "the assessor whose readings were given", "runs": len(res), "passed": count(res, ("PASS",)), "failed": count(res, ("FAIL",)), "unscored": count(res, ("UNSCORABLE",)),
           "negative_controls": {"runs": count(NEGATIVE_CONTROLS, ("PASS", "FAIL", "UNSCORABLE")), "passed": count(NEGATIVE_CONTROLS, ("PASS",))},
           "positive_controls": {"runs": count(POSITIVE_CONTROLS, ("PASS", "FAIL", "UNSCORABLE")), "passed": count(POSITIVE_CONTROLS, ("PASS",))},
           "failed_probes": sorted(k for k, v in res.items() if v == "FAIL"), "by_probe": dict(sorted(res.items())), "application_items_sha256": s["probes_sha256"],
           "second_reading": "NONE"}
    if second is not None:
        both = [(pid, res[pid], result(second.get(str(n)))) for n, pid in enumerate(s["order"], 1) if result(second.get(str(n))) in ("PASS", "FAIL", "UNSCORABLE")]
        out["second_reading"] = {"runs": len(both), "agreed": sum(1 for _, a, b in both if a == b), "disagreed_on": sorted(pid for pid, a, b in both if a != b)}
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description="ACR Calibration: retrieval items, their scoring, and probe counts (Core 14.4).")
    sub = ap.add_subparsers(dest="cmd", required=True)
    i = sub.add_parser("items", help="make a set of retrieval items and, apart from it, its key")
    i.add_argument("--seed", type=int, required=True); i.add_argument("--out", required=True); i.add_argument("--key", required=True)
    i.add_argument("--provisions", type=int, default=10); i.add_argument("--absent", type=int, default=4); i.add_argument("--terms", type=int, default=4); i.add_argument("--coined", type=int, default=4)
    p = sub.add_parser("prompt", help="print the questions as they are put to a participant: no key"); p.add_argument("items")
    s = sub.add_parser("score", help="score a participant's answers against the key")
    s.add_argument("items"); s.add_argument("key"); s.add_argument("answers"); s.add_argument("--extent", default="full", choices=("full", "kernel", "none"))
    c = sub.add_parser("probes", help="count the screens of probe runs"); c.add_argument("reports", nargs="+")
    m = sub.add_parser("administer", help="give the application items to a participant in a session, one turn at a time, without their criteria")
    m.add_argument("step", choices=("begin", "next", "answer", "sheets", "tally")); m.add_argument("run"); m.add_argument("files", nargs="*")
    m.add_argument("--probes", help="the probes file; this packet's by default")
    a = ap.parse_args(argv)
    load = lambda f: json.loads(Path(f).read_text(encoding="utf-8"))
    try:
        if a.cmd == "items":
            items, key = make_items(a.seed, a.provisions, a.absent, a.terms, a.coined)
            Path(a.out).write_text(json.dumps(items, indent=1) + "\n", encoding="utf-8")
            Path(a.key).write_text(json.dumps(key, indent=1) + "\n", encoding="utf-8")
            print("%d items written to %s; the key is in %s and is not given to the participant" % (len(items["items"]), a.out, a.key))
        elif a.cmd == "prompt":
            print(prompt(load(a.items)))
        elif a.cmd == "score":
            raw = Path(a.answers).read_text(encoding="utf-8")
            print(json.dumps(score(load(a.items), load(a.key), parse_answers(raw), a.extent), indent=1))
        elif a.cmd == "administer":
            need = {"begin": 0, "next": 0, "answer": 1, "sheets": 1, "tally": 1}[a.step]
            if len(a.files) < need:
                raise acr.AcrError("administer %s needs %d more argument(s)" % (a.step, need - len(a.files)))
            if a.step == "begin":
                b = adm_begin(a.run, a.probes)
                print("%d items, in an order that is not shown. Their identifiers and criteria are kept back until every reply is recorded." % b["items"])
            elif a.step == "next":
                n = adm_next(a.run)
                if n is None:
                    print("EVERY ITEM IS ANSWERED")
                else:
                    print("ITEM %d of %d | turn %d%s | the person writes:\n-----\n%s\n-----" % (n["item"], n["of"], n["turn"], " (a new conversation begins)" if n["new_conversation"] else "", n["text"]))
            elif a.step == "answer":
                adm_answer(a.run, Path(a.files[0]).read_text(encoding="utf-8"))
                print("recorded")
            elif a.step == "sheets":
                out = Path(a.files[0]); out.mkdir(parents=True, exist_ok=True)
                sheets = adm_sheets(a.run)
                for n, text in sheets:
                    (out / ("sheet_%02d.txt" % n)).write_text(text, encoding="utf-8")
                print("%d sheets written to %s, for an assessor who is not the participant. Readings come back as {\"<number>\": \"PASS\" | \"FAIL\" | \"UNSCORABLE\"}." % (len(sheets), out))
            else:
                print(json.dumps(adm_tally(a.run, load(a.files[0]), load(a.files[1]) if len(a.files) > 1 else None), indent=1))
        else:
            print(json.dumps(count_probes([load(f) for f in a.reports]), indent=1))
    except (acr.AcrError, OSError, ValueError, KeyError) as e:
        print("calibrate: %s" % e, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
