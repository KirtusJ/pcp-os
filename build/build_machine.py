#!/usr/bin/env python3
"""Write the machine form of the release: one JSON document a program can load, and the schemas for the
records a program writes or reads.

    python3 build_machine.py <method packet dir> <PCP OS Rust workspace> <design dir> <out dir>

Nothing here restates a rule. Every provision is carried as an address (module, number, title) with the
SHA-256 of its exact text and its position in the Specification, so a program fetches the governing words
from the pinned file and can tell whether they changed. Standard library only.
"""
import csv, hashlib, json, re, sys
from pathlib import Path

METHOD, RUST, DESIGN, OUT = (Path(a) for a in sys.argv[1:5])
sha = lambda b: hashlib.sha256(b).hexdigest()
KEYWORDS = ("MUST NOT", "MUST", "SHALL NOT", "SHALL", "SHOULD NOT", "SHOULD", "REQUIRED", "RECOMMENDED", "MAY", "OPTIONAL")   # BCP 14 (RFC 2119, RFC 8174): capitals only

def keyword_counts(text):
    out, rest = {}, text
    for k in KEYWORDS:      # longest first, so MUST NOT is not also counted as MUST
        n = len(re.findall(r"\b%s\b" % k, rest))
        if n:
            out[k] = n
            rest = re.sub(r"\b%s\b" % k, " ", rest)
    return out

