"""Simulated scenarios against the packaged PCP OS program, as a host would drive it.
Every scenario runs in its own empty directory against a plugin folder taken from a release archive.
Two oracles besides each scenario's own expectations:
  - the program's own `verify` (chain and guards), and
  - the second implementation in Python, which folds the same register independently.
usage: sim.py <plugins root: holds Personal/ Commercial/ Enterprise/ each with pcp-os> <os-python dir> <cookbook.json> <out.json> [seeds]"""
import json, os, subprocess, sys, tempfile, pathlib, random, hashlib, shutil, copy, itertools, threading, concurrent.futures as cf
PLUG, PY, COOK, OUT = pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve(), json.load(open(sys.argv[3])), sys.argv[4]
SEEDS = int(sys.argv[5]) if len(sys.argv) > 5 else 200
sys.path.insert(0, str(PY)); from pcpos import model; from pcpos.base import strict_loads, canon, sha_bytes
EDITIONS = ["Personal", "Commercial", "Enterprise"]
BASE_ENV = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
RESULTS = []
NOW = threading.local()        # the scenario this thread is running: every workspace it closes is compared by both implementations
CLOSURES = {"compared": 0, "with a register": 0}; CLOSURES_LOCK = threading.Lock()
STOP_WORDS = ["stop", "halt", "pause", "stop acr", "halt acr", "pause acr", "drop acr", "end acr", "stop pcp", "halt pcp", "pause pcp", "stop the review", "end the review", "pause the review", "forget the protocol", "-acr halt"]

class S:
    def __init__(self, edition="Personal", env=None, name="ws"):
        self.plugin = PLUG / edition / "pcp-os"; self.exe = str(self.plugin / "servers/pcpos"); self.edition = edition
        self.top = pathlib.Path(tempfile.mkdtemp(prefix="pcpsim-")).resolve(); self.root = self.top / name; self.root.mkdir()
        self.env = dict(BASE_ENV, CLAUDE_PLUGIN_ROOT=str(self.plugin), CLAUDE_PROJECT_DIR=str(self.root)); self.env.update(env or {})
        self.reg = self.root / ".pcp/register.jsonl"
    def hook(self, event, obj, raw=None):
        r = subprocess.run([self.exe, "hook", event], input=raw if raw is not None else json.dumps(obj).encode(), env=self.env, cwd=self.root, capture_output=True, timeout=60)
        return r.returncode, r.stdout.decode(errors="replace"), r.stderr.decode(errors="replace")
    def prompt(self, text, **kw):
        return self.hook("prompt", dict({"hook_event_name": "UserPromptSubmit", "prompt": text}, **kw))[1]
    def typed(self, cmd, args="", both=True):
        out = self.hook("prompt", {"hook_event_name": "UserPromptExpansion", "expansion_type": "slash_command", "command_name": "pcp-os:" + cmd, "command_args": args, "prompt": "x"})[1]
        if both: out += self.hook("prompt", {"hook_event_name": "UserPromptSubmit", "prompt": f"/pcp-os:{cmd} {args}".strip()})[1]
        return out
    def mcp_raw(self, lines, env=None):
        r = subprocess.run([self.exe, "mcp"], input=("\n".join(lines) + "\n").encode(), env=dict(self.env, PCPOS_ROOT=str(self.root), **(env or {})), cwd=self.root, capture_output=True, timeout=120)
        return r.returncode, r.stdout.decode(errors="replace").splitlines(), r.stderr.decode(errors="replace")
    def mcp(self, calls, env=None):
        lines = [json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}})]
        lines += [json.dumps({"jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": {"name": n, "arguments": a}}) for i, (n, a) in enumerate(calls)]
        rc, out, err = self.mcp_raw(lines, env)
        res = []
        for l in out[1:]:
            j = json.loads(l)
            res.append(j["result"]["content"][0]["text"] if "result" in j else "ERROR " + json.dumps(j.get("error")))
        return res
    def cli(self, *args):
        r = subprocess.run([self.exe, *args], env=self.env, cwd=self.root, capture_output=True, timeout=60)
        return r.returncode, r.stdout.decode(errors="replace") + r.stderr.decode(errors="replace")
    def bytes(self): return self.reg.read_bytes() if self.reg.exists() else None
    def events(self): return [json.loads(l) for l in self.reg.read_text().splitlines()] if self.reg.exists() else []
    def kinds(self): return [e["kind"] for e in self.events()]
    def begin_adopted(self):
        self.typed("review", "the claim"); assert "ADOPTED" in self.kinds()
    def cook(self, upto=None):
        return self.mcp([("acr_submit", {"kind": k, "body": b}) for k, b in COOK[:upto]])
    def close(self):
        # Before a workspace is discarded, whatever register it holds, sound or damaged, is read by both implementations.
        # They need not find it sound; they must agree on whether it is, and on its head where it is.
        try:
            raw = self.bytes()
            with CLOSURES_LOCK:
                CLOSURES["compared"] += 1; CLOSURES["with a register"] += raw is not None
            if raw:
                for x in oracles(self, expect_clean=None):
                    getattr(NOW, "found", []).append("at close: " + x)
        finally:
            shutil.rmtree(self.top, ignore_errors=True)

def oracles(s, expect_clean=True):
    """Both implementations on the register as it stands. Returns a list of disagreements."""
    bad = []
    raw = s.bytes()
    if raw is None: return bad
    rc, out = s.cli("verify", str(s.reg))
    rust_clean = rc in (0, 1) and "chain intact" in out and "guards replayed: 0 problem(s)" in out
    try:
        lines = raw.split(b"\n"); assert lines[-1] == b"", "no final newline"; lines = lines[:-1]
        evs = [strict_loads(l) for l in lines]
        chain = all(canon(e) == l for e, l in zip(evs, lines)) and all(e["seq"] == i for i, e in enumerate(evs)) and \
                all(e["prev"] == (sha_bytes(lines[i - 1]) if i else "0" * 64) for i, e in enumerate(evs))
        _, problems = model.fold(evs, check=True)
        py_clean = chain and not problems
        py_head = sha_bytes(lines[-1]) if lines else None
    except Exception as e:
        py_clean, py_head, problems = False, None, [repr(e)]
    if rust_clean != py_clean: bad.append(f"implementations disagree: rust clean={rust_clean} python clean={py_clean} {problems[:2]} {out[-300:]}")
    if rust_clean and py_head and py_head not in out: bad.append("heads differ between implementations")
    if expect_clean is True and not rust_clean: bad.append("register does not verify: " + out[-400:])
    if expect_clean is False and rust_clean: bad.append("a damaged register verified")
    return bad

def scenario(family, name, edition="Personal"):
    def deco(fn):
        def run():
            s = None; checks = []; NOW.found = []
            def ck(label, ok, detail="", summary=False):
                # summary: a line that says nothing failed in a loop above it. It is kept for the reader and is not counted as a check.
                checks.append({"check": label, "ok": bool(ok), "detail": "" if ok else str(detail)[:600], **({"summary": True} if summary else {})})
            try:
                s = fn(ck)
            except Exception as e:
                import traceback; checks.append({"check": "scenario ran", "ok": False, "detail": traceback.format_exc()[-800:]})
            for x in NOW.found: checks.append({"check": "both implementations agree on every workspace closed", "ok": False, "detail": x[:600]})
            NOW.found = []
            return {"family": family, "name": name, "edition": edition, "checks": checks, "ok": all(c["ok"] for c in checks) and bool(checks)}
        RESULTS.append(run); return fn
    return deco

# ---------- F1: presence is not use -------------------------------------------------------------------
ORDINARY = ["Fix the typo in this sentence.", "what does this function do?", "ACRYLIC paint dries fast", "the pcp of the matter", "acr", "please review my PR", "stop", "halt", "pause.", "STOP!",
            "", "   ", "é" * 500, "x" * 100000, "/help", "/other:review x", "/pcp:review x", "stop the review", "\u0000binary\u0001", "Use the acronym ACRONYM", "PCPX and ACRs", "adversarial claims are fun"]
for ed in EDITIONS:
    for i, text in enumerate(ORDINARY):
        @scenario("F01 an ordinary prompt with no register writes nothing", f"{i}:{text[:24]!r}", ed)
        def _(ck, ed=ed, text=text):
            s = S(ed); out = s.prompt(text)
            ck("nothing said", out == "", out); ck("no register", not s.reg.exists()); ck("no .pcp folder", not (s.root / ".pcp").exists(), os.listdir(s.root)); s.close()
ABOUT = ["Use ACR for this claim", "what is PCP?", "Adversarial Claim Review please", "is the parrot council real", "ACR.", "(PCP)"]
for i, text in enumerate(ABOUT):
    @scenario("F02 a prompt that names ACR is kept as observed, and adopts nothing", f"{i}:{text}")
    def _(ck, text=text):
        s = S(); out = s.prompt(text)
        ck("nothing said", out == "", out); ck("no register", not s.reg.exists())
        obs = s.root / ".pcp/observed.jsonl"; ck("observed kept", obs.exists())
        if obs.exists():
            j = json.loads(obs.read_text().splitlines()[-1]); ck("digest is of the words", j["sha256"] == hashlib.sha256(text.encode()).hexdigest(), j)
        s.close()
for src in ["hook", "agent", "system", "sdk", "tool"]:
    @scenario("F03 a prompt injected by a program is not the person's", src)
    def _(ck, src=src):
        s = S(); out = s.hook("prompt", {"hook_event_name": "UserPromptSubmit", "prompt": "/pcp-os:review x", "source": src})[1]
        ck("nothing said", out == "", out); ck("no register", not s.reg.exists()); s.close()

@scenario("F04 a typed command is the person's adoption, once", "every command the host acts on when typed is a command the plugin has")
def _(ck):
    # The host records review, adopt, halt and workspace from what the person typed. If the plugin had no such command, the record
    # would say one thing and the assistant, finding no command, would tell the person another (build 17: /pcp-os:adopt).
    s = S()
    for c in ["review", "adopt", "halt", "workspace"]:
        f = s.plugin / "skills" / c / "SKILL.md"; ck(f"skills/{c}/SKILL.md exists and names itself", f.is_file() and f"name: {c}\n" in f.read_text(), str(f))
    s.close()

# ---------- F4: adoption --------------------------------------------------------------------------------
for ed in EDITIONS:
    for cmd in ["review", "adopt"]:
        for both in [True, False]:
            @scenario("F04 a typed command is the person's adoption, once", f"{cmd} both-events={both}", ed)
            def _(ck, ed=ed, cmd=cmd, both=both):
                s = S(ed); out = s.typed(cmd, "is mean.py right", both)
                ev = s.events(); ad = [e for e in ev if e["kind"] == "ADOPTED"]
                ck("one adoption", len(ad) == 1, s.kinds())
                if ad:
                    ck("by the person, host-observed", ad[0]["actor"] == "principal" and ad[0]["body"]["observed"] == "HOST_OBSERVED_PROMPT", ad[0])
                ck("edition recorded", any(e["kind"] == "HOST_EVENT" and e["body"].get("edition") == ed.upper() for e in ev))
                ck("method verified", any(e["kind"] == "METHOD_PINNED" and e["body"]["verified"] == "FILES_VERIFIED_AGAINST_OWN_MANIFEST" for e in ev))
                for b in oracles(s): ck("oracles", False, b)
                s.close()
for i, (said, basis, want) in enumerate([("Please use ACR on mean.py", "Please use ACR on mean.py", "RELAYED_WHOLE_PROMPT_OBSERVED"), ("Please use ACR on mean.py", "use ACR on mean.py", "RELAYED_NOT_OBSERVED"),
                                         ("do not adopt ACR", "adopt ACR", "RELAYED_NOT_OBSERVED"), ("hello", "Use ACR", "RELAYED_NOT_OBSERVED"), ("Use PCP here  ", "Use PCP here", "RELAYED_WHOLE_PROMPT_OBSERVED")]):
    @scenario("F05 an adoption the assistant relays is marked relayed, and observed only if it is the whole prompt", f"{i}:{said!r}->{basis!r}")
    def _(ck, said=said, basis=basis, want=want):
        s = S(); s.prompt(said); r = s.mcp([("acr_begin", {}), ("acr_adopt", {"basis": basis}), ("acr_adopt", {"basis": basis})])
        ad = [e for e in s.events() if e["kind"] == "ADOPTED"]
        ck("one adoption", len(ad) == 1, r)
        if ad: ck("actor is the session, not the person", ad[0]["actor"] == "assistant", ad[0]); ck("observed as " + want, ad[0]["body"]["observed"] == want, ad[0]["body"])
        ck("second adopt writes nothing", "nothing was written" in r[2], r[2])
        for b in oracles(s): ck("oracles", False, b)
        s.close()

# ---------- F6: the documented review, whole, in each edition ------------------------------------------
for ed in EDITIONS:
    @scenario("F06 the documented review runs to CLOSED_COMPLETE", ed, ed)
    def _(ck, ed=ed):
        s = S(ed); s.begin_adopted(); r = s.cook()
        ck("every event written", all(x.startswith("WRITTEN") for x in r), [x[:120] for x in r if not x.startswith("WRITTEN")])
        st = s.mcp([("acr_status", {})])[0]; ck("closed complete", "CLOSED_COMPLETE" in st, st[-300:])
        for b in oracles(s): ck("oracles", False, b)
        s.close()

# ---------- F7: halting at every point, with every word -------------------------------------------------
VARIANTS = lambda w: [w, w.upper(), w + ".", w.capitalize() + "!", "  " + w + "  "]
for k in range(0, len(COOK) + 1):
    w = STOP_WORDS[k % len(STOP_WORDS)]; v = VARIANTS(w)[k % 5]
    @scenario("F07 a bare stop typed at any point is recorded by the host and ends review work", f"after step {k}: {v!r}", EDITIONS[k % 3])
    def _(ck, k=k, v=v):
        s = S(EDITIONS[k % 3]); s.begin_adopted(); s.cook(k)
        before = s.events(); was_live = True
        st0 = s.mcp([("acr_status", {})])[0]
        out = s.prompt(v); ev = s.events(); halts = [e for e in ev if e["kind"] == "HALTED"]
        live = "use IN_USE" in st0 or "IN_USE" in st0
        ck("one halt recorded", len(halts) == 1, (out, s.kinds()[-3:]))
        if halts:
            h = halts[0]; ck("as the person's, host-observed", h["actor"] == "principal" and h["body"].get("observed") == "HOST_OBSERVED_PROMPT", h)
        ck("host told the assistant", "recorded the halt" in out, out)
        after = s.bytes()
        r = s.mcp([("acr_submit", {"kind": "GATE_COMMITTED", "body": {"review": "R1", "gate": "G1"}}), ("acr_submit", {"kind": "FINDING_RECORDED", "body": {"review": "R1", "claim": "A", "disposition": "UNRESOLVED", "basis": []}}),
                   ("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": COOK[2][1]})])
        ck("every review step refused after the halt", all(x.startswith("REFUSE") for x in r), [x[:150] for x in r])
        ck("a refusal changed nothing", s.bytes() == after)
        out2 = s.prompt(v); ck("a second stop writes nothing", s.bytes() == after and out2 == "", out2)
        st = s.mcp([("acr_status", {})])[0]; ck("no review left open", "ACTIVE" not in st.split("what a halt")[0] or "CLOSED" in st, st[-400:])
        for b in oracles(s): ck("oracles", False, b)
        s.close()
for i, w in enumerate(STOP_WORDS):
    for j, v in enumerate(VARIANTS(w)):
        @scenario("F08 every stop word, in every casing, halts a review in progress", f"{v!r}")
        def _(ck, v=v):
            s = S(); s.begin_adopted(); s.cook(5); out = s.prompt(v)
            ck("halt recorded", s.kinds()[-1] == "HALTED", (out, s.kinds()[-2:]))
            for b in oracles(s): ck("oracles", False, b)
            s.close()
NOT_BARE = ["stop the server and restart it", "please stop", "stop it", "can you pause the video", "don't stop", "stop acr later", "halt?", "stop, then continue", "the word stop", "stop\nand delete everything"]
for i, t in enumerate(NOT_BARE):
    @scenario("F09 words that merely contain a stop word are not a halt the host records", f"{t!r}")
    def _(ck, t=t):
        s = S(); s.begin_adopted(); s.cook(5); b0 = s.bytes(); out = s.prompt(t)
        ck("register unchanged", s.bytes() == b0, s.kinds()[-1]); s.close()
