#!/usr/bin/env python3
"""Publisher-side assembly of the three editions of PCP 1.0.

Each edition archive holds: the edition's own documents (license, notice, price schedule, evidence
status, guide, readme), the Implementation packet (one archive, byte-identical in every edition) and the
Readers packet rendered in the edition's theme. Usage: python3 assemble_editions.py [EDITION ...]"""
import hashlib, json, shutil, subprocess, sys, zipfile
from pathlib import Path
import edition_texts as T
from make_zips import build as zip_build

B = Path("/home/claude/build"); PKG = B / "pkg"; OUT = B / "out"
sha = lambda b: hashlib.sha256(b).hexdigest()
THEMES = json.loads((B / "themes.json").read_text())

VERIFY_READERS = r'''#!/usr/bin/env python3
"""Local check for a PCP Version 1.0 Readers packet. Standard library only.

Checks the file inventory and digests against MANIFEST.json, that every PDF is a
complete container, and, if the Implementation packet directory is given, that
each PDF generated from an Implementation source records the digest of that
source there and that the Implementation manifest fingerprint matches the pin.
The edition guide's source lies beside this packet, at the top level of the
edition, and is checked by the edition's own verifier. A pass is a local fact
about these files; it authenticates nobody, validates no meaning and certifies nothing.
"""
import hashlib, json, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent


def sha(b):
    return hashlib.sha256(b).hexdigest()


def main(argv):
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip()); print("usage: python3 verify_readers.py [path-to-Implementation-packet]"); return 0
    try:
        man = json.loads((HERE / "MANIFEST.json").read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        print("FAIL_READERS_CHECK: MANIFEST.json unreadable: %s" % type(e).__name__); return 1
    if man.get("format") != "pcp-readers-local-integrity/2.0" or man.get("edition") not in ("PERSONAL", "COMMERCIAL", "ENTERPRISE"):
        print("FAIL_READERS_CHECK: unsupported manifest format or edition"); return 1
    files = {e["path"]: e for e in man["files"]}
    actual = sorted(p.relative_to(HERE).as_posix() for p in HERE.rglob("*")
                    if p.is_file() and p.name != "MANIFEST.json" and "__pycache__" not in p.parts)
    if actual != sorted(files) or man.get("payload_count") != len(files):
        print("FAIL_READERS_CHECK: inventory differs from manifest: %s" % sorted(set(actual) ^ set(files))[:5]); return 1
    for p, e in files.items():
        data = (HERE / p).read_bytes()
        if len(data) != e["bytes"] or sha(data) != e["sha256"]:
            print("FAIL_READERS_CHECK: content differs from manifest: %s" % p); return 1
        if p.endswith(".pdf") and not (data.startswith(b"%PDF-") and b"%%EOF" in data[-2048:]):
            print("FAIL_READERS_CHECK: incomplete PDF container: %s" % p); return 1
    if argv:
        impl = Path(argv[0]).resolve()
        if not (impl / "MANIFEST.json").is_file():
            print("FAIL_READERS_CHECK: no MANIFEST.json at %s; give the extracted Implementation packet directory" % impl); return 1
        im = (impl / "MANIFEST.json").read_bytes()
        if sha(im) != man["implementation_packet"]["manifest_sha256"]:
            print("FAIL_READERS_CHECK: Implementation manifest fingerprint differs from the pin"); return 1
        digest = {e["path"]: e["sha256"] for e in json.loads(im)["files"]}
        for p, e in files.items():
            if p.endswith(".pdf") and e.get("source_packet") == "IMPLEMENTATION":
                src = e["generated_from"]
                if digest.get(src) != e["source_sha256"] or sha((impl / src).read_bytes()) != e["source_sha256"]:
                    print("FAIL_READERS_CHECK: source digest for %s differs in the Implementation packet" % p); return 1
        print("PASS_READERS_CHECK %d files, %s edition; sources agree with the Implementation packet at %s" % (len(files), man["edition"], impl))
    else:
        print("PASS_READERS_CHECK %d files, %s edition (Implementation packet not given; source digests not compared)" % (len(files), man["edition"]))
    print("Readers fingerprint (SHA-256 of MANIFEST.json): %s" % sha((HERE / "MANIFEST.json").read_bytes()))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
'''

