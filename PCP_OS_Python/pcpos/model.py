"""State derived from the register, and the guards an event must pass before it is written.

Every guard cites the ACR clause it enforces. A guard is a necessary condition that can be checked
without judgment. Passing one establishes nothing about whether a finding is right.
"""
from __future__ import annotations
import re, uuid
from . import machines
from .base import PcposError, REFUSED, NOT_IN_USE, USAGE, INTEGRITY, HEX64, LABEL, check_label, sha_bytes, strict_loads

DISPOSITIONS = ("SUPPORTED_IN_SCOPE", "CONDITIONAL", "REFUTED_IN_SCOPE", "NOT_IDENTIFIED_IN_SCOPE",
                "DISPUTED", "UNRESOLVED", "INCOMPARABLE")
GATES = ("G1", "G2", "G3", "G4", "G5", "G6")
GATE_NAMES = {"G1": "Grounding", "G2": "Faithful Case", "G3": "Challenge Map", "G4": "Examination and Construction",
              "G5": "Checking and Judgment", "G6": "Findings and Release"}
PROCEDURES = ("SCREENING", "FOCUSED", "FULL")
MINIMUM_GATES = ("G1", "G5", "G6")
GROUNDING = ("NONE", "RECALL_ONLY", "SUPPLIED_ONLY", "EXTERNAL_INSPECTED")      # ascending
ORIGINS = ("SUPPLIED", "PRODUCED", "RETRIEVED")
CHECK_OBSERVED = ("HOST_OBSERVED_TOOL", "RAN_BY_THIS_PROGRAM", "REPORTED_BY_PARTICIPANT")      # how the record came by a check
CLAIM_ORIGINS = ("COMMISSIONER", "TARGET_TEXT", "REVIEWER_CONSTRUCTED")
CLAIM_KINDS = ("UNSPECIFIED", "EXTERNAL_FACT", "MECHANICAL_CHECK", "DERIVATION")
ACTOR_KINDS = ("HUMAN", "AUTOMATED", "UNKNOWN")
LIFECYCLES = ("ACTIVE", "PAUSED", "CLOSED_COMPLETE", "CLOSED_INCOMPLETE", "ABORTED")
TERMINAL = ("CLOSED_COMPLETE", "CLOSED_INCOMPLETE", "ABORTED")
EXTENTS = ("NONE", "PARTIAL", "TOTAL", "UNKNOWN")
OBJECTION_ENDS = ("RESOLVED", "WITHDRAWN", "OPEN_RECORDED")
COMPATIBILITY = ("SAME_METHOD", "DIFFERENT_METHOD_ASSESSED", "DIFFERENT_METHOD_UNKNOWN", "METHOD_NOT_STATED")
TRACE = ("FULL", "SUMMARY", "NONE", "UNKNOWN")
STRUCTURED = {"REVIEW_COMMISSIONED", "CLAIM_ADDED", "GATE_COMMITTED", "OBJECTION_RAISED", "OBJECTION_RESOLVED",
              "FINDING_RECORDED", "REVIEW_CLOSED", "SESSION_OPENED", "REVIEW_PAUSED", "REVIEW_RESUMED"}   # stopped by a halt (Core 15.2, step 1)
HOST_KINDS = {"WORKSPACE_CREATED", "METHOD_PINNED", "SYSTEM_STATE", "LINK_STATE", "HOST_EVENT", "ENVIRONMENT_RECORDED", "TASK_ENDED"}
DIRECTION_KINDS = {"ADOPTED", "PARTICIPANT_ADDED", "STANDARD_SET"}
KINDS = STRUCTURED | HOST_KINDS | DIRECTION_KINDS | {"HALTED", "ARTIFACT_ADDED",
                      "TREE_PINNED", "ARTIFACT_READ", "INSPECTION_DECLARED", "CHECK_RAN", "CONDITION_REGISTERED",
                      "SUBMISSION_ADMITTED", "OBLIGATION_OPENED", "OBLIGATION_CLOSED", "RUN_COMPLETED", "NOTE", "TASK_STARTED", "DELIVERY_STATE"}
UNREGISTERED_OK = {"DELIVERY_STATE", "WORKSPACE_CREATED", "METHOD_PINNED", "PARTICIPANT_ADDED", "HOST_EVENT", "SYSTEM_STATE", "LINK_STATE", "TASK_ENDED"}
WORKERS = ("PROGRAM", "MODEL")
TASK_ENDS = ("DONE", "FAILED", "STOPPED", "LOST")
STANDARD_METHOD = ("NATIVE_OR_PINNED", "NATIVE_ONLY")                    # ascending strictness
STANDARD_MODEL_FINDINGS = ("AS_METHOD", "HUMAN_CLOSES", "MECHANICAL_ONLY")  # ascending strictness
STANDARD_FLAGS = ("human_adoption", "dual_authorization", "static_tasks", "offline_only")
BASELINE = {"method": "NATIVE_OR_PINNED", "model_findings": "AS_METHOD", "human_adoption": False, "dual_authorization": False,
            "static_tasks": False, "offline_only": False, "tasks": []}


def is_int(v):
    """An integer and not a boolean (in Python a boolean is an integer; in the record it is not)."""
    return isinstance(v, int) and not isinstance(v, bool)


def flag(b, key, what):
    """A field that is true, false or absent. Nothing else is read as a truth value."""
    v = b.get(key)
    if v is None:
        return False
    if not isinstance(v, bool):
        refuse("%s is true or false" % what)
    return v


def event_class(kind):
    if kind in HOST_KINDS or kind.startswith("X_"):
        return "HOST"
    if kind == "HALTED":
        return "HALT"
    if kind in DIRECTION_KINDS:
        return "DIRECTION"
    if kind in STRUCTURED:
        return "REVIEW"
    if kind == "TASK_STARTED":
        return "TASK"
    return "RECORD"


def permits(system, cls):
    """The coupling rule from the outside in: may an event of this class be accepted in this system state?"""
    if system is None:
        return True                      # the register is not kept by a system that records its own state
    if system == "STOPPED":
        return False
    if cls == "HOST":
        return True
    if system in ("START", "VERIFY", "STOPPING"):
        return False
    if cls == "HALT":
        return True
    return system in ("OPERATIONAL", "DEGRADED")
_CTRL = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")


def refuse(message, clause=None, hint=None):
    raise PcposError(REFUSED, message, hint, clause)


def text(value, what, limit=8000, required=True):
    if value is None and not required:
        return None
    if not isinstance(value, str) or not value.strip():
        refuse("%s is required and must be text" % what)
    if _CTRL.search(value):
        refuse("%s contains a control character" % what)
    if len(value) > limit:
        refuse("%s is %d characters; the limit is %d" % (what, len(value), limit), hint="store long text as an artifact and refer to it")
    return value.strip()


def one_of(value, allowed, what):
    if value not in allowed:
        refuse("%s %r is not one of: %s" % (what, value, ", ".join(allowed)),
               "Core 15.6" if what in ("disposition", "procedure", "gate") else None,
               "ACR defines these values only; an undefined code has no ACR meaning")
    return value


