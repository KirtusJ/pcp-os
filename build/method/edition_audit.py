#!/usr/bin/env python3
"""Publisher-side audit across the three edition archives, each from a clean extraction."""
import hashlib, json, re, shutil, subprocess, sys, tempfile, zipfile
from pathlib import Path
import pymupdf
OUT = Path("/home/claude/build/out"); sha = lambda b: hashlib.sha256(b).hexdigest()
ED = {"PERSONAL": "Personal", "COMMERCIAL": "Commercial", "ENTERPRISE": "Enterprise"}
res = {"checks": [], "editions": {}}
def ok(name, cond, detail=""):
    res["checks"].append({"check": name, "result": "PASS" if cond else "FAIL", "detail": detail}); print(("PASS " if cond else "FAIL ") + name, detail)
def run(root, *flags):
    o = subprocess.run([sys.executable, "-B", *flags, str(root / "verify_edition.py")], capture_output=True, text=True)
    return o.returncode == 0 and o.stdout.startswith("PASS"), o.stdout.strip().split("\n")[0][:150]
td = Path(tempfile.mkdtemp()); roots = {}; body = {}; impl = {}
for E, s in ED.items():
    z = OUT / ("PCP_1.0_%s_Edition.zip" % s); d = td / E; zipfile.ZipFile(z).extractall(d); root = d / ("PCP_1.0_%s_Edition" % s); roots[E] = root
    top = sorted(p.name for p in root.iterdir())
    ok("%s: top-level inventory" % E, top == sorted(["ACR_1.0_Implementation.zip", "PCP_1.0_Readers.zip", "EDITION.json", "EDITION_GUIDE.md", "EVIDENCE_STATUS.md", "LICENSE.md", "MANIFEST.json", "NOTICE.md", "PRICING.md", "README.md", "verify_edition.py"]), str(len(top)))
    a, la = run(root); b, lb = run(root, "-OO"); ok("%s: verify_edition, normal and optimized" % E, a and b, la)
    impl[E] = sha((root / "ACR_1.0_Implementation.zip").read_bytes())
    r = d / "r"; zipfile.ZipFile(root / "PCP_1.0_Readers.zip").extractall(r); r = r / "PCP_1.0_Readers"
    pdfs = sorted(r.rglob("*.pdf")); bad = []; others = [o.upper() + " EDITION" for k, o in ED.items() if k != E]; mine = s.upper() + " EDITION"
    body[E] = {}
    for p in pdfs:
        doc = pymupdf.open(p); first = doc[0].get_text(); rel = p.relative_to(r).as_posix()
        if mine not in first.upper() or any(o in first.upper() for o in others): bad.append(rel + ":cover")
        if rel.startswith("MODULES/") != ("Evidence status: untested and not peer reviewed" in first): bad.append(rel + ":evidence")
        m = doc.metadata
        if m.get("author") != "Kirtus Justus" or (s + " Edition") not in (m.get("title") or ""): bad.append(rel + ":meta")
        skip = 2 if rel.startswith("MODULES/") else 0
        txt = "".join(pg.get_text("text", clip=pymupdf.Rect(0, 60, pg.rect.width, pg.rect.height - 55)) for i, pg in enumerate(doc) if i >= skip)
        body[E][rel] = sha(re.sub(r"\s+", "", txt).replace(s.upper() + "EDITION", "").encode())
    ok("%s: 23 PDFs carry this edition and the author; the 14 module covers, and only they, carry the evidence line" % E, len(pdfs) == 23 and not bad, ",".join(bad[:4]))
    texts = {p.relative_to(d).as_posix(): p.read_text(encoding="utf-8") for p in list(root.glob("*.md")) + list(r.rglob("*.md"))}
    lic_other = [f for f, t in texts.items() for k, o in ED.items() if k != E and ("| %s Edition License" % o) in t]
    ok("%s: no other edition's license text" % E, not lic_other, ",".join(lic_other))
    ed = json.loads((root / "EDITION.json").read_text()); rm = json.loads((r / "MANIFEST.json").read_text())
    ok("%s: identity records agree" % E, ed["edition"] == E == rm["edition"] and ed["creator"] == "Kirtus Justus")
    res["editions"][E] = {"archive": z.name, "sha256": sha(z.read_bytes()), "edition_fingerprint": sha((root / "MANIFEST.json").read_bytes()),
                          "readers_sha256": ed["packets"]["readers"]["sha256"], "readers_fingerprint": ed["packets"]["readers"]["manifest_sha256"],
                          "pdf_pages": sum(pymupdf.open(p).page_count for p in pdfs)}
ok("Implementation packet byte-identical in all three editions", len(set(impl.values())) == 1, list(impl.values())[0])
res["implementation_sha256"] = list(impl.values())[0]
common = [k for k in body["PERSONAL"] if k != "EDITION/Edition_Guide.pdf"]
diff = [k for k in common if not (body["PERSONAL"][k] == body["COMMERCIAL"][k] == body["ENTERPRISE"][k])]
ok("Body text of the 22 shared reading copies identical across editions", not diff, ",".join(diff[:3]))
# Implementation: no product name, no edition marker, no excluded vocabulary
i = td / "impl"; zipfile.ZipFile(roots["PERSONAL"] / "ACR_1.0_Implementation.zip").extractall(i)
low = "\n".join(p.read_text(encoding="utf-8", errors="replace").lower() for p in i.rglob("*") if p.is_file())
ok("Implementation: no product name and no edition marker", "pcp" not in re.findall(r"[a-z]+", low) and "parrot" not in low and not any((s.lower() + " edition") in low for s in ED.values()))
g = subprocess.run([sys.executable, "vocabulary_gate.py", "scan", str(i)], capture_output=True, text=True, cwd="/home/claude/build")
ok("Implementation: vocabulary gate", g.stdout.strip().endswith("0 occurrences"), g.stdout.strip().split("\n")[-1])
# Edition-level fault injection (each on a disposable copy of the Personal edition unless stated)
def remanifest(root):
    files = sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p.name != "MANIFEST.json" and "__pycache__" not in p.parts)
    m = json.loads((root / "MANIFEST.json").read_text()); m["payload_count"] = len(files)
    m["files"] = [{"path": f, "bytes": (root / f).stat().st_size, "sha256": sha((root / f).read_bytes())} for f in files]
    (root / "MANIFEST.json").write_text(json.dumps(m, indent=2) + "\n")