VERIFY_EDITION = r'''#!/usr/bin/env python3
"""Local check for one edition of PCP Version 1.0. Standard library only.

  python3 verify_edition.py            full check
  python3 verify_edition.py --quick    skip the packets' own verifiers

Checks the inventory and digests of this folder against MANIFEST.json; that the
two packets are the ones EDITION.json names; that the license, notice, price
schedule and evidence statement carry their controlling statements for this
edition; that the Readers packet belongs to this edition and pins this
Implementation packet; that the edition guide's reading copy was generated from
the guide beside this file; and, unless --quick, runs each packet's own verifier
on a temporary extraction. A pass is a local fact about these files. It
authenticates nobody, validates no meaning, and is not legal advice or a
statement that the product has been tested.
"""
import hashlib, json, subprocess, sys, tempfile, zipfile
from pathlib import Path
HERE = Path(__file__).resolve().parent
EDITIONS = {"PERSONAL": "Personal Edition", "COMMERCIAL": "Commercial Edition", "ENTERPRISE": "Enterprise Edition"}
PRICE_ROWS = ("| Personal | **US$5.00** | One payment for one license held by one natural person. Not a subscription: no renewal and no recurring charge. |",
              "| Commercial | No published price | Licensed only under a written agreement negotiated individually with the rights holder. |",
              "| Enterprise | No published price | Licensed only under a written agreement negotiated individually with the rights holder. |")
EVIDENCE = ("**Not peer reviewed.**", "**Not independently reviewed.**", "**Not empirically evaluated.**", "**No observational evidence of effect.**")


class Fail(Exception):
    pass


def need(cond, msg):
    if not cond:
        raise Fail(msg)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def text(name):
    return (HERE / name).read_text(encoding="utf-8")


def extract(zpath, dest):
    with zipfile.ZipFile(zpath) as z:
        for n in z.namelist():
            need(not n.startswith("/") and ".." not in n.split("/") and "\\" not in n, "unsafe path in %s" % zpath.name)
        z.extractall(dest)
    tops = sorted(p for p in dest.iterdir() if p.is_dir())
    need(len(tops) == 1, "%s must hold exactly one top-level folder" % zpath.name)
    return tops[0]


def verify(quick):
    man = json.loads(text("MANIFEST.json")); ed = json.loads(text("EDITION.json"))
    need(man.get("format") == "pcp-edition-local-integrity/1.0", "unsupported manifest format")
    edition = ed.get("edition"); need(edition in EDITIONS and man.get("edition") == edition, "edition identity")
    name = EDITIONS[edition]
    files = {e["path"]: e for e in man["files"]}
    actual = sorted(p.relative_to(HERE).as_posix() for p in HERE.rglob("*")
                    if p.is_file() and p.name != "MANIFEST.json" and "__pycache__" not in p.parts)
    need(actual == sorted(files) and man.get("payload_count") == len(files), "inventory differs from manifest: %s" % sorted(set(actual) ^ set(files))[:5])
    for p, e in files.items():
        data = (HERE / p).read_bytes()
        need(len(data) == e["bytes"] and sha(data) == e["sha256"], "content differs from manifest: %s" % p)
    # Identity and fixed points
    need(ed.get("product") == "Parrot Council Protocol" and ed.get("abbreviation") == "PCP" and ed.get("method") == "Adversarial Claim Review"
         and ed.get("version") == "1.0" and ed.get("creator") == "Kirtus Justus", "product identity")
    need(ed.get("licensing") == "PROPRIETARY_ALL_RIGHTS_RESERVED" and ed.get("license_model") == "LICENSED_NOT_SOLD", "licensing status")
    want = ({"amount": "5.00", "currency": "USD", "basis": "SINGLE_PURCHASE", "subscription": False} if edition == "PERSONAL"
            else {"amount": None, "currency": None, "basis": "INDIVIDUALLY_NEGOTIATED_WRITTEN_AGREEMENT", "published_price": False})
    need(ed.get("price") == want, "price record for this edition")
    need(ed.get("evidence") == {"peer_review": "NOT_PERFORMED", "independent_review": "NOT_PERFORMED", "empirical_evaluation": "NOT_PERFORMED",
                                "behavioral_validation": "NOT_PERFORMED", "observational_evidence": "NONE_OF_EFFECT"}, "evidence record")
    # Controlling statements
    pricing = text("PRICING.md")
    need("**This schedule is fixed.**" in pricing and all(r in pricing for r in PRICE_ROWS), "price schedule")
    notice = text("NOTICE.md")
    need("**Proprietary material. All rights reserved.**" in notice and "Copyright (c) 2026 Kirtus Justus. All rights reserved" in notice
         and "PCP is licensed, not sold." in notice and "Holding a copy grants nothing." in notice, "notice")
    ev = text("EVIDENCE_STATUS.md"); need(all(x in ev for x in EVIDENCE), "evidence statement")
    lic = text("LICENSE.md")
    need(lic.startswith("# Parrot Council Protocol (PCP) 1.0 | %s License" % name) and "The Materials are proprietary. They are licensed, not sold" in lic
         and all(o not in lic.splitlines()[0] for k, o in EDITIONS.items() if k != edition), "license belongs to this edition")
    if edition == "PERSONAL":
        need("**License fee:** US$5.00, paid once" in lic and "one natural person" in lic, "Personal license terms")
    else:
        need("**Without a signed Agreement there is no license.**" in lic and "no price is published" in lic, "organization license conditions")
    need(text("EDITION_GUIDE.md").startswith("# PCP 1.0 | %s Guide" % name) and name in text("README.md").splitlines()[0], "guide and readme belong to this edition")
    # Packets
    ip, rp = HERE / ed["packets"]["implementation"]["file"], HERE / ed["packets"]["readers"]["file"]
    need(sha(ip.read_bytes()) == ed["packets"]["implementation"]["sha256"], "Implementation packet differs from EDITION.json")
    need(sha(rp.read_bytes()) == ed["packets"]["readers"]["sha256"], "Readers packet differs from EDITION.json")
    with tempfile.TemporaryDirectory() as td:
        impl = extract(ip, Path(td) / "i"); rd = extract(rp, Path(td) / "r")
        im = (impl / "MANIFEST.json").read_bytes()
        need(sha(im) == ed["packets"]["implementation"]["manifest_sha256"], "Implementation fingerprint differs from EDITION.json")
        rman = json.loads((rd / "MANIFEST.json").read_text(encoding="utf-8"))
        need(rman.get("edition") == edition, "the Readers packet belongs to another edition")
        need(rman["implementation_packet"]["manifest_sha256"] == sha(im), "the Readers packet pins another Implementation packet")
        need(sha((rd / "MANIFEST.json").read_bytes()) == ed["packets"]["readers"]["manifest_sha256"], "Readers fingerprint differs from EDITION.json")
        guide = [e for e in rman["files"] if e.get("source_packet") == "EDITION"]
        need(len(guide) == 1 and guide[0]["generated_from"] == "EDITION_GUIDE.md"
             and guide[0]["source_sha256"] == sha((HERE / "EDITION_GUIDE.md").read_bytes()), "edition guide reading copy does not match EDITION_GUIDE.md")
        for p in impl.rglob("*"):
            if p.is_file() and p.suffix in (".md", ".json", ".csv", ".py"):
                low = p.read_text(encoding="utf-8", errors="replace").lower()
                need(all(n.lower() not in low for n in EDITIONS.values()), "edition marker inside the Implementation packet: %s" % p.name)
        if not quick:
            for cmd in ([sys.executable, "-B", str(impl / "TOOLS/verify_release.py")],
                        [sys.executable, "-B", str(rd / "verify_readers.py"), str(impl)]):
                out = subprocess.run(cmd, capture_output=True, text=True)
                need(out.returncode == 0 and out.stdout.startswith("PASS"), "packet verifier failed: %s" % (out.stdout.strip().split("\n")[0][:160] or out.stderr.strip()[-160:]))
    return edition, len(files), sha((HERE / "MANIFEST.json").read_bytes()), ed["packets"]["implementation"]["sha256"]


def main(argv):
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip()); return 0
    try:
        edition, n, fp, impl = verify("--quick" in argv)
    except Fail as e:
        print("FAIL_EDITION_CHECK: %s" % e); return 1
    except (OSError, KeyError, ValueError, TypeError, IndexError, zipfile.BadZipFile) as e:
        print("FAIL_EDITION_CHECK: %s: %s" % (type(e).__name__, e)); return 1
    print("PASS_EDITION_CHECK %s edition, %d files%s (not authenticated, not certified, not peer reviewed, behavior not tested)"
          % (edition, n, "" if "--quick" not in argv else ", packet verifiers skipped"))
    print("edition fingerprint (SHA-256 of MANIFEST.json): %s" % fp)
    print("Implementation packet SHA-256 (the same in every edition): %s" % impl)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
'''