class State:
    def __init__(self):
        self.created = False
        self.method = None
        self.participants, self.artifacts, self.names, self.trees = {}, {}, {}, {}
        self.adopted, self.adoption_log = False, []
        self.checks, self.reviews, self.objections, self.conditions = {}, {}, {}, {}
        self.submissions, self.obligations, self.runs, self.inspections = {}, {}, {}, {}
        self.findings = {}
        self.system, self.link, self.standard, self.tasks, self.unclean_starts = None, "OFFLINE", None, {}, 0
        self.consumed = set()
        self.delivery = {}
        self.seq = -1
        self.counters = {"K": 0, "F": 0, "O": 0, "C": 0, "B": 0, "R": 0, "T": 0}

    def next_id(self, letter):
        return "%s%d" % (letter, self.counters[letter] + 1)

    # ---- lookups ----------------------------------------------------------------
    def artifact(self, token):
        """Resolve an artifact name or a digest prefix of eight or more hexadecimal characters."""
        if token in self.names:
            return self.names[token]
        if re.fullmatch(r"[0-9a-f]{8,64}", token):
            hits = [d for d in self.artifacts if d.startswith(token)]
            if len(hits) == 1:
                return hits[0]
            if len(hits) > 1:
                refuse("%r matches %d artifacts; give more characters" % (token, len(hits)))
        refuse("no artifact named or beginning %r" % token, hint="list them with: pcpos list artifacts")

    def review(self, label, active=False, working=True):
        r = self.reviews.get(label) if isinstance(label, str) else None
        if r is None:
            refuse("no review labelled %r" % (label,), hint="list them with: pcpos status")
        if active and r["lifecycle"] in TERMINAL:
            refuse("review %s is %s; its assessment is sealed" % (label, r["lifecycle"]), "Core 7.1",
                   "renewed assessment needs a successor session: pcpos session open %s --reason <text>" % label)
        if active and working and r["lifecycle"] == "PAUSED":
            refuse("review %s is PAUSED; no step is recorded until it is resumed" % label, None, "pcpos resume %s" % label)
        return r

    def standard_now(self):
        return self.standard or BASELINE

    def model_driven(self, label):
        """True unless the participant is a person, or an automated participant recorded without a model.
        An unknown kind is treated as model-driven: the stricter reading."""
        p = self.participants.get(label, {})
        if p.get("kind") == "HUMAN":
            return False
        return not (p.get("kind") == "AUTOMATED" and p.get("model") is None)

    def human_principal(self, label):
        p = self.participants.get(label) if isinstance(label, str) else None
        return bool(p and p["principal"] and p["kind"] == "HUMAN")

    def principals(self):
        return [p for p in self.participants.values() if p["principal"]]


# ---- references ------------------------------------------------------------------
def parse_ref(state, token):
    """Turn 'K3', 'F2', a name, a digest prefix or 'tree:NAME/path' into a reference record."""
    extent = None
    if "=" in token:
        token, extent = token.split("=", 1)
        extent = extent.strip() or None
    token = token.strip()
    m = re.fullmatch(r"([KF])(\d+)", token)
    if m:
        ref = {"t": "check" if m.group(1) == "K" else "finding", "id": token}
    elif token.startswith("tree:"):
        name, _, path = token[5:].partition("/")
        ref = {"t": "member", "tree": name, "path": path}
    else:
        ref = {"t": "artifact", "sha256": state.artifact(token)}
    return {"ref": ref, "extent": extent} if extent else {"ref": ref}


def _ref_key(ref):
    if ref["t"] == "artifact":
        return ref["sha256"]
    if ref["t"] == "member":
        return "tree:%s/%s" % (ref["tree"], ref["path"])
    return ref["id"]


def _check_basis(state, items, what, store=None):
    """Each reference exists, and each inspected item says how much of it was inspected (Core 4.3, 8.2)."""
    out = []
    if not isinstance(items, list):
        refuse("%s must be a list of references" % what)
    for it in items:
        if not isinstance(it, dict) or not isinstance(it.get("ref"), dict):
            refuse("%s holds a malformed reference" % what)
        ref, extent = it["ref"], it.get("extent")
        t = ref.get("t")
        if t == "artifact":
            if ref.get("sha256") not in state.artifacts:
                refuse("%s refers to an artifact that is not in the record" % what)
        elif t == "member":
            if not isinstance(ref.get("tree"), str) or not isinstance(ref.get("path"), str):
                refuse("%s holds a malformed reference" % what)
            tree = state.trees.get(ref.get("tree"))
            if tree is None:
                refuse("%s refers to a tree that is not pinned: %r" % (what, ref.get("tree")))
            if store is not None:
                entries = {e[0]: e for e in strict_loads(store.get(tree["manifest"]))["entries"]}
                if ref.get("path") not in entries:
                    refuse("%r is not a file of the pinned tree %s" % (ref.get("path"), ref["tree"]))
                ref = dict(ref, sha256=entries[ref["path"]][2])
        elif t == "check":
            if ref.get("id") not in state.checks:
                refuse("%s refers to a check that was not run through this record: %r" % (what, ref.get("id")), "Core 15.8")
        elif t == "finding":
            if ref.get("id") not in state.findings:
                refuse("%s refers to a finding that is not in the record: %r" % (what, ref.get("id")))
        else:
            refuse("%s holds a reference of unknown type %r" % (what, t))
        if t in ("artifact", "member"):
            if extent is not None:
                extent = text(extent, "the inspected extent", 500)
            elif _ref_key(ref) not in state.inspections:
                refuse("no inspection is recorded for %s" % _ref_key(ref)[:40], "Core 4.3, 8.2",
                       "say how much of it was inspected: REF=\"what was read\", or run: pcpos inspect REF --extent <text>")
        out.append({"ref": ref, "extent": extent} if extent else {"ref": ref})
    return out


def _observed_grounding(state, basis):
    """The strongest grounding for which the record holds an inspected item. An upper bound, set by observation."""
    best = 0
    for it in basis:
        ref = it["ref"]
        if ref["t"] == "artifact":
            origin = state.artifacts[ref["sha256"]]["origin"]
            best = max(best, 3 if origin == "RETRIEVED" else 2)
        elif ref["t"] in ("member", "check"):
            best = max(best, 2)
    return GROUNDING[best]


def _relation(state, review, actor):
    """What the record can say about the relation between the party assessing and the party that authored the
    subject. It reports dimensions and never the word 'independent' (Core 4.2, 6.2)."""
    author = review.get("author")
    a, b = state.participants.get(actor, {}), state.participants.get(author, {}) if author else {}
    if author is None:
        label, lineage = "AUTHOR_NOT_RECORDED", "UNKNOWN"
    elif author == actor:
        label, lineage = "SELF_CHECK", "SAME"
    else:
        la, lb = a.get("lineage"), b.get("lineage")
        lineage = "UNKNOWN" if not la or not lb else ("SAME" if la == lb else "DIFFERENT")
        label = {"SAME": "SEPARATE_ACTOR_SAME_LINEAGE", "DIFFERENT": "SEPARATE_ACTOR_DIFFERENT_LINEAGE"}.get(lineage, "SEPARATE_ACTOR_LINEAGE_UNKNOWN")
    return {"label": label, "author": author, "lineage": lineage, "exposure": "NOT_OBSERVED"}