def sub(root, f, a, b):
    p = root / f; t = p.read_text(); assert a in t, (f, a); p.write_text(t.replace(a, b, 1))
def swap_readers(root): shutil.copy(roots["ENTERPRISE"] / "PCP_1.0_Readers.zip", root / "PCP_1.0_Readers.zip")
def swap_readers_repin(root):
    swap_readers(root); e = json.loads((root / "EDITION.json").read_text()); o = json.loads((roots["ENTERPRISE"] / "EDITION.json").read_text())
    e["packets"]["readers"] = o["packets"]["readers"]; (root / "EDITION.json").write_text(json.dumps(e, indent=2) + "\n")
def price_json(root):
    e = json.loads((root / "EDITION.json").read_text()); e["price"]["amount"] = "0.00"; (root / "EDITION.json").write_text(json.dumps(e, indent=2) + "\n")
CASES = [
 ("E01 price changed in the schedule", lambda r: sub(r, "PRICING.md", "**US$5.00**", "**US$4.00**"), False, "FAIL"),
 ("E02 price changed, manifest recomputed", lambda r: sub(r, "PRICING.md", "**US$5.00**", "**US$4.00**"), True, "FAIL"),
 ("E03 Personal made a subscription, manifest recomputed", lambda r: sub(r, "PRICING.md", "Not a subscription: no renewal and no recurring charge.", "Billed monthly."), True, "FAIL"),
 ("E04 a Commercial list price published, manifest recomputed", lambda r: sub(r, "PRICING.md", "| Commercial | No published price |", "| Commercial | US$500 |"), True, "FAIL"),
 ("E05 notice rewritten as open source, manifest recomputed", lambda r: sub(r, "NOTICE.md", "**Proprietary material. All rights reserved.**", "**Released under the MIT License.**"), True, "FAIL"),
 ("E06 evidence statement weakened, manifest recomputed", lambda r: sub(r, "EVIDENCE_STATUS.md", "**Not peer reviewed.**", "**Peer reviewed.**"), True, "FAIL"),
 ("E07 license replaced by another edition's, manifest recomputed", lambda r: shutil.copy(roots["COMMERCIAL"] / "LICENSE.md", r / "LICENSE.md"), True, "FAIL"),
 ("E08 Readers packet of another edition substituted", swap_readers, False, "FAIL"),
 ("E09 Readers packet of another edition substituted, identity and manifest recomputed", swap_readers_repin, True, "FAIL"),
 ("E10 price record altered in EDITION.json, manifest recomputed", price_json, True, "FAIL"),
 ("E11 extra file added", lambda r: (r / "FREE_LICENSE.md").write_text("All rights granted."), False, "FAIL"),
 ("E12 creator changed, manifest recomputed", lambda r: sub(r, "EDITION.json", '"creator": "Kirtus Justus"', '"creator": "Someone Else"'), True, "FAIL"),
 ("E13 guide edited without regenerating its reading copy, manifest recomputed", lambda r: sub(r, "EDITION_GUIDE.md", "**INFORMATIVE.**", "**BINDING.**"), True, "FAIL"),
 ("E15 license model altered in EDITION.json, manifest recomputed", lambda r: sub(r, "EDITION.json", '"license_model": "LICENSED_NOT_SOLD"', '"license_model": "PUBLIC_DOMAIN"'), True, "FAIL"),
 ("E16 notice no longer says the product is licensed, manifest recomputed", lambda r: sub(r, "NOTICE.md", "PCP is licensed, not sold.", "PCP may be used freely."), True, "FAIL"),
 ("E17 license opening rewritten as a grant of all rights, manifest recomputed", lambda r: sub(r, "LICENSE.md", "The Materials are proprietary. They are licensed, not sold", "The Materials are free to use"), True, "FAIL"),
 ("E14 harmless readme wording, manifest recomputed (control: accepted by design)", lambda r: sub(r, "README.md", "## Order of reading", "## Reading order"), True, "PASS"),
]
fi = []
for name, fn, reman, expected in CASES:
    c = td / ("m_" + name.split()[0]); shutil.copytree(roots["PERSONAL"], c); fn(c)
    if reman: remanifest(c)
    got, line = run(c, "--quick") if False else run(c)
    fi.append({"case": name, "expected": expected, "observed": "PASS" if got else "FAIL", "line": line})
bad = [f for f in fi if f["expected"] != f["observed"]]
ok("Edition fault injection: %d cases as expected" % len(fi), not bad, str(bad[:2]))
res["fault_injection"] = fi
Path("/home/claude/build/edition_audit_results.json").write_text(json.dumps(res, indent=1) + "\n")
shutil.rmtree(td)
print("TOTAL", len(res["checks"]), "checks,", sum(c["result"] == "FAIL" for c in res["checks"]), "failed")
