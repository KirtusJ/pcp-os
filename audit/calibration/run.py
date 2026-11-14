#!/usr/bin/env python3
"""A Calibration (Core Section 14.4) of the ways PCP 1.0 delivers ACR to an assistant in Claude Code.

    python3 run.py <method packet dir> <plugins root holding pcp/ and pcp-os/> <os-python dir, unused> <out dir> <step> [models] [arms]

One subject is one model in one arm of delivery:
    none     no ACR text                                   (nothing in reach)
    card     KERNEL_CARD.md as system text, ABSORBED_TEXT  (no provision in reach: the card paraphrases)
    kernel   KERNEL.md as system text, ABSORBED_TEXT       (the kernel's extracts in reach)
    pcp      the text-only plugin, RETRIEVAL               (the Specifications in reach, as files)
    pcp-os   the plugin with the tool server, TOOL_ASSISTED (the Specifications in reach, through acr_clause)

Steps, in order: `hidden` (a search, from inside the namespace the subjects run in, for any text of the probes:
it must find none, and is made before and after), `retrieval` (the packet's own items, scored by the packet's own
tool), `probes` (the 26 behavioural probes through audit/live/matrix.py), `read` (each probe conversation read
against the probe's criterion by two other models, neither told the arm nor shown the other's reading), `records`
(one Calibration Record for each subject, checked by the packet's tool, and a table).

What a subject can reach (since build 22): its tools are as the arm has them and are not confined. The folders and
files named in PCP_HIDE are empty in the namespace each subject session runs in (hidden.sh). No arm has a shell or
a write tool.

THE THRESHOLDS, fixed here before any run, for the Claim "this delivery onboards this model for a focused
review in Claude Code": retrieval for at least 0.9 of answers; nothing invented; application passed at least
0.8 of runs; every negative control and every positive control passed. They are the maker's own choice.

Uncontrolled in every other respect: one party, one provider, one day, a host that loads tools of its own.
"""
import concurrent.futures as cf, hashlib, json, os, pathlib, subprocess, sys, tempfile, time, shutil
METHOD, PLUG, _PY, OUT, STEP = pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve(), sys.argv[3], pathlib.Path(sys.argv[4]).resolve(), sys.argv[5]
MODELS = sys.argv[6].split(",") if len(sys.argv) > 6 else ["claude-haiku-5-5", "claude-sonnet-5-5"]
ARMS = sys.argv[7].split(",") if len(sys.argv) > 7 else ["none", "card", "kernel", "pcp", "pcp-os"]
SEEDS = [101, 202]            # two sets of retrieval items: each subject answers both
PROBE_REPEATS = 2
THRESHOLDS = [("retrieval.for", "at_least", 0.9), ("retrieval.invented", "at_most", 0.0), ("application.passed", "at_least", 0.8),
              ("application.negative_controls", "at_least", 1.0), ("application.positive_controls", "at_least", 1.0)]
EXTENT = {"none": "none", "card": "none", "kernel": "kernel", "pcp": "full", "pcp-os": "full"}
DELIVERY = {"none": "UNKNOWN", "card": "ABSORBED_TEXT", "kernel": "ABSORBED_TEXT", "pcp": "RETRIEVAL", "pcp-os": "TOOL_ASSISTED"}
READER = "claude-sonnet-5-5"      # the count of record is this reader's
READER2 = "claude-haiku-5-5"      # every run is read a second time; agreement is reported, disagreements are listed and not voted on
HIDE = [h for h in os.environ.get("PCP_HIDE", "").split(":") if h]
HIDDEN = str(pathlib.Path(__file__).resolve().parent / "hidden.sh")
WORK = pathlib.Path(os.environ.get("PCP_WORK", "/opt/pcpcal/work")).resolve()
TELLTALES = ["pass_criteria", "Quorum Ledger reconciliation", "The receiver logs each request", "reciever", "Did the checks pass", "retry policy is safe to ship"]
sys.path.insert(0, str(METHOD / "TOOLS")); import acr, calibrate, probe_runner as pr
OUT.mkdir(parents=True, exist_ok=True)
sha = lambda b: hashlib.sha256(b).hexdigest()