# ---- guards ------------------------------------------------------------------------
def guard(state, kind, body, actor, store=None):
    """Return the body to write, with computed fields filled in, or raise. Idempotent on its own output."""
    if kind not in KINDS and not kind.startswith("X_"):
        refuse("unknown event kind %r" % kind)
    if kind not in ("WORKSPACE_CREATED",) and not state.created:
        refuse("the register does not begin with WORKSPACE_CREATED")
    if kind in ("SYSTEM_STATE", "LINK_STATE", "TASK_ENDED") and actor != "host":
        refuse("%s is written by the system itself" % kind)
    if kind not in UNREGISTERED_OK and actor not in state.participants:
        refuse("the acting participant %r is not registered" % actor, hint="register with: pcpos participant add <label> ...   or pass --as <label>")
    boot = kind == "SYSTEM_STATE" and isinstance(body, dict) and body.get("event") == "BOOT"    # a new start begins a new run
    if not boot and not permits(state.system, event_class(kind)):
        refuse("the system is in state %s, in which no %s event is accepted" % (state.system, event_class(kind)))
    if kind in STRUCTURED and not state.adopted:
        raise PcposError(NOT_IN_USE, "ACR is not in use in this workspace, so no review step is recorded",
                         "ACR is used when the person directing the work asks for it: pcpos adopt --by <principal> --basis \"<their words>\"",
                         "Core 2.1, 15.1, 15.2")
    b = dict(body)
    fn = _GUARDS.get(kind)
    try:
        b = fn(state, b, actor, store) if fn else b
        _standard(state, kind, b, actor)
    except (TypeError, AttributeError, KeyError, IndexError, ValueError):
        # A field of the wrong type reached a lookup or a comparison. Fail closed, and say so.
        raise PcposError(REFUSED, "a field of the event has the wrong form", None, "FORM")
    return b


def _g_created(state, b, actor, store):
    if state.created:
        refuse("the workspace is already created")
    return b


def _g_method(state, b, actor, store):
    for k in ("release_fingerprint", "core_descriptor_sha256"):
        if not HEX64.match(str(b.get(k, ""))):
            refuse("the method identity needs %s as 64 hexadecimal characters" % k, "Core 14.1, 14.2")
    one_of(b.get("verified"), ("FILES_VERIFIED_AGAINST_OWN_MANIFEST", "DECLARED_NOT_VERIFIED"), "verification")
    if b.get("presence") is not None:
        one_of(b["presence"], ("NATIVE", "PINNED"), "presence")
    return b


def _g_participant(state, b, actor, store):
    check_label(b.get("label"), "participant label")
    if b["label"] in state.participants:
        refuse("participant %r is already registered" % b["label"])
    one_of(b.get("kind"), ACTOR_KINDS, "actor kind")
    if b.get("id") is None:
        b["id"] = "acr:participant:%s" % uuid.uuid4()
    elif not isinstance(b["id"], str) or not b["id"]:
        refuse("a participant id, when given, is text")
    b["principal"] = flag(b, "principal", "principal")
    for k in ("name", "model", "provider", "lineage"):
        if b.get(k) is not None:
            b[k] = text(b[k], k, 200)
    return b


def _g_adopt(state, b, actor, store):
    by = b.get("by")
    if by != "host" and not state.participants.get(by, {}).get("principal"):
        refuse("ACR use is decided by the person directing the work or by the host; %r is neither" % by, "Core 15.1")
    b["basis"] = text(b.get("basis"), "the basis (the instruction in its original words)", 2000)
    return b


def _g_halt(state, b, actor, store):
    by = b.get("by")
    if by != "host" and not state.participants.get(by, {}).get("principal"):
        refuse("a halt of ACR use comes from the person directing the work or the host; %r is neither" % by, "Core 15.2",
               "any participant may stop its own part: simply stop, and say so")
    b["closed"] = sorted(l for l, r in state.reviews.items() if r["lifecycle"] not in TERMINAL)
    return b


def _g_artifact(state, b, actor, store):
    if not HEX64.match(str(b.get("sha256", ""))):
        refuse("an artifact needs its SHA-256")
    one_of(b.get("origin"), ORIGINS, "origin")
    if not is_int(b.get("size")) or b["size"] < 0:
        refuse("an artifact needs its size in bytes")
    if b.get("name") is not None:
        check_label(b["name"], "artifact name")
        if state.names.get(b["name"], b["sha256"]) != b["sha256"]:
            refuse("the name %r already belongs to another artifact" % b["name"], hint="names are not reused; choose another")
    if b["origin"] == "RETRIEVED" and not isinstance(b.get("source"), dict):
        refuse("a retrieved artifact records where and when it was retrieved")
    b["stored"] = flag(b, "stored", "stored")
    return b


def _g_tree(state, b, actor, store):
    check_label(b.get("name"), "tree name")
    if b["name"] in state.trees:
        refuse("a tree named %r is already pinned" % b["name"], "Core 7.4", "a pinned snapshot is not replaced; pin the new state under a new name")
    if not HEX64.match(str(b.get("manifest", ""))):
        refuse("a pinned tree needs the digest of its manifest")
    one_of(b.get("origin"), ("SUPPLIED", "PRODUCED"), "origin")
    return b


def _g_read(state, b, actor, store):
    ref = b.get("ref", {})
    _check_basis(state, [{"ref": ref, "extent": "x"}], "the item read", store)
    return b


def _g_inspect(state, b, actor, store):
    b["extent"] = text(b.get("extent"), "the inspected extent", 500)
    it = _check_basis(state, [{"ref": b.get("ref", {}), "extent": b["extent"]}], "the inspected item", store)[0]
    b["ref"] = it["ref"]
    return b


def _g_check(state, b, actor, store):
    b["id"] = state.next_id("K") if b.get("id") in (None, state.next_id("K")) else refuse("check id out of sequence")
    if not (isinstance(b.get("argv"), list) and b["argv"] and all(isinstance(a, str) for a in b["argv"])):
        refuse("a check records the exact command that was run")
    for k in ("stdout", "stderr"):
        if not HEX64.match(str(b.get(k, ""))):
            refuse("a check records the digest of its %s" % k)
    if not is_int(b.get("exit")) and b.get("exit") is not None:
        refuse("a check records its exit status")
    if not is_int(b.get("expect_exit")) or not isinstance(b.get("timeout"), bool):
        refuse("a check records the exit status it expected and whether it timed out")
    if b.get("observed") is not None:
        one_of(b.get("observed"), CHECK_OBSERVED, "how the check was observed")
    return b


def _claim(state, c, existing):
    check_label(c.get("label"), "claim label")
    if c["label"] in existing:
        refuse("claim %r already exists in this review" % c["label"])
    c["text"] = text(c.get("text"), "the claim, in its original words")
    c["text_sha256"] = sha_bytes(c["text"].encode("utf-8"))
    one_of(c.get("origin"), CLAIM_ORIGINS, "claim origin")
    c["kind"] = one_of(c.get("kind") or "UNSPECIFIED", CLAIM_KINDS, "claim kind")
    c["criterion"] = text(c.get("criterion"), "the criterion", 2000, required=False)
    if c.get("substitute_of") is not None and c["substitute_of"] not in existing:
        refuse("a substitute claim names the commissioned claim it stands beside; %r is not one" % c["substitute_of"], "Core 5.2")
    return c