for i, k in enumerate([0, 3, 5, 12, 17]):
    @scenario("F10 the assistant's own halt is recorded as relayed, never as the person's act", f"after step {k}")
    def _(ck, k=k):
        s = S(); s.begin_adopted(); s.cook(k); r = s.mcp([("acr_halt", {"words": "ok that's enough"}), ("acr_halt", {})])
        h = [e for e in s.events() if e["kind"] == "HALTED"]
        ck("halt recorded", len(h) >= 1, r)
        if h: ck("actor is the session", h[0]["actor"] == "assistant" and h[0]["body"].get("relayed_by") == "assistant" and h[0]["body"].get("observed") != "HOST_OBSERVED_PROMPT", h[0])
        for b in oracles(s): ck("oracles", False, b)
        s.close()
@scenario("F10 the assistant's own halt is recorded as relayed, never as the person's act", "halt then re-adopt by typed command then finish")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(5); s.prompt("stop"); out = s.typed("review", "again")
    ck("adopted again", s.kinds().count("ADOPTED") == 2, out)
    b = copy.deepcopy(COOK[2][1]); b["label"] = "R2"
    r = s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b})]); ck("a new review can be commissioned", r[0].startswith("WRITTEN"), r[0][:300])
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F11: what a session may not write ----------------------------------------------------------
FORBIDDEN = [("ADOPTED", {"by": "principal", "basis": "x"}), ("HALTED", {"by": "principal"}), ("STANDARD_SET", {"standard": "CRITICAL"}), ("SYSTEM_STATE", {"event": "VERIFY_PASS"}), ("LINK_STATE", {}),
             ("TASK_STARTED", {"label": "t"}), ("TASK_ENDED", {"id": "T1", "outcome": "DONE"}), ("WORKSPACE_CREATED", {}), ("METHOD_PINNED", {}), ("HOST_EVENT", {"event": "X"}), ("X_ANY", {}),
             ("PARTICIPANT_ADDED", {"label": "bob", "kind": "HUMAN", "principal": True}), ("PARTICIPANT_ADDED", {"label": "bob", "kind": "HUMAN", "principal": False}),
             ("PARTICIPANT_ADDED", {"label": "host", "kind": "AUTOMATED", "principal": False}), ("PARTICIPANT_ADDED", {"label": "system", "kind": "AUTOMATED"}), ("NOT_A_KIND", {}), ("", {}), ("review_commissioned", COOK[2][1])]
for i, (k, b) in enumerate(FORBIDDEN):
    for when in ["in use", "not in use"]:
        @scenario("F11 a session cannot write as a person, as the host or as the system", f"{k or '(empty)'} #{i} {when}")
        def _(ck, k=k, b=b, when=when):
            s = S()
            if when == "in use": s.begin_adopted()
            else: s.mcp([("acr_begin", {})])
            b0 = s.bytes(); r = s.mcp([("acr_submit", {"kind": k, "body": b}), ("acr_offer", {"kind": k, "body": b}), ("acr_submit", {"kind": k, "body": b, "actor": "principal"})])
            ck("submit refused", r[0].startswith("REFUSE") or r[0].startswith("ERROR"), r[0][:200]); ck("offer refused", r[1].startswith("REFUSE") or r[1].startswith("ERROR"), r[1][:200])
            ck("naming an actor is an error", "actor is not an argument" in r[2], r[2][:200]); ck("nothing written", s.bytes() == b0)
            s.close()
for lab in ["principal", "host"]:
    @scenario("F11 a session cannot write as a person, as the host or as the system", f"session started under the label {lab}")
    def _(ck, lab=lab):
        s = S(); s.begin_adopted(); b0 = s.bytes()
        r = s.mcp([("acr_submit", {"kind": COOK[0][0], "body": COOK[0][1]}), ("acr_halt", {})], env={"PCPOS_ACTOR": lab, "PCPOS_PRINCIPAL": "principal" if lab != "principal" else "principal"})
        ev = s.events()[len(b0.splitlines()):]
        ck("no event written by a session as the person", not any(e["actor"] == "principal" and e["kind"] != "HALTED" for e in ev), (r, [(e["actor"], e["kind"]) for e in ev]))
        for x in oracles(s): ck("oracles", False, x)
        s.close()

# ---------- F12: every documented event, damaged ---------------------------------------------------------
def mutations(body, rng):
    out = []
    for key in list(body):
        b = copy.deepcopy(body); del b[key]; out.append(("drop " + key, b))
        for wrong in ([], 7, None, {"x": 1}, "", "x" * 20000, True):
            if type(wrong) != type(body[key]):
                b = copy.deepcopy(body); b[key] = wrong; out.append((f"{key}={wrong!r:.12}", b))
    b = copy.deepcopy(body); b["extra_field"] = "x"; out.append(("extra field", b))
    return out
rng0 = random.Random(1)
for idx, (kind, body) in enumerate(COOK):
    muts = mutations(body, rng0)
    @scenario("F12 a damaged event is refused or written consistently, and a refusal writes nothing", f"step {idx} {kind}: {len(muts)} mutations")
    def _(ck, idx=idx, kind=kind, muts=muts):
        s = S(); s.begin_adopted(); s.cook(idx); b0 = s.bytes()
        offers = s.mcp([("acr_offer", {"kind": kind, "body": b}) for _, b in muts])
        ck("offers write nothing", s.bytes() == b0)
        ck("every offer answered ACCEPT or REFUSE", all(o.startswith(("ACCEPT", "REFUSE")) for o in offers), [o[:120] for o in offers if not o.startswith(("ACCEPT", "REFUSE"))][:3])
        n_ref = 0
        for (label, b), o in zip(muts, offers):
            t = S(); t.begin_adopted(); t.cook(idx); c0 = t.bytes(); r = t.mcp([("acr_submit", {"kind": kind, "body": b})])[0]
            if o.startswith("REFUSE"):
                n_ref += 1
                if not (r.startswith("REFUSE") and t.bytes() == c0): ck(f"refused offer, refused submit, nothing written ({label})", False, r[:200])
            else:
                if not r.startswith("WRITTEN"): ck(f"accepted offer is written ({label})", False, r[:200])
                for x in oracles(t): ck(f"oracles after accepted mutation ({label})", False, x)
            t.close()
        ck(f"consistent across {len(muts)} mutations ({n_ref} refused)", True, summary=True)
        s.close()
for idx in range(2, len(COOK)):
    @scenario("F13 an event offered out of its order", f"step {idx} {COOK[idx][0]} without step {idx-1}")
    def _(ck, idx=idx):
        s = S(); s.begin_adopted(); s.cook(idx - 1); b0 = s.bytes(); r = s.mcp([("acr_submit", {"kind": COOK[idx][0], "body": COOK[idx][1]})])[0]
        if r.startswith("REFUSE"): ck("refusal writes nothing", s.bytes() == b0)
        else: ck("written", r.startswith("WRITTEN"), r[:200])
        for x in oracles(s): ck("oracles", False, x)
        s.close()