def write(p, s):
    p.parent.mkdir(parents=True, exist_ok=True); p.write_text(s, encoding="utf-8")


def main(editions):
    OUT.mkdir(exist_ok=True)
    for d in PKG.rglob("__pycache__"): shutil.rmtree(d)
    impl_zip = OUT / "ACR_1.0_Implementation.zip"
    impl_sha = zip_build(PKG, "ACR_1.0_Implementation", impl_zip)
    assert impl_sha == zip_build(PKG, "ACR_1.0_Implementation", OUT / "_again.zip"), "Implementation archive is not reproducible"
    (OUT / "_again.zip").unlink()
    impl_fp = sha((PKG / "MANIFEST.json").read_bytes())
    impl_digest = {e["path"]: e["sha256"] for e in json.loads((PKG / "MANIFEST.json").read_text())["files"]}
    summary = {"implementation": {"sha256": impl_sha, "manifest_sha256": impl_fp}, "editions": {}}
    for E in editions:
        meta = T.ED[E]; name = meta["name"]; stage = B / "ed" / meta["folder"]
        if stage.exists(): shutil.rmtree(stage)
        stage.mkdir(parents=True)
        # 1. Edition documents
        write(stage / "LICENSE.md", T.LICENSE[E]); write(stage / "NOTICE.md", T.NOTICE)
        write(stage / "PRICING.md", T.PRICING); write(stage / "EVIDENCE_STATUS.md", T.EVIDENCE % {"date": T.DATE})
        write(stage / "EDITION_GUIDE.md", T.GUIDE[E])
        write(stage / "README.md", T.README % {"name": name, "date": T.DATE, "effect": T.README_EFFECT[E]})
        # 2. Reading copies in the edition theme
        books = B / ("books_" + E)
        if books.exists(): shutil.rmtree(books)
        shutil.copytree(PKG, books)
        out = subprocess.run([sys.executable, "build_books.py", str(books), E, str(stage / "EDITION_GUIDE.md")], cwd=B, capture_output=True, text=True)
        assert out.returncode == 0, out.stderr[-800:]
        report = json.loads((B / ("book_build_report_%s.json" % E)).read_text())
        assert len(report) == 23 and all(r["body_text_equals_source"] and r["tagged"] for r in report), [r["pdf"] for r in report if not r["body_text_equals_source"]]
        # 3. Readers packet
        R = B / ("readers_" + E)
        if R.exists(): shutil.rmtree(R)
        for r in report:
            (R / r["pdf"]).parent.mkdir(parents=True, exist_ok=True); shutil.copy(books / r["pdf"], R / r["pdf"])
        write(R / "README.md", T.READERS_README % {"name": name, "impl_fp": impl_fp})
        write(R / "RELEASE/Brand_And_Book_Identity.md", T.BRAND % {"name": name, "short": meta["short"]})
        write(R / "verify_readers.py", VERIFY_READERS)
        by_pdf = {r["pdf"]: r for r in report}; entries = []
        for p in sorted(q.relative_to(R).as_posix() for q in R.rglob("*") if q.is_file()):
            data = (R / p).read_bytes(); e = {"path": p, "bytes": len(data), "sha256": sha(data)}
            if p.endswith(".pdf"):
                r = by_pdf[p]
                if r["source"] == "EDITION_GUIDE.md":
                    e.update({"generated_from": "EDITION_GUIDE.md", "source_packet": "EDITION", "source_sha256": sha((stage / "EDITION_GUIDE.md").read_bytes()), "pages": r["pages"]})
                else:
                    assert impl_digest[r["source"]] == sha((PKG / r["source"]).read_bytes())
                    e.update({"generated_from": r["source"], "source_packet": "IMPLEMENTATION", "source_sha256": impl_digest[r["source"]], "pages": r["pages"]})
            entries.append(e)
        th = THEMES[E]
        rman = {"format": "pcp-readers-local-integrity/2.0", "product": "Parrot Council Protocol", "product_version": "1.0", "packet": "READERS",
                "edition": E, "theme": {k: th[k] for k in ("field", "text", "sig", "ink", "th", "soft")},
                "implementation_packet": {"name": "ACR_1.0_Implementation.zip", "manifest_sha256": impl_fp},
                "pdf_build": {"renderer": "WeasyPrint 70.0", "variant": "pdf/ua-1", "fonts": ["Inter", "DejaVu Sans Mono"],
                              "check": "For each PDF, the text of the pages after any cover and contents page, with whitespace and list markers removed, equalled the text of the rendered source. This compares words, not layout or meaning."},
                "payload_count": len(entries), "files": entries}
        write(R / "MANIFEST.json", json.dumps(rman, indent=2) + "\n")
        readers_fp = sha((R / "MANIFEST.json").read_bytes())
        readers_sha = zip_build(R, "PCP_1.0_Readers", stage / "PCP_1.0_Readers.zip")
        shutil.copy(impl_zip, stage / "ACR_1.0_Implementation.zip")
        # 4. Edition identity, verifier, manifest
        price = ({"amount": "5.00", "currency": "USD", "basis": "SINGLE_PURCHASE", "subscription": False} if E == "PERSONAL"
                 else {"amount": None, "currency": None, "basis": "INDIVIDUALLY_NEGOTIATED_WRITTEN_AGREEMENT", "published_price": False})
        ed = {"product": "Parrot Council Protocol", "abbreviation": "PCP", "method": "Adversarial Claim Review", "version": "1.0",
              "edition": E, "edition_name": name, "creator": "Kirtus Justus", "build_date": "2026-11-13", "publication_state": "PRE_PUBLICATION",
              "licensing": "PROPRIETARY_ALL_RIGHTS_RESERVED", "license_model": "LICENSED_NOT_SOLD", "price": price,
              "evidence": {"peer_review": "NOT_PERFORMED", "independent_review": "NOT_PERFORMED", "empirical_evaluation": "NOT_PERFORMED",
                           "behavioral_validation": "NOT_PERFORMED", "observational_evidence": "NONE_OF_EFFECT"},
              "theme": {k: th[k] for k in ("field", "text", "sig", "ink")},
              "packets": {"implementation": {"file": "ACR_1.0_Implementation.zip", "sha256": impl_sha, "manifest_sha256": impl_fp, "identical_in_every_edition": True},
                          "readers": {"file": "PCP_1.0_Readers.zip", "sha256": readers_sha, "manifest_sha256": readers_fp}}}
        write(stage / "EDITION.json", json.dumps(ed, indent=2) + "\n"); write(stage / "verify_edition.py", VERIFY_EDITION)
        files = sorted(p.relative_to(stage).as_posix() for p in stage.rglob("*") if p.is_file())
        man = {"format": "pcp-edition-local-integrity/1.0", "product": "Parrot Council Protocol", "product_version": "1.0", "edition": E,
               "payload_count": len(files), "files": [{"path": f, "bytes": (stage / f).stat().st_size, "sha256": sha((stage / f).read_bytes())} for f in files]}
        write(stage / "MANIFEST.json", json.dumps(man, indent=2) + "\n")
        ed_zip = OUT / (meta["folder"] + ".zip"); ed_sha = zip_build(stage, meta["folder"], ed_zip)
        summary["editions"][E] = {"archive": ed_zip.name, "sha256": ed_sha, "edition_fingerprint": sha((stage / "MANIFEST.json").read_bytes()),
                                  "readers_sha256": readers_sha, "readers_fingerprint": readers_fp, "pdf_pages": sum(r["pages"] for r in report)}
        print(E, ed_sha[:16], "readers", readers_sha[:16], "pages", summary["editions"][E]["pdf_pages"])
    (B / "edition_build_summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    print("implementation", impl_sha)


if __name__ == "__main__":
    main(sys.argv[1:] or list(T.ED))