def _g_commission(state, b, actor, store):
    if state.method is None:
        refuse("no method identity is recorded", "Core 14.1, 14.2", "pin the ACR release in use: pcpos method pin   (or: pcpos method declare ...)")
    check_label(b.get("label"), "review label")
    if b["label"] in state.reviews:
        refuse("a review labelled %r already exists" % b["label"], "Core 2.4", "an identical retry creates nothing; a new review takes a new label")
    if b.get("id") is None:
        b["id"] = "acr:review:%s" % uuid.uuid4()
    elif not isinstance(b["id"], str) or not b["id"]:
        refuse("a review id, when given, is text")
    b["title"] = text(b.get("title"), "title", 300, required=False)
    tgt = b.get("target") or {}
    if tgt.get("t") == "tree":
        if tgt.get("name") not in state.trees:
            refuse("the target tree %r is not pinned" % tgt.get("name"), "Core 2.1, 2.2", "pin the exact subject first: pcpos pin <directory> --name <label>")
    elif tgt.get("t") == "artifact":
        if tgt.get("sha256") not in state.artifacts:
            refuse("the target artifact is not in the record", "Core 2.1, 2.2")
    else:
        refuse("a review binds exactly one target: a pinned tree or an artifact", "Core 2.1")
    one_of(b.get("extent"), ("TOTAL", "PARTIAL"), "intended extent")
    one_of(b.get("arrangement") or "UNARY", ("UNARY", "COMPARATIVE"), "arrangement")
    b["arrangement"] = b.get("arrangement") or "UNARY"
    one_of(b.get("procedure"), PROCEDURES, "procedure")
    gates = b.get("required_gates") or (list(GATES) if b["procedure"] == "FULL" else list(MINIMUM_GATES))
    if not isinstance(gates, list) or any(g not in GATES for g in gates) or list(gates) != sorted(set(gates), key=GATES.index):
        refuse("required gates are a unique ordered subset of G1 to G6", "Core 6.1")
    if not set(MINIMUM_GATES) <= set(gates) or (b["procedure"] == "FULL" and list(gates) != list(GATES)):
        refuse("%s requires %s" % (b["procedure"], "all six gates" if b["procedure"] == "FULL" else "G1, G5 and G6"), "Core 1.2, 6.1")
    b["required_gates"] = list(gates)
    if b.get("director") not in state.participants:
        refuse("the Director is a registered participant; %r is not" % b.get("director"), "Core 3.2")
    if b.get("author") is not None and b["author"] not in state.participants:
        refuse("the author of the subject, when stated, is a registered participant; %r is not" % b["author"])
    if b.get("parent") is not None and b["parent"] not in state.reviews:
        refuse("the parent review %r does not exist" % b["parent"], "Core 7.2")
    if b.get("parent") is not None:
        b["parent_basis"] = {"seq": state.seq, "session": state.reviews[b["parent"]]["session"]}
    mods = b.get("modules") or []
    if not isinstance(mods, list) or any(not isinstance(m, dict) or not HEX64.match(str(m.get("sha256", ""))) for m in mods):
        refuse("each selected module is recorded by identifier, version and digest", "Core 14.1")
    b["modules"] = mods
    claims, seen = [], {}
    if not isinstance(b.get("claims"), list) or not b["claims"]:
        refuse("a review needs at least one claim, in its original words, with its origin", "Core 2.1, 6.1")
    for c in b["claims"]:
        if not isinstance(c, dict):
            refuse("each claim is an object with its label, text and origin", "Core 2.1, 6.1")
        c = _claim(state, dict(c), seen)
        seen[c["label"]] = c; claims.append(c)
    b["claims"] = claims
    b["expectation"] = text(b.get("expectation") or "NO_DIRECTIONAL_EXPECTATION", "the opening expectation", 1000)
    return b


def _g_claim_added(state, b, actor, store):
    r = state.review(b.get("review"), active=True)
    if not isinstance(b.get("claim"), dict):
        refuse("the claim is an object with its label, text and origin")
    b["claim"] = _claim(state, dict(b["claim"]), r["claims"])
    if "G1" in r["passed_gates"] and b["claim"]["origin"] != "REVIEWER_CONSTRUCTED":
        refuse("G1 is committed: a further commissioned claim is an amendment of the review, not an addition", "Core 6.1",
               "commission it as a child review (--parent %s), or close this session and open a successor; a narrowed claim of the reviewer's own is added with --origin REVIEWER_CONSTRUCTED" % r["label"])
    return b


def _g_gate(state, b, actor, store):
    r = state.review(b.get("review"), active=True)
    gate = one_of(b.get("gate"), GATES, "gate")
    if gate not in r["required_gates"]:
        refuse("%s is not a required gate of review %s (%s: %s)" % (gate, r["label"], r["procedure"], " ".join(r["required_gates"])), "Core 6.1")
    if gate in r["passed_gates"]:
        refuse("%s is already committed in this session" % gate)
    expected = r["required_gates"][len(r["passed_gates"])]
    if gate != expected:
        refuse("%s cannot be committed before %s; completed gates form an ordered prefix" % (gate, expected), "Core 6.1")
    b["basis"] = _check_basis(state, b.get("basis") or [], "the gate's evidence", store)
    b["note"] = text(b.get("note"), "note", 2000, required=False)
    if gate != "G1" and not b["basis"] and not b["note"]:
        refuse("%s (%s) is committed with its evidence: a reference or a note of what was done" % (gate, GATE_NAMES[gate]), "Core 6.1, 7.1",
               "a finished artifact is not a passed gate; add --basis <ref> or --note <text>")
    if gate == "G5":
        missing = [c for c in r["claim_order"] if not _current(state, r, c)]
        if missing:
            refuse("G5 needs a finding for each claim; none yet for: %s" % ", ".join(missing), "Core 6.1")
    return b


def _g_objection(state, b, actor, store):
    r = state.review(b.get("review"), active=True)
    b["id"] = state.next_id("O") if b.get("id") in (None, state.next_id("O")) else refuse("objection id out of sequence")
    if b.get("claim") is not None and b["claim"] not in r["claims"]:
        refuse("claim %r is not in review %s" % (b["claim"], r["label"]))
    b["text"] = text(b.get("text"), "the objection")
    b["basis"] = _check_basis(state, b.get("basis") or [], "the objection's basis", store)
    return b


def _g_objection_end(state, b, actor, store):
    o = state.objections.get(b.get("id")) if isinstance(b.get("id"), str) else None
    if o is None:
        refuse("no objection %r" % (b.get("id"),))
    if o["status"] != "OPEN":
        refuse("objection %s is already %s" % (o["id"], o["status"]))
    state.review(o["review"], active=True)
    one_of(b.get("resolution"), OBJECTION_ENDS, "resolution")
    b["how"] = text(b.get("how"), "how it was resolved, or why it stays open", 2000)
    b["basis"] = _check_basis(state, b.get("basis") or [], "the resolution's basis", store)
    return b


def _current(state, review, claim):
    """Current findings on a claim: the latest by each actor."""
    return [state.findings[i] for (c, a), i in sorted(review["current"].items()) if c == claim]