# ---------- F14: writers and heads ------------------------------------------------------------------------
@scenario("F14 concurrent writers leave one intact chain", "24 processes submitting at once")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(3); n0 = len(s.events())
    def one(i): return s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": dict(COOK[3][1], extent=f"lines {i} to {i+1}")})])[0]
    with cf.ThreadPoolExecutor(24) as ex: r = list(ex.map(one, range(24)))
    w = sum(x.startswith("WRITTEN") for x in r); ck("each answer is WRITTEN or REFUSE", all(x.startswith(("WRITTEN", "REFUSE")) for x in r), [x[:100] for x in r][:3])
    ck("events on disk equal writes acknowledged", len(s.events()) - n0 == w, (len(s.events()) - n0, w))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F14 concurrent writers leave one intact chain", "a stale expected_head is refused")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(3); head = hashlib.sha256(s.bytes().splitlines()[-1]).hexdigest()
    r = s.mcp([("acr_submit", {"kind": COOK[3][0], "body": COOK[3][1], "expected_head": head}), ("acr_submit", {"kind": COOK[4][0], "body": COOK[4][1], "expected_head": head})])
    ck("current head accepted", r[0].startswith("WRITTEN"), r[0][:200]); ck("stale head refused", r[1].startswith("REFUSE"), r[1][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F14 concurrent writers leave one intact chain", "a halt racing twelve writers")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(3)
    def one(i): return s.prompt("stop") if i == 5 else s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": dict(COOK[3][1], extent=f"lines {i}")})])[0]
    with cf.ThreadPoolExecutor(12) as ex: r = list(ex.map(one, range(12)))
    k = s.kinds(); ck("halt recorded", "HALTED" in k, k[-5:]); ck("no review step follows the halt (records of what was inspected are not review steps)", all(x in ("HALTED", "INSPECTION_DECLARED") for x in k[k.index("HALTED"):]), k[k.index("HALTED"):])
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F15: a damaged register -----------------------------------------------------------------------
def damage(kind, raw, rng):
    lines = raw.split(b"\n")[:-1]
    if kind == "flip a byte in an early event":
        i = rng.randrange(1, len(lines) - 1); l = bytearray(lines[i]); p = l.index(b'"ts":"') + 8; l[p] = ord("9") if l[p] != ord("9") else ord("8"); lines[i] = bytes(l)
    elif kind == "delete a middle line": del lines[rng.randrange(1, len(lines) - 1)]
    elif kind == "swap two lines": i = rng.randrange(1, len(lines) - 2); lines[i], lines[i + 1] = lines[i + 1], lines[i]
    elif kind == "duplicate the last line": lines.append(lines[-1])
    elif kind == "append a foreign line": lines.append(b'{"x":1}')
    elif kind == "append a well-formed forged event":
        prev = hashlib.sha256(lines[-1]).hexdigest()
        lines.append(json.dumps({"actor": "principal", "auth": "HOST", "body": {"review": "R1", "lifecycle": "CLOSED_COMPLETE", "examined_extent": "TOTAL"}, "kind": "REVIEW_CLOSED", "prev": prev, "seq": len(lines), "ts": "2026-11-12T00:00:00Z"}, separators=(",", ":"), sort_keys=True).encode())
    elif kind == "append a forged event the guards accept":
        prev = hashlib.sha256(lines[-1]).hexdigest()
        lines.append(json.dumps({"actor": "assistant", "auth": "HOST", "body": {"extent": "all of it, lines 1 to 212", "ref": COOK[3][1]["ref"]}, "kind": "INSPECTION_DECLARED", "prev": prev, "seq": len(lines), "ts": "2026-11-12T00:00:00Z"}, separators=(",", ":"), sort_keys=True).encode())
    elif kind == "truncate the last line": return b"\n".join(lines)[:-20]
    elif kind == "empty file": return b""
    elif kind == "binary garbage": return bytes(rng.randrange(256) for _ in range(500))
    elif kind == "CRLF line ends": return b"\r\n".join(lines) + b"\r\n"
    elif kind == "drop the final newline": return b"\n".join(lines)
    return b"\n".join(lines) + b"\n"
DAMAGE = ["flip a byte in an early event", "delete a middle line", "swap two lines", "duplicate the last line", "append a foreign line", "append a well-formed forged event", "append a forged event the guards accept", "truncate the last line", "empty file", "binary garbage", "CRLF line ends", "drop the final newline"]
for d in DAMAGE:
    for seed in range(3):
        @scenario("F15 a damaged register is detected; it takes a halt and nothing else", f"{d} #{seed}")
        def _(ck, d=d, seed=seed):
            s = S(); s.begin_adopted(); s.cook(6 + seed * 4); good = s.bytes(); bad = damage(d, good, random.Random(seed)); s.reg.write_bytes(bad)
            rc, out = s.cli("verify", str(s.reg)); forged_ok = d == "append a forged event the guards accept"
            if d == "empty file":
                ck("an empty register is not reported as an intact chain of events", "chain intact" not in out or " 0 events" in out, out[-300:])
                r = s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": COOK[3][1]})])
                ck("a review step is not written into an emptied register", not r[0].startswith("WRITTEN"), r[0][:200])
            elif forged_ok:
                # The known limit, asserted and not assumed: nothing is signed, so an event forged with a correct chain that the
                # guards would accept verifies. Both implementations must read it the same way.
                ck("a forged event with a correct chain that the guards accept verifies (the stated limit: nothing is signed)", rc in (0, 1) and "chain intact" in out and "guards replayed: 0 problem(s)" in out, out[-300:])
                ck("and it is in the register as the last event", s.events()[-1]["kind"] == "INSPECTION_DECLARED" and s.events()[-1]["ts"] == "2026-11-12T00:00:00Z")
                for x in oracles(s, expect_clean=True): ck("oracles", False, x)
            elif d == "append a well-formed forged event":
                # Its chain is correct, so the chain says nothing; the replay of the guards is what finds it: a review closed as complete without its gates.
                ck("the chain is intact and the guards' replay reports the forged closing", "chain intact" in out and "guards replayed: 1 problem(s)" in out and "CLOSED_COMPLETE needs every required gate" in out, out[-400:])
                for x in oracles(s, expect_clean=False): ck("oracles", False, x)
            else:
                ck("verify fails", rc != 0, out[-300:])
                for x in oracles(s, expect_clean=False): ck("oracles", False, x)
            if d not in ("empty file",) and not forged_ok:
                r = s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": COOK[3][1]}), ("acr_adopt", {"basis": "x"})])
                ck("a review step is not written onto a damaged register", not r[0].startswith("WRITTEN"), r[0][:200])
                now = s.bytes(); ck("the damaged bytes were not overwritten or truncated", now.startswith(bad[:len(bad) - 1]) if now else False, (len(bad), len(now or b"")))
                out = s.prompt("stop"); ck("the hooks do not crash on it", "panicked" not in out, out[:200])
                st = s.mcp([("acr_status", {}), ("acr_verify", {})]); ck("status and verify each answer, and neither answer is a crash", len(st) == 2 and all(isinstance(x, str) and x and "panicked" not in x for x in st), st)
            s.close()
@scenario("F15 a damaged register is detected; it takes a halt and nothing else", "repair removes only an incomplete final line")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(6); good = s.bytes(); s.reg.write_bytes(good + b'{"actor":"assis')
    rc, out = s.cli("verify", str(s.reg)); ck("incomplete line fails verify", rc != 0, out[-200:])
    rc, out = s.cli("repair", str(s.reg)); ck("repair succeeds", rc == 0, out); ck("the bytes are the good register again", s.bytes() == good)
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F16: where the register may be ------------------------------------------------------------------
for i, (label, setup, regarg) in enumerate([
    ("register is a symbolic link to a file outside", lambda s: ((s.root / ".pcp").mkdir(), os.symlink(s.top / "outside.jsonl", s.reg)), None),
    (".pcp is a symbolic link to a folder outside", lambda s: ((s.top / "out").mkdir(), os.symlink(s.top / "out", s.root / ".pcp")), None),
    ("register argument climbs out with ..", lambda s: None, "../escape.jsonl"),
    ("register argument is absolute", lambda s: None, "/tmp/pcp-escape-%d.jsonl" % os.getpid()),
    ("register argument through a linked folder", lambda s: ((s.top / "out2").mkdir(), os.symlink(s.top / "out2", s.root / "lnk")), "lnk/r.jsonl")]):
    @scenario("F16 a register is never reached through a link or outside the served directory", label)
    def _(ck, setup=setup, regarg=regarg, label=label):
        s = S(); setup(s); a = {"register": regarg} if regarg else {}
        r = s.mcp([("acr_begin", a), ("acr_status", a)]); out = s.typed("review", "x") if not regarg else ""
        outside = [p for p in s.top.rglob("*") if p.is_file() and s.root not in p.parents and p.name != "ws"]
        ck("nothing written outside the served directory", not outside and not os.path.exists("/tmp/pcp-escape-%d.jsonl" % os.getpid()), (outside, r))
        ck("the tool says why", "outside the directory" in r[0] or "not a path inside" in r[0] or "link" in r[0] or r[0].startswith("ERROR"), r[0][:200]); s.close()

# ---------- F17: subagents are tasks the host observes ---------------------------------------------------------
SUB = [("with a final message", {"last_assistant_message": "The claim fails for an empty list."}, "DONE"), ("with a transcript file only", {"agent_transcript_path": "TRANSCRIPT"}, "DONE"),
       ("with nothing to read back", {}, "FAILED"), ("with an empty message", {"last_assistant_message": ""}, "FAILED")]
for label, extra, want in SUB:
    for ed in EDITIONS:
        @scenario("F17 a subagent is a task whose end the host observes", label, ed)
        def _(ck, extra=extra, want=want, ed=ed):
            s = S(ed); s.begin_adopted(); s.cook(5)
            s.hook("subagent-start", {"hook_event_name": "SubagentStart", "agent_type": "challenger", "agent_id": "a-1"})
            ck("task started by the host's observation", s.kinds()[-1] == "TASK_STARTED", s.kinds()[-2:])
            e = dict(extra)
            if e.get("agent_transcript_path"): p = s.top / "t.jsonl"; p.write_text("transcript"); e["agent_transcript_path"] = str(p)
            s.hook("subagent-stop", dict({"hook_event_name": "SubagentStop", "agent_id": "a-1"}, **e))
            end = [x for x in s.events() if x["kind"] == "TASK_ENDED"]
            ck("one end recorded", len(end) == 1, s.kinds()[-3:])
            if end: ck("outcome " + want, end[0]["body"]["outcome"] == want and end[0]["actor"] == "host", end[0]["body"])
            n = len(s.events()); s.hook("subagent-stop", dict({"hook_event_name": "SubagentStop", "agent_id": "a-1"}, **e)); ck("a second stop for the same attempt writes nothing", len(s.events()) == n)
            s.hook("subagent-stop", {"hook_event_name": "SubagentStop", "agent_id": "never-started", "last_assistant_message": "x"}); ck("a stop for an unknown attempt writes nothing", len(s.events()) == n)
            for x in oracles(s): ck("oracles", False, x)
            s.close()
@scenario("F17 a subagent is a task whose end the host observes", "a halt asks it to stop, and its late result is not DONE")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(5); s.hook("subagent-start", {"agent_type": "checker", "agent_id": "a-9"}); out = s.prompt("stop")
    st = s.mcp([("acr_status", {})])[0]; ck("status says the task was asked to stop, not that it stopped", "STOP" in st.upper(), st[-500:])
    s.hook("subagent-stop", {"agent_id": "a-9", "last_assistant_message": "done, all good"})
    end = [x for x in s.events() if x["kind"] == "TASK_ENDED"]; ck("ended as STOPPED, not DONE", end and end[0]["body"]["outcome"] == "STOPPED", end)
    for x in oracles(s): ck("oracles", False, x)
    s.close()
for label, pre in [("no register", lambda s: None), ("register begun, ACR not in use", lambda s: s.mcp([("acr_begin", {})])), ("after a halt", lambda s: (s.begin_adopted(), s.prompt("stop")))]:
    @scenario("F17 a subagent is a task whose end the host observes", "a subagent outside ACR use is not recorded: " + label)
    def _(ck, pre=pre):
        s = S(); pre(s); b0 = s.bytes(); s.hook("subagent-start", {"agent_type": "Explore", "agent_id": "z"}); s.hook("subagent-stop", {"agent_id": "z", "last_assistant_message": "x"})
        ck("nothing written", s.bytes() == b0); s.close()
@scenario("F17 a subagent is a task whose end the host observes", "forty subagents at once")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(5)
    def go(i):
        s.hook("subagent-start", {"agent_type": "checker", "agent_id": f"p-{i}"}); s.hook("subagent-stop", {"agent_id": f"p-{i}", "last_assistant_message": f"result {i}"})
    with cf.ThreadPoolExecutor(20) as ex: list(ex.map(go, range(40)))
    k = s.kinds(); ck("forty started and forty ended", k.count("TASK_STARTED") == 40 and k.count("TASK_ENDED") == 40, (k.count("TASK_STARTED"), k.count("TASK_ENDED")))
    ck("all DONE", all(e["body"]["outcome"] == "DONE" for e in s.events() if e["kind"] == "TASK_ENDED"))
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F18: the write fence ---------------------------------------------------------------------------------
def fence_cases(s):
    r = s.root
    (r / "sub").mkdir(exist_ok=True); 
    if not (r / "lnk").exists(): os.symlink(r / ".pcp", r / "lnk")
    if not (r / "reglink").exists(): os.symlink(s.reg, r / "reglink")
    deny = [".pcp/register.jsonl", str(s.reg), "./.pcp/register.jsonl", "sub/../.pcp/register.jsonl", ".pcp/observed.jsonl", ".pcp/new_file.txt", ".pcp/sub/deeper.txt", "lnk/register.jsonl", "reglink",
            ".PCP/register.jsonl", ".pcp/REGISTER.JSONL", str(r) + "//.pcp//register.jsonl", ".pcp/../.pcp/register.jsonl"]
    allow = ["notes.md", "sub/register.jsonl", "pcp/register.jsonl", ".pcpx/register.jsonl", "register.jsonl", "/tmp/elsewhere.txt", ".pcp.bak", "src/.pcp_notes"]
    return deny, allow
for tool, key in [("Write", "file_path"), ("Edit", "file_path"), ("MultiEdit", "file_path"), ("NotebookEdit", "notebook_path")]:
    @scenario("F18 a file tool cannot write the register or what stands beside it", tool)
    def _(ck, tool=tool, key=key):
        s = S(); s.begin_adopted(); deny, allow = fence_cases(s)
        for p in deny:
            rc, out, err = s.hook("pre-tool", {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {key: p}})
            if '"permissionDecision":"deny"' not in out: ck("denied: " + p, False, out or err)
        for p in allow:
            rc, out, err = s.hook("pre-tool", {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {key: p}})
            if out != "" or rc != 0: ck("allowed in silence: " + p, False, out or err)
        ck(f"{len(deny)} denied, {len(allow)} allowed", True, summary=True); s.close()
@scenario("F18 a file tool cannot write the register or what stands beside it", "with no register at all the fence still holds for the path, and says nothing elsewhere")
def _(ck):
    s = S(); rc, out, _ = s.hook("pre-tool", {"tool_name": "Write", "tool_input": {"file_path": "a.txt"}}); ck("silent", out == "" and rc == 0, out)
    rc, out, _ = s.hook("pre-tool", {"tool_name": "Write", "tool_input": {"file_path": ".pcp/register.jsonl"}}); ck("the register's own path is denied even before it exists", "deny" in out, out); s.close()
@scenario("F18 a file tool cannot write the register or what stands beside it", "a custom register path is fenced")
def _(ck):
    s = S(env={"PCPOS_REGISTER": "records/acr.jsonl"}); s.typed("review", "x")
    ck("register at the custom path", (s.root / "records/acr.jsonl").exists(), os.listdir(s.root))
    for p, want in [("records/acr.jsonl", True), ("records/observed.jsonl", True), ("records/x", True), (".pcp/register.jsonl", False)]:
        out = s.hook("pre-tool", {"tool_name": "Edit", "tool_input": {"file_path": p}})[1]; ck(("denied " if want else "allowed ") + p, ("deny" in out) == want, out)
    s.close()

# ---------- F19: session start --------------------------------------------------------------------------------------
for src in ["startup", "resume", "compact", "clear", ""]:
    for state in ["no register", "begun, not in use", "in use, delivered", "halted"]:
        @scenario("F19 the start of a session", f"{src or '(none)'} / {state}")
        def _(ck, src=src, state=state):
            s = S()
            if state == "begun, not in use": s.mcp([("acr_begin", {})])
            if state in ("in use, delivered", "halted"): s.begin_adopted(); s.cook(1)
            if state == "halted": s.prompt("stop")
            b0 = s.bytes(); rc, out, err = s.hook("session-start", {"hook_event_name": "SessionStart", "source": src, "model": "claude-x"})
            live = state == "in use, delivered"
            ck("exit 0", rc == 0, err)
            if not live: ck("silent and nothing written when ACR is not in use", out == "" and s.bytes() == b0, out)
            else:
                ck("the status line is given", "ACR record: use IN_USE" in out, out)
                reset = [e for e in s.events() if e["kind"] == "DELIVERY_STATE" and e["body"].get("event") == "CONTEXT_RESET"]
                ck("a context reset is recorded exactly when the host cleared or compacted", (len(reset) == 1) == (src in ("compact", "clear")), (src, len(reset)))
                if src in ("compact", "clear"):
                    r = s.mcp([("acr_submit", {"kind": COOK[1][0], "body": COOK[1][1]}), ("acr_submit", {"kind": COOK[2][0], "body": COOK[2][1]})])
                    st = s.mcp([("acr_status", {})])[0]; ck("after a reset the status says this session no longer holds the method text", "ATTENTION" in st and ("reset" in st.lower() or "deliver" in st.lower()), st[:900])
            for x in oracles(s): ck("oracles", False, x)
            s.close()

# ---------- F20: workspaces and modules --------------------------------------------------------------------------------
WS = [("science statistics", True), ("science,statistics", True), ("science+statistics", True), ("core", None), ("science science", None), ("nonsense", False), ("mathematics", True), ("sci", None), ("QA", None),
      ("quality_assurance", True), ("quality-assurance", None), ("SCIENCE", None), ("science nonsense", False), ("../../etc", False), ("", None)]
for args, valid in WS:
    @scenario("F20 a workspace's modules are set by the command the person typed", repr(args))
    def _(ck, args=args, valid=valid):
        s = S(); out = s.typed("workspace", args); k = s.kinds()
        ck("no adoption from a workspace command", "ADOPTED" not in k, k)
        ck("the host answers once per event and never crashes", "panicked" not in out, out)
        if valid is True: ck("recorded", "recorded at event" in out, out)
        if valid is False: ck("refused or not recorded", "recorded at event" not in out, out)
        n = len(s.events()); again = s.typed("workspace", args)
        if valid: ck("the same command again is answered and leaves the events already written in place", "panicked" not in again and s.kinds()[:n] == k and len(s.events()) >= n, again[:200])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
for mods, want in [([], True), (["science"], True), (["science", "statistics"], True), (["science", "science"], False), (["core"], False), (["nonsense"], False), (["sci"], None)]:
    @scenario("F21 a review is commissioned with modules the method text at hand actually holds", repr(mods))
    def _(ck, mods=mods, want=want):
        s = S(); s.begin_adopted(); s.cook(2); listing = s.mcp([("acr_modules", {})])[0]
        import re
        found = {m.group(1): (m.group(2), m.group(3)) for m in re.finditer(r"(acr\.[a-z_]+)\s+(?:version\s+)?([0-9][^\s,;]*)[^\n]*?([0-9a-f]{64})", listing)}
        ck("seven modules listed with digests", len(found) == 7, listing[:600])
        b = copy.deepcopy(COOK[2][1]); b["label"] = "R2"
        def ent(n):
            key = "acr." + {"sci": "science"}.get(n, n) if n != "nonsense" else "acr.nonsense"
            v, d = found.get(key, ("1.0", "0" * 64)); return {"id": "acr." + n if n in ("sci",) else key, "version": v, "sha256": d}
        b["modules"] = [ent(n) for n in mods]
        r = s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b})])[0]
        if want is True: ck("written", r.startswith("WRITTEN"), r[:300])
        if want is False: ck("refused", r.startswith("REFUSE"), r[:300])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario("F21 a review is commissioned with modules the method text at hand actually holds", "a wrong digest for a real module is refused")
def _(ck):
    s = S(); s.begin_adopted(); s.cook(2); b = copy.deepcopy(COOK[2][1]); b["label"] = "R2"; b["modules"] = [{"id": "acr.science", "version": "1.0", "sha256": "1" * 64}]
    r = s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b})])[0]; ck("refused", r.startswith("REFUSE"), r[:300]); s.close()

# ---------- F22: clause lookup against the machine form -----------------------------------------------------------------
@scenario("F22 every provision the machine form lists is returned by the program, byte for byte", "all modules")
def _(ck):
    import re
    with open(os.environ["PCP_MACHINE"]) as reference:
        modules = json.load(reference)["modules"]
    s = S()
    calls, digests = [], []
    for module in modules:
        for provision in module["provisions"]:
            calls.append(("acr_clause", {"module": module["key"], "section": provision["number"]}))
            digests.append(provision["text_sha256"])
    replies = s.mcp(calls)
    ck("all 260 requested provisions have one reply", len(calls) == len(replies) == 260,
       {"requests": len(calls), "replies": len(replies)})
    bad_headers, bad_bodies = [], []
    for (_, request), digest, reply in zip(calls, digests, replies):
        header, separator, body = reply.partition("\n\n")
        module_line, _, provision_line = header.partition("\n")
        section = re.escape(request["section"])
        if (not module_line.startswith(f"acr.{request['module']} ")
                or not re.fullmatch(rf"PROVISION {section} \| lines [0-9]+\.\.[0-9]+ \| SHA-256 of this text {digest}", provision_line)):
            bad_headers.append(request)
        if not separator or hashlib.sha256(body.encode("utf-8")).hexdigest() != digest:
            bad_bodies.append(request)
    ck("each reply names its module, provision and expected text digest", not bad_headers, bad_headers[:5])
    ck("the exact returned text hashes to the expected digest", not bad_bodies, bad_bodies[:5])
    ck("lookup puts nothing in use and writes nothing", not s.reg.exists())
    s.close()

# ---------- F23: the tool server as a protocol --------------------------------------------------------------------------
PROTO = [("each supported protocol version is echoed", [json.dumps({"jsonrpc": "2.0", "id": i, "method": "initialize", "params": {"protocolVersion": v}}) for i, v in enumerate(["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"])]),
         ("an unknown protocol version gets a supported one", [json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}})]),
         ("a line that is not JSON", ["this is not json", json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})]),
         ("an unknown method", [json.dumps({"jsonrpc": "2.0", "id": 3, "method": "nonsense/method"}), json.dumps({"jsonrpc": "2.0", "id": 4, "method": "tools/list"})]),
         ("a notification gets no reply", [json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}), json.dumps({"jsonrpc": "2.0", "id": 5, "method": "ping"})]),
         ("an unknown tool", [json.dumps({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "acr_nope", "arguments": {}}})]),
         ("arguments of the wrong shape", [json.dumps({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "acr_submit", "arguments": "text"}}), json.dumps({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "acr_submit", "arguments": {"kind": 5, "body": []}}}), json.dumps({"jsonrpc": "2.0", "id": 9, "method": "tools/call"})]),
         ("a very large line", [json.dumps({"jsonrpc": "2.0", "id": 10, "method": "tools/call", "params": {"name": "acr_adopt", "arguments": {"basis": "x" * 3000000}}}), json.dumps({"jsonrpc": "2.0", "id": 11, "method": "ping"})]),
         ("deeply nested JSON", ["[" * 100000, json.dumps({"jsonrpc": "2.0", "id": 12, "method": "ping"})]),
         ("resources and prompts lists", [json.dumps({"jsonrpc": "2.0", "id": 13, "method": "resources/list"}), json.dumps({"jsonrpc": "2.0", "id": 14, "method": "prompts/list"})]),
         ("string and null ids", [json.dumps({"jsonrpc": "2.0", "id": "abc", "method": "ping"}), json.dumps({"jsonrpc": "2.0", "id": None, "method": "ping"})]),
         ("a batch array", [json.dumps([{"jsonrpc": "2.0", "id": 1, "method": "ping"}, {"jsonrpc": "2.0", "id": 2, "method": "ping"}]), json.dumps({"jsonrpc": "2.0", "id": 15, "method": "ping"})]),
         ("empty lines and no input", ["", "", ""])]
