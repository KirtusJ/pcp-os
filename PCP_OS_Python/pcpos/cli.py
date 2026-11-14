"""Command line. One command, one act, one line of result. Exit codes are in REFERENCE/Exit_Codes.csv."""
from __future__ import annotations
import argparse, json, os, sys
from pathlib import Path
from . import VERSION, DEFAULT_LABEL
from .base import PcposError, OK, NEGATIVE, USAGE, INTEGRITY, out_text, strict_loads, sha_bytes
from .store import Workspace, find_workspace
from .ops import Ops
from . import model, view, bag, doctor, method as method_mod


def T(value):
    """A text argument. '@path' reads the text from a file, which avoids shell quoting on every system."""
    if value is None or not value.startswith("@"):
        return value
    raw = Path(value[1:]).read_bytes()
    if raw[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return raw.decode("utf-16").strip()             # a file written by an older PowerShell redirect
    return raw.decode("utf-8-sig").strip()


def label_text(args):
    """The product label for display only. It comes from a label file when one is named; the program itself knows one name."""
    path = getattr(args, "label_file", None) or os.environ.get("PCPOS_LABEL_FILE")
    if path:
        try:
            lab = strict_loads(Path(path).read_bytes()).get("label")
            if isinstance(lab, str) and 0 < len(lab) <= 40:
                return lab
        except (OSError, ValueError):
            pass
    return DEFAULT_LABEL


def _ops(args):
    return Ops(Workspace(find_workspace(args.ws)), args.actor)


def _done(args, ev_head, text):
    ev, head = ev_head
    if args.json:
        out_text(json.dumps({"ok": True, "event": ev["seq"], "kind": ev["kind"], "head": head, "body": ev["body"]}, sort_keys=True))
    else:
        out_text("%s | event %d head %s" % (text, ev["seq"], head[:12]))
    return OK


# ---- commands -------------------------------------------------------------------------
def c_init(a):
    ops, note = Ops.init(a.directory, a.label, a.principal, T(a.principal_name), a.method)
    st = ops.state()
    out_text("workspace created at %s\n%s" % (ops.ws.root, view.method_line(st)))
    if note:
        out_text("note: %s\n  next: %s" % (note.message, note.hint or "pin it later with: pcpos method pin"))
    out_text("ACR is not in use. Nothing was commissioned. Register a participant: pcpos participant add <label> --lineage <model family>")
    return OK


def c_status(a):
    ops = _ops(a); events, head = ops.ws.events(); st = model.fold(events)
    if a.json:
        out_text(json.dumps({"events": len(events), "head": head, "in_use": st.adopted, "method": st.method,
                             "reviews": {l: {"lifecycle": r["lifecycle"], "session": r["session"], "passed_gates": r["passed_gates"],
                                             "required_gates": r["required_gates"]} for l, r in st.reviews.items()}}, sort_keys=True))
    else:
        out_text(view.status(st, len(events), head))
    return OK


def c_brief(a):
    ops = _ops(a); events, head = ops.ws.events(); st = model.fold(events)
    try:
        actor = ops.actor(st)
    except PcposError:
        actor = None
    out_text(view.brief(st, len(events), head, actor, a.review, a.max_bytes))
    return OK


def c_anchor(a):
    out_text(_ops(a).anchor()); return OK


def c_verify(a):
    rep = _ops(a).verify(deep=not a.quick, anchor=a.anchor)
    if a.json:
        out_text(json.dumps(rep, sort_keys=True))
    else:
        out_text("register: %d events, chain intact, head %s" % (rep["events"], rep["head"]))
        out_text("guards replayed: %s" % ("every event passes" if not rep["guard_problems"] else "%d event(s) would not pass" % len(rep["guard_problems"])))
        for seq, kind, msg in rep["guard_problems"][:20]:
            out_text("  event %d %s: %s" % (seq, kind, msg))
        if not a.quick:
            out_text("objects: %s; %d recorded by reference only" % ("all stored objects match their digests" if not rep["altered_objects"] else
                                                                   "%d ALTERED" % len(rep["altered_objects"]), rep["objects_by_reference_only"]))
        if rep["anchor"]:
            out_text("anchor: %s" % {"EQUAL": "this register is exactly the anchored state", "ANCESTOR": "the anchored state is an ancestor: history since then was only added to",
                                     "HISTORY_DIFFERS": "HISTORY DIFFERS from the anchored state: earlier events were rewritten",
                                     "REGISTER_IS_SHORTER": "this register is SHORTER than the anchored state: events were removed"}[rep["anchor"]])
        out_text("result: %s (a local fact about these files; it authenticates nobody and says nothing about whether a finding is right)" % ("PASS" if rep["ok"] else "FAIL"))
    return OK if rep["ok"] else INTEGRITY


def c_repair(a):
    ws = Workspace(find_workspace(a.ws)); n = ws.repair()
    out_text("removed an incomplete final line of %d bytes" % n if n else "the register ends in a complete line; nothing was changed"); return OK


def c_unlock(a):
    ws = Workspace(find_workspace(a.ws))
    out_text("lock removed" if ws.unlock() else "no lock was present"); return OK


def c_config(a):
    ops = _ops(a)
    if a.value is None:
        out_text(json.dumps(ops.config(), sort_keys=True))
    else:
        if a.key not in ("actor",):
            raise PcposError(USAGE, "the only setting is: actor")
        ops.set_config(**{a.key: a.value}); out_text("%s set to %s (local to this copy of the workspace; not part of the record)" % (a.key, a.value))
    return OK


def c_participant(a):
    return _done(a, _ops(a).participant_add(a.label, a.kind, T(a.name), a.model, a.provider, a.lineage, a.principal), "participant %s registered" % a.label)


def c_method(a):
    if a.action in ("pin", "declare"):
        ops = _ops(a)
        if a.action == "pin":
            return _done(a, ops.method_pin(a.method, a.accept_other_release), "method pinned")
        return _done(a, ops.method_declare(a.fingerprint, a.core_descriptor, T(a.note)), "method identity declared, not verified")
    try:
        root = find_workspace(a.ws)
    except PcposError:
        root = None
    m = method_mod.open_method(root, a.method)
    if a.action == "status":
        req = method_mod.required()
        out_text("ACR Implementation packet at %s\nrelease fingerprint %s\ncore descriptor %s\nfiles %d, each checked against the packet's manifest\nrequired by PCP OS %s: %s"
                 % (m.path, m.ident["release_fingerprint"], m.ident["core_descriptor_sha256"], m.ident["files"], VERSION,
                    "this exact release" if m.ident["matches_required"] else "a DIFFERENT release (%s)" % req["release_fingerprint"][:12]))
    elif a.action == "modules":
        out_text("\n".join("%s %s %s" % (x["id"], x["version"], x["sha256"]) for x in m.modules()))
    elif a.action == "kernel":
        out_text(m.kernel(False))
    elif a.action == "card":
        out_text(m.kernel(True))
    elif a.action == "clause":
        if len(a.args) != 2:
            raise PcposError(USAGE, "usage: pcpos method clause <component> <provision>   for example: pcpos method clause core 5.2")
        out_text(m.clause(a.args[0], a.args[1]))
    elif a.action == "toc":
        out_text(m.toc(a.args[0] if a.args else "core"))
    elif a.action == "find":
        out_text(json.dumps(m.find(a.args), indent=1, ensure_ascii=False))
    elif a.action == "term":
        out_text(json.dumps(m.term(" ".join(a.args)), indent=1, ensure_ascii=False))
    return OK


def c_adopt(a):
    return _done(a, _ops(a).adopt(a.by, T(a.basis)), "ACR is in use, by direction of %s" % a.by)


def c_halt(a):
    ops = _ops(a)
    st = ops.state()
    by = a.by or (st.principals()[0]["label"] if len(st.principals()) == 1 else None)
    if by is None:
        raise PcposError(USAGE, "say who halts: --by <principal>")
    ev, head = ops.halt(by, T(a.words))
    closed = ev["body"]["closed"]
    msg = "ACR use has stopped. No further review step will be recorded." + \
          (" Closed incomplete with their actual coverage: %s." % ", ".join(closed) if closed else " No review was open.") + \
          " Ordinary work continues; ACR resumes only on a new instruction (pcpos adopt)."
    return _done(a, (ev, head), msg)


def c_add(a):
    ops = _ops(a); ev, digest = ops.add_file(a.file, a.name, a.origin)
    if ev is None:
        out_text("already recorded: %s" % digest); return OK
    return _done(a, (ev, ops.ws.events()[1]), "artifact %s %s (%d bytes)" % (digest[:12], a.origin, ev["body"]["size"]))


def c_fetch(a):
    ops = _ops(a); ev, digest, src, n = ops.fetch(a.url, a.name, a.max_bytes, a.timeout)
    text = "retrieved %d bytes from %s (HTTP %d) as %s; retrieval is not inspection: record what you read with pcpos inspect" % (n, src["final_url"], src["status"], digest[:12])
    if ev is None:
        out_text("already recorded: " + text); return OK
    return _done(a, (ev, ops.ws.events()[1]), text)


def c_pin(a):
    ops = _ops(a); ev, issues = ops.pin(a.path, a.name, a.origin, a.store)
    b = ev["body"]; p = b["portability"]
    lines = "tree %s pinned: %d files, %d bytes, manifest %s%s" % (a.name, b["files"], b["bytes"], b["manifest"][:12],
                                                               "" if b.get("stored") else " (by reference: the bytes stay where they are)")
    if b.get("excluded_byproducts") or b.get("symlinks_not_followed"):
        lines += "\n  not included: %d interpreter by-product file(s), %d symbolic link(s)" % (b.get("excluded_byproducts", 0), b.get("symlinks_not_followed", 0))
    lines += "\n  longest relative path %d: on Windows without long-path support, keep the holding folder to %d characters or fewer" % (p["longest_relative_path"], p["windows_root_budget"])
    if issues:
        lines += "\n  portability: %d name(s) would fail or collide on another system, first: %s" % (len(issues), issues[0])
    return _done(a, (ev, ops.ws.events()[1]), lines)


def c_tree(a):
    miss, chg, add = _ops(a).tree_diff(a.name, a.directory)
    if not (miss or chg or add):
        out_text("%s matches the pinned tree %s" % (a.directory, a.name)); return OK
    out_text("%s differs from the pinned tree %s: %d missing, %d changed, %d added" % (a.directory, a.name, len(miss), len(chg), len(add)))
    for tag, items in (("missing", miss), ("changed", chg), ("added", add)):
        for i in items[:10]:
            out_text("  %s %s" % (tag, i))
    return NEGATIVE


def c_show(a):
    data, span = _ops(a).read(a.ref, a.start, a.length, a.source)
    if a.out:
        Path(a.out).write_bytes(data); out_text("wrote %s (%s); the delivery is recorded" % (a.out, span))
    else:
        try:
            out_text(data.decode("utf-8"))
        except UnicodeDecodeError:
            out_text("[%s; not text. Write it to a file with --out <path>]" % span)
    return OK


def c_inspect(a):
    return _done(a, _ops(a).inspect(a.ref, T(a.extent)), "inspection recorded for %s" % a.ref)


def c_check(a):
    argv = a.argv[1:] if a.argv[:1] == ["--"] else a.argv
    ops = _ops(a)
    ev = ops.check(argv, a.name, a.cwd, a.timeout, a.expect_exit, a.tree, a.at, a.allow_drift)
    b = ev["body"]; head = ops.ws.events()[1]
    met = (b["exit"] == b["expect_exit"]) and not b["timeout"]
    tree = ""
    if b["tree"]:
        tree = " | ran on pinned tree %s; the tree %s" % (b["tree"]["name"], "is unchanged after the run" if b["tree"]["matched_after"] else "WAS CHANGED by the run")
    text = "check %s: %sexit %s (expected %s) in %d ms%s | output %s" % (b["id"], "TIMED OUT, " if b["timeout"] else "", b["exit"], b["expect_exit"], b["duration_ms"], tree, b["stdout"][:12])
    if a.json:
        out_text(json.dumps({"ok": met, "event": ev["seq"], "head": head, "body": b}, sort_keys=True))
    else:
        out_text("%s | event %d head %s" % (text, ev["seq"], head[:12]))
        if a.tail:
            tail = ops.ws.get(b["stdout"]).decode("utf-8", "replace").splitlines()[-a.tail:]
            out_text("\n".join("  " + l for l in tail))
    return OK if met else NEGATIVE


def _claims(a):
    if a.claims_file:
        claims = strict_loads(Path(a.claims_file).read_bytes())
        if not isinstance(claims, list):
            raise PcposError(USAGE, "--claims-file holds a JSON list of claims, each with label, text, origin and optionally criterion and kind")
        return claims
    if not a.claim:
        raise PcposError(USAGE, "give the claim: --claim <text or @file> --origin <origin>   (or several with --claims-file)")
    return [{"label": a.claim_label, "text": T(a.claim), "origin": a.origin, "criterion": T(a.criterion), "kind": a.kind}]


def c_commission(a):
    ops = _ops(a)
    ev_head = ops.commission(a.label, a.target, _claims(a), a.procedure, a.extent, a.director, T(a.title), a.module or (),
                             a.gates.split(",") if a.gates else None, a.author, a.parent, a.arrangement, T(a.expectation))
    b = ev_head[0]["body"]
    return _done(a, ev_head, "review %s commissioned: %s, gates %s, %d claim(s)%s" % (a.label, b["procedure"], " ".join(b["required_gates"]), len(b["claims"]),
                                                                                    "; modules " + ", ".join(m["id"] for m in b["modules"]) if b["modules"] else ""))


def c_claim(a):
    claim = {"label": a.label, "text": T(a.text), "origin": a.origin, "criterion": T(a.criterion), "kind": a.kind, "substitute_of": a.substitute_of}
    return _done(a, _ops(a).claim_add(a.review, claim), "claim %s added to %s%s" % (a.label, a.review, " beside %s, which keeps its own disposition" % a.substitute_of if a.substitute_of else ""))


def c_gate(a):
    return _done(a, _ops(a).gate(a.review, a.gate, a.basis or (), T(a.note)), "%s %s committed for %s" % (a.gate, model.GATE_NAMES.get(a.gate, ""), a.review))


def c_objection(a):
    ops = _ops(a)
    if a.action == "raise":
        ev_head = ops.objection(a.review, T(a.text), a.claim, a.basis or ())
        return _done(a, ev_head, "objection %s raised on %s" % (ev_head[0]["body"]["id"], a.review))
    return _done(a, ops.objection_end(a.id, a.resolution, T(a.how), a.basis or ()), "objection %s %s" % (a.id, a.resolution))


def c_finding(a):
    ops = _ops(a)
    ev_head = ops.finding(a.review, a.claim, a.disposition, a.basis or (), a.contrary or (), [T(c) for c in (a.condition or ())],
                          T(a.estimate), T(a.left_open), a.grounding, a.noncritical or ())
    b = ev_head[0]["body"]
    return _done(a, ev_head, "finding %s: %s/%s %s | grounding %s (record holds up to %s) | %s | %s | form checks passed; substance is not assessed by this program"
                 % (b["id"], a.review, a.claim, b["disposition"], b["grounding"], b["grounding_observed"], b["execution"], b["relation"]["label"]))


def c_close(a):
    ev_head = _ops(a).close(a.review, a.lifecycle, a.extent, T(a.note))
    cov = ev_head[0]["body"]["coverage"]
    return _done(a, ev_head, "review %s %s | gates %s of %s | claims without a finding: %s | open objections: %s"
                 % (a.review, a.lifecycle, " ".join(cov["passed_gates"]) or "none", " ".join(cov["required_gates"]),
                    ", ".join(cov["claims_without_finding"]) or "none", ", ".join(cov["open_objections"]) or "none"))


def c_session(a):
    ev_head = _ops(a).session_open(a.review, T(a.reason))
    return _done(a, ev_head, "review %s: successor session %d opened; earlier findings stay sealed as history and the basis is to be revalidated" % (a.review, ev_head[0]["body"]["session"]))


def c_condition(a):
    ev_head = _ops(a).condition(T(a.text), T(a.origin), a.review)
    return _done(a, ev_head, "condition %s registered in its original words" % ev_head[0]["body"]["id"])


def c_obligation(a):
    ops = _ops(a)
    if a.action == "open":
        ev_head = ops.obligation(T(a.text), a.review)
        return _done(a, ev_head, "obligation %s opened" % ev_head[0]["body"]["id"])
    return _done(a, ops.obligation_end(a.id, T(a.how)), "obligation %s closed" % a.id)


def c_note(a):
    return _done(a, _ops(a).note(T(a.text)), "note recorded")


def c_list(a):
    out_text(view.listing(_ops(a).state(), a.what)); return OK


def c_log(a):
    events, _ = _ops(a).ws.events()
    for ev in events[-a.n:]:
        if a.json:
            out_text(json.dumps(ev, sort_keys=True))
        else:
            b = ev["body"]
            brief = b.get("label") or b.get("id") or b.get("name") or b.get("review") or (b.get("sha256") or "")[:12]
            out_text("%5d %s %-22s %-10s %s" % (ev["seq"], ev["ts"], ev["kind"], ev["actor"], brief))
    return OK


def c_export(a):
    out, digest, n, head = bag.export(_ops(a), a.out)
    out_text("bundle %s\nsha256 %s\n%d events, head %s\nlaid out as a BagIt 1.0 bag (RFC 8493). Carry this beside it: %s" % (out, digest, n, head, _ops(a).anchor()))
    return OK


def c_bundle(a):
    rep = bag.Bundle(a.file).verify()
    out_text("bundle %s\nworkspace %s (%s)\n%d events, chain intact, head %s\n%d object(s), every listed file matches its manifest\n%s\nguards replayed: %s"
             % (a.file, rep["workspace"], rep["label"], rep["events"], rep["head"], rep["objects"], view.method_line(rep["state"]),
                "every event passes" if not rep["guard_problems"] else "%d event(s) would not pass" % len(rep["guard_problems"])))
    if a.anchor:
        res = Ops.anchor_check(a.anchor, bag.parse_register(bag.Bundle(a.file).read("data/register.jsonl"))[0])
        out_text("anchor: %s" % res)
        if res not in ("EQUAL", "ANCESTOR"):
            return INTEGRITY
    out_text("result: %s (nothing in the bundle was executed; its findings are its author's statements)" % ("PASS" if not rep["guard_problems"] else "FAIL"))
    return OK if not rep["guard_problems"] else INTEGRITY


def c_admit(a):
    ops = _ops(a)
    ev, rep, compat, taken = bag.admit(ops, a.file, a.label, T(a.assessed))
    text = "submission %s admitted: %d events, head %s, %d artifact(s) taken in as supplied material | method: %s" % (a.label, rep["events"], rep["head"][:12], taken, compat)
    if compat == "DIFFERENT_METHOD_UNKNOWN":
        text += "\n  its record was made under a different ACR release and no compatibility assessment is recorded: a finding here that depends on its findings stays open on that point (Core 7.2, 14.1)"
    if compat == "METHOD_NOT_STATED":
        text += "\n  its record names no ACR release"
    return _done(a, (ev, ops.ws.events()[1]), text)


def c_submission(a):
    ops = _ops(a); st = ops.state()
    s = st.submissions.get(a.label)
    if s is None:
        raise PcposError(USAGE, "no submission labelled %r" % a.label)
    events, head = bag.parse_register(ops.ws.get(s["register"]))
    theirs = model.fold(events)
    out_text("submission %s (statements of its author; none is a finding of this record)\n%s" % (a.label, view.status(theirs, len(events), head)))
    out_text(view.listing(theirs, "findings"))
    return OK


def c_fidelity(a):
    ops = _ops(a); rec = view.fidelity(ops.state(), a.review)
    try:
        res = ops.method().acr.check_fidelity(rec)
        rec["shape_check_by_acr"] = res["result"]
        if res["issues"]:
            rec["shape_issues"] = res["issues"]
    except PcposError:
        rec["shape_check_by_acr"] = "NOT_RUN_METHOD_PACKET_NOT_AVAILABLE"
    out_text(json.dumps(rec, indent=1, sort_keys=True)); return OK


def c_doctor(a):
    rows = doctor.basics()
    if a.net:
        rows += doctor.network()
    if a.requires:
        interp = None
        try:
            interp = _ops(a).interpreter()[0]
        except PcposError:
            pass
        rows += doctor.requirements(a.requires, interp)
    if a.tree:
        rows += doctor.tree(a.tree)
    try:
        m = method_mod.open_method(None, a.method, allow_install=False)
        rows.append({"level": "ok" if m.ident["matches_required"] else "warn", "key": "ACR packet",
                     "detail": "%s, release %s%s" % (m.path, m.ident["release_fingerprint"][:12], "" if m.ident["matches_required"] else " (not the required release)")})
    except PcposError as e:
        rows.append({"level": "note", "key": "ACR packet", "detail": e.message + (". " + e.hint if e.hint else "")})
    out_text(json.dumps(rows, sort_keys=True) if a.json else doctor.render(rows))
    return OK if not any(r["level"] == "fail" for r in rows) else NEGATIVE


def c_env(a):
    ev, n = doctor.create_env(_ops(a), a.packages or (), a.requirements, a.inherit)
    out_text("environment ready inside the workspace: Python %s, %d package(s); checks started with 'python' now use it | event %d" % (ev[0]["body"]["python"], n, ev[0]["seq"]))
    return OK


def c_version(a):
    req = method_mod.required()
    out_text("%s %s\nrecord profile ACR-OS-RECORD-1\nrequires ACR release %s (core %s)\nPython %s" % (label_text(a), VERSION, req["release_fingerprint"], req["core_descriptor_sha256"], sys.version.split()[0]))
    return OK


def c_run(a):
    from . import host
    return host.cmd_run(a, _ops(a))


def c_host(a):
    from . import host
    return host.cmd_host(a, _ops(a))


def c_serve(a):
    from . import server
    return server.main(Workspace(find_workspace(a.ws)), a.actor)


# ---- parser ---------------------------------------------------------------------------
def build():
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--ws", help="workspace directory (default: PCPOS_WS, the current directory or an ancestor, or ./pcp-os)")
    common.add_argument("--as", dest="actor", help="label of the acting participant")
    common.add_argument("--json", action="store_true", help="machine-readable output")
    common.add_argument("--method", help="path to the ACR Implementation packet (folder or sealed archive)")
    common.add_argument("--label-file", help=argparse.SUPPRESS)
    p = argparse.ArgumentParser(prog="pcpos", description="PCP OS %s: a record and enforcement layer for Adversarial Claim Review. "
                                "It records what was done and refuses what ACR's rules exclude; it does not make a finding true." % VERSION)
    sub = p.add_subparsers(dest="cmd", metavar="<command>")

    def cmd(name, fn, help_):
        sp = sub.add_parser(name, parents=[common], help=help_, description=help_)
        sp.set_defaults(fn=fn)
        return sp

    s = cmd("init", c_init, "create a workspace in a new or empty directory; pins the ACR packet if one is beside the program")
    s.add_argument("directory"); s.add_argument("--label", default="workspace"); s.add_argument("--principal", default="principal", help="label of the person directing the work")
    s.add_argument("--principal-name")
    cmd("status", c_status, "what the record holds, briefly")
    s = cmd("brief", c_brief, "the State Brief: the smallest view sufficient for the next act (Core 8.1)")
    s.add_argument("review", nargs="?"); s.add_argument("--max-bytes", type=int)
    cmd("anchor", c_anchor, "one line that identifies the record's present state; keep it outside the workspace")
    s = cmd("verify", c_verify, "check the chain, replay every guard and check stored objects")
    s.add_argument("--quick", action="store_true"); s.add_argument("--anchor")
    cmd("repair", c_repair, "remove an incomplete final register line left by an interrupted write")
    cmd("unlock", c_unlock, "remove a stale lock")
    s = cmd("config", c_config, "local settings (not part of the record)"); s.add_argument("key", nargs="?", default="actor"); s.add_argument("value", nargs="?")
    s = cmd("participant", c_participant, "register a participant")
    s.add_argument("action", choices=["add"]); s.add_argument("label"); s.add_argument("--kind", default="AUTOMATED", choices=model.ACTOR_KINDS)
    s.add_argument("--name"); s.add_argument("--model"); s.add_argument("--provider"); s.add_argument("--lineage", help="model family, for example the provider and model line")
    s.add_argument("--principal", action="store_true")
    s = cmd("method", c_method, "the ACR packet: status, pin, declare, kernel, card, clause, toc, find, term, modules")
    s.add_argument("action", choices=["status", "pin", "declare", "kernel", "card", "clause", "toc", "find", "term", "modules"]); s.add_argument("args", nargs="*")
    s.add_argument("--accept-other-release", action="store_true"); s.add_argument("--fingerprint"); s.add_argument("--core-descriptor"); s.add_argument("--note")
    s = cmd("adopt", c_adopt, "record that the person directing the work asked for ACR"); s.add_argument("--by", required=True); s.add_argument("--basis", required=True, help="their instruction in its original words, or @file")
    s = cmd("halt", c_halt, "stop ACR use at once (Core 15.2)"); s.add_argument("--by"); s.add_argument("--words")
    s = cmd("add", c_add, "record a file as an artifact"); s.add_argument("file"); s.add_argument("--name"); s.add_argument("--origin", default="SUPPLIED", choices=["SUPPLIED", "PRODUCED"])
    s = cmd("fetch", c_fetch, "retrieve an http(s) address and record the bytes, the address and the time")
    s.add_argument("url"); s.add_argument("--name"); s.add_argument("--max-bytes", type=int, default=64 * 1024 * 1024); s.add_argument("--timeout", type=int, default=30)
    s = cmd("pin", c_pin, "pin the exact state of a directory or zip archive as a named tree"); s.add_argument("path"); s.add_argument("--name", required=True)
    s.add_argument("--origin", default="SUPPLIED", choices=["SUPPLIED", "PRODUCED"]); s.add_argument("--store", action="store_true", help="also copy the bytes into the workspace")
    s = cmd("tree", c_tree, "compare a directory with a pinned tree"); s.add_argument("action", choices=["verify"]); s.add_argument("name"); s.add_argument("directory")
    s = cmd("show", c_show, "print an artifact or tree member, checked against its digest; the delivery is recorded")
    s.add_argument("ref"); s.add_argument("--from", dest="source"); s.add_argument("--start", type=int, default=0); s.add_argument("--length", type=int); s.add_argument("--out")
    s = cmd("inspect", c_inspect, "record how much of an item was inspected"); s.add_argument("ref"); s.add_argument("--extent", required=True)
    s = cmd("check", c_check, "run a command and record what happened: pcpos check [options] -- <command> [arguments]")
    s.add_argument("--name"); s.add_argument("--cwd"); s.add_argument("--timeout", type=int); s.add_argument("--expect-exit", type=int, default=0)
    s.add_argument("--tree"); s.add_argument("--at"); s.add_argument("--allow-drift", action="store_true"); s.add_argument("--tail", type=int, default=0)
    s.add_argument("argv", nargs=argparse.REMAINDER)
    s = cmd("commission", c_commission, "commission a review: one target, the claims in their original words, the procedure")
    s.add_argument("label"); s.add_argument("--target", required=True); s.add_argument("--procedure", default="FOCUSED", choices=model.PROCEDURES)
    s.add_argument("--extent", default="TOTAL", choices=["TOTAL", "PARTIAL"]); s.add_argument("--arrangement", default="UNARY", choices=["UNARY", "COMPARATIVE"])
    s.add_argument("--gates", help="comma-separated, for example G1,G3,G5,G6"); s.add_argument("--director"); s.add_argument("--author", help="participant who authored the subject, if known")
    s.add_argument("--parent"); s.add_argument("--title"); s.add_argument("--module", action="append"); s.add_argument("--expectation")
    s.add_argument("--claim"); s.add_argument("--claim-label", default="A"); s.add_argument("--origin", default="COMMISSIONER", choices=model.CLAIM_ORIGINS)
    s.add_argument("--criterion"); s.add_argument("--kind", default="UNSPECIFIED", choices=model.CLAIM_KINDS); s.add_argument("--claims-file")
    s = cmd("claim", c_claim, "add a claim; a narrowed claim stands beside the commissioned one (Core 5.2)")
    s.add_argument("action", choices=["add"]); s.add_argument("review"); s.add_argument("label"); s.add_argument("--text", required=True)
    s.add_argument("--origin", default="REVIEWER_CONSTRUCTED", choices=model.CLAIM_ORIGINS); s.add_argument("--criterion"); s.add_argument("--kind", default="UNSPECIFIED", choices=model.CLAIM_KINDS)
    s.add_argument("--substitute-of")
    s = cmd("gate", c_gate, "commit a gate with its evidence"); s.add_argument("review"); s.add_argument("gate"); s.add_argument("--basis", action="append"); s.add_argument("--note")
    s = cmd("objection", c_objection, "raise or resolve an objection")
    s.add_argument("action", choices=["raise", "resolve"]); s.add_argument("target", help="review label (raise) or objection id (resolve)")
    s.add_argument("--text"); s.add_argument("--claim"); s.add_argument("--basis", action="append"); s.add_argument("--how"); s.add_argument("--resolution", default="RESOLVED", choices=model.OBJECTION_ENDS)
    s = cmd("finding", c_finding, "record a finding on a claim as commissioned")
    s.add_argument("review"); s.add_argument("claim"); s.add_argument("disposition"); s.add_argument("--basis", action="append", help="REF or REF=\"what was inspected\"")
    s.add_argument("--contrary", action="append"); s.add_argument("--condition", action="append"); s.add_argument("--estimate"); s.add_argument("--left-open")
    s.add_argument("--grounding", choices=model.GROUNDING); s.add_argument("--noncritical", action="append")
    s = cmd("close", c_close, "close a review session with its actual coverage"); s.add_argument("review"); s.add_argument("--lifecycle", required=True, choices=model.TERMINAL)
    s.add_argument("--extent", default="UNKNOWN", choices=model.EXTENTS); s.add_argument("--note")
    s = cmd("session", c_session, "open a successor session of a closed review"); s.add_argument("action", choices=["open"]); s.add_argument("review"); s.add_argument("--reason", required=True)
    s = cmd("condition", c_condition, "register a controlling condition in its original words (Core 15.5)")
    s.add_argument("action", choices=["add"]); s.add_argument("--text", required=True); s.add_argument("--origin", required=True); s.add_argument("--review")
    s = cmd("obligation", c_obligation, "open or close a remaining obligation"); s.add_argument("action", choices=["open", "close"]); s.add_argument("id", nargs="?")
    s.add_argument("--text"); s.add_argument("--review"); s.add_argument("--how")
    s = cmd("note", c_note, "record a note"); s.add_argument("text")
    s = cmd("list", c_list, "list items of one kind"); s.add_argument("what", choices=["artifacts", "trees", "checks", "findings", "objections", "conditions", "obligations", "submissions", "participants", "runs"])
    s = cmd("log", c_log, "the latest events"); s.add_argument("-n", type=int, default=20)
    s = cmd("export", c_export, "write the workspace as one bundle for another session or party"); s.add_argument("--out")
    s = cmd("bundle", c_bundle, "check a received bundle without executing anything in it"); s.add_argument("action", choices=["verify"]); s.add_argument("file"); s.add_argument("--anchor")
    s = cmd("admit", c_admit, "admit another party's bundle as a submission"); s.add_argument("file"); s.add_argument("--label", required=True); s.add_argument("--assessed", help="compatibility assessment when its ACR release differs")
    s = cmd("submission", c_submission, "show what an admitted submission states"); s.add_argument("action", choices=["show"]); s.add_argument("label")
    s = cmd("fidelity", c_fidelity, "the Core 15.8 record, written from the register"); s.add_argument("review", nargs="?")
    s = cmd("doctor", c_doctor, "what this machine has and lacks; --net tests reachability, --requires scans a directory's imports, --tree audits names and path lengths")
    s.add_argument("--net", action="store_true"); s.add_argument("--requires"); s.add_argument("--tree")
    s = cmd("env", c_env, "create a Python environment inside the workspace and install packages into it")
    s.add_argument("action", choices=["create"]); s.add_argument("--packages", nargs="*"); s.add_argument("--requirements"); s.add_argument("--inherit", action="store_true", help="also see packages of the host Python")
    s = cmd("run", c_run, "one separate model run on stated material; what was sent is recorded as its exposure")
    s.add_argument("--provider", required=True, choices=["anthropic", "openai", "command"]); s.add_argument("--model"); s.add_argument("--base-url"); s.add_argument("--command")
    s.add_argument("--participant", required=True, help="registered participant label for this run"); s.add_argument("--task", required=True, help="the instruction, or @file")
    s.add_argument("--include", action="append", help="artifact or tree member to send"); s.add_argument("--from", dest="source"); s.add_argument("--review")
    s.add_argument("--kernel", choices=["none", "card", "kernel"], default="kernel"); s.add_argument("--max-tokens", type=int, default=4000); s.add_argument("--timeout", type=int, default=600)
    s.add_argument("--name")
    s = cmd("host", c_host, "an interactive session in which halt, source separation and tool permission are enforced by this program")
    s.add_argument("--provider", required=True, choices=["anthropic", "openai", "command"]); s.add_argument("--model"); s.add_argument("--base-url"); s.add_argument("--command")
    s.add_argument("--principal"); s.add_argument("--participant", required=True)
    s.add_argument("--max-tokens", type=int, default=4000); s.add_argument("--timeout", type=int, default=600)
    cmd("serve", c_serve, "serve the record's operations to a tool-using host over standard input and output")
    cmd("version", c_version, "program, record profile and required ACR release")
    return p


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    p = build()
    if not argv:
        p.print_help(); return USAGE
    try:
        a = p.parse_args(argv)
    except SystemExit as e:
        return USAGE if e.code not in (0, None) else OK
    # 'objection' takes its first positional as a review label or an objection id
    if a.cmd == "objection":
        a.review = a.id = a.target
    try:
        return a.fn(a)
    except PcposError as e:
        if getattr(a, "json", False):
            out_text(json.dumps({"ok": False, "code": e.code, "error": e.message, "next": e.hint, "clause": e.clause}, sort_keys=True))
        else:
            sys.stderr.write(e.render() + "\n")
        return e.code
    except KeyboardInterrupt:
        sys.stderr.write("pcpos: interrupted; nothing partial was recorded\n")
        return 130
    except BrokenPipeError:
        return OK


if __name__ == "__main__":
    sys.exit(main())
