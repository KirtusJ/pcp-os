#!/usr/bin/env python3
"""Fault injection against the release verifier. Each case runs on a disposable copy.
Level M: the manifest is recomputed coherently but the verifier's pins are left alone.
Level P: the manifest and the verifier's pins are both recomputed (an actor who rewrites the verifier's pin table)."""
import hashlib, json, re, shutil, subprocess, sys, tempfile, csv
from pathlib import Path
SRC = Path("/home/claude/build/pkg").resolve(); TPL = Path("verify_release.template.py").read_text()
sha = lambda b: hashlib.sha256(b).hexdigest()

def remanifest(root, repin):
    for d in root.rglob("__pycache__"): shutil.rmtree(d)
    (root / "MANIFEST.json").unlink(missing_ok=True)
    files = sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file())
    if repin:
        pins = {f: sha((root / f).read_bytes()) for f in files if f != "TOOLS/verify_release.py"}
        body = "EXPECTED_PINS = {\n" + "".join("    %r: %r,\n" % kv for kv in sorted(pins.items())) + "}"
        (root / "TOOLS/verify_release.py").write_text(TPL.replace("EXPECTED_PINS = {}  # @@PINS@@", body))
    man = {"format": "acr-release-local-integrity/2.0", "payload_count": len(files),
           "files": [{"path": f, "bytes": (root / f).stat().st_size, "sha256": sha((root / f).read_bytes())} for f in files]}
    (root / "MANIFEST.json").write_text(json.dumps(man, indent=2) + "\n")

def rederive(root):
    """Level Q: the actor also regenerates every derived file (descriptor, components, catalog, index, kernel) and the pins."""
    work = root.parent / (root.name + "_assemble"); work.mkdir()
    (work / "pkg").symlink_to(root, target_is_directory=True)
    shutil.copy("assemble.py", work / "assemble.py"); shutil.copy("verify_release.template.py", work / "verify_release.template.py")
    out = subprocess.run([sys.executable, "assemble.py", "a"], cwd=work, capture_output=True, text=True)
    assert out.returncode == 0, out.stderr[-500:]

def sub(root, path, old, new, count=1):
    p = root / path; s = p.read_text(encoding="utf-8"); assert s.count(old) >= 1, (path, old); p.write_text(s.replace(old, new, count), encoding="utf-8")

def m_manifest_raw(root, fn):
    p = root / "MANIFEST.json"; p.write_text(fn(p.read_text()))

def weaken_probe(root):
    p = root / "INTEGRATION/probes.json"; d = json.loads(p.read_text(encoding="utf-8"))
    d["probes"][0]["screens"] = []; p.write_text(json.dumps(d, indent=1, ensure_ascii=False), encoding="utf-8")