def _g_finding(state, b, actor, store):
    r = state.review(b.get("review"), active=True)
    if "G1" not in r["passed_gates"]:
        refuse("no finding before G1: the subject, the claims and their origin are committed first", "Core 6.1",
               "commit grounding with: pcpos gate %s G1" % r["label"])
    claim = r["claims"].get(b.get("claim"))
    if claim is None:
        refuse("claim %r is not in review %s; a finding answers a claim as commissioned" % (b.get("claim"), r["label"]), "Core 5.2",
               "if the claim has to be narrowed, add the narrower one beside it: pcpos claim add %s ... --substitute-of <claim>" % r["label"])
    disp = one_of(b.get("disposition"), DISPOSITIONS, "disposition")
    b["id"] = state.next_id("F") if b.get("id") in (None, state.next_id("F")) else refuse("finding id out of sequence")
    basis = _check_basis(state, b.get("basis") or [], "the basis", store)
    contrary = _check_basis(state, b.get("contrary") or [], "the contrary evidence", store)
    b["basis"], b["contrary"] = basis, contrary
    every = basis + contrary
    conds = b.get("conditions") or []
    if not isinstance(conds, list):
        refuse("conditions are a list of statements")
    b["conditions"] = [text(c, "a condition", 2000) for c in conds]
    b["estimate"] = text(b.get("estimate"), "the estimate", 2000, required=False)
    b["left_open"] = text(b.get("left_open"), "what is left open", 4000, required=False)
    noncritical = b.get("noncritical") or []
    if not isinstance(noncritical, list) or any(not isinstance(x, str) for x in noncritical):
        refuse("noncritical is a list of finding ids")
    # what was observed
    checks = [state.checks[i["ref"]["id"]] for i in every if i["ref"]["t"] == "check"]
    # EXECUTED says the record itself holds that a check ran; a check only reported by a participant leaves that unknown (Core 15.8)
    b["execution"] = "REASONED" if not checks else "UNKNOWN" if all(k.get("observed") == "REPORTED_BY_PARTICIPANT" for k in checks) else "EXECUTED"
    observed = _observed_grounding(state, every)
    b["grounding_observed"] = observed
    declared = b.get("grounding_declared")
    if declared is not None:
        one_of(declared, GROUNDING, "grounding")
        if GROUNDING.index(declared) > GROUNDING.index(observed) and not (declared == "RECALL_ONLY" and observed == "NONE"):
            refuse("grounding %s is declared but the record holds inspected material only up to %s" % (declared, observed), "Core 15.8",
                   "retrieve and inspect the source through the record (pcpos fetch, pcpos inspect), or declare what was actually done")
        b["grounding"] = declared
    elif observed == "NONE":
        refuse("this finding cites nothing inspected or run; say what it rests on", "Core 15.8",
               "add --grounding RECALL_ONLY (memory without inspection) or --grounding NONE")
    else:
        b["grounding"] = observed
    # dispositions that need something checked
    if disp in ("SUPPORTED_IN_SCOPE", "REFUTED_IN_SCOPE", "NOT_IDENTIFIED_IN_SCOPE") and not every:
        refuse("%s needs something this review inspected or ran" % disp, "Core 5.2",
               "lack of support is not refutation and an unsuccessful search is not a witness; without a checked basis the claim is UNRESOLVED or CONDITIONAL")
    if disp == "REFUTED_IN_SCOPE" and not contrary:
        refuse("REFUTED_IN_SCOPE needs checked contrary evidence, named with --contrary", "Core 5.2")
    if disp in ("SUPPORTED_IN_SCOPE", "REFUTED_IN_SCOPE") and not claim["criterion"]:
        refuse("%s is assessed against a stated criterion, and claim %s has none" % (disp, claim["label"]), "Core 5.1, 5.2",
               "the criterion is fixed with the claim, before the evidence: commission the claim with --criterion, or add a claim that has one")
    if disp == "CONDITIONAL" and not b["conditions"]:
        refuse("CONDITIONAL names the essential assumptions it depends on", "Core 5.2", "add --condition <text> for each")
    if disp == "SUPPORTED_IN_SCOPE":
        if claim["kind"] == "EXTERNAL_FACT" and b["grounding"] in ("RECALL_ONLY", "NONE"):
            refuse("a claim about an external fact that rests on recall cannot be SUPPORTED_IN_SCOPE", "Core 15.8",
                   "report CONDITIONAL or UNRESOLVED, with a labelled estimate if one is useful (--estimate)")
        if claim["kind"] == "MECHANICAL_CHECK":
            good = [k for k in checks if k["exit"] is not None and k["exit"] == k["expect_exit"] and not k["timeout"]]
            if not good:
                refuse("a mechanically checkable claim is supported by an executed check that met its expectation, and none is cited", "Core 5.1, 15.8")
        weak = [i["ref"]["id"] for i in basis if i["ref"]["t"] == "finding"
                and state.findings[i["ref"]["id"]]["disposition"] != "SUPPORTED_IN_SCOPE"
                and i["ref"]["id"] not in (b.get("noncritical") or [])]
        if weak:
            refuse("the basis includes finding(s) %s that are not SUPPORTED_IN_SCOPE" % ", ".join(weak), "Core 5.2",
                   "CONDITIONAL takes precedence when an acceptance-critical premise is unestablished; if the premise is not critical, say so with --noncritical <id>")
    # What a review writes out for itself is its reasoning: recording that text does not make it evidence the review checked
    # (Core 4.3). Material in the record before the review was commissioned, a check, a tree and a finding are not its own product.
    def own(i):
        a = state.artifacts.get(i["ref"].get("sha256")) if i["ref"]["t"] == "artifact" else None
        return bool(a) and a.get("produced", False) and a["seq"] > r["seq"]
    if disp == "REFUTED_IN_SCOPE" and all(own(i) for i in contrary):
        refuse("REFUTED_IN_SCOPE needs checked contrary evidence, and all that is cited against the claim is text this review itself produced", "Core 4.3, 5.2",
               "cite the check that was run or the material that was inspected; where there is neither, the claim is UNRESOLVED (Core 5.1)")
    if disp == "SUPPORTED_IN_SCOPE" and all(own(i) for i in every):
        refuse("SUPPORTED_IN_SCOPE needs something this review inspected or ran, and all that is cited is text this review itself produced", "Core 4.3, 5.2")
    if claim["kind"] == "MECHANICAL_CHECK" and disp == "REFUTED_IN_SCOPE" and not [k for k in checks if k["exit"] is not None and not k["timeout"]]:
        refuse("a mechanically checkable claim is refuted by an executed check that ran to its end, and none is cited", "Core 5.1")
    b["noncritical"] = sorted(b.get("noncritical") or [])
    b["relation"] = _relation(state, r, actor)
    b["supersedes"] = r["current"].get((b["claim"], actor))
    b["session"] = r["session"]
    return b


def _coverage(state, r):
    return {"passed_gates": list(r["passed_gates"]), "required_gates": list(r["required_gates"]),
            "claims_without_finding": [c for c in r["claim_order"] if not _current(state, r, c)],
            "open_objections": sorted(o["id"] for o in state.objections.values() if o["review"] == r["label"] and o["status"] == "OPEN"),
            "findings": sum(1 for f in state.findings.values() if f["review"] == r["label"] and f["session"] == r["session"])}