def provisions(spec_text):
    """Numbered headings and numbered paragraphs, each with the digest of its text up to the next one."""
    lines = spec_text.split("\n")
    marks = []
    for i, line in enumerate(lines):
        m = re.match(r"^(#{2,3}) (?:(\d+(?:\.\d+)*)\.? |Appendix ([A-Z])\.? )(.*)$", line)
        if m:
            marks.append((i, m.group(2) or "Appendix " + m.group(3), m.group(4).strip(), "HEADING", len(m.group(1))))
            continue
        m = re.match(r"^(\d+\.\d+)\. (.*)$", line)
        if m:
            marks.append((i, m.group(1), " ".join(m.group(2).split()[:12]), "NUMBERED_REQUIREMENT", 9))
    out = []
    for n, (i, num, title, kind, depth) in enumerate(marks):
        # a heading's text runs to the next heading of the same or a higher level; a numbered paragraph's to the next mark
        end = len(lines)
        for j, _, _, k2, d2 in marks[n + 1:]:
            if kind == "NUMBERED_REQUIREMENT" or (k2 == "HEADING" and d2 <= depth):
                end = j
                break
        body = "\n".join(lines[i:end]).strip() + "\n"
        out.append({"number": num, "title": title, "kind": kind, "first_line": i + 1, "last_line": end, "words": len(body.split()),
                    "text_sha256": sha(body.encode()), "keywords": keyword_counts(body)})
    return out

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "schemas").mkdir(exist_ok=True)
    manifest_raw = (METHOD / "MANIFEST.json").read_bytes()
    listed = {f["path"]: f["sha256"] for f in json.loads(manifest_raw)["files"]}
    modules = []
    for d in sorted((METHOD / "MODULES").iterdir()):
        spec = d / "SOURCE" / "Specification.md"
        if not spec.is_file():
            continue
        raw = spec.read_bytes(); text = raw.decode()
        m = re.search(r"^\*\*Component:\*\* `(acr\.[a-z_]+)`, Version ([0-9.]+[0-9])", text, re.M)
        if not m:
            continue
        rel = spec.relative_to(METHOD).as_posix(); ov = (d / "SOURCE" / "Overview.md")
        assert listed[rel] == sha(raw), rel
        req = re.search(r"^\*\*Required core:\*\* `(acr\.core)`, Version ([0-9.]+[0-9])", text, re.M)
        modules.append({"key": m.group(1)[4:], "id": m.group(1), "version": m.group(2), "role": "CORE" if m.group(1) == "acr.core" else "SPECIALIST",
                        "requires": [] if m.group(1) == "acr.core" else [{"id": req.group(1), "version": req.group(2)}] if req else [],
                        "selected_by": "ALWAYS_WHEN_ACR_IS_IN_USE" if m.group(1) == "acr.core" else "THE_PERSON_OR_A_POLICY_THEY_SET",
                        "specification": {"path": rel, "sha256": sha(raw), "bytes": len(raw), "words": len(text.split()), "keywords": keyword_counts(text)},
                        "overview": {"path": ov.relative_to(METHOD).as_posix(), "sha256": sha(ov.read_bytes()), "words": len(ov.read_text().split())},
                        "provisions": provisions(text)})
    terms = list(csv.DictReader((METHOD / "REFERENCE/Defined_Terms.csv").open()))
    tiers = list(csv.DictReader((METHOD / "REFERENCE/Load_Tiers.csv").open()))
    machines = {}
    for line in (RUST / "vectors/machines.tsv").read_text().splitlines():
        if line.startswith("#") or not line.strip():
            continue
        f = line.split("\t")
        mm = machines.setdefault(f[0], {"initial": None, "final": [], "transitions": []})
        if f[1] == "INITIAL":
            mm["initial"] = f[2]; mm["final"] = f[4:] if len(f) > 4 else []
        else:
            mm["transitions"].append({"state": f[1], "event": f[2], "next": None if f[3] == "-" else f[3]})
    acr_rs = (RUST / "crates/pcpos-core/src/acr.rs").read_text()
    kinds = {}
    for name, cls in (("STRUCTURED", "REVIEW_STEP"), ("HOST_KINDS", "HOST"), ("DIRECTION_KINDS", "DIRECTION"), ("RECORD_KINDS", "RECORD")):
        body = re.search(r"const %s: \[&str; \d+\] = \[(.*?)\];" % name, acr_rs, re.S).group(1)
        for k in re.findall(r'"([A-Z_]+)"', body):
            kinds[k] = cls
    kinds["TASK_STARTED"] = "TASK"; kinds["HALTED"] = "HALT"
    codes = {n: int(v) for n, v in re.findall(r"pub const ([A-Z_]+): u8 = (\d+);", acr_rs)}
    trace = [dict(zip(("guard", "clause", "what"), l.split("\t")[:3])) for l in (RUST / "docs/Guard_Clause_Trace.tsv").read_text().splitlines()[1:] if l.strip()]
    doc = {
        "format": "pcp-machine-form/1",
        "about": "Parrot Council Protocol (PCP) 1.0, the method Adversarial Claim Review (ACR) 1.0, as data. Addresses and digests, not restated rules: the Markdown Specifications govern.",
        "method_release": {"manifest_sha256": sha(manifest_raw), "files": len(listed), "kernel_sha256": listed["KERNEL.md"], "kernel_card_sha256": listed["KERNEL_CARD.md"]},
        "normative_language": {"standard": "BCP 14 (RFC 2119, RFC 8174): the key words bind only in capitals", "keywords": list(KEYWORDS),
                               "limit": "Five Specifications number headings only, and many requirement sentences carry no key word. A provision here is therefore an address with a digest, not a requirement record with a level."},
        "modules": modules, "defined_terms": terms, "load_tiers": tiers,
        "register": {"record_form": "ACR-OS-RECORD-1", "event_kinds": dict(sorted(kinds.items())), "exit_codes": codes, "state_machines": machines, "guard_clause_trace": trace},
        "design": {"tokens": "pcp.tokens.json (Design Tokens Community Group format)", "tokens_sha256": sha((DESIGN / "pcp.tokens.json").read_bytes())},
    }
    (OUT / "pcp.machine.json").write_text(json.dumps(doc, indent=1) + "\n")
    hex64 = {"type": "string", "pattern": "^[0-9a-f]{64}$"}
    (OUT / "schemas/register-event.schema.json").write_text(json.dumps({
        "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": "pcp:schema:register-event:1", "title": "One line of a PCP OS register",
        "description": "The envelope of every event. The body's rules are the guards', which depend on the record so far and cannot be stated as a schema: `pcpos verify` replays them.",
        "type": "object", "required": ["seq", "ts", "actor", "auth", "kind", "prev", "body"], "additionalProperties": False,
        "properties": {"seq": {"type": "integer", "minimum": 0}, "ts": {"type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$"},
                       "actor": {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"}, "auth": {"enum": ["HOST", "DECLARED"]},
                       "kind": {"anyOf": [{"enum": sorted(kinds)}, {"type": "string", "pattern": "^X_[A-Z0-9_]+$"}]}, "prev": hex64, "body": {"type": "object"}}}, indent=1) + "\n")
    (OUT / "schemas/edition-profile.schema.json").write_text(json.dumps({
        "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": "pcp:schema:edition-profile:1", "title": "edition/EDITION.json: what an edition is, for a program",
        "type": "object", "required": ["format", "product", "method", "version", "build", "edition", "licensed_to", "price", "people", "method_release", "theme"],
        "properties": {"format": {"const": "pcp-edition-profile/1"}, "product": {"const": "Parrot Council Protocol"}, "method": {"const": "Adversarial Claim Review"},
                       "version": {"type": "string"}, "build": {"type": "integer"}, "edition": {"enum": ["PERSONAL", "COMMERCIAL", "ENTERPRISE"]}, "creator": {"const": "Kirtus Justus"},
                       "price": {"type": "object", "required": ["basis"], "properties": {"basis": {"enum": ["SINGLE_PURCHASE", "INDIVIDUALLY_NEGOTIATED_WRITTEN_AGREEMENT"]}}},
                       "people": {"type": "object", "required": ["second_person_available", "organization_floor"]},
                       "method_release": {"type": "object", "required": ["manifest_sha256"], "properties": {"manifest_sha256": hex64, "kernel_sha256": hex64}},
                       "theme": {"type": "object", "patternProperties": {".*": {"type": "string", "pattern": "^#[0-9A-F]{6}$"}}}}}, indent=1) + "\n")
    print(json.dumps({"modules": [(m["key"], len(m["provisions"]), m["specification"]["keywords"]) for m in modules], "terms": len(terms), "machines": {k: len(v["transitions"]) for k, v in machines.items()},
                      "event_kinds": len(kinds), "trace_rows": len(trace), "bytes": (OUT / "pcp.machine.json").stat().st_size}, indent=1))

main()