for label, lines in PROTO:
    @scenario("F23 the tool server answers a host correctly whatever arrives", label)
    def _(ck, label=label, lines=lines):
        s = S(); rc, out, err = s.mcp_raw(lines)
        ck("no crash", rc == 0 and "panicked" not in err, (rc, err[-300:]))
        ok = True
        for l in out:
            try: j = json.loads(l); ok &= j.get("jsonrpc") == "2.0" and ("result" in j or "error" in j)
            except Exception: ok = False
        ck("every line written is a JSON-RPC response", ok, out[:3]); ck("no register appears from talk alone", not s.reg.exists())
        want_ids = [json.loads(l).get("id") for l in lines if l.startswith("{") and "id" in json.loads(l) and json.loads(l)["id"] is not None]
        got = [json.loads(l).get("id") for l in out]
        ck("each request with an id is answered, in order", [g for g in got if g is not None][-len(want_ids):] == want_ids if want_ids else True, (want_ids, got))
        if label.startswith("each supported"): ck("versions echoed", [json.loads(l)["result"]["protocolVersion"] for l in out] == ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"], out)
        s.close()
for raw in [b"", b"not json", b"[]", b'"text"', b"{", b'{"prompt": 5}', b'{"prompt": null, "cwd": 7}', b"\xff\xfe", b'{"cwd":"/nonexistent/dir","prompt":"/pcp-os:review x"}']:
    for ev in ["prompt", "session-start", "subagent-start", "subagent-stop", "pre-tool", "nonsense"]:
        @scenario("F24 a hook given malformed input fails quietly and writes nothing", f"{ev}: {raw[:20]!r}")
        def _(ck, raw=raw, ev=ev):
            s = S(); s.begin_adopted(); b0 = s.bytes(); rc, out, err = s.hook(ev, None, raw=raw)
            ck("no crash", "panicked" not in err and rc in (0, 1, 2), (rc, err[-200:])); ck("never exit 2, which a host reads as a block", rc != 2, (rc, err[-200:]))
            ck("register unchanged", s.bytes() == b0, s.kinds()[-2:]); s.close()

# ---------- F25: the person at a terminal -----------------------------------------------------------------------------------
@scenario("F25 a person at a terminal", "init, adopt, standard, halt")
def _(ck):
    s = S(); r = str(s.root / "r.jsonl")
    rc, out = s.cli("init", r, "--principal", "kirtus", "--name", "Kirtus Justus"); ck("init", rc == 0, out)
    rc, out = s.cli("init", r, "--principal", "kirtus"); ck("a second init does not overwrite", rc != 0 or "already" in out, out)
    rc, out = s.cli("adopt", r, "--by", "kirtus", "--basis", "use ACR"); ck("adopt as the person", rc == 0, out)
    rc, out = s.cli("adopt", r, "--by", "nobody", "--basis", "use ACR"); ck("adopt by an unknown label is refused", rc != 0, out)
    rc, out = s.cli("standard", r, "--by", "kirtus", "CRITICAL"); ck("a two-person standard is refused in the Personal edition, with the reason", rc != 0 and "one person" in out, out)
    rc, out = s.cli("standard", r, "--by", "kirtus", "BASELINE"); ck("standard set", rc == 0, out)
    rc, out = s.cli("halt", r, "--by", "kirtus", "--words", "stop"); ck("halt", rc == 0, out)
    rc, out = s.cli("halt", r, "--by", "kirtus"); ck("a second halt is harmless", "panicked" not in out, out)
    rc, out = s.cli("verify", r); ck("verifies", rc == 0 and "0 problem(s)" in out, out[-300:])
    ev = [json.loads(l) for l in open(r)]; ck("the person's acts are the person's", all(e["actor"] == "kirtus" for e in ev if e["kind"] in ("ADOPTED", "HALTED", "STANDARD_SET")), [(e["actor"], e["kind"]) for e in ev])
    s.close()
for args in [["status"], ["verify"], ["status", "/nonexistent"], ["verify", "/etc/passwd"], ["machine", "NOPE"], ["machine"], ["method", "/nonexistent"], ["clause"], ["nonsense"], [], ["init"], ["halt", "x.jsonl"], ["adopt", "x.jsonl", "--by"], ["standard", "x.jsonl", "--by", "a", "NOPE"], ["hook"], ["modules", "/tmp"]]:
    @scenario("F25 a person at a terminal", "bad arguments: " + " ".join(args))
    def _(ck, args=args):
        s = S(); rc, out = s.cli(*args); ck("no crash", "panicked" not in out and rc in (0, 1, 2, 3, 4, 5, 6, 64), (rc, out[-300:])); ck("nothing created", not any(s.root.iterdir()), list(s.root.iterdir())); s.close()

# ---------- F27: words given only in a conversation -------------------------------------------------------------------------
TEXTS = ["The sum of the whole numbers from 1 to 100 is 5,500.", "x", "line one\nline two\n", "é ü 漢字 ☃", " leading and trailing ", "{\"json\": true}", "a" * 100000]
for i, txt in enumerate(TEXTS):
    @scenario("F27 a claim typed in the conversation is recorded without any file tool or shell", f"{i}: {txt[:20]!r}", EDITIONS[i % 3])
    def _(ck, txt=txt, i=i):
        s = S(EDITIONS[i % 3]); s.begin_adopted(); d = hashlib.sha256(txt.encode()).hexdigest()
        r = s.mcp([("acr_artifact", {"text": txt, "record": False}), ("acr_artifact", {"text": txt, "name": "claim"})])
        ck("dry run returns the digest and writes nothing", d in r[0] and "Nothing was written" in r[0], r[0][:200])
        ck("recorded with the digest the server computed", r[1].startswith("WRITTEN") and d in r[1], r[1][:300])
        kept = s.root / ".pcp/artifacts" / d
        ck("the bytes are kept beside the register, exactly", kept.exists() and kept.read_bytes() == txt.encode())
        ev = s.events()[-1]; ck("the event says the server was handed the text and did not read a file", ev["body"].get("read_by") == "HOST_FROM_TOOL_ARGUMENT" and ev["body"]["sha256"] == d and ev["body"]["size"] == len(txt.encode()), ev["body"])
        b = copy.deepcopy(COOK[2][1]); b["target"] = {"t": "artifact", "sha256": d}
        r = s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b}), ("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": d}, "extent": "all of it"}}), ("acr_submit", {"kind": "GATE_COMMITTED", "body": {"review": "R1", "gate": "G1"}})])
        ck("a review is commissioned on it and its first gate committed", all(x.startswith("WRITTEN") for x in r), [x[:200] for x in r])
        out = s.hook("pre-tool", {"tool_name": "Edit", "tool_input": {"file_path": ".pcp/artifacts/" + d}})[1]; ck("a file tool may not alter the kept bytes", "deny" in out, out)
        for x in oracles(s): ck("oracles", False, x)
        s.close()
for label, args in [("empty text", {"text": ""}), ("text and path together", {"text": "a", "path": "b"}), ("neither", {}), ("text of the wrong type", {"text": 5}), ("too large", {"text": "x" * (1024 * 1024 + 1)}), ("a bad origin", {"text": "a", "origin": "RETRIEVED"}), ("no register", {"text": "a"})]:
    @scenario("F27 a claim typed in the conversation is recorded without any file tool or shell", "refused: " + label)
    def _(ck, label=label, args=args):
        s = S()
        if label != "no register": s.begin_adopted()
        b0 = s.bytes(); r = s.mcp([("acr_artifact", args)])[0]
        ck("refused with a reason", not r.startswith("WRITTEN"), r[:200]); ck("nothing written", s.bytes() == b0 and not (s.root / ".pcp/artifacts").exists(), os.listdir(s.root)); s.close()
@scenario("F27 a claim typed in the conversation is recorded without any file tool or shell", "the artifacts folder is a link to elsewhere")
def _(ck):
    s = S(); s.begin_adopted(); (s.top / "out").mkdir(); os.symlink(s.top / "out", s.root / ".pcp/artifacts"); b0 = s.bytes()
    r = s.mcp([("acr_artifact", {"text": "abc"})])[0]
    ck("nothing written outside", not any((s.top / "out").iterdir()), r[:200]); ck("nothing recorded", s.bytes() == b0, r[:200]); s.close()

# ---------- F28: a person's own modules ---------------------------------------------------------------------------------------
def spec(ident, version="1.0", extra=""):
    return f"# T\n\n**Component:** `{ident}`, Version {version}  \n{extra}\n## 1. Scope\n\nThe text of {ident}.\n\n1.1. A requirement.\n"
for i, (ident, version, ok) in enumerate([("local.tides", "1.0", True), ("local.tides", "2.4.1-rc.2+build.4", True), ("local.a_b", "10.0.1", True), ("acr.tides", "1.0", False), ("local.science", "1.0", False), ("local.core", "1.0", False),
                                          ("tides", "1.0", False), ("local.", "1.0", False), ("local.x.y", "1.0", False), ("local.tides", "", False), ("local.tides", "v1", False), ("local.Tides Spec", "1.0", False), ("other.tides", "1.0", False)]):
    @scenario("F28 a person adds a module of their own; it never displaces the method's", f"{ident} {version!r}", EDITIONS[i % 3])
    def _(ck, ident=ident, version=version, ok=ok, i=i):
        s = S(EDITIONS[i % 3]); s.begin_adopted(); (s.root / "in").mkdir(); (s.root / "in/Specification.md").write_text(spec(ident, version)); b0 = s.bytes()
        rc, out = s.cli("module", "add", ".pcp/register.jsonl", "in", "--by", "principal")
        if not ok:
            ck("refused", rc != 0, out); ck("nothing stored and nothing recorded", s.bytes() == b0 and not (s.root / ".pcp/modules").exists(), out)
        else:
            d = hashlib.sha256(spec(ident, version).encode()).hexdigest(); name = ident.split(".")[1]
            ck("added", rc == 0 and d in out, out); ev = s.events()[-1]
            ck("recorded with its digest and whole version", ev["kind"] == "HOST_EVENT" and ev["body"]["event"] == "MODULE_ADDED" and ev["body"]["module"] == {"id": ident, "version": version, "sha256": d}, ev["body"])
            r = s.mcp([("acr_modules", {}), ("acr_clause", {"module": name, "section": "1.1"}), ("acr_clause", {"module": "core", "section": "15.2"})])
            ck("listed as ADDED with the seven of the method", "ADDED" in r[0] and r[0].count("SPECIALIST") == 6 and d in r[0], r[0][:900])
            ck("its clause is served", f"{ident} {version}" in r[1] and "A requirement" in r[1], r[1][:200]); ck("Core is still Core", "acr.core 1.0" in r[2] and "649211ff2d765e06b5d5eb31dc80af6f97aedea242f2dcb5b77b3fa270ef34d6" in r[2], r[2][:200])
            b = copy.deepcopy(COOK[2][1]); b["modules"] = [{"id": ident, "version": version, "sha256": d}]; s.cook(2)
            r = s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b})])[0]; ck("a review selects it by digest", r.startswith("WRITTEN"), r[:300])
            b2 = copy.deepcopy(b); b2["label"] = "R2"; b2["modules"][0]["sha256"] = "0" * 64
            r = s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b2})])[0]; ck("and not by another digest", r.startswith("REFUSE"), r[:300])
            out = s.typed("workspace", name); ck("the person's typed workspace command selects it", "recorded at event" in out and ident in out, out)
            ck("the assistant's file tools cannot write it", "deny" in s.hook("pre-tool", {"tool_name": "Write", "tool_input": {"file_path": f".pcp/modules/{name}/SOURCE/Specification.md"}})[1])
            rc, out = s.cli("module", "remove", ".pcp/register.jsonl", name, "--by", "principal"); ck("removed", rc == 0 and s.events()[-1]["body"]["event"] == "MODULE_REMOVED", out)
            r = s.mcp([("acr_clause", {"module": name})])[0]; ck("no longer served", "is not a module" in r, r[:200])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario("F28 a person adds a module of their own; it never displaces the method's", "a session cannot add a module through the tools")
def _(ck):
    s = S(); s.begin_adopted(); b0 = s.bytes()
    r = s.mcp([("acr_submit", {"kind": "HOST_EVENT", "body": {"event": "MODULE_ADDED", "module": {"id": "local.x", "version": "1", "sha256": "0" * 64}}})])[0]
    ck("refused", r.startswith("REFUSE"), r[:200]); ck("nothing written", s.bytes() == b0); s.close()
for label, ident in [("a folder dropped in by hand that claims Core", "acr.core"), ("one that takes a method module's name", "local.science"), ("two that claim the same name", "local.twin")]:
    @scenario("F28 a person adds a module of their own; it never displaces the method's", label)
    def _(ck, label=label, ident=ident):
        s = S(); s.begin_adopted()
        for n in (["a", "b"] if "two" in label else ["a"]):
            d = s.root / ".pcp/modules" / n / "SOURCE"; d.mkdir(parents=True); (d / "Specification.md").write_text(spec(ident) + n)
        r = s.mcp([("acr_modules", {}), ("acr_clause", {"module": ident.split(".")[1], "section": "1"}), ("acr_clause", {"module": "core", "section": "15.2"}), ("acr_clause", {"module": "science", "section": "1"})])
        ck("reported, not used", "NOTE" in r[0] and "ADDED" not in r[0], r[0][-600:])
        ck("the method's own text is what is served", "649211ff2d765e06b5d5eb31dc80af6f97aedea242f2dcb5b77b3fa270ef34d6" in r[2] and "acr.science 1.0" in r[3], (r[2][:120], r[3][:120]))
        if "two" in label: ck("an identifier two folders claim is served from neither", "ambiguous" in r[1], r[1][:300])
        else: ck("the rogue text is never served", "rogue" not in r[1] and (r[1].startswith("acr.") or "not a module" in r[1]), r[1][:200])
        s.close()
@scenario("F28 a person adds a module of their own; it never displaces the method's", "verify says why it exits 1")
def _(ck):
    s = S(); s.prompt("hello"); s.mcp([("acr_begin", {}), ("acr_adopt", {"basis": "Use ACR"})]); rc, out = s.cli("verify", ".pcp/register.jsonl")
    ck("exit 1 with the reason stated", rc == 1 and "exit status 1: the bytes verify" in out and "0 problem(s)" in out, out); s.close()
@scenario("F05 an adoption the assistant relays is marked relayed, and observed only if it is the whole prompt", "the same words with other spacing and line breaks are the same prompt")
def _(ck):
    s = S(); s.prompt("Use ACR. Review this claim: 'x'. I would like this confirmed."); r = s.mcp([("acr_begin", {}), ("acr_adopt", {"basis": "Use ACR. Review this claim: 'x'.\n\nI would like   this confirmed."})])
    ad = [e for e in s.events() if e["kind"] == "ADOPTED"]; ck("observed", ad and ad[0]["body"]["observed"] == "RELAYED_WHOLE_PROMPT_OBSERVED", r[1][-200:])
    t = S(); t.prompt("do not adopt ACR"); t.mcp([("acr_begin", {}), ("acr_adopt", {"basis": "adopt ACR"})]); ad = [e for e in t.events() if e["kind"] == "ADOPTED"]
    ck("a fragment is still not the prompt", ad and ad[0]["body"]["observed"] == "RELAYED_NOT_OBSERVED"); s.close(); t.close()

# ---------- F26: random walks ------------------------------------------------------------------------------------------------

@scenario("F23 the tool server answers a host correctly whatever arrives", "a delivery record under a name that is not the session's label is refused, and the answer gives the label")
def _(ck):
    s = S(); s.typed("adopt"); b0 = s.bytes()
    r = s.mcp([("acr_submit", {"kind": "DELIVERY_STATE", "body": dict(COOK[0][1], agent="Claude Haiku 5.5")}), ("acr_submit", {"kind": "DELIVERY_STATE", "body": COOK[0][1]})])
    ck("refused, nothing written, and the label is given", r[0].startswith("REFUSE") and "`assistant`" in r[0], r[0][:300])
    ck("with the label it is written", r[1].startswith("WRITTEN") and s.bytes().startswith(b0), r[1][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F29: what a review wrote for itself is not evidence it checked -----------------------------------------------------
H = lambda x: hashlib.sha256(x.encode()).hexdigest()
CLAIM = "The sum of the whole numbers from 1 to 100 is 5,500."
OWN = "Pair 1 with 100, 2 with 99, and so on: 50 pairs of 101, which is 5,050, not 5,500."
def review_to_g1(s, kind, before=()):
    """A review of words typed in the conversation, to G1, with one claim of the given kind. `before`: texts recorded as PRODUCED before it is commissioned."""
    s.typed("review", '"%s"' % CLAIM)
    calls = [("acr_submit", {"kind": "DELIVERY_STATE", "body": COOK[0][1]}), ("acr_artifact", {"text": CLAIM, "name": "target"})]
    calls += [("acr_artifact", {"text": x, "origin": "PRODUCED"}) for x in before]
    calls += [("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": H(x)}, "extent": "all of it"}}) for x in before]
    body = {"label": "R1", "title": "Sum of 1 to 100", "target": {"t": "artifact", "sha256": H(CLAIM)}, "extent": "TOTAL", "procedure": "FOCUSED", "director": "principal",
            "claims": [{"label": "A", "text": CLAIM, "origin": "COMMISSIONER", "kind": kind, "criterion": "Refuted if the sum is shown, by derivation and an independent calculation, to differ from 5,500."}]}
    calls += [("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": body}), ("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": H(CLAIM)}, "extent": "all of it"}}),
              ("acr_submit", {"kind": "GATE_COMMITTED", "body": {"review": "R1", "gate": "G1"}})]
    r = s.mcp(calls); assert all(x.startswith("WRITTEN") for x in r), r