def _g_close(state, b, actor, store):
    r = state.review(b.get("review"), active=True, working=False)
    one_of(b.get("lifecycle"), TERMINAL, "closing lifecycle")
    event = {"CLOSED_COMPLETE": "CLOSE_COMPLETE", "CLOSED_INCOMPLETE": "CLOSE_INCOMPLETE", "ABORTED": "ABORT"}[b["lifecycle"]]
    if machines.step("REVIEW", r["lifecycle"], event) is None:
        refuse("a review that is %s is not closed as %s" % (r["lifecycle"], b["lifecycle"]), None, "resume it first: pcpos resume %s" % r["label"])
    cov = _coverage(state, r)
    if b["lifecycle"] == "CLOSED_COMPLETE":
        if cov["passed_gates"] != cov["required_gates"]:
            refuse("CLOSED_COMPLETE needs every required gate; committed: %s of %s" % (" ".join(cov["passed_gates"]) or "none", " ".join(cov["required_gates"])),
                   "Core 7.1", "close with its actual coverage instead: --lifecycle CLOSED_INCOMPLETE")
        if cov["claims_without_finding"]:
            refuse("CLOSED_COMPLETE needs a finding for each commissioned claim; none for: %s" % ", ".join(cov["claims_without_finding"]), "Core 5.2, 7.1")
    b["coverage"] = cov
    b["examined_extent"] = one_of(b.get("examined_extent") or "UNKNOWN", EXTENTS, "examined extent")
    b["note"] = text(b.get("note"), "note", 2000, required=False)
    return b


def _g_session(state, b, actor, store):
    r = state.review(b.get("review"))
    if r["lifecycle"] not in TERMINAL:
        refuse("review %s has an open session (%s)" % (r["label"], r["lifecycle"]), "Core 7.1")
    b["session"] = r["session"] + 1
    b["reason"] = text(b.get("reason"), "the reason for a successor session", 2000)
    return b


def _g_condition(state, b, actor, store):
    b["id"] = state.next_id("C") if b.get("id") in (None, state.next_id("C")) else refuse("condition id out of sequence")
    b["text"] = text(b.get("text"), "the condition, in its original words", 4000)
    b["origin"] = text(b.get("origin"), "where the condition comes from", 500)
    if b.get("review") is not None:
        state.review(b["review"])
    return b


def _g_submission(state, b, actor, store):
    check_label(b.get("label"), "submission label")
    if b["label"] in state.submissions:
        refuse("a submission labelled %r is already admitted" % b["label"])
    one_of(b.get("compatibility"), COMPATIBILITY, "compatibility")
    if b["compatibility"] == "DIFFERENT_METHOD_ASSESSED":
        b["note"] = text(b.get("note"), "the compatibility assessment", 2000)
    return b


def _g_obligation(state, b, actor, store):
    b["id"] = state.next_id("B") if b.get("id") in (None, state.next_id("B")) else refuse("obligation id out of sequence")
    b["text"] = text(b.get("text"), "the obligation", 4000)
    return b


def _g_obligation_end(state, b, actor, store):
    o = state.obligations.get(b.get("id")) if isinstance(b.get("id"), str) else None
    if o is None or o["status"] != "OPEN":
        refuse("no open obligation %r" % (b.get("id"),))
    b["how"] = text(b.get("how"), "how it was discharged", 2000)
    return b


def _g_run(state, b, actor, store):
    b["id"] = state.next_id("R") if b.get("id") in (None, state.next_id("R")) else refuse("run id out of sequence")
    one_of(b.get("trace") or "UNKNOWN", TRACE, "trace")
    return b


def _g_pause(state, b, actor, store):
    r = state.review(b.get("review"), active=True, working=False)
    if machines.step("REVIEW", r["lifecycle"], "PAUSE") is None:
        refuse("review %s is %s and cannot be paused" % (r["label"], r["lifecycle"]))
    b["reason"] = text(b.get("reason"), "the reason for the pause", 2000)
    b["brief"] = _check_basis(state, b.get("brief") or [], "the state brief", store)
    return b


def _g_resume(state, b, actor, store):
    r = state.review(b.get("review"), active=True, working=False)
    if machines.step("REVIEW", r["lifecycle"], "RESUME") is None:
        refuse("review %s is %s; only a paused review is resumed" % (r["label"], r["lifecycle"]))
    b["note"] = text(b.get("note"), "note", 2000, required=False)
    return b


def _g_system(state, b, actor, store):
    event = b.get("event")
    if event == "BOOT":
        b["from"], b["to"] = state.system, "START"
        b["unclean"] = state.system not in (None, "STOPPED")       # the run before this one did not stop in order
    else:
        if state.system is None:
            refuse("no start of the system is recorded; the first SYSTEM_STATE event is BOOT")
        if not isinstance(event, str) or event not in machines.MACHINES["SYSTEM"]["events"]:
            refuse("%r is not an event of the system" % (event,))
        nxt = machines.step("SYSTEM", state.system, event)
        if nxt is None:
            refuse("the system is %s; %s is not accepted in that state" % (state.system, event))
        b["from"], b["to"] = state.system, nxt
    b["reason"] = text(b.get("reason"), "reason", 2000, required=False)
    by = b.get("authorized_by")
    if by is not None and not (isinstance(by, list) and all(isinstance(x, str) for x in by)):
        refuse("authorized_by is a list of participant labels")
    return b


def _g_link(state, b, actor, store):
    if b.get("event") not in ("UP", "DOWN"):
        refuse("a connectivity event is UP or DOWN")
    b["to"] = machines.step("LINK", state.link, b["event"])
    return b


def _g_delivery(state, b, actor, store):
    if actor != "host" and actor not in state.participants:
        refuse("the acting participant %r is not registered" % actor)
    agent = b.get("agent")
    if not isinstance(agent, str) or agent not in state.participants:
        refuse("delivery is recorded for a registered participant")
    event = b.get("event")
    if not isinstance(event, str) or event not in machines.MACHINES["DELIVERY"]["events"]:
        refuse("a delivery event is DELIVER, CONTEXT_RESET or RELEASE_CHANGED")
    if event == "DELIVER":
        if not isinstance(b.get("text"), str) or not HEX64.match(b["text"]):
            refuse("a delivery names what was delivered by its SHA-256")
    elif b.get("text") is not None:
        refuse("only a delivery names a text")
    b["to"] = machines.step("DELIVERY", state.delivery.get(agent, "NOT_DELIVERED"), event)
    return b


def _g_task_start(state, b, actor, store):
    b["id"] = state.next_id("T") if b.get("id") in (None, state.next_id("T")) else refuse("task id out of sequence")
    check_label(b.get("label"), "task label")
    one_of(b.get("worker"), WORKERS, "worker")
    if b.get("review") is not None:
        state.review(b["review"], active=True)
    for k in ("quiet_limit", "budget"):
        if not is_int(b.get(k)) or b[k] < 0:
            refuse("a task records its %s as a whole number, zero or more" % k)
    return b


def _g_task_end(state, b, actor, store):
    t = state.tasks.get(b.get("id")) if isinstance(b.get("id"), str) else None
    if t is None:
        refuse("no task %r was started in this record" % (b.get("id"),))
    if t["outcome"] is not None:
        refuse("task %s has already ended (%s)" % (t["id"], t["outcome"]))
    one_of(b.get("outcome"), TASK_ENDS, "outcome")
    for k in ("result", "read_back"):
        if b.get(k) is not None and not (isinstance(b[k], str) and HEX64.match(b[k])):
            refuse("a result, and what was read back, are each named by SHA-256")
    if b["outcome"] == "DONE":
        if t.get("stop_requested") is not None:
            refuse("a stop was requested for this task: it ends STOPPED, FAILED or LOST, and what it produced afterwards is not its result", clause="Core 7.3")
        a = state.artifacts.get(b.get("result")) if isinstance(b.get("result"), str) else None
        if a is None:
            refuse("a task is DONE only when its result is in the record; none is named, or the one named is not there",
                   hint="the task's own report that it finished is not its result: add the result as an artifact, then end the task with its digest", clause="Core 7.3")
        if b.get("read_back") != b.get("result"):
            refuse("a task is DONE only when the digest read back after it exited equals the digest it reported", clause="Core 7.3")
        if a["origin"] != "PRODUCED":
            refuse("a task's result is something produced in this record, not supplied or retrieved material", clause="Core 7.3")
        if a["last_seq"] <= t["seq"]:
            refuse("the result was recorded before the task started; record what was read back after it exited", clause="Core 7.3")
        if a["last_seq"] in state.consumed:
            refuse("that record of the result already settled another task; record this task's own read-back", clause="Core 7.3")
    if not is_int(b.get("spent")) or b["spent"] < 0:
        refuse("a task records what it spent as a whole number, zero or more")
    b["note"] = text(b.get("note"), "note", 2000, required=False)
    return b