def blank_receipt_screen(root):
    p = root / "INTEGRATION/probes.json"; d = json.loads(p.read_text(encoding="utf-8"))
    q = [x for x in d["probes"] if x["id"] == "P26"][0]; q["screens"][0]["pattern"] = "zzzz"
    p.write_text(json.dumps(d, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")

CASES = [
 ("M01 handbook prose altered", "M", lambda r: sub(r, "GUIDES/Safety_Handbook.md", "INFORMATIVE, NOT GOVERNING", "CERTIFIED FOR LIFE-CRITICAL USE"), "FAIL"),
 ("M02 a PDF inserted into the text-only packet", "M", lambda r: (r / "GUIDES/Entry_Card.pdf").write_bytes(b"%PDF-1.4\n%%EOF\n"), "FAIL"),
 ("M03 fabricated LICENSE inserted", "M", lambda r: (r / "LICENSE.md").write_text("Operative licence: all rights granted."), "FAIL"),
 ("M04 file removed", "M", lambda r: (r / "GUIDES/Entry_Card.md").unlink(), "FAIL"),
 ("M05 known limitations say the packet is certified", "M", lambda r: sub(r, "RELEASE/Known_Limitations.md", "not an LLM service, a regulated safety certification", "a regulated safety certification"), "FAIL"),
 ("M06 Core Specification sentence altered", "M", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "MUST NOT ask for a reason", "MAY ask for a reason"), "FAIL"),
 ("P01 handbook prose altered, pins rewritten (documented limit)", "P", lambda r: sub(r, "GUIDES/Safety_Handbook.md", "INFORMATIVE, NOT GOVERNING", "CERTIFIED FOR LIFE-CRITICAL USE"), "PASS"),
 ("P02 catalog says QA is automatically selected", "P", lambda r: sub(r, "REFERENCE/Module_Catalog.csv", "1.0,RELEASED,NOT_SELECTED_BY_DISTRIBUTION,false", "1.0,RELEASED,NOT_SELECTED_BY_DISTRIBUTION,true"), "FAIL"),
 ("P03 catalog version differs from source", "P", lambda r: sub(r, "REFERENCE/Module_Catalog.csv", "acr.quality_assurance,ACR Quality Assurance,SPECIALIST,1.0,", "acr.quality_assurance,ACR Quality Assurance,SPECIALIST,1.1,"), "FAIL"),
 ("P04 Components digest differs from source", "P", lambda r: sub(r, "RELEASE/Components.json", '"source_sha256": "', '"source_sha256": "0', 1), "FAIL"),
 ("P05 clause index row removed", "P", lambda r: (r / "REFERENCE/Clause_Index.csv").write_bytes(b"\r\n".join((r / "REFERENCE/Clause_Index.csv").read_bytes().split(b"\r\n")[:100] + (r / "REFERENCE/Clause_Index.csv").read_bytes().split(b"\r\n")[101:])), "FAIL"),
 ("P06 Core Specification altered without new descriptor", "P", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "MUST NOT ask for a reason", "MAY ask for a reason"), "FAIL"),
 ("P07 module pin to Core altered", "P", lambda r: sub(r, "MODULES/03_Science/SOURCE/Specification.md", "Core descriptor SHA-256 `", "Core descriptor SHA-256 `0"), "FAIL"),
 ("P08 kernel floor text altered", "P", lambda r: sub(r, "KERNEL.md", "MUST NOT ask for a reason", "MAY ask for a reason"), "FAIL"),
 ("P09 skill kernel replica altered", "P", lambda r: sub(r, "INTEGRATION/agent-skill/acr/KERNEL.md", "A halt is not a physical", "A halt is a physical"), "FAIL"),
 ("P10 persona permitted in defined terms", "P", lambda r: sub(r, "REFERENCE/Defined_Terms.csv", "Recorder,duty,\"Maintains identities, lineage, current records and preserved history\",3.1,NO", "Recorder,duty,\"Maintains identities, lineage, current records and preserved history\",3.1,YES"), "FAIL"),
 ("P11 release status says public domain", "P", lambda r: sub(r, "RELEASE/Release_Status.json", "PROPRIETARY_ALL_RIGHTS_RESERVED", "PUBLIC_DOMAIN"), "FAIL"),
 ("P12 release status claims behavioral validation", "P", lambda r: sub(r, "RELEASE/Release_Status.json", '"NOT_PERFORMED"', '"PERFORMED"'), "FAIL"),
 ("P13 kernel card cites a missing section", "P", lambda r: sub(r, "KERNEL_CARD.md", "(15.6)", "(15.9)"), "FAIL"),
 ("P14 probe screens removed so a bad reply passes", "P", weaken_probe, "FAIL"),
 ("P15 load tiers edited", "P", lambda r: sub(r, "REFERENCE/Load_Tiers.csv", "T1,Kernel,1,", "T1,Kernel,1,9"), "FAIL"),
 ("P16 crosswalk row removed", "P", lambda r: (r / "REFERENCE/Path_Crosswalk.csv").write_text("\n".join((r / "REFERENCE/Path_Crosswalk.csv").read_text().split("\n")[:-2]) + "\n"), "FAIL"),
 ("P17 fabricated LICENSE inserted, pins rewritten (documented limit)", "P", lambda r: (r / "LICENSE.md").write_text("Operative licence: all rights granted."), "PASS"),
 ("P18 conditional contract identity differs from Core", "P", lambda r: sub(r, "SUPPLEMENTS/Core_Conditional/Methods_Catalog.md", "Conditional attachment to `acr.core`, Version 1.0", "Conditional attachment to `acr.core`, Version 1.1"), "FAIL"),
 ("P19 guide cites a Core section that does not exist", "P", lambda r: sub(r, "GUIDES/Safety_Handbook.md", "Core \u00a7\u00a72.1, 15.1.", "Core \u00a7\u00a72.1, 15.11."), "FAIL"),
 ("P20 skill name differs from its folder", "P", lambda r: sub(r, "INTEGRATION/agent-skill/acr/SKILL.md", "name: acr", "name: ACR_Skill"), "FAIL"),
 ("P21 host wrapper permission default flipped to allow", "P", lambda r: sub(r, "TOOLS/acr_host.py", "self.allow = allow or (lambda name, arguments: False)", "self.allow = allow or (lambda name, arguments: True)"), "FAIL"),
 ("P22 command parser made case-insensitive", "P", lambda r: sub(r, "TOOLS/command_interface.py", "recognized = text.startswith(PREFIX) and", "recognized = text.lower().startswith(PREFIX) and"), "FAIL"),
 ("Q01 metaphorical role name reintroduced, everything re-derived", "Q", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "| Recorder | Maintains identities", "| Scribe | Maintains identities"), "FAIL"),
 ("Q02 duty renamed in Core but not in Defined_Terms, everything re-derived", "Q", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "| Monitor | Detects declared changes", "| Watcher | Detects declared changes"), "FAIL"),
 ("Q03 revision tag reintroduced in Core, everything re-derived", "Q", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "**Date:** 13 November 2026", "**Revision R15 drafted by:** nobody  \n**Date:** 13 November 2026"), "FAIL"),
 ("Q04 metaphorical term in a guide, everything re-derived", "Q", lambda r: sub(r, "GUIDES/Safety_Handbook.md", "INFORMATIVE, NOT GOVERNING", "INFORMATIVE. The Council and its Parrots are NOT GOVERNING"), "FAIL"),
 ("Q06 old record field name reintroduced, everything re-derived", "Q", lambda r: sub(r, "SUPPLEMENTS/Core_Conditional/Record_Contracts.md", "| review_ref; session_ref;", "| council_ref; session_ref;"), "FAIL"),
 ("Q07 excluded two-word term in a guide, everything re-derived", "Q", lambda r: sub(r, "GUIDES/Safety_Handbook.md", "INFORMATIVE, NOT GOVERNING", "INFORMATIVE (a storm drill is NOT GOVERNING)"), "FAIL"),
 ("P26 excluded word inside a Python test string, pins rewritten", "P", lambda r: sub(r, "TOOLS/test_command_interface.py", '"-acr continue"', '"-acr autopilot"'), "FAIL"),
 ("P27 a PDF inserted, pins rewritten", "P", lambda r: (r / "GUIDES/Entry_Card.pdf").write_bytes(b"%PDF-1.4\n%%EOF\n"), "FAIL"),
 ("P28 product name reintroduced in a guide, pins rewritten", "P", lambda r: sub(r, "GUIDES/Safety_Handbook.md", "INFORMATIVE, NOT GOVERNING", "INFORMATIVE, NOT GOVERNING (PCP edition)"), "FAIL"),
 ("Q08 old command prefix reintroduced in the Core, everything re-derived", "Q", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "`-acr halt`", "`-pcp halt`"), "FAIL"),
 ("Q05 harmless Core edit, everything re-derived (control: accepted by design)", "Q", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "Templates and utilities do not override requirements.", "Templates and utilities do not override requirements, ever."), "PASS"),
 ("P23 revision tag reintroduced in a module", "P", lambda r: sub(r, "MODULES/02_Mathematics/SOURCE/Overview.md", "**Date:** 11 November 2026", "**Release:** PCPM-2026-11-11-R7  \n**Date:** 11 November 2026"), "FAIL"),
 ("P24 metaphorical role name reintroduced", "P", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "| Recorder | Maintains identities", "| Scribe | Maintains identities"), "FAIL"),
 ("P25 duty renamed in Core but not in Defined_Terms", "P", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "| Monitor | Detects declared changes", "| Watcher | Detects declared changes"), "FAIL"),
 ("P29 host wrapper halt no longer cancels the in-flight request, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr_host.py", "ok = bool(self.model_cancel())", "ok = True"), "FAIL"),
 ("P30 claim-substitution sentence removed from the kernel only, pins rewritten", "P", lambda r: sub(r, "KERNEL.md", "never the substitute's finding in its place", "or the substitute's finding in its place"), "FAIL"),
 ("P31 notice removed, pins rewritten", "P", lambda r: (r / "NOTICE.md").unlink(), "FAIL"),
 ("P32 notice rewritten as a permissive grant, pins rewritten", "P", lambda r: sub(r, "NOTICE.md", "Holding a copy grants nothing.", "Holding a copy grants a free license."), "FAIL"),
 ("P33 evidence status weakened in the limits file, pins rewritten", "P", lambda r: sub(r, "RELEASE/Known_Limitations.md", "**Not peer reviewed.**", "**Peer reviewed.**"), "FAIL"),
 ("Q09 Core 1.1 evidence rule reversed, everything re-derived", "Q", lambda r: sub(r, "MODULES/01_Core/SOURCE/Specification.md", "A participant MUST NOT describe ACR as validated, proven, certified or peer reviewed.", "A participant MAY describe ACR as validated."), "FAIL"),
 ("P40 evidence pointer removed from the start document, pins rewritten", "P", lambda r: sub(r, "START_HERE.md", "read `RELEASE/Known_Limitations.md` before relying on a result", "rely on it"), "FAIL"),
 ("P41 notice no longer says the packet is supplied only under license, pins rewritten", "P", lambda r: sub(r, "NOTICE.md", "It is supplied only under license", "It may be used freely"), "FAIL"),
 ("P42 trace sheet fills in a difference by itself, pins rewritten", "P", lambda r: sub(r, "TOOLS/probe_runner.py", '{row: "PENDING" for row in TRACE_ROWS}', '{row: "NONE_FOUND" for row in TRACE_ROWS}'), "FAIL"),
 ("P43 receipt sentence removed from the kernel only, pins rewritten", "P", lambda r: sub(r, "KERNEL.md", ", and it does not put ACR in use", ""), "FAIL"),
 ("P44 receipt probe screen for an overstated acknowledgment removed, pins rewritten", "P", blank_receipt_screen, "FAIL"),
 ("P45 arm file path written into the report, pins rewritten", "P", lambda r: sub(r, "TOOLS/probe_runner.py", '"arm_text_sha256": _sha(system.encode("utf-8")) if system else None,', '"arm_text_sha256": _sha(system.encode("utf-8")) if system else None, "arm_file": getattr(args, "arm_file", None),'), "FAIL"),
 ("P34 release status claims peer review, pins rewritten", "P", lambda r: sub(r, "RELEASE/Release_Status.json", '"peer_review": "NOT_PERFORMED"', '"peer_review": "PERFORMED"'), "FAIL"),
 ("P35 fidelity checker accepts a reasoning trace as proof of processing, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr.py", 'c.get("basis") == "REASONING_TRACE" and c.get("asserted") is True', 'c.get("basis") == "REASONING_TRACE" and False'), "FAIL"),
 ("P36 kernel sentence on reasoning traces altered, pins rewritten", "P", lambda r: sub(r, "KERNEL.md", "is an output of the participant, not a view of its processing", "is a faithful view of its processing"), "FAIL"),
 ("P37 host no longer reports a tool that is inside its call when a halt arrives, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr_host.py", 'if e["status"] == "IN_CALL":', 'if e["status"] == "NEVER":'), "FAIL"),
 ("P38 probe report drops the request fields it sent, pins rewritten", "P", lambda r: sub(r, "TOOLS/probe_runner.py", '"request_fields_sent": request,', '"request_fields_sent": {},'), "FAIL"),
 ("P39 coined-code screen disabled, pins rewritten", "P", lambda r: sub(r, "TOOLS/probe_runner.py", "return not [c for c in _CODE.findall(reply) if c not in allowed]", "return True"), "FAIL"),
 ("P46 lookup tool serves the first of two folders that declare one identifier, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr.py", "twice[key] = [out.pop(key)[1], rel]", "pass"), "FAIL"),
 ("P47 lookup tool cuts a version to its first two numbers, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr.py", "Version (\\S+)", "Version (\\d+\\.\\d+)"), "FAIL"),
 ("P48 a supported Claim of onboarding is no longer asked what the measurement changed, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr.py", "            if not (isinstance(confinement, str) and confinement.strip()):\n                issues.append(\"claim: SUPPORTED_IN_SCOPE needs configuration.confinement stated", "            if False:\n                issues.append(\"claim: SUPPORTED_IN_SCOPE needs configuration.confinement stated"), "FAIL"),
 ("P49 the criteria are handed out before every reply is recorded, pins rewritten", "P", lambda r: sub(r, "TOOLS/calibrate.py", 'if any(len(s["replies"].get(pid, [])) < len(by[pid]["turns"]) for pid in s["order"]):', "if False:"), "FAIL"),
 ("P50 a turn given to the participant carries the item's identifier, pins rewritten", "P", lambda r: sub(r, "TOOLS/calibrate.py", '"new_conversation": i == 0, "text": by[pid]["turns"][i]}', '"new_conversation": i == 0, "text": by[pid]["turns"][i], "id": pid}'), "FAIL"),
 ("P51 a second reading may cover more runs than were read, pins rewritten", "P", lambda r: sub(r, "TOOLS/acr.py", 'elif nat(a.get("runs")) and second["runs"] > a["runs"]:', "elif False:"), "FAIL"),
 ("R01 manifest format string changed", "RAW", lambda r: m_manifest_raw(r, lambda s: s.replace("integrity/2.0", "integrity/9.9")), "FAIL"),
 ("R02 manifest byte count as float", "RAW", lambda r: m_manifest_raw(r, lambda s: re.sub(r'"bytes": (\d+)', r'"bytes": \1.0', s, count=1)), "FAIL"),
 ("R03 duplicate manifest key", "RAW", lambda r: m_manifest_raw(r, lambda s: s.replace('"payload_count"', '"format": "acr-release-local-integrity/2.0",\n  "payload_count"', 1)), "FAIL"),
 ("R04 undeclared extra file", "RAW", lambda r: (r / "notes.txt").write_text("x"), "FAIL"),
]