ART = lambda x: {"ref": {"t": "artifact", "sha256": H(x)}}
FIND = lambda disp, basis=(), contrary=(), **kw: ("acr_submit", {"kind": "FINDING_RECORDED", "body": dict({"review": "R1", "claim": "A", "disposition": disp, "basis": list(basis), "contrary": list(contrary)}, **kw)})
for i, (kind, ed) in enumerate(itertools.product(["DERIVATION", "UNSPECIFIED", "EXTERNAL_FACT", "MECHANICAL_CHECK"], EDITIONS)):
    @scenario("F29 what a review wrote for itself is not evidence it checked", f"refuting a {kind} claim on the reviewer's own derivation", ed)
    def _(ck, kind=kind, ed=ed):
        s = S(ed); review_to_g1(s, kind)
        r = s.mcp([("acr_artifact", {"text": OWN, "origin": "PRODUCED", "name": "derivation"}), ("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": H(OWN)}, "extent": "all of it, my own derivation"}})])
        ck("the derivation is recorded as the review's product", all(x.startswith("WRITTEN") for x in r), r); b0 = s.bytes()
        r = s.mcp([FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [ART(OWN)]), ("acr_offer", FIND("REFUTED_IN_SCOPE", [], [ART(OWN)])[1]), FIND("SUPPORTED_IN_SCOPE", [ART(OWN)])])
        ck("the refutation that register A of build 17 holds is refused", r[0].startswith("REFUSE") and "Core 4.3, 5.2" in r[0] and "UNRESOLVED" in r[0], r[0][:300])
        ck("the offer says the same and names the clause", r[1].startswith("REFUSE") and "Core 4.3, 5.2" in r[1], r[1][:200])
        ck("support resting on the same text alone is refused", r[2].startswith("REFUSE"), r[2][:200])
        ck("a refusal writes nothing", s.bytes() == b0)
        r = s.mcp([FIND("UNRESOLVED", [ART(CLAIM), ART(OWN)], left_open="No check was run: the derivation is reasoning. sum(range(1, 101)) would settle it."),
                   ("acr_submit", {"kind": "GATE_COMMITTED", "body": {"review": "R1", "gate": "G5", "basis": [{"ref": {"t": "finding", "id": "F1"}}]}}), ("acr_submit", {"kind": "GATE_COMMITTED", "body": {"review": "R1", "gate": "G6", "note": "Reported to the person as unresolved: no check was run."}}),
                   ("acr_submit", {"kind": "REVIEW_CLOSED", "body": {"review": "R1", "lifecycle": "CLOSED_COMPLETE", "examined_extent": "TOTAL"}}), ("acr_status", {})])
        ck("UNRESOLVED, citing the derivation as what was reasoned, is accepted and the review closes", all(x.startswith("WRITTEN") for x in r[:4]), [x[:160] for x in r[:4]])
        f = [e for e in s.events() if e["kind"] == "FINDING_RECORDED"]
        ck("one finding, UNRESOLVED and REASONED", len(f) == 1 and f[0]["body"]["disposition"] == "UNRESOLVED" and f[0]["body"]["execution"] == "REASONED", f)
        ck("the status says what the finding rests on", "FINDINGS" in r[4] and "no check was run" in r[4], r[4][-700:])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario("F29 what a review wrote for itself is not evidence it checked", "material produced before the review was commissioned is material the review was given")
def _(ck):
    s = S(); review_to_g1(s, "DERIVATION", before=[OWN])
    r = s.mcp([FIND("REFUTED_IN_SCOPE", [], [ART(OWN)])]); ck("accepted", r[0].startswith("WRITTEN"), r[0][:300])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F29 what a review wrote for itself is not evidence it checked", "recording the product again as SUPPLIED does not make it supplied")
def _(ck):
    s = S(); review_to_g1(s, "DERIVATION")
    r = s.mcp([("acr_artifact", {"text": OWN, "origin": "PRODUCED"}), ("acr_artifact", {"text": OWN, "origin": "SUPPLIED"}), FIND("REFUTED_IN_SCOPE", [], [dict(ART(OWN), extent="all")])])
    ck("still refused", r[2].startswith("REFUSE") and "Core 4.3, 5.2" in r[2], r[2][:300])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F29 what a review wrote for itself is not evidence it checked", "the product beside inspected material: the finding is accepted, and the limit is stated")
def _(ck):
    # The guards cannot tell whether the target's own words are contrary to the claim: that is judgment. What they hold is that the
    # review's own text is never the whole of the evidence. This scenario fixes that boundary so that nobody reads more into the rule.
    s = S(); review_to_g1(s, "DERIVATION")
    r = s.mcp([("acr_artifact", {"text": OWN, "origin": "PRODUCED"}), FIND("REFUTED_IN_SCOPE", [], [dict(ART(OWN), extent="all"), ART(CLAIM)]), ("acr_status", {})])
    ck("accepted, as REASONED", r[1].startswith("WRITTEN") and s.events()[-1]["body"]["execution"] == "REASONED", r[1][:300])
    ck("and the status says no check was run", "no check was run" in r[2], r[2][-500:])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F29 what a review wrote for itself is not evidence it checked", "a subagent's returned message alone refutes nothing")
def _(ck):
    s = S(); review_to_g1(s, "DERIVATION"); msg = "I checked it: the sum is 5,050."
    s.hook("subagent-start", {"hook_event_name": "SubagentStart", "agent_id": "a1", "agent_type": "checker"})
    s.hook("subagent-stop", {"hook_event_name": "SubagentStop", "agent_id": "a1", "agent_type": "checker", "last_assistant_message": msg})
    ck("the task ended DONE and its message is in the record", [e["body"]["outcome"] for e in s.events() if e["kind"] == "TASK_ENDED"] == ["DONE"], s.kinds())
    r = s.mcp([FIND("REFUTED_IN_SCOPE", [], [dict(ART(msg), extent="all of the returned message")])])
    ck("refused: what a task of the review returned is the review's product", r[0].startswith("REFUSE") and "Core 4.3, 5.2" in r[0], r[0][:300])
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F30: a check is what the host saw run, or it says it is a report ---------------------------------------------------
CMD = 'python3 -c "print(sum(range(1,101)))"'
def ran(s, command=CMD, stdout="5050", stderr="", tid="toolu_1", tool="Bash", interrupted=False, ms=97):
    return s.hook("post-tool", {"hook_event_name": "PostToolUse", "tool_name": tool, "tool_input": {"command": command}, "tool_use_id": tid, "duration_ms": ms,
                                "tool_response": {"stdout": stdout, "stderr": stderr, "interrupted": interrupted, "isImage": False}})
def failed(s, command, error, tid="toolu_f", interrupt=False):
    return s.hook("post-tool", {"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_input": {"command": command}, "tool_use_id": tid, "error": error, "is_interrupt": interrupt})
SEEN = lambda s: s.root / ".pcp/observed_tools.jsonl"
for ed in EDITIONS:
    @scenario("F30 a check is what the host saw run, or it says it is a report", "a command the host saw is recorded as observed, once", ed)
    def _(ck, ed=ed):
        s = S(ed); review_to_g1(s, "MECHANICAL_CHECK"); b_before = s.bytes(); rc, out, err = ran(s)
        ck("the hook says nothing and does not fail", rc == 0 and out == "", (rc, out, err))
        kept = SEEN(s).read_text() if SEEN(s).exists() else ""
        ck("the host kept its observation", kept.count("\n") == 1 and json.loads(kept)["stdout"] == H("5050") and json.loads(kept)["exit"] == 0, kept)
        ck("and not the output itself", '"5050"' not in kept and "5050\"" not in kept.replace(H("5050"), ""), kept)
        b0 = s.bytes(); ck("observing writes nothing in the register", bool(b0) and b0 == b_before, (len(b_before or b""), len(b0 or b"")))
        r = s.mcp([("acr_check", {"command": CMD, "name": "sum"}), ("acr_check", {"command": CMD}), ("acr_check", {"command": CMD + " "}), ("acr_check", {"command": "python3 never_run.py"})])
        k = [e for e in s.events() if e["kind"] == "CHECK_RAN"]
        ck("one check, written by the session and marked as the host's observation", len(k) == 1 and k[0]["actor"] == "assistant" and k[0]["body"]["observed"] == "HOST_OBSERVED_TOOL", (r[0][:200], k))
        if k:
            b = k[0]["body"]; ck("with the host's exit status and digests, and the command as given", b["exit"] == 0 and b["stdout"] == H("5050") and b["stderr"] == H("") and b["argv"] == [CMD] and b["tool_use_id"] == "toolu_1" and b["timeout"] is False, b)
        ck("the same observation is not recorded twice", r[1].startswith("REFUSE") and "Nothing was written" in r[1], r[1][:200])
        ck("a command that differs by a space was not seen", r[2].startswith("REFUSE"), r[2][:200])
        ck("a command never run is not a check, and the answer says what follows", r[3].startswith("REFUSE") and "UNRESOLVED" in r[3] and "Core 5.1" in r[3], r[3][:300])
        ck("with nothing left unrecorded, the answer lists no command", "has seen no shell command here that is not already recorded" in r[3], r[3][:400])
        r = s.mcp([FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [{"ref": {"t": "check", "id": "K1"}}]), ("acr_status", {})])
        ck("the mechanical claim is refuted on the observed check, as EXECUTED", r[0].startswith("WRITTEN") and s.events()[-1]["body"]["execution"] == "EXECUTED", r[0][:300])
        ck("the status says the host saw it run", "check K1, which the host saw run" in r[1], r[1][-600:])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "a check the session submits itself is recorded as its report, whatever it claims")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    for i, claim in enumerate(["HOST_OBSERVED_TOOL", "RAN_BY_THIS_PROGRAM", None, "nonsense", 7]):
        body = dict(COOK[5][1]); body.update({} if claim is None else {"observed": claim})
        r = s.mcp([("acr_offer", {"kind": "CHECK_RAN", "body": body}), ("acr_submit", {"kind": "CHECK_RAN", "body": body})])
        ck(f"written as a report ({claim!r})", r[1].startswith("WRITTEN") and s.events()[-1]["body"]["observed"] == "REPORTED_BY_PARTICIPANT", (r[1][:200], s.events()[-1]["body"]))
        ck(f"and the offer said so first ({claim!r})", r[0].startswith("ACCEPT") and "REPORTED_BY_PARTICIPANT" in r[0], r[0][:300])
    r = s.mcp([FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [{"ref": {"t": "check", "id": "K1"}}]), ("acr_status", {})])
    ck("the status says the check was reported and not seen", "check K1, reported by assistant and not seen by the host" in r[1], r[1][-600:])
    ck("and the finding that rests on it alone does not say EXECUTED", r[0].startswith("WRITTEN") and s.events()[-1]["body"]["execution"] == "UNKNOWN", (r[0][:200], s.events()[-1]["body"].get("execution")))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "a failed command: its exit status is the host's, and it cannot support the claim")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); c = 'python3 -c "import sys; print(7); sys.exit(3)"'
    failed(s, c, "Exit code 3\n7"); r = s.mcp([("acr_check", {"command": c}), FIND("SUPPORTED_IN_SCOPE", [{"ref": {"t": "check", "id": "K1"}}]), FIND("REFUTED_IN_SCOPE", [], [{"ref": {"t": "check", "id": "K1"}}])])
    b = s.events()[[e["kind"] for e in s.events()].index("CHECK_RAN")]["body"]
    ck("exit 3 as the host reported it, expectation not met", b["exit"] == 3 and b["expect_exit"] == 0 and "NOT met" in r[0] and b["output"] == "ONE_TEXT_ON_FAILURE" and b["stdout"] == H("7"), (r[0][:300], b))
    ck("it cannot support the mechanical claim", r[1].startswith("REFUSE") and "Core 5.1" in r[1], r[1][:200])
    ck("it can refute it", r[2].startswith("WRITTEN"), r[2][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "an interrupted command, and one whose failure names no exit status, settle nothing")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    ran(s, "sleep 100", "", interrupted=True, tid="t1"); failed(s, "sleep 200", "Command timed out after 2m 0s", tid="t2")
    r = s.mcp([("acr_check", {"command": "sleep 100"}), ("acr_check", {"command": "sleep 200"}),
               FIND("SUPPORTED_IN_SCOPE", [{"ref": {"t": "check", "id": "K1"}}, {"ref": {"t": "check", "id": "K2"}}]), FIND("REFUTED_IN_SCOPE", [], [{"ref": {"t": "check", "id": "K1"}}, {"ref": {"t": "check", "id": "K2"}}])])
    ks = [e["body"] for e in s.events() if e["kind"] == "CHECK_RAN"]
    ck("both recorded with no exit status and as not having run to their end", len(ks) == 2 and all(k["exit"] is None and k["timeout"] is True for k in ks), ks)
    ck("neither supports nor refutes a mechanical claim", r[2].startswith("REFUSE") and r[3].startswith("REFUSE") and "ran to its end" in r[3], (r[2][:200], r[3][:200]))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
for i, (label, setup) in enumerate([("no register at all", lambda s: None), ("a register, and ACR never adopted", lambda s: s.mcp([("acr_begin", {})])),
                                    ("ACR halted", lambda s: (s.begin_adopted(), s.prompt("stop")))]):
    @scenario("F30 a check is what the host saw run, or it says it is a report", "outside ACR use the host keeps nothing of a shell command: " + label)
    def _(ck, setup=setup, label=label):
        s = S(); setup(s); before = s.bytes(); rc, out, err = ran(s); rc2, out2, err2 = failed(s, "false", "Exit code 1\n")
        ck("silent, no failure", (rc, out, rc2, out2) == (0, "", 0, ""), (rc, out, err, rc2, out2, err2))
        ck("nothing kept", not SEEN(s).exists(), os.listdir(s.root)); ck("register untouched", s.bytes() == before)
        if label == "no register at all": ck("no .pcp folder", not (s.root / ".pcp").exists())
        s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "only the shell tool is observed; other tools, long commands and malformed input are not kept and crash nothing")
def _(ck):
    s = S(); s.begin_adopted()
    for tool in ["Read", "Write", "WebFetch", "mcp__x__y", ""]:
        rc, out, err = ran(s, tool=tool); ck(f"{tool!r}: silent", rc == 0 and out == "", (rc, out, err))
    rc, out, err = ran(s, command="x" * 20000); ck("a 20,000-character command: silent", rc == 0 and out == "", (rc, out, err))
    ck("nothing kept", not SEEN(s).exists())
    for raw in [b"", b"{", b"null", b"[]", b'{"tool_name":"Bash"}', b'{"tool_name":"Bash","tool_input":7}', b'{"tool_name":"Bash","tool_input":{"command":9}}', b'{"tool_name":"Bash","tool_input":{"command":"a"},"tool_response":"x"}',
                b'{"tool_name":"Bash","tool_input":{"command":"b"},"hook_event_name":"PostToolUseFailure"}', b"\xff\xfe"]:
        rc, out, err = s.hook("post-tool", None, raw=raw); ck(f"malformed {raw[:30]!r}: no crash", "panicked" not in err and rc in (0, 1, 2), (rc, err[:200]))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "the host keeps its newest 200 observations and a file tool cannot write them")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    for i in range(205): ran(s, f"echo {i}", str(i), tid=f"t{i}")
    lines = SEEN(s).read_text().splitlines(); ck("200 kept, the newest", len(lines) == 200 and json.loads(lines[-1])["command"] == "echo 204" and json.loads(lines[0])["command"] == "echo 5", (len(lines), lines[-1][:80]))
    for tool, key in [("Write", "file_path"), ("Edit", "file_path")]:
        rc, out, err = s.hook("pre-tool", {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {key: ".pcp/observed_tools.jsonl"}})
        ck(f"{tool} to the observations is denied", '"permissionDecision":"deny"' in out, out or err)
    r = s.mcp([("acr_check", {"command": "echo 3"}), ("acr_check", {"command": "echo 204"})])
    ck("an observation that was dropped is not a check; the newest is", r[0].startswith("REFUSE") and r[1].startswith("WRITTEN"), (r[0][:120], r[1][:120]))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "the same command run twice is two observations and two checks; acr_check takes the newest unrecorded")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); ran(s, CMD, "5050", tid="first"); ran(s, CMD, "5051", tid="second")
    r = s.mcp([("acr_check", {"command": CMD}), ("acr_check", {"command": CMD}), ("acr_check", {"command": CMD})])
    ks = [e["body"] for e in s.events() if e["kind"] == "CHECK_RAN"]
    ck("two checks, newest first, each with its own digest", [(k["tool_use_id"], k["stdout"]) for k in ks] == [("second", H("5051")), ("first", H("5050"))], ks)
    ck("a third is refused", r[2].startswith("REFUSE"), r[2][:120])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "a command passed on with something added is refused, and the answer shows what the host did see")