def _at_least(new, old):
    return (STANDARD_METHOD.index(new["method"]) >= STANDARD_METHOD.index(old["method"])
            and STANDARD_MODEL_FINDINGS.index(new["model_findings"]) >= STANDARD_MODEL_FINDINGS.index(old["model_findings"])
            and all(new[k] or not old[k] for k in STANDARD_FLAGS))


def _g_standard(state, b, actor, store):
    one_of(b.get("method"), STANDARD_METHOD, "method")
    one_of(b.get("model_findings"), STANDARD_MODEL_FINDINGS, "model_findings")
    for k in STANDARD_FLAGS:
        if not isinstance(b.get(k), bool):
            refuse("%s is true or false" % k)
    tasks = b.get("tasks")
    if not isinstance(tasks, list) or any(not isinstance(t, str) or not LABEL.match(t) for t in tasks) or len(set(tasks)) != len(tasks):
        refuse("tasks is a list of distinct task labels (empty unless the set of tasks is fixed)")
    if tasks and not b["static_tasks"]:
        refuse("tasks are listed only when the set of tasks is fixed (static_tasks)")
    if not state.human_principal(b.get("by")) or b["by"] != actor:
        refuse("a standard is set by a person directing the work, acting as themselves")
    old = state.standard_now()
    b["relaxes"] = not _at_least(b, old) or (old["static_tasks"] and not set(old["tasks"]) >= set(tasks))
    second = b.get("second")
    if second is not None and (not state.human_principal(second) or second == b["by"]):
        refuse("the second authorization is that of a different person directing the work")
    if b["relaxes"] and old["dual_authorization"] and second is None:
        refuse("the standard in force requires two people to relax it; name the second with their agreement on record")
    return b


def _standard(state, kind, b, actor):
    """What the installation's standard adds to the method's rules. A standard narrows; it never widens."""
    st = state.standard_now()
    if st["method"] == "NATIVE_ONLY":
        if kind == "METHOD_PINNED" and b.get("presence") != "NATIVE":
            refuse("the standard in force admits only the method built into the system release")
        if kind == "REVIEW_COMMISSIONED" and (state.method or {}).get("presence") != "NATIVE":
            refuse("the standard in force admits only the method built into the system release, and another is pinned")
    if st["human_adoption"] and kind == "ADOPTED" and (not state.human_principal(b.get("by")) or b.get("by") != actor):
        refuse("the standard in force lets only a person adopt ACR use, acting as themselves", "Core 15.1")
    if st["offline_only"]:
        if kind == "ARTIFACT_ADDED" and b.get("origin") == "RETRIEVED":
            refuse("the standard in force keeps this system offline; nothing is retrieved")
        if kind == "LINK_STATE" and b.get("event") == "UP":
            refuse("the standard in force keeps this system offline")
    if st["static_tasks"] and kind == "TASK_STARTED" and b.get("label") not in st["tasks"]:
        refuse("the standard in force fixes the set of tasks, and %r is not one of them" % (b.get("label"),))
    if st["dual_authorization"] and kind == "SYSTEM_STATE" and b.get("event") == "MAINTENANCE_BEGIN":
        by = b.get("authorized_by") or []
        if len(by) != 2 or by[0] == by[1] or not all(state.human_principal(x) for x in by):
            refuse("the standard in force requires two different people to authorize maintenance (authorized_by)")
    if st["model_findings"] in ("HUMAN_CLOSES", "MECHANICAL_ONLY"):
        if kind == "REVIEW_CLOSED" and b.get("lifecycle") == "CLOSED_COMPLETE" and state.participants[actor]["kind"] != "HUMAN":
            r = state.reviews[b["review"]]
            if any(state.model_driven(a) for (c, a) in r["current"]):
                refuse("the standard in force has a person close, as complete, a review that holds a model's finding")
    if st["model_findings"] == "MECHANICAL_ONLY" and kind == "FINDING_RECORDED" and state.model_driven(actor):
        claim = state.reviews[b["review"]]["claims"][b["claim"]]
        if b["disposition"] in ("SUPPORTED_IN_SCOPE", "REFUTED_IN_SCOPE") and claim["kind"] != "MECHANICAL_CHECK":
            refuse("the standard in force lets a model record %s only on a claim that is a mechanical check" % b["disposition"],
                   hint="record what the model found as UNRESOLVED or CONDITIONAL with its basis, for a person to decide")


_GUARDS = {"DELIVERY_STATE": _g_delivery, "REVIEW_PAUSED": _g_pause, "REVIEW_RESUMED": _g_resume, "SYSTEM_STATE": _g_system, "LINK_STATE": _g_link,
           "TASK_STARTED": _g_task_start, "TASK_ENDED": _g_task_end, "STANDARD_SET": _g_standard,
           "WORKSPACE_CREATED": _g_created, "METHOD_PINNED": _g_method, "PARTICIPANT_ADDED": _g_participant,
           "ADOPTED": _g_adopt, "HALTED": _g_halt, "ARTIFACT_ADDED": _g_artifact, "TREE_PINNED": _g_tree,
           "ARTIFACT_READ": _g_read, "INSPECTION_DECLARED": _g_inspect, "CHECK_RAN": _g_check,
           "REVIEW_COMMISSIONED": _g_commission, "CLAIM_ADDED": _g_claim_added, "GATE_COMMITTED": _g_gate,
           "OBJECTION_RAISED": _g_objection, "OBJECTION_RESOLVED": _g_objection_end, "FINDING_RECORDED": _g_finding,
           "REVIEW_CLOSED": _g_close, "SESSION_OPENED": _g_session, "CONDITION_REGISTERED": _g_condition,
           "SUBMISSION_ADMITTED": _g_submission, "OBLIGATION_OPENED": _g_obligation, "OBLIGATION_CLOSED": _g_obligation_end,
           "RUN_COMPLETED": _g_run}