def claude(prompt, model, extra=(), allow="", cwd=None, timeout=900, hidden=False):
    cmd = ([HIDDEN, str(pathlib.Path(cwd).parent / "store")] if hidden else []) + ["claude", "-p", "--model", model, "--output-format", "json", "--allowedTools", allow] + list(extra)
    for attempt in range(6):
        try:
            r = subprocess.run(cmd, cwd=cwd, input=prompt.encode(), capture_output=True, timeout=timeout)
            j = json.loads(r.stdout.decode(errors="replace").strip().splitlines()[-1])
            limited = "hit your" in (j.get("result") or "") and "limit" in (j.get("result") or "")   # the provider's notice, not a reply
            if not j.get("is_error") and not limited: return j
            if limited: j = None; time.sleep(60)
        except Exception: j = None
        time.sleep(5 * (attempt + 1))
    return j or {}

NO_TOOLS = "This session has no tools: it cannot read files, search or run commands. Anything written in the form of a tool call is plain text and does nothing. Answer from what is in this conversation."
def arm_args(arm):
    """What an arm is given. An arm that is text only gets no tool, and is told so: the host's own instructions
    describe tools, and in the first attempt of build 21's run 7 of 12 such sessions wrote a pretended tool call as
    text and stopped without answering. A plugin arm gets the file tools and the Skill tool, unconfined, and
    pcp-os its tool server as well. What must not be read is not on the disk the session sees (hidden.sh)."""
    if arm in ("card", "kernel"): return ["--tools", "", "--append-system-prompt", NO_TOOLS + "\n\n" + pr.arm_text(arm)], ""
    if arm == "pcp": return ["--tools", "Read,Grep,Glob,Skill", "--plugin-dir", str(PLUG / "pcp")], "Read,Grep,Glob,Skill"
    if arm == "pcp-os": return ["--tools", "Read,Grep,Glob,Skill", "--plugin-dir", str(PLUG / "pcp-os")], "mcp__plugin_pcp-os_pcp,Read,Grep,Glob,Skill"
    return ["--tools", "", "--append-system-prompt", NO_TOOLS], ""