def _(ck):
    # Found live: a reviewer gave acr_check `cd <dir> && <command>` where the host had seen `<command>`, was refused twice and recorded UNRESOLVED for a check that ran.
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); ran(s, CMD, "5050", tid="t1"); ran(s, "ls", "a\nb", tid="t2")
    r = s.mcp([("acr_check", {"command": "cd /somewhere && " + CMD})]); b0 = s.bytes()
    ck("refused, and both unrecorded commands are shown exactly, newest first", r[0].startswith("REFUSE") and r[0].index("  ls\n") < r[0].index("  " + CMD + "\n") and "Nothing was written" in r[0], r[0])
    r = s.mcp([("acr_check", {"command": CMD}), ("acr_check", {"command": "cd /somewhere && " + CMD})])
    ck("with the exact text it is recorded", r[0].startswith("WRITTEN"), r[0][:200])
    ck("and it is then no longer offered", r[1].startswith("REFUSE") and ("  " + CMD + "\n") not in r[1] and "  ls\n" in r[1], r[1])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario("F30 a check is what the host saw run, or it says it is a report", "acr_check is refused after a halt, on a register with no session, and with bad arguments")
def _(ck):
    s = S(); r = s.mcp([("acr_check", {"command": CMD})]); ck("no register: says so", "no register" in r[0], r[0][:200])
    s.begin_adopted(); ran(s, tid="early"); r = s.mcp([("acr_check", {"command": CMD})])
    ck("in use with no review open: refused, since a check is run for a review", r[0].startswith("REFUSE") and "No review is open" in r[0] and "CHECK_RAN" not in s.kinds(), r[0][:200])
    s2 = S(); review_to_g1(s2, "MECHANICAL_CHECK"); s, s_old = s2, s; s_old.close(); ran(s)
    r = s.mcp([("acr_check", {}), ("acr_check", {"command": 5}), ("acr_check", {"command": CMD, "expect_exit": "0"}), ("acr_check", {"command": CMD, "actor": "principal"})])
    ck("bad arguments are answered as errors and write no check", all(not x.startswith("WRITTEN") and ("argument" in x) for x in r) and "CHECK_RAN" not in s.kinds(), [x[:100] for x in r])
    r = s.mcp([("acr_check", {"command": CMD, "expect_exit": 0})]); ck("recorded while a review is open", r[0].startswith("WRITTEN"), r[0][:200])
    s.prompt("stop"); ran(s, "echo after", "after", tid="late"); r = s.mcp([("acr_check", {"command": "echo after"})])
    ck("after a halt nothing is observed, so there is nothing to record", r[0].startswith("REFUSE"), r[0][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F31: what was gathered for this review, and what only says so (build 19) ------------------------------------------
# From a second AI system's audit of build 18: an observation made before a review was reused in it; the review's own words
# passed as SUPPLIED; a reported check marked EXECUTED; malformed hook input read as a successful command.
F31 = "F31 evidence is what this review gathered, recorded as it came"
@scenario(F31, "a command the host saw before the review was commissioned is not a check for it; run again, it is")
def _(ck):
    s = S(); s.begin_adopted(); ran(s, tid="before"); review_to_g1(s, "MECHANICAL_CHECK")
    r = s.mcp([("acr_check", {"command": CMD})])
    ck("refused, and the answer says the run came before this review", r[0].startswith("REFUSE") and "before the review now open was commissioned" in r[0] and "CHECK_RAN" not in s.kinds(), r[0][:400])
    ck("the stale command is not offered as one to record", ("  " + CMD + "\n") not in r[0], r[0])
    ran(s, tid="after"); r = s.mcp([("acr_check", {"command": CMD})]); k = [e["body"] for e in s.events() if e["kind"] == "CHECK_RAN"]
    ck("the run made after commissioning is recorded, and it is that run", r[0].startswith("WRITTEN") and len(k) == 1 and k[0]["tool_use_id"] == "after", (r[0][:200], k))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F31, "an observation from one review is not carried through a halt into the next")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); ran(s, tid="r1"); s.prompt("stop"); s.typed("review", '"again"')
    body = {"label": "R2", "title": "Again", "target": {"t": "artifact", "sha256": H(CLAIM)}, "extent": "TOTAL", "procedure": "FOCUSED", "director": "principal",
            "claims": [{"label": "A", "text": CLAIM, "origin": "COMMISSIONER", "kind": "MECHANICAL_CHECK", "criterion": "Refuted if a calculation gives another sum."}]}
    r = s.mcp([("acr_submit", {"kind": "DELIVERY_STATE", "body": COOK[0][1]}), ("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": body}), ("acr_submit", {"kind": "GATE_COMMITTED", "body": {"review": "R2", "gate": "G1"}}), ("acr_check", {"command": CMD})])
    ck("the second review is open", all(x.startswith("WRITTEN") for x in r[:3]), [x[:160] for x in r[:3]])
    ck("the first review's observation is refused in it", r[3].startswith("REFUSE") and "CHECK_RAN" not in s.kinds(), r[3][:300])
    f = dict(FIND("REFUTED_IN_SCOPE", [], [{"ref": {"t": "check", "id": "K1"}}])[1]); f["body"] = dict(f["body"], review="R2")
    r = s.mcp([("acr_submit", f)]); ck("so no refutation can cite it", r[0].startswith("REFUSE"), r[0][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
MALFORMED = [("no result at all", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m1"}),
             ("a result that is text", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m2", "tool_response": "5050"}),
             ("a result that is a list", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m3", "tool_response": ["5050"]}),
             ("a result with no stdout", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m4", "tool_response": {"stderr": "", "interrupted": False}}),
             ("stdout that is a number", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m5", "tool_response": {"stdout": 5050, "stderr": "", "interrupted": False}}),
             ("no stderr", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m6", "tool_response": {"stdout": "5050", "interrupted": False}}),
             ("no tool call named", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_response": {"stdout": "5050", "stderr": "", "interrupted": False}}),
             ("an empty tool call id", {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "", "tool_response": {"stdout": "5050", "stderr": "", "interrupted": False}}),
             ("no event name", {"tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m9", "tool_response": {"stdout": "5050", "stderr": "", "interrupted": False}}),
             ("another event's name", {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m10", "tool_response": {"stdout": "5050", "stderr": "", "interrupted": False}}),
             ("a failure with no error text", {"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m11", "is_interrupt": False}),
             ("a failure whose error is a number", {"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m12", "error": 0, "is_interrupt": False}),
             ("a failure dressed with a result", {"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "m13", "error": "", "is_interrupt": False, "tool_response": {"stdout": "5050", "stderr": "", "interrupted": False}})]
for label, obj in MALFORMED:
    @scenario(F31, "hook input that is not the host's report of a command is never a check: " + label)
    def _(ck, obj=obj):
        s = S(); review_to_g1(s, "MECHANICAL_CHECK"); rc, out, err = s.hook("post-tool", obj)
        ck("silent, no failure", rc == 0 and out == "" and "panicked" not in err, (rc, out, err[:200]))
        ck("nothing kept", not SEEN(s).exists(), SEEN(s).read_text() if SEEN(s).exists() else "")
        r = s.mcp([("acr_check", {"command": CMD})]); ck("and no check can be recorded from it", r[0].startswith("REFUSE") and "CHECK_RAN" not in s.kinds(), r[0][:200])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario(F31, "words handed over during a review are the review's own unless the person typed them")
def _(ck):
    s = S(); review_to_g1(s, "DERIVATION")
    r = s.mcp([("acr_artifact", {"text": OWN, "origin": "SUPPLIED", "name": "working"}), ("acr_artifact", {"text": OWN}), ("acr_artifact", {"text": CLAIM, "name": "claim-again"})])
    a = [e["body"] for e in s.events() if e["kind"] == "ARTIFACT_ADDED"]
    ck("asked for as SUPPLIED, recorded as PRODUCED, and the answer says why", a[-3]["origin"] == "PRODUCED" and "Recorded as PRODUCED, not SUPPLIED" in r[0], (r[0][-300:], a[-3]))
    ck("with no origin given, the same", a[-2]["origin"] == "PRODUCED", a[-2])
    ck("words the host saw the person type stay SUPPLIED", a[-1]["origin"] == "SUPPLIED" and "Recorded as PRODUCED" not in r[2], (r[2][-200:], a[-1]))
    r = s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": H(OWN)}, "extent": "all of it"}}), FIND("REFUTED_IN_SCOPE", [], [ART(OWN)]), FIND("SUPPORTED_IN_SCOPE", [ART(OWN)])])
    ck("a refutation resting on them alone is refused under Core 4.3, 5.2", r[1].startswith("REFUSE") and "Core 4.3, 5.2" in r[1], r[1][:300])
    ck("and so is support", r[2].startswith("REFUSE") and "Core 4.3, 5.2" in r[2], r[2][:300])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F31, "a file written since the review was commissioned is not material it was given; an older file is")
def _(ck):
    s = S(); (s.root / "given.md").write_text("given before\n"); os.utime(s.root / "given.md", (1700000000, 1700000000)); review_to_g1(s, "DERIVATION")
    (s.root / "written.md").write_text(OWN); future = __import__("time").time() + 5; os.utime(s.root / "written.md", (future, future))
    r = s.mcp([("acr_artifact", {"path": "written.md", "origin": "SUPPLIED"}), ("acr_artifact", {"path": "given.md", "origin": "SUPPLIED"})])
    a = [e["body"] for e in s.events() if e["kind"] == "ARTIFACT_ADDED"]
    ck("the new file is recorded as PRODUCED, and the answer says why", a[-2]["origin"] == "PRODUCED" and a[-2]["read_by"] == "HOST" and "last written after the open review was commissioned" in r[0], (r[0][-300:], a[-2]))
    ck("the older file stays SUPPLIED", a[-1]["origin"] == "SUPPLIED", a[-1])
    r = s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": H(OWN)}, "extent": "all of it"}}), FIND("REFUTED_IN_SCOPE", [], [ART(OWN)])])
    ck("a refutation resting on the new file alone is refused", r[1].startswith("REFUSE") and "Core 4.3, 5.2" in r[1], r[1][:300])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F31, "material the server cannot read is not taken as SUPPLIED on the session's word while a review is open")
def _(ck):
    s = S(); s.begin_adopted(); art = lambda d, origin, **kw: ("acr_submit", {"kind": "ARTIFACT_ADDED", "body": dict({"sha256": H(d), "size": len(d), "origin": origin, "stored": False}, **kw)})
    r = s.mcp([art("before", "SUPPLIED")]); ck("before any review it is accepted, as the documented first step is", r[0].startswith("WRITTEN"), r[0][:200])
    s.close(); s = S(); review_to_g1(s, "DERIVATION"); b0 = s.bytes()
    r = s.mcp([art("during", "SUPPLIED"), ("acr_offer", art("during", "SUPPLIED")[1])])
    ck("during a review it is refused, with what to do instead, and nothing is written", r[0].startswith("REFUSE") and "acr_artifact" in r[0] and "Core 4.3" in r[0] and s.bytes() == b0, r[0][:300])
    r = s.mcp([art("mine", "PRODUCED"), art("fetched", "RETRIEVED", source={"where": "https://example.org/x", "when": "2026-11-12T00:00:00Z"})])
    ck("its own working as PRODUCED, and a retrieved source with where and when, are accepted", all(x.startswith("WRITTEN") for x in r), [x[:200] for x in r])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F31, "EXECUTED says the record holds that a check ran: a report alone is UNKNOWN, a report beside an observed check is EXECUTED")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    r = s.mcp([("acr_submit", {"kind": "CHECK_RAN", "body": dict(COOK[5][1])}), FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [{"ref": {"t": "check", "id": "K1"}}])])
    ck("a finding on a reported check alone is accepted and marked UNKNOWN", r[1].startswith("WRITTEN") and s.events()[-1]["body"]["execution"] == "UNKNOWN", (r[1][:200], s.events()[-1]["body"].get("execution")))
    ran(s); r = s.mcp([("acr_check", {"command": CMD}), FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [{"ref": {"t": "check", "id": "K1"}}, {"ref": {"t": "check", "id": "K2"}}]), FIND("UNRESOLVED", [ART(CLAIM)]), ("acr_status", {})])
    ck("with a check the host saw beside it, EXECUTED", r[1].startswith("WRITTEN") and s.events()[-2]["body"]["execution"] == "EXECUTED", r[1][:200])
    ck("with no check cited, REASONED", s.events()[-1]["body"]["execution"] == "REASONED", s.events()[-1]["body"])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F31, "the limit, fixed so that nobody reads more into it: a line written straight into the observations file is taken for the host's")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); n = len(s.events())
    SEEN(s).write_text(json.dumps({"ts": "2026-11-12T00:00:00Z", "tool_use_id": "forged", "command": CMD, "exit": 0, "timeout": False, "stdout": H("5050"), "stderr": H(""), "output": "STDOUT_AND_STDERR", "output_bytes": 4, "at_event": n, "duration_ms": 1}) + "\n")
    r = s.mcp([("acr_check", {"command": CMD})]); k = [e["body"] for e in s.events() if e["kind"] == "CHECK_RAN"]
    ck("recorded as HOST_OBSERVED_TOOL: any program running as the person can write that file, and nothing is signed", r[0].startswith("WRITTEN") and k and k[0]["observed"] == "HOST_OBSERVED_TOOL", r[0][:200])
    rc, out, err = s.hook("pre-tool", {"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"file_path": ".pcp/observed_tools.jsonl"}})
    ck("what is closed is the assistant's file tools, not a shell", '"permissionDecision":"deny"' in out, out or err)
    for x in oracles(s): ck("oracles", False, x)
    s.close()
for ed in EDITIONS:
    @scenario(F31, "a plugin carries the method's text and not the packet's programs or probes", ed)
    def _(ck, ed=ed):
        s = S(ed); m = s.plugin / "method"; t = PLUG / ed / "pcp" / "method"
        man = {f["path"]: f["sha256"] for f in json.load(open(m / "MANIFEST.json"))["files"]}
        for name, root, tools in (("pcp-os", m, set()), ("pcp", t, {"TOOLS/acr.py"})):
            have = {x.relative_to(root).as_posix() for x in root.rglob("*") if x.is_file()} - {"MANIFEST.json"}
            ck(f"{name}: no probe, no model answer, and no program but the one it uses", not any(x.startswith("INTEGRATION/") for x in have) and {x for x in have if x.startswith("TOOLS/")} == tools, sorted(x for x in have if x.startswith(("INTEGRATION/", "TOOLS/"))))
            ck(f"{name}: every other file of the packet is there, each the packet's own bytes", {x for x in man if not x.startswith(("INTEGRATION/", "TOOLS/"))} <= have and all(hashlib.sha256((root / x).read_bytes()).hexdigest() == man[x] for x in have), len(have))
        r = subprocess.run([s.exe, "method", str(m)], capture_output=True, text=True, env=s.env)
        ck("the program finds the text whole and says what is absent and why", r.returncode == 0 and "0 differing" in r.stdout and "which a plugin does not carry" in r.stdout, (r.returncode, r.stdout))
        s.mcp([("acr_begin", {})]); pin = [e["body"] for e in s.events() if e["kind"] == "METHOD_PINNED"]
        ck("and a register begun here records the text as verified", pin and pin[0]["verified"] == "FILES_VERIFIED_AGAINST_OWN_MANIFEST", pin)
        c = pathlib.Path(tempfile.mkdtemp(prefix="pcpsim-m-")) / "method"; shutil.copytree(m, c); (c / "KERNEL_CARD.md").unlink()
        r = subprocess.run([s.exe, "method", str(c)], capture_output=True, text=True, env=s.env); ck("with a file of the text missing it does not", r.returncode == 1, (r.returncode, r.stdout))
        (c / "KERNEL.md").write_text("changed"); r = subprocess.run([s.exe, "method", str(c)], capture_output=True, text=True, env=s.env); ck("nor with one changed", r.returncode != 0 and "KERNEL.md" in r.stdout, (r.returncode, r.stdout))
        shutil.rmtree(c.parent); s.close()
@scenario(F31, "the packet's lookup tool and the program agree on an altered packet: one identifier in two folders is served from neither, and a version is read whole")
def _(ck):
    s = S(); t = PLUG / "Personal" / "pcp" / "method"; c = pathlib.Path(tempfile.mkdtemp(prefix="pcpsim-a-")) / "method"; shutil.copytree(t, c)
    run = lambda *a: subprocess.run([sys.executable, "-B", str(c / "TOOLS/acr.py"), *a], capture_output=True, text=True)
    r = run("clause", "core", "15.2"); ck("the tool carried by the text-only plugin fetches a clause", r.returncode == 0 and "### 15.2" in r.stdout, r.stderr[:200])
    r = run("parse"); ck("and says plainly what it does not carry", r.returncode == 2 and "command_interface.py is not beside this tool" in r.stderr, (r.returncode, r.stderr[:200]))
    spec = (c / "MODULES/03_Science/SOURCE/Specification.md").read_text()
    d = c / "MODULES/00_Before/SOURCE"; d.mkdir(parents=True); d.joinpath("Specification.md").write_text(spec)
    e = c / "MODULES/08_Sample/SOURCE"; e.mkdir(parents=True); e.joinpath("Specification.md").write_text(spec.replace("`acr.science`, Version 1.0", "`acr.sample`, Version 2.4.1-rc.2+build.4", 1))
    r = run("clause", "science", "1"); ck("the tool serves the doubled identifier from neither folder and names both", r.returncode == 2 and "00_Before" in r.stderr and "03_Science" in r.stderr, (r.returncode, r.stderr[:300]))
    r = run("clause", "sample", "1"); ck("and reads the version whole", r.returncode == 0 and "version 2.4.1-rc.2+build.4" in r.stdout.splitlines()[0], r.stdout[:200] + r.stderr[:200])
    r = subprocess.run([s.exe, "modules", str(c)], capture_output=True, text=True, env=s.env)
    ck("the program does the same with the same folders", "acr.science" not in [l.split()[1] for l in r.stdout.splitlines() if len(l.split()) > 1 and l.split()[1].startswith("acr.")] and "2.4.1-rc.2+build.4" in r.stdout and "neither is used" in r.stdout, r.stdout[-700:])
    shutil.rmtree(c.parent); s.close()

# ---------- F32: whether a command finished, and whether a file came first (build 20) --------------------------------------------
# From the second AI system's recheck of build 19: an interruption flag of the wrong type was read as "not interrupted", and a
# file first written in the second a review was commissioned stayed SUPPLIED. Found beside them here: a report of failure that
# names exit status 0 was kept as a command that passed.
F32 = "F32 a report says in the host's own form whether the command finished; a file is given only if it is shown to be earlier"
WRONG = [("absent", None), ("the text true", "true"), ("the text false", "false"), ("the number 1", 1), ("the number 0", 0), ("a list", []), ("a map", {}), ("null", "NULL")]
def with_flag(obj, where, key, v):
    o = copy.deepcopy(obj); d = o[where] if where else o
    if v is None: d.pop(key, None)
    else: d[key] = None if v == "NULL" else v
    return o
GOOD_RAN = {"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "w1", "duration_ms": 97, "tool_response": {"stdout": "5050", "stderr": "", "interrupted": False, "isImage": False}}
GOOD_FAILED = {"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_input": {"command": CMD}, "tool_use_id": "w2", "error": "Exit code 1\nno", "is_interrupt": False}
def nothing_kept(ck, obj):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); b0 = s.bytes(); rc, out, err = s.hook("post-tool", obj)
    ck("silent, no failure", rc == 0 and out == "" and "panicked" not in err, (rc, out, err[:200]))
    ck("nothing kept, and the register untouched", not SEEN(s).exists() and s.bytes() == b0, SEEN(s).read_text() if SEEN(s).exists() else "")
    r = s.mcp([("acr_check", {"command": CMD})]); ck("no check can be recorded from it", r[0].startswith("REFUSE") and "CHECK_RAN" not in s.kinds(), r[0][:200])
    r = s.mcp([FIND("REFUTED_IN_SCOPE", [], [{"ref": {"t": "check", "id": "K1"}}])]); ck("and no finding can cite one", r[0].startswith("REFUSE") and "FINDING_RECORDED" not in s.kinds(), r[0][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
for label, v in WRONG:
    @scenario(F32, "a report of success whose interruption flag is not true or false is not kept: " + label)
    def _(ck, v=v): nothing_kept(ck, with_flag(GOOD_RAN, "tool_response", "interrupted", v))
    @scenario(F32, "a report of failure whose interruption flag is not true or false is not kept: " + label)
    def _(ck, v=v): nothing_kept(ck, with_flag(GOOD_FAILED, "", "is_interrupt", v))
for flag in (False, True):
    @scenario(F32, "a report of failure that names exit status 0 contradicts itself and is not kept: is_interrupt %s" % str(flag).lower())
    def _(ck, flag=flag): nothing_kept(ck, dict(GOOD_FAILED, error="Exit code 0\n5050", is_interrupt=flag))
@scenario(F32, "the host's own forms are kept as before: finished, interrupted, failed, failed and interrupted")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    s.hook("post-tool", GOOD_RAN); s.hook("post-tool", dict(with_flag(GOOD_RAN, "tool_response", "interrupted", True), tool_use_id="w3", tool_input={"command": "sleep 100"}))
    s.hook("post-tool", dict(GOOD_FAILED, tool_input={"command": "false"})); s.hook("post-tool", dict(GOOD_FAILED, tool_input={"command": "sleep 200"}, tool_use_id="w4", is_interrupt=True))
    seen = [json.loads(l) for l in SEEN(s).read_text().splitlines()]
    ck("four observations: exit 0; no exit and interrupted; exit 1; exit 1 and interrupted", [(o["exit"], o["timeout"]) for o in seen] == [(0, False), (None, True), (1, False), (1, True)], seen)
    r = s.mcp([("acr_check", {"command": CMD}), ("acr_check", {"command": "sleep 100"}), ("acr_check", {"command": "sleep 200"})])
    ck("the finished command met its expectation; the interrupted ones did not", "the expectation was met" in r[0] and "NOT met" in r[1] and "NOT met" in r[2], [x[:200] for x in r])
    r = s.mcp([FIND("REFUTED_IN_SCOPE", [], [{"ref": {"t": "check", "id": "K2"}}]), FIND("REFUTED_IN_SCOPE", [], [{"ref": {"t": "check", "id": "K3"}}]), FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [{"ref": {"t": "check", "id": "K1"}}])])
    ck("a finding on an interrupted command alone is refused, in either form", r[0].startswith("REFUSE") and r[1].startswith("REFUSE"), [x[:160] for x in r[:2]])
    ck("a finding on the finished one is accepted and EXECUTED", r[2].startswith("WRITTEN") and s.events()[-1]["body"]["execution"] == "EXECUTED", r[2][:200])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
def utc_second(t): return __import__("time").strftime("%Y-%m-%dT%H:%M:%SZ", __import__("time").gmtime(t))
def new_file_case(later):
    """A file that does not exist when the review is commissioned and is then written normally: no time is set by the test."""
    time = __import__("time")
    for attempt in range(60):
        while time.time() % 1 > 0.2: time.sleep(0.01)          # begin early in a second, so that commissioning and the write can share it
        s = S(); review_to_g1(s, "DERIVATION"); since = [e["ts"] for e in s.events() if e["kind"] == "REVIEW_COMMISSIONED"][-1]
        f = s.root / "new-working.txt"; absent = not f.exists()
        if later:
            while utc_second(time.time()) <= since: time.sleep(0.02)
        f.write_text(OWN); wrote = utc_second(f.stat().st_mtime)
        if absent and (wrote > since if later else wrote == since): return s, since, wrote, attempt + 1
        s.close()
    return None, None, None, 60
for later in (False, True):
    @scenario(F32, "a file first written after commissioning, with no time set by the test, is not SUPPLIED: " + ("after the next second" if later else "in the same second"))
    def _(ck, later=later):
        s, since, wrote, attempts = new_file_case(later)
        ck("the case was set up: the file was absent at commissioning and written %s" % ("in a later second" if later else "in the same second"), s is not None, attempts)
        if s is None: return
        r = s.mcp([("acr_artifact", {"path": "new-working.txt", "origin": "SUPPLIED"})]); a = [e["body"] for e in s.events() if e["kind"] == "ARTIFACT_ADDED"][-1]
        ck("recorded as PRODUCED, and the answer says why", a["origin"] == "PRODUCED" and a["read_by"] == "HOST" and ("last written after the open review was commissioned" if later else "in the same second as the open review was commissioned") in r[0], (since, wrote, r[0][-300:], a))
        r = s.mcp([("acr_submit", {"kind": "INSPECTION_DECLARED", "body": {"ref": {"t": "artifact", "sha256": H(OWN)}, "extent": "all of it"}}), FIND("REFUTED_IN_SCOPE", [], [ART(OWN)])])
        ck("a refutation resting on it alone is refused", r[1].startswith("REFUSE") and "Core 4.3, 5.2" in r[1], r[1][:300])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario(F32, "the second of commissioning is the edge: a file written in the second before is given, in that second is not")
def _(ck):
    cal = __import__("calendar"); time = __import__("time")
    s = S(); review_to_g1(s, "DERIVATION"); since = [e["ts"] for e in s.events() if e["kind"] == "REVIEW_COMMISSIONED"][-1]
    t0 = cal.timegm(time.strptime(since, "%Y-%m-%dT%H:%M:%SZ"))
    for name, at in (("before.txt", t0 * 10**9 - 1000), ("start.txt", t0 * 10**9), ("end.txt", t0 * 10**9 + 999999000)):
        (s.root / name).write_text(name); os.utime(s.root / name, ns=(at, at))
    r = s.mcp([("acr_artifact", {"path": n, "origin": "SUPPLIED"}) for n in ("before.txt", "start.txt", "end.txt")]); a = [e["body"]["origin"] for e in s.events() if e["kind"] == "ARTIFACT_ADDED"][-3:]
    ck("before: SUPPLIED; at the first and the last instant of the second: PRODUCED", a == ["SUPPLIED", "PRODUCED", "PRODUCED"] and "Recorded as PRODUCED" not in r[0], (since, a, [x[-200:] for x in r]))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F32, "acr_artifact's record argument is true or false: another type is refused and writes nothing")
def _(ck):
    s = S(); s.begin_adopted(); (s.root / "t.txt").write_text("t\n"); b0 = s.bytes()
    r = s.mcp([("acr_artifact", {"path": "t.txt", "record": "false"}), ("acr_artifact", {"text": "words", "record": 0}), ("acr_artifact", {"path": "t.txt", "record": False})])
    ck("refused twice, nothing written", all("argument record is true or false" in x for x in r[:2]) and "Nothing was written" in r[2] and s.bytes() == b0, [x[:160] for x in r])
    for x in oracles(s): ck("oracles", False, x)
    s.close()

# ---------- F33: instruments, and the calibrate skill (build 21) -----------------------------------------------------------------
# The method's modules name instruments a host can run itself (Core 14.1). The program runs nine and records each as a
# check it ran. An instrument computes one property of its input and nothing else.
F33 = "F33 an instrument is computed by the program itself and recorded as a check it ran"
INSTR = lambda name, inp, **kw: ("acr_instrument", dict({"instrument": name, "input": inp}, **kw))
@scenario(F33, "with no shell at all, the sum claim is refuted on an exact calculation the program made")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    r = s.mcp([INSTR("mathematics.exact", {"expression": "sum(k, 1, 100, k) = 5500"}), FIND("REFUTED_IN_SCOPE", [ART(CLAIM)], [{"ref": {"t": "check", "id": "K1"}}])])
    k = [e["body"] for e in s.events() if e["kind"] == "CHECK_RAN"]
    ck("one check, run by the program, whose property does not hold", r[0].startswith("WRITTEN") and "the left side is 5050" in r[0] and len(k) == 1 and k[0]["observed"] == "RAN_BY_THIS_PROGRAM" and k[0]["exit"] == 1 and k[0]["instrument"] == "mathematics.exact", (r[0][:300], k))
    ck("a refutation that cites it is accepted and EXECUTED", r[1].startswith("WRITTEN") and s.events()[-1]["body"]["execution"] == "EXECUTED", r[1][:300])
    ck("nothing was observed of any shell", not SEEN(s).exists())
    for x in oracles(s): ck("oracles", False, x)
    s.close()
PAIRS = [("mathematics.exact", {"expression": "1/3 + 1/6 = 0.5"}, {"expression": "0.1 + 0.2 = 0.31"}), ("statistics.grim", {"mean": "3.48", "n": 25}, {"mean": "5.19", "n": 28}),
         ("statistics.percent", {"percent": "37.5", "total": 48}, {"percent": "37.4", "total": 48}), ("engineering.units", {"from": "1 in", "to": "25.4 mm"}, {"from": "1 lbf", "to": "1 N"}),
         ("cryptography.digest", {"text": "abc", "claimed": H("abc")}, {"text": "abd", "claimed": H("abc")})]
for name, yes, no in PAIRS:
    @scenario(F33, "%s: a known true input holds and a known false one does not" % name)
    def _(ck, name=name, yes=yes, no=no):
        s = S(); review_to_g1(s, "MECHANICAL_CHECK"); r = s.mcp([INSTR(name, yes), INSTR(name, no)])
        k = [e["body"] for e in s.events() if e["kind"] == "CHECK_RAN"]
        ck("two checks: exit 0 for the true input, exit 1 for the false", [x["exit"] for x in k] == [0, 1] and "HOLDS" in r[0] and "DOES NOT HOLD" in r[1], [x[:200] for x in r])
        for x in oracles(s): ck("oracles", False, x)
        s.close()
@scenario(F33, "the register's own instruments read what it holds: which artifact entered first, and what each claim has")
def _(ck):
    s = S(); review_to_g1(s, "DERIVATION")
    r = s.mcp([("acr_artifact", {"text": OWN}), INSTR("science.order", {"earlier": H(CLAIM), "later": H(OWN)}), INSTR("science.order", {"earlier": H(OWN), "later": H(CLAIM)}), INSTR("quality.coverage", {"review": "R1"})])
    ck("the claim entered the record before the working, and not the other way round", "IN THAT ORDER" in r[1] and "NOT IN THAT ORDER" in r[2], [x[:200] for x in r[1:3]])
    ck("coverage says the one claim has no finding", "claim A: NO FINDING" in r[3] and "DOES NOT HOLD" in r[3], r[3][:400])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F33, "what an instrument cannot compute, and an instrument that does not exist, write nothing")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); b0 = s.bytes()
    r = s.mcp([INSTR("mathematics.exact", {"expression": "1/0"}), INSTR("mathematics.exact", {"expression": "2^0.5"}), INSTR("mathematics.exact", {"expression": "170! * 170!"}), INSTR("mathematics.prove", {}),
               INSTR("statistics.grim", {"mean": "about 3", "n": 25}), INSTR("engineering.units", {"from": "3 furlongs", "to": "3 m"}), INSTR("science.order", {"earlier": H("x"), "later": H("y")}),
               INSTR("mathematics.exact", {"expression": "2 + 2 = 4"}, record=False), INSTR("list", {})])
    ck("seven refusals, each saying why, and the register untouched", s.bytes() == b0 and all(not x.startswith("WRITTEN") for x in r) and "division by zero" in r[0] and "no instrument named" in r[3], [x[:120] for x in r[:7]])
    ck("record false computes and writes nothing; list names eight", "TRUE" in r[7] and "Nothing was written" in r[7] and r[8].count("\n  ") == 8, (r[7][:200], r[8][:200]))
    s.close(); s = S(); s.begin_adopted(); b0 = s.bytes()
    r = s.mcp([INSTR("mathematics.exact", {"expression": "2 + 2 = 4"})]); ck("with no review open it is computed and not recorded", "TRUE" in r[0] and "No review is open" in r[0] and s.bytes() == b0, r[0][:300])
    s.close()
@scenario(F33, "a Calibration Record is checked by the program: its counts must agree and it cannot claim more than they show")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    rec = {"subject": {}, "configuration": {"delivery": "TOOL_ASSISTED"}, "assessor": {"relation": "HOST_OR_TOOL"}, "instrument": {}, "acquisition": {}, "retrieval": {"items": 22, "runs": 2, "for": 40, "against": 4, "unscored": 0, "invented": 1},
           "application": "NOT_MEASURED", "thresholds": [{"measure": "retrieval.for", "at_least": "0.9", "result": "MET"}]}
    over = dict(rec, claim={"disposition": "SUPPORTED_IN_SCOPE"}); own = dict(rec, assessor={"relation": "SELF"}, application={"runs": 2, "passed": 2, "failed": 0, "unscored": 0, "negative_controls": {"runs": 1, "passed": 1}, "positive_controls": {"runs": 1, "passed": 1}})
    r = s.mcp([INSTR("core.calibration", {"record": rec}), INSTR("core.calibration", {"record": over}), INSTR("core.calibration", {"record": own})])
    ck("a record whose numbers agree is consistent, and the answer says that is not evidence of onboarding", "CONSISTENT AS DECLARED" in r[0] and "retrieval.for: 40 of 44" in r[0] and "not evidence of onboarding" in r[0], r[0][:400])
    ck("a claim of onboarding without its controls is inconsistent", "INCONSISTENT" in r[1] and "controls to have run and passed" in r[1], r[1][:400])
    ck("application scored by the participant itself is a self-check", "INCONSISTENT" in r[2] and "self-check" in r[2], r[2][:400])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
for ed in EDITIONS:
    @scenario(F33, "both plugins carry the calibrate skill, and it tells the assistant it does not score itself", ed)
    def _(ck, ed=ed):
        for plug in ("pcp", "pcp-os"):
            f = PLUG / ed / plug / "skills/calibrate/SKILL.md"
            ck("%s: the skill is there and says so" % plug, f.is_file() and "you do not score yourself" in f.read_text() and "NONE" in f.read_text() and "Do not say that you are calibrated" in f.read_text(), f)
        spec = (PLUG / ed / "pcp-os" / "method/MODULES/01_Core/SOURCE/Specification.md").read_text()
        ck("the method text in the plugin defines Calibration and instruments", "### 14.4 Calibration" in spec and "A module MAY name **instruments**" in spec)

# ---------- F34: what the first Calibration taught (build 22) ---------------------------------------------------------------------
# In the Calibration of build 21 the measuring arrangement kept an assistant's file tools out of the plugin's folder, and
# most reviews with pcp-os could not be commissioned: the skill told the assistant to read two files there. The program now
# serves both itself. The method's fifteenth round also asks a supported Claim of onboarding for its confinement, counts a
# second reading, and asks nothing after a halt where the review was the whole of the task.
F34 = "F34 the program serves the kernel text and the event bodies itself, and a Calibration Record says what was measured"
@scenario(F34, "the kernel text and the event bodies come from the program, byte for byte, and reading them begins nothing")
def _(ck):
    s = S(); r = s.mcp([("acr_kernel", {}), ("acr_events", {})])
    kernel = (s.plugin / "method/KERNEL.md").read_bytes(); bodies = (s.plugin / "skills/review/events.md").read_bytes()
    ck("the kernel as served ends with the file, whole, and carries its digest", r[0].endswith(kernel.decode()) and hashlib.sha256(kernel).hexdigest() in r[0].split("\n")[0], r[0][:200])
    ck("the event bodies as served end with the file, whole, and carry its digest", r[1].endswith(bodies.decode()) and hashlib.sha256(bodies).hexdigest() in r[1].split("\n")[0], r[1][:200])
    ck("the digest served with the kernel is the one the review skill records as delivered", hashlib.sha256(kernel).hexdigest() in (s.plugin / "skills/review/SKILL.md").read_text())
    ck("no register was begun and ACR is not in use", not s.reg.exists())
    s.close()
@scenario(F34, "started from no packaged folder the program has no list of event bodies, says so, and names what answers instead")
def _(ck):
    s = S(); method = str(s.plugin / "method")
    s.env.pop("CLAUDE_PLUGIN_ROOT"); s.env["PCPOS_METHOD"] = method
    r = s.mcp([("acr_events", {}), ("acr_kernel", {})])
    ck("no list, and acr_offer is named", "acr_offer" in r[0] and "REVIEW_COMMISSIONED" not in r[0], r[0][:300])
    ck("the kernel is still served from the method text named", r[1].endswith((pathlib.Path(method) / "KERNEL.md").read_text()), r[1][:120])
    s.close()
@scenario(F34, "a review is commissioned and its first gate committed by an assistant that has no file tool at all, from what the two tools returned")
def _(ck):
    s = S()
    r = s.mcp([("acr_kernel", {}), ("acr_events", {})])
    digest = r[0].split("SHA-256 ")[1].split(" ")[0]
    ck("the list of bodies names the delivery event with that digest", digest in r[1], r[1][:200])
    review_to_g1(s, "MECHANICAL_CHECK")
    kinds = [e["kind"] for e in s.events()]
    ck("delivery, commission and the first gate are in the record", "DELIVERY_STATE" in kinds and "REVIEW_COMMISSIONED" in kinds and "GATE_COMMITTED" in kinds, kinds)
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F34, "a bare stop where a review is the whole of the work: the host records the halt and tells the assistant there is nothing to ask")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK"); out = s.prompt("stop")
    ck("the halt is recorded from what the person typed", "The host recorded the halt" in out and s.events()[-1]["kind"] == "HALTED" or any(e["kind"] == "HALTED" for e in s.events()), out[:300])
    ck("the note says that nothing need be asked where the review was the whole task, and when one question is in order", "there is nothing to ask" in out and "ask one short question" in out, out[:500])
    for x in oracles(s): ck("oracles", False, x)
    s.close()
@scenario(F34, "a Calibration Record: a supported Claim of onboarding states its confinement, and a second reading cannot cover more than was read")
def _(ck):
    s = S(); review_to_g1(s, "MECHANICAL_CHECK")
    rec = {"subject": {}, "configuration": {"delivery": "RETRIEVAL"}, "assessor": {"relation": "SEPARATE_PARTY"}, "instrument": {}, "acquisition": {}, "retrieval": {"items": 22, "runs": 1, "for": 22, "against": 0, "unscored": 0, "invented": 0},
           "application": {"runs": 52, "passed": 50, "failed": 2, "unscored": 0, "negative_controls": {"runs": 4, "passed": 4}, "positive_controls": {"runs": 2, "passed": 2}, "second_reading": {"runs": 52, "agreed": 49}},
           "thresholds": [{"measure": "application.passed", "at_least": "0.8", "result": "MET"}], "claim": {"disposition": "SUPPORTED_IN_SCOPE"}}
    stated = copy.deepcopy(rec); stated["configuration"]["confinement"] = "NONE"
    more = copy.deepcopy(stated); more["application"]["second_reading"] = {"runs": 60, "agreed": 49}
    over = copy.deepcopy(stated); over["application"]["second_reading"] = {"runs": 10, "agreed": 11}
    none = copy.deepcopy(stated); none["application"]["second_reading"] = "NONE"
    r = s.mcp([INSTR("core.calibration", {"record": x}) for x in (rec, stated, more, over, none)])
    ck("without its confinement the supported Claim is inconsistent", "INCONSISTENT" in r[0] and "needs configuration.confinement stated" in r[0], r[0][:400])
    ck("with it, and with a second reading or NONE, the record is consistent", "CONSISTENT AS DECLARED" in r[1] and "CONSISTENT AS DECLARED" in r[4], (r[1][:200], r[4][:200]))
    ck("a second reading of more runs than were read, or agreeing on more than it covers, is inconsistent", "only 52 were read" in r[2] and "agreed no more than runs" in r[3], (r[2][:300], r[3][:300]))
    for x in oracles(s): ck("oracles", False, x)
    s.close()
for ed in EDITIONS:
    @scenario(F34, "the plugin's skills send the assistant to the program for the kernel and the event bodies, and its method text is the fifteenth round's", ed)
    def _(ck, ed=ed):
        system = (PLUG / ed / "pcp-os" / "skills/system/SKILL.md").read_text()
        ck("the system skill says what the kernel does since build 21 and no longer says it cannot load a program (it said so in build 21, wrongly)",
           "loads ordinary programs from files" in system and "a way to load a program from outside the kernel image" not in system and "cannot yet run a program" not in system)
        review = (PLUG / ed / "pcp-os" / "skills/review/SKILL.md").read_text()
        ck("the review skill names acr_kernel and acr_events before any file", "Call `acr_kernel`" in review and "`acr_events` gives the exact body" in review and "Read `${CLAUDE_PLUGIN_ROOT}/method/KERNEL.md` in full" not in review)
        for plug in ("pcp", "pcp-os"):
            halt = (PLUG / ed / plug / "skills/halt/SKILL.md").read_text()
            ck("%s: the halt skill asks nothing where the review was the whole of the task" % plug, "there is nothing to ask" in halt)
            spec = (PLUG / ed / plug / "method/MODULES/01_Core/SOURCE/Specification.md").read_text()
            ck("%s: the Core says the configuration measured is the configuration as used, and that a reading can be wrong" % plug,
               "The configuration measured is the configuration as it is used." in spec and "An assessor's reading of a reply is itself an observation and can be wrong." in spec and "there is nothing to ask" in spec)

def walk(seed):
    rng = random.Random(seed); ed = EDITIONS[seed % 3]; s = S(ed); log = []; checks = []; step_i = 0; next_review = 1; agents = []; prefix = b""
    def ck(label, ok, detail=""):
        if not ok: checks.append({"check": label, "ok": False, "detail": (str(detail) + " | walk: " + " > ".join(log[-12:]))[:900]})
    for n in range(rng.randrange(8, 40)):
        act = rng.choice(["typed-review", "ordinary", "about", "stop", "cook", "cook", "cook", "cook-random", "mutant", "halt-tool", "adopt-tool", "begin", "sub-start", "sub-stop", "session", "workspace", "fence", "status", "forbidden", "commission"])
        log.append(act); before = s.bytes() or b""
        try:
            if act == "typed-review": s.typed(rng.choice(["review", "adopt"]), "claim %d" % n, rng.random() < .5)
            elif act == "ordinary": out = s.prompt(rng.choice(ORDINARY)); 
            elif act == "about": s.prompt(rng.choice(ABOUT))
            elif act == "stop": s.prompt(rng.choice(VARIANTS(rng.choice(STOP_WORDS))))
            elif act == "cook":
                r = s.mcp([("acr_submit", {"kind": COOK[step_i % len(COOK)][0], "body": COOK[step_i % len(COOK)][1]})])[0]
                if r.startswith("WRITTEN"): step_i += 1
                ck("answer is WRITTEN or REFUSE", r.startswith(("WRITTEN", "REFUSE", "there is no register", "ERROR")), r[:200])
            elif act == "cook-random": k, b = rng.choice(COOK); s.mcp([("acr_submit", {"kind": k, "body": b})])
            elif act == "mutant":
                k, b = rng.choice(COOK); lab, mb = rng.choice(mutations(b, rng)); r = s.mcp([("acr_offer", {"kind": k, "body": mb}), ("acr_submit", {"kind": k, "body": mb})])
                ck("offer and submit agree", r[0].startswith("ACCEPT") == r[1].startswith("WRITTEN") or r[0].startswith(("there is", "ERROR")), (lab, r[0][:100], r[1][:100]))
            elif act == "halt-tool": s.mcp([("acr_halt", {"words": "enough"})])
            elif act == "adopt-tool": s.mcp([("acr_adopt", {"basis": rng.choice(ABOUT)})])
            elif act == "begin": s.mcp([("acr_begin", {})])
            elif act == "sub-start": a = "ag-%d" % n; agents.append(a); s.hook("subagent-start", {"agent_type": rng.choice(["challenger", "checker", "Explore", "x y z!"]), "agent_id": a})
            elif act == "sub-stop" and agents: s.hook("subagent-stop", {"agent_id": rng.choice(agents), "last_assistant_message": rng.choice(["", "result"])})
            elif act == "session": s.hook("session-start", {"source": rng.choice(["startup", "resume", "compact", "clear"])})
            elif act == "workspace": s.typed("workspace", rng.choice(WS)[0])
            elif act == "fence":
                out = s.hook("pre-tool", {"tool_name": "Write", "tool_input": {"file_path": ".pcp/register.jsonl"}})[1]; ck("register fenced", "deny" in out, out)
            elif act == "status":
                r = s.mcp([("acr_status", {}), ("acr_verify", {})]); ck("reading writes nothing", (s.bytes() or b"") == before)
            elif act == "forbidden":
                k, b = rng.choice(FORBIDDEN); r = s.mcp([("acr_submit", {"kind": k, "body": b})])[0]; ck("forbidden kind not written", (s.bytes() or b"") == before, (k, r[:100]))
            elif act == "commission":
                b = copy.deepcopy(COOK[2][1]); next_review += 1; b["label"] = "R%d" % next_review; s.mcp([("acr_submit", {"kind": "REVIEW_COMMISSIONED", "body": b})])
        except subprocess.TimeoutExpired:
            ck("no hang", False, act)
        now = s.bytes() or b""
        ck("the register only grows: what was written stays written", now.startswith(before), act)
        evs = s.events()
        for e in evs[len(before.splitlines()):]:
            if e["actor"] == "principal":
                ck("an event in the person's name came from what the host itself observed", e["body"].get("observed") == "HOST_OBSERVED_PROMPT", e)
            if e["kind"] in ("TASK_STARTED", "TASK_ENDED", "ADOPTED", "HALTED", "WORKSPACE_CREATED", "METHOD_PINNED", "HOST_EVENT") and act in ("cook", "cook-random", "mutant", "forbidden", "commission", "status"):
                ck("a host-only kind was not written through acr_submit", False, e)
    for x in oracles(s): ck("oracles", False, x)
    # after a final halt, the state is quiescent
    s.prompt("stop"); st = s.mcp([("acr_status", {})])[0] if s.reg.exists() else ""
    if s.reg.exists():
        for x in oracles(s): ck("oracles after the final stop", False, x)
        k = s.kinds()
        if "ADOPTED" in k: ck("after the last stop ACR is not in use", "NOT_IN_USE" in st, st[:300])
    NOW.found = []; n_ev = len(s.events()); s.close()
    for x in NOW.found: checks.append({"check": "both implementations agree on every workspace closed", "ok": False, "detail": x[:600]})
    return {"family": "F26 random walks of a host through everything above", "name": f"seed {seed}: {len(log)} acts, {n_ev} events", "edition": ed, "checks": checks or [{"check": "all invariants held", "ok": True, "detail": ""}], "ok": not checks}
for seed in range(SEEDS): RESULTS.append(lambda seed=seed: walk(seed))

if __name__ == "__main__":
    with cf.ThreadPoolExecutor(6) as ex: done = list(ex.map(lambda f: f(), RESULTS))
    only = os.environ.get("PCPSIM_ONLY")          # for a quick look at some families while working; a release run sets nothing
    if only: done = [d for d in done if d["family"].startswith(tuple(only.split(",")))]
    fam = {}
    for d in done: f = fam.setdefault(d["family"], [0, 0]); f[0] += 1; f[1] += d["ok"]
    json.dump({"scenarios": len(done), "passed": sum(d["ok"] for d in done), "checks": sum(1 for d in done for c in d["checks"] if not c.get("summary")), "summary_lines": sum(1 for d in done for c in d["checks"] if c.get("summary")),
               "workspaces_closed": CLOSURES["compared"], "registers_read_by_both_implementations_at_close": CLOSURES["with a register"], "families": fam, "results": done}, open(OUT, "w"), indent=1)
    for k in sorted(fam): print(f"{fam[k][1]:4d}/{fam[k][0]:<4d} {k}")
    print("scenarios", len(done), "passed", sum(d["ok"] for d in done), "| checks", sum(1 for d in done for c in d["checks"] if not c.get("summary")),
          "| workspaces closed", CLOSURES["compared"], "of which held a register, read by both implementations", CLOSURES["with a register"])
    for d in done:
        if not d["ok"]:
            for c in d["checks"]:
                if not c["ok"]: print("FAIL", d["family"][:3], d["name"][:60], "|", c["check"], "|", c["detail"][:300].replace("\n", " "))