# ---- applying events ---------------------------------------------------------------
def apply(state, ev):
    kind, b, actor = ev["kind"], ev["body"], ev["actor"]
    state.seq = ev["seq"]
    if kind == "WORKSPACE_CREATED":
        state.created = True
    elif kind == "METHOD_PINNED":
        if state.method is not None:
            for agent, d in state.delivery.items():
                state.delivery[agent] = machines.step("DELIVERY", d, "RELEASE_CHANGED")
        state.method = dict(b, seq=ev["seq"])
    elif kind == "DELIVERY_STATE":
        state.delivery[b["agent"]] = b["to"]
    elif kind == "PARTICIPANT_ADDED":
        state.participants[b["label"]] = b
    elif kind == "ADOPTED":
        state.adopted = True; state.adoption_log.append(("ADOPTED", ev["seq"], b["by"]))
    elif kind == "HALTED":
        state.adopted = False; state.adoption_log.append(("HALTED", ev["seq"], b["by"]))
        for label in b.get("closed", []):
            r = state.reviews[label]
            r["lifecycle"] = "CLOSED_INCOMPLETE"; r["closed_by"] = "HALT"
            for t in state.tasks.values():      # the halt asks the tasks of each review it closes to stop
                if t["outcome"] is None and t.get("stop_requested") is None and t.get("review") == label:
                    t["stop_requested"] = ev["seq"]
    elif kind == "ARTIFACT_ADDED":
        a = state.artifacts.setdefault(b["sha256"], dict(b, names=[], seq=ev["seq"], actor=actor))
        if b.get("name"):
            state.names[b["name"]] = b["sha256"]
            if b["name"] not in a["names"]:
                a["names"].append(b["name"])
        a["stored"] = a["stored"] or b["stored"]
        if b.get("origin") == "PRODUCED" or "last_seq" not in a:      # as the native guard: a later SUPPLIED record does not move a product
            a["last_seq"] = ev["seq"]
        if b.get("origin") == "PRODUCED":
            a["produced"] = True
    elif kind == "TREE_PINNED":
        state.trees[b["name"]] = dict(b, seq=ev["seq"])
    elif kind == "ARTIFACT_READ":
        state.inspections.setdefault(_ref_key(b["ref"]), []).append("delivered by the record: %s" % b.get("span", "all"))
    elif kind == "INSPECTION_DECLARED":
        state.inspections.setdefault(_ref_key(b["ref"]), []).append(b["extent"])
    elif kind == "CHECK_RAN":
        state.checks[b["id"]] = dict(b, seq=ev["seq"], actor=actor); state.counters["K"] += 1
    elif kind == "REVIEW_COMMISSIONED":
        r = dict(b, lifecycle="ACTIVE", session=1, passed_gates=[], current={}, seq=ev["seq"], history=[])
        r["claims"] = {c["label"]: c for c in b["claims"]}
        r["claim_order"] = [c["label"] for c in b["claims"]]
        state.reviews[b["label"]] = r
    elif kind == "CLAIM_ADDED":
        r = state.reviews[b["review"]]
        r["claims"][b["claim"]["label"]] = b["claim"]; r["claim_order"].append(b["claim"]["label"])
    elif kind == "GATE_COMMITTED":
        state.reviews[b["review"]]["passed_gates"].append(b["gate"])
    elif kind == "OBJECTION_RAISED":
        state.objections[b["id"]] = dict(b, status="OPEN", actor=actor); state.counters["O"] += 1
    elif kind == "OBJECTION_RESOLVED":
        state.objections[b["id"]].update(status=b["resolution"], how=b["how"])
    elif kind == "FINDING_RECORDED":
        state.findings[b["id"]] = dict(b, seq=ev["seq"], actor=actor, ts=ev["ts"]); state.counters["F"] += 1
        state.reviews[b["review"]]["current"][(b["claim"], actor)] = b["id"]
        for it in b["basis"] + b["contrary"]:
            if it.get("extent"):
                state.inspections.setdefault(_ref_key(it["ref"]), []).append(it["extent"])
    elif kind == "REVIEW_CLOSED":
        r = state.reviews[b["review"]]
        r["lifecycle"] = b["lifecycle"]; r["closed_by"] = "CLOSE"; r["coverage"] = b["coverage"]
    elif kind == "SESSION_OPENED":
        r = state.reviews[b["review"]]
        r["history"].append({"session": r["session"], "lifecycle": r["lifecycle"], "passed_gates": r["passed_gates"]})
        r.update(session=b["session"], lifecycle="ACTIVE", passed_gates=[], current={})
    elif kind == "CONDITION_REGISTERED":
        state.conditions[b["id"]] = b; state.counters["C"] += 1
    elif kind == "SUBMISSION_ADMITTED":
        state.submissions[b["label"]] = dict(b, seq=ev["seq"])
    elif kind == "OBLIGATION_OPENED":
        state.obligations[b["id"]] = dict(b, status="OPEN"); state.counters["B"] += 1
    elif kind == "OBLIGATION_CLOSED":
        state.obligations[b["id"]].update(status="CLOSED", how=b["how"])
    elif kind == "RUN_COMPLETED":
        state.runs[b["id"]] = dict(b, seq=ev["seq"]); state.counters["R"] += 1
    elif kind == "REVIEW_PAUSED":
        state.reviews[b["review"]]["lifecycle"] = "PAUSED"
    elif kind == "REVIEW_RESUMED":
        state.reviews[b["review"]]["lifecycle"] = "ACTIVE"
    elif kind == "SYSTEM_STATE":
        state.system = b["to"]
        state.unclean_starts += 1 if b.get("unclean") else 0
        if b.get("event") in ("INTEGRITY_FAULT", "VERIFY_FAILED", "SHUTDOWN", "MAINTENANCE_BEGIN"):
            for t in state.tasks.values():      # these transitions ask every task to stop
                if t["outcome"] is None and t.get("stop_requested") is None:
                    t["stop_requested"] = ev["seq"]
    elif kind == "LINK_STATE":
        state.link = b["to"]
    elif kind == "TASK_STARTED":
        state.tasks[b["id"]] = dict(b, outcome=None, actor=actor, seq=ev["seq"], stop_requested=None); state.counters["T"] += 1
    elif kind == "TASK_ENDED":
        state.tasks[b["id"]].update(outcome=b["outcome"], result=b.get("result"), spent=b["spent"])
        if b["outcome"] == "DONE" and b.get("result") in state.artifacts:
            state.consumed.add(state.artifacts[b["result"]]["last_seq"])
    elif kind == "STANDARD_SET":
        state.standard = {k: b[k] for k in ("method", "model_findings", "tasks") + STANDARD_FLAGS}
    elif kind == "HOST_EVENT":
        pass
    return state


def fold(events, check=False, store=None):
    """Derive the state. With check=True, replay every guard and collect events that would not have passed."""
    state, problems = State(), []
    for ev in events:
        if ev["kind"] not in KINDS:
            if ev["kind"].startswith("X_"):
                state.seq = ev["seq"]; continue                # a declared extension: carried, not interpreted
            raise PcposError(INTEGRITY, "event %d has kind %r, which this program version does not define" % (ev["seq"], ev["kind"]),
                             "the record was written by a different program version; use that version")
        if check:
            try:
                expect = guard(state, ev["kind"], ev["body"], ev["actor"], store)
                if expect != ev["body"]:
                    diff = sorted(k for k in set(expect) | set(ev["body"]) if expect.get(k) != ev["body"].get(k))
                    problems.append((ev["seq"], ev["kind"], "recorded fields differ from what the guards compute: %s" % ", ".join(diff)))
            except PcposError as e:
                problems.append((ev["seq"], ev["kind"], e.message))
        try:
            apply(state, ev)
        except (KeyError, TypeError) as e:
            raise PcposError(INTEGRITY, "event %d (%s) refers to something the record does not hold: %s" % (ev["seq"], ev["kind"], e))
    return (state, problems) if check else state