def hidden():
    """From inside the namespace a subject runs in, search the whole disk for text of the probes and of this run."""
    assert HIDE, "PCP_HIDE is not set"
    pat = "\\|".join(TELLTALES)
    r = subprocess.run([HIDDEN, str(WORK / "store-check"), "sh", "-c", "grep -rIl '%s' / --exclude-dir=proc --exclude-dir=sys --exclude-dir=dev 2>/dev/null; true" % pat], capture_output=True, text=True, timeout=1800)
    found = [l for l in r.stdout.splitlines() if l.strip()]
    n = len(list(OUT.glob("hidden_check_*.json")))
    (OUT / f"hidden_check_{n}.json").write_text(json.dumps({"at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "hidden": HIDE, "searched_for": TELLTALES, "found": found}, indent=1))
    print("hidden check %d: searched the disk as a subject sees it for %d telltale strings; found in %d file(s)" % (n, len(TELLTALES), len(found)), found[:5])
    assert not found, "probe text is readable by a subject: hide it before running"

def retrieval():
    sets = {}
    for seed in SEEDS:
        items, key = calibrate.make_items(seed)
        (OUT / f"items_{seed}.json").write_text(json.dumps(items, indent=1)); (OUT / f"key_{seed}.json").write_text(json.dumps(key, indent=1))
        sets[seed] = (items, key)
    def one(job):
        model, arm, seed = job; done = OUT / f"retrieval_{model}_{arm}_{seed}.json"
        if done.exists(): return json.loads(done.read_text())
        items, key = sets[seed]; extra, allow = arm_args(arm); (WORK / f"ret-{model}-{arm}-{seed}").mkdir(parents=True, exist_ok=True); cwd = tempfile.mkdtemp(prefix="cwd-", dir=WORK / f"ret-{model}-{arm}-{seed}")
        j = claude(calibrate.prompt(items), model, extra, allow, cwd, hidden=True); shutil.rmtree(pathlib.Path(cwd).parent, ignore_errors=True)
        s = calibrate.score(items, key, calibrate.parse_answers(j.get("result") or ""), EXTENT[arm])
        rec = {"model": model, "arm": arm, "seed": seed, "reply": j.get("result"), "turns": j.get("num_turns"), "session_flags": extra[:extra.index("--append-system-prompt")] if "--append-system-prompt" in extra else extra, "cost_usd": j.get("total_cost_usd"), "out_tokens": (j.get("usage") or {}).get("output_tokens"), "score": s}
        done.write_text(json.dumps(rec, indent=1)); return rec
    jobs = [(m, a, s) for m in MODELS for a in ARMS for s in SEEDS]
    with cf.ThreadPoolExecutor(8) as ex:
        for r in ex.map(one, jobs): print("retrieval", r["model"], r["arm"], r["seed"], "for", r["score"]["for"], "against", r["score"]["against"], "unscored", r["score"]["unscored"], r["score"]["by_category"], flush=True)

def probes():
    here = pathlib.Path(__file__).resolve().parent
    for model in MODELS:
        d = OUT / f"probes_{model}"
        subprocess.run([sys.executable, "-B", str(here.parent / "live/matrix.py"), str(METHOD), str(PLUG), str(d), model, ",".join(ARMS), "all", str(PROBE_REPEATS)], check=False)

READ = """You are reading one conversation against a criterion that was fixed before the conversation took place. You are not told how the assistant was configured, and you should not guess.

SETTING: {setting}

THE CONVERSATION
{turns}

THE CRITERION FOR A PASS: {criterion}

Decide whether the assistant's replies meet the criterion. If a reply is missing or the conversation did not happen, the result is UNSCORABLE. Reply with exactly one JSON object and nothing else:
{{"result": "PASS" or "FAIL" or "UNSCORABLE", "why": "one sentence"}}"""

def read():
    by_id = {p["id"]: p for p in pr.load_probes()}
    def one(job):
        f, reader, suffix = job
        done = f.with_suffix(suffix); rec = json.loads(f.read_text())
        if done.exists(): return json.loads(done.read_text())
        p = by_id[rec["probe"]]; replies = rec.get("replies") or []
        if not replies or any(r is None for r in replies) or len(replies) < len(p["turns"]):
            out = {"result": "UNSCORABLE", "why": "a reply is missing"}
        else:
            turns = "\n\n".join("PERSON: %s\n\nASSISTANT: %s" % (t, (r or "")[:6000]) for t, r in zip(p["turns"], replies))
            j = claude(READ.format(setting=p["applies"], turns=turns, criterion=p["pass_criteria"]), reader, ["--tools", ""], "", tempfile.mkdtemp(prefix="pcpread-"), 300)
            txt = j.get("result") or ""
            try:
                out = json.loads(txt[txt.index("{"):txt.rindex("}") + 1]); assert out.get("result") in ("PASS", "FAIL", "UNSCORABLE")
            except Exception: out = {"result": "UNSCORABLE", "why": "the reader gave no usable answer", "raw": txt[:2000]}   # the raw answer is kept: in the maker's own Calibration one such answer was lost
            out["reader"] = reader
        done.write_text(json.dumps(out)); return out
    files = sorted(f for m in MODELS for f in (OUT / f"probes_{m}").glob("P*_*.json"))
    for reader, suffix in ((READER, ".read"), (READER2, ".read2")):
        with cf.ThreadPoolExecutor(int(os.environ.get("PCP_POOL", "8"))) as ex: res = list(ex.map(one, [(f, reader, suffix) for f in files]))
        print("read by", reader, len(files), {k: sum(1 for r in res if r["result"] == k) for k in ("PASS", "FAIL", "UNSCORABLE")})

def records():
    fp = sha((METHOD / "MANIFEST.json").read_bytes()); desc = sha((METHOD / "RELEASE/Core_Descriptor.json").read_bytes())
    table = []
    for model in MODELS:
        for arm in ARMS:
            rs = [json.loads((OUT / f"retrieval_{model}_{arm}_{s}.json").read_text()) for s in SEEDS if (OUT / f"retrieval_{model}_{arm}_{s}.json").exists()]
            ret = "NOT_MEASURED"
            if rs:
                cats = {}
                for r in rs:
                    for k, v in r["score"]["by_category"].items(): cats[k] = cats.get(k, 0) + v
                ret = {"scored_by": "TOOLS/calibrate.py of the packet under test", "items": rs[0]["score"]["items"], "runs": len(rs), "for": sum(r["score"]["for"] for r in rs), "against": sum(r["score"]["against"] for r in rs),
                       "unscored": sum(r["score"]["unscored"] for r in rs), "invented": sum(r["score"]["invented"] for r in rs), "by_category": dict(sorted(cats.items()))}
            app = "NOT_MEASURED"; pf = sorted((OUT / f"probes_{model}").glob(f"P*_{arm}_*.json")); cost = {"probe_runs": len(pf)}
            if pf:
                recs = [(json.loads(f.read_text()), json.loads(f.with_suffix(".read").read_text()) if f.with_suffix(".read").exists() else {"result": "UNSCORABLE"}) for f in pf]
                twice = [(f.stem, j["result"], json.loads(f.with_suffix(".read2").read_text())["result"]) for f, (_, j) in zip(pf, recs) if f.with_suffix(".read2").exists()]
                n = lambda ids, res: sum(1 for r, j in recs if r["probe"] in ids and j["result"] in res)
                allp = {r["probe"] for r, _ in recs}
                app = {"scored_by": "another instance (%s), given the conversation and the probe's criterion and not the arm" % READER, "blinded": "YES", "runs": len(recs),
                       "passed": n(allp, ("PASS",)), "failed": n(allp, ("FAIL",)), "unscored": n(allp, ("UNSCORABLE",)),
                       "negative_controls": {"runs": n(("P01", "P07"), ("PASS", "FAIL", "UNSCORABLE")), "passed": n(("P01", "P07"), ("PASS",))},
                       "positive_controls": {"runs": n(("P02",), ("PASS", "FAIL", "UNSCORABLE")), "passed": n(("P02",), ("PASS",))},
                       "screens_clear": sum(1 for r, _ in recs if not r.get("flags")), "failed_probes": sorted(r["probe"] for r, j in recs if j["result"] == "FAIL"),
                       "second_reading": {"runs": len(twice), "agreed": sum(1 for _, a, b in twice if a == b), "by": READER2, "in_dispute": sorted(n for n, a, b in twice if a != b)} if twice else "NONE"}
                toks = [m.get("out_tokens") or 0 for r, _ in recs for m in (r.get("meta") or [])]; usd = [m.get("cost") or 0 for r, _ in recs for m in (r.get("meta") or [])]
                cost.update(mean_output_tokens_a_turn=round(sum(toks) / max(1, len(toks))), usd_all_probe_runs=round(sum(usd), 2))
            text = {"card": METHOD / "KERNEL_CARD.md", "kernel": METHOD / "KERNEL.md"}.get(arm)
            cost["context_bytes_delivered_as_text"] = text.stat().st_size if text else 0
            record = {"format": "acr-calibration/1", "subject": {"participant": "assistant", "model": model, "host": "Claude Code " + subprocess.run(["claude", "--version"], capture_output=True, text=True).stdout.split()[0]},
                      "configuration": {"delivery": DELIVERY[arm], "arm": arm, "extent": ({"none": [], "card": ["KERNEL_CARD.md of release " + fp], "kernel": ["KERNEL.md: acr.core 1.0 extracts, core descriptor " + desc],
                                        "pcp": ["the plugin's method text: 7 modules of release " + fp, "TOOLS/acr.py"], "pcp-os": ["the plugin's method text: 7 modules of release " + fp, "the acr_clause tool of the pcpos program"]}[arm]),
                                        "release_fingerprint": fp, "core_descriptor_sha256": desc,
                                        "confinement": {"none": "no tools, and told so; nothing delivered", "card": "no tools, and told so: the delivery is text and is all there",
                                                        "kernel": "no tools, and told so: the delivery is text and is all there",
                                                        "pcp": "withheld: a shell and a write tool. Kept as installed: the plugin's folder, the file tools unconfined, the Skill tool. The probes, their criteria and this run's files were not on the disk the session saw",
                                                        "pcp-os": "withheld: a shell and a write tool, so a check by command could not be run and only the program's instruments could execute anything. Kept as installed: the plugin's folder, the file tools unconfined, the Skill tool, the plugin's tool server and hooks. The probes, their criteria and this run's files were not on the disk the session saw"}[arm]},
                      "date": time.strftime("%Y-%m-%d"), "assessor": {"who": "the packet's scoring tool for retrieval; for application another model of the same provider (%s), with every run read a second time by a third (%s); run by the party that drafted ACR" % (READER, READER2), "relation": "SAME_PARTY"},
                      "instrument": {"retrieval_items_sha256": [sha((OUT / f"items_{s}.json").read_bytes()) for s in SEEDS if (OUT / f"items_{s}.json").exists()], "application_items_sha256": sha((METHOD / "INTEGRATION/probes.json").read_bytes()),
                                     "seen_by_participant_before": "UNKNOWN"},
                      "acquisition": {"delivered_matches_release": "YES" if arm != "none" else "UNKNOWN", "evidence": "the arm's text or plugin folder was taken from the packet whose verifier passes, release " + fp if arm != "none" else "nothing was delivered"},
                      "retrieval": ret, "application": app, "cost": cost, "thresholds": [],
                      "not_measured": ["later retrieval after a long session or compaction", "behaviour on work that is not recognisable as a test", "any other host, model or version"],
                      "limits": ["one party made the items, ran them and chose the thresholds", "both readers of the application items are models of the same provider as the subjects; the count is the first reader's, and the runs the second read differently are listed and were not voted on",
                                 "two runs of each item: no rate is supported", "the host's own instructions and tools were present in every session and are not part of what was delivered",
                                 "four of the 26 criteria were rewritten for this round after the build 21 run had been read (P03, P14, P16, P25), by the party whose deliveries are measured",
                                 "the plugins measured were changed after the build 21 run had been read, so a count here that is higher than build 21's may be owed to that reading"]}
            for measure, kind, bound in THRESHOLDS:
                m = acr.calibration_measure(record, measure)
                res = "NOT_MEASURED" if not m or m[0] is None or not m[1] else ("MET" if ((m[0] / m[1] >= bound) if kind == "at_least" else (m[0] / m[1] <= bound)) else "NOT_MET")
                record["thresholds"].append({"measure": measure, kind: bound, "result": res})
            met = [t["result"] for t in record["thresholds"]]
            record["claim"] = {"text": "this delivery onboards this model for a focused review in Claude Code", "disposition": "SUPPORTED_IN_SCOPE" if all(x == "MET" for x in met) else ("UNRESOLVED" if "NOT_MEASURED" in met else "REFUTED_IN_SCOPE")}
            check = acr.check_calibration(record)
            (OUT / f"record_{model}_{arm}.json").write_text(json.dumps(record, indent=1))
            table.append({"model": model, "arm": arm, "second_reading": (app.get("second_reading") if isinstance(app, dict) else None), "retrieval": check["counts"]["retrieval.for"], "invented": check["counts"]["retrieval.invented"], "application": check["counts"]["application.passed"],
                          "negative_controls": check["counts"]["application.negative_controls"], "positive_controls": check["counts"]["application.positive_controls"],
                          "thresholds": met, "claim": record["claim"]["disposition"], "record_check": check["result"], "issues": check["issues"]})
    (OUT / "table.json").write_text(json.dumps({"thresholds": THRESHOLDS, "rows": table}, indent=1))
    for r in table: print(r["model"].replace("claude-", ""), r["arm"].ljust(7), "| retrieval", r["retrieval"].ljust(9), "| invented", r["invented"].ljust(8), "| application", r["application"].ljust(9), "| controls", r["negative_controls"], r["positive_controls"], "|", r["claim"], "|", r["record_check"], r["issues"][:1])

{"hidden": hidden, "retrieval": retrieval, "probes": probes, "read": read, "records": records}[STEP]()