def run(root, flag):
    out = subprocess.run([sys.executable, "-B"] + flag + [str(root / "TOOLS/verify_release.py")], capture_output=True, text=True)
    return "PASS" if out.returncode == 0 and out.stdout.startswith("PASS") else "FAIL", out.stdout.strip().split("\n")[0][:150]

ONLY = sys.argv[1:]
if ONLY: CASES = [c for c in CASES if c[0].split()[0] in ONLY]
results = []
with tempfile.TemporaryDirectory() as td:
    ctl = Path(td) / "control"; shutil.copytree(SRC, ctl)
    for flag in ([], ["-OO"]):
        results.append({"case": "C00 intact control", "mode": "optimized" if flag else "normal", "expected": "PASS", "observed": run(ctl, flag)[0]})
    for name, level, fn, expected in CASES:
        root = Path(td) / name.split()[0]; shutil.copytree(SRC, root)
        for d in root.rglob("__pycache__"): shutil.rmtree(d)
        fn(root)
        if level == "Q": rederive(root)
        if level in ("M", "P", "Q"): remanifest(root, level != "M")
        for flag in ([], ["-OO"]):
            got, line = run(root, flag)
            results.append({"case": name, "level": level, "mode": "optimized" if flag else "normal", "expected": expected, "observed": got, "verifier_line": line})
bad = [r for r in results if r["expected"] != r["observed"]]
Path("mutation_results_partial.json" if ONLY else "mutation_results.json").write_text(json.dumps(results, indent=1))
print(len(results), "runs;", len(bad), "unexpected")
for r in bad: print("  UNEXPECTED", r)
for r in results[2::2]: print(" ", r["observed"], "|", r["case"], "|", r.get("verifier_line", "")[:80])
