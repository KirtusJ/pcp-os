#!/usr/bin/env python3
"""The PCP OS documents: PDF reading copies in an edition's theme, the requirements as data, and the
default backgrounds as resolution-independent vector files.

    python3 build_os_docs.py <os-docs dir> <design dir> <EDITION> <out dir>

Needs WeasyPrint and markdown-it-py for the PDFs. Everything else is the standard library.
"""
import hashlib, json, os, re, sys
from pathlib import Path
os.environ.setdefault("SOURCE_DATE_EPOCH", "1794614400")  # 2026-11-13T12:00:00Z: the reading copies carry the build date, not the minute they were made, so that two builds of one source are the same bytes
SRC, DESIGN, EDITION, OUT = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4])
T = json.loads((DESIGN / "pcp.tokens.json").read_text())
ed = {k: v["$value"] for k, v in T["edition"][EDITION.lower()].items() if isinstance(v, dict)}
name = T["edition"][EDITION.lower()]["$description"]
sha = lambda b: hashlib.sha256(b).hexdigest()

# The emblem, as the covers draw it: one bird, with the module's mark on the wing.
BIRD = {"core": dict(body="#187C78", head="#E2F2ED", beak="#E3A24B", eye="#18324B", wing="#41BAD7", mark=None, markc=None),
        "mathematics": dict(body="#A8415F", head="#F3DFE5", beak="#F7D377", eye="#481425", wing="#E3ADBF", mark="arcs", markc="#701A32"),
        "science": dict(body="#006B86", head="#F4FDFF", beak="#F0A42F", eye="#103844", wing="#103844", mark="ring", markc="#A5E7F5"),
        "engineering": dict(body="#7A2E0E", head="#FBE4D5", beak="#29180F", eye="#29180F", wing="#F6C453", mark="chevrons", markc="#29180F"),
        "cryptography": dict(body="#7B55B0", head="#EEE6FA", beak="#C7A8F0", eye="#34214F", wing="#1D1330", mark="keyhole", markc="#C7A8F0"),
        "statistics": dict(body="#152137", head="#DCE6F2", beak="#F0A42F", eye="#152137", wing="#5B8DEF", mark="bars", markc="#FFFFFF"),
        "quality_assurance": dict(body="#2F5D0F", head="#F6FBE8", beak="#F0A42F", eye="#183047", wing="#183047", mark="check", markc="#B5D94B")}

def bird(key):
    m = BIRD[key]; c = m["markc"]; k = m["mark"]; mark = ""
    if k == "arcs": mark = '<path d="M32 82 A17 17 0 0 1 66 82" fill="none" stroke="%s" stroke-width="4.5" stroke-linecap="round"/><path d="M41 82 A8 8 0 0 1 57 82" fill="none" stroke="%s" stroke-width="4.5" stroke-linecap="round"/>' % (c, c)
    elif k == "ring": mark = '<circle cx="49" cy="72" r="12" fill="none" stroke="%s" stroke-width="4.5"/><circle cx="49" cy="72" r="4" fill="%s"/>' % (c, c)
    elif k == "chevrons": mark = '<polyline points="36,72 49,59 62,72" fill="none" stroke="%s" stroke-width="5" stroke-linejoin="round" stroke-linecap="round"/><polyline points="36,87 49,74 62,87" fill="none" stroke="%s" stroke-width="5" stroke-linejoin="round" stroke-linecap="round"/>' % (c, c)
    elif k == "keyhole": mark = '<circle cx="49" cy="64" r="8" fill="%s"/><polygon points="45,68 53,68 57,88 41,88" fill="%s"/>' % (c, c)
    elif k == "bars": mark = '<rect x="35" y="76" width="7" height="12" fill="%s"/><rect x="45.5" y="66" width="7" height="22" fill="%s"/><rect x="56" y="56" width="7" height="32" fill="%s"/>' % (c, c, c)
    elif k == "check": mark = '<polyline points="35,72 45,84 63,58" fill="none" stroke="%s" stroke-width="6" stroke-linejoin="round" stroke-linecap="round"/>' % c
    return ('<ellipse cx="54.7" cy="61.1" rx="41" ry="48.7" fill="%s"/><circle cx="67" cy="29.8" r="27.7" fill="%s"/><polygon points="88.5,19.5 117.2,33.9 88.5,42.1" fill="%s"/>'
            '<circle cx="73.1" cy="23.6" r="4.4" fill="%s"/><ellipse cx="49" cy="71.3" rx="27.15" ry="30.2" fill="%s"/>%s<polygon points="23.9,94.4 3.4,130.2 44.4,105.6" fill="%s"/>'
            % (m["body"], m["head"], m["beak"], m["eye"], m["wing"], mark, m["body"]))

def background(key):
    """One vector file per workspace. It has no fixed size: the viewBox is a unit the display scales, the emblem
    stays whole at any aspect ratio (xMaxYMid meet inside a slice field), and nothing in it is a bitmap."""
    mod = T["module"][key]; ws = T["workspace"][key][EDITION.lower()]
    plate, accent = mod["plate"]["$value"], ws["accent"]["$value"]
    title = "PCP %s workspace, %s" % (mod["name"]["$value"], name)
    # Everything that must stay whole lies inside x 270..1330 and y 160..740: the part of the picture that
    # every screen from 4:3 to 21:9 shows when the field is scaled to cover it.
    rings = "".join('<circle cx="1190" cy="450" r="%d"/>' % r for r in range(250, 2400, 150))
    words = "%s workspace" % mod["name"]["$value"] if key != "core" else "PCP OS"
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 900" preserveAspectRatio="xMidYMid slice" role="img" aria-label="%s">\n'
            '<title>%s</title>\n'
            '<rect x="-4000" y="-4000" width="9600" height="8900" fill="%s"/>\n'
            '<g opacity="0.07" fill="none" stroke="%s" stroke-width="2">%s</g>\n'
            '<rect x="1050" y="299" width="280" height="302" rx="40" fill="%s" stroke="%s" stroke-width="5"/>\n'
            '<g transform="translate(1091 335) scale(1.72)">%s</g>\n'
            '<rect x="270" y="372" width="96" height="9" fill="%s"/>\n'
            '<text x="270" y="446" font-family="Inter, system-ui, sans-serif" font-weight="700" font-size="%d" fill="%s">%s</text>\n'
            '<text x="270" y="496" font-family="Inter, system-ui, sans-serif" font-weight="400" font-size="25" fill="%s">Parrot Council Protocol · %s</text>\n'
            '<text x="270" y="716" font-family="Inter, system-ui, sans-serif" font-weight="400" font-size="18" fill="%s">Evidence before agreement.</text>\n'
            '</svg>\n') % (title, title, ed["field"], ed["text"], rings, plate, accent, bird(key), accent, 54 if len(words) <= 22 else 44, ed["text"], words, ed["soft"], name, ed["soft"])

def requirements(spec):
    out = []; section = None
    for line in spec.splitlines():
        m = re.match(r"^## (\d+)\. (.*)$", line)
        if m: section = {"number": int(m.group(1)), "title": m.group(2)}
        m = re.match(r"^\*\*([A-Z]-\d+)\.\*\* (.*?) \*(BUILT|PARTIAL|DESIGN)(.*)\*$", line)
        if m:
            text = m.group(2); kws = re.findall(r"\b(MUST NOT|MUST|SHOULD NOT|SHOULD|MAY)\b", text)
            assert 1 <= len(kws) <= 2, (m.group(1), kws)
            out.append({"id": m.group(1), "section": section, "levels": kws, "text": text, "status": m.group(3), "evidence": m.group(4).lstrip(":. ").rstrip(".") or None})
    return out

CSS = """
@page { size: A4; margin: 25mm 20mm 22mm 20mm;
  @top-left { content: string(runhead); font: 700 6.6pt Inter; letter-spacing: .06em; color: var(--ink); }
  @bottom-left { content: string(runfoot); font: 400 6.6pt Inter; color: #5B6B7A; }
  @bottom-right { content: counter(page); font: 400 6.6pt Inter; color: #5B6B7A; } }
@page cover { margin: 0; @top-left { content: none } @bottom-left { content: none } @bottom-right { content: none } }
html { font: 400 9.4pt/1.5 Inter, 'DejaVu Sans', sans-serif; color: #18222D; font-variant-ligatures: none; }
body { margin: 0; }
.run { position: absolute; width: 0; height: 0; overflow: hidden; }
.runhead { string-set: runhead content(); } .runfoot { string-set: runfoot content(); }
.cover { page: cover; position: relative; width: 210mm; height: 297mm; background: var(--bg); color: var(--fg); break-after: page; overflow: hidden; }
.cover .label { position: absolute; left: 48pt; top: 56pt; font: 700 12pt Inter; letter-spacing: .02em; }
.cover .bar { position: absolute; left: 48pt; top: 81pt; width: 54pt; height: 5pt; background: var(--accent); }
.cover .title { position: absolute; left: 48pt; top: 136pt; font: 700 37pt/52pt Inter; margin: 0; }
.cover .tag { position: absolute; left: 48pt; top: 300pt; width: 310pt; font: 400 16pt/25pt Inter; }
.cover .plate { position: absolute; right: 48pt; top: 118pt; width: 128pt; height: 138pt; border-radius: 18pt; border: 2pt solid var(--accent); background: #10283F; }
.cover .plate svg { position: absolute; left: 19pt; top: 19.5pt; }
.cover .chip { position: absolute; right: 48pt; top: 569pt; font: 700 8.5pt Inter; letter-spacing: .09em; color: var(--accent); border: 1.2pt solid var(--accent); border-radius: 10pt; padding: 3pt 9pt; }
.cover .rule { position: absolute; left: 48pt; right: 48pt; top: 556pt; border-top: 1pt solid var(--accent); }
.cover .kind { position: absolute; left: 48pt; top: 573pt; font: 700 12pt Inter; color: var(--accent); }
.cover .who { position: absolute; left: 48pt; top: 604pt; font: 700 13pt Inter; }
.cover .credit { position: absolute; left: 48pt; top: 628pt; font: 400 10pt Inter; color: var(--soft); }
.cover .notes { position: absolute; left: 48pt; top: 700pt; width: 460pt; font: 400 9.5pt/15pt Inter; color: var(--soft); }
h1 { font: 700 19pt/1.25 Inter; margin: 0 0 8pt; color: #18222D; }
h2 { font: 700 13pt/1.3 Inter; margin: 18pt 0 6pt; color: var(--ink); break-after: avoid; }
p { margin: 0 0 7pt; } ul, ol { margin: 0 0 7pt; padding-left: 16pt; }
code { font: 400 8.3pt 'DejaVu Sans Mono', monospace; background: #F1F4F7; padding: 0 2pt; border-radius: 2pt; word-break: break-word; }
em { color: #44525F; }
table { width: 100%; border-collapse: collapse; margin: 4pt 0 10pt; font-size: 8.5pt; line-height: 1.4; }
th { background: var(--th); color: #fff; text-align: left; font-weight: 700; padding: 5pt 7pt; }
td { padding: 5pt 7pt; border-bottom: .4pt solid #DDE3EA; vertical-align: top; } tr:nth-child(even) td { background: #F7F9FB; }
thead { display: table-row-group; } tr { break-inside: avoid; }
"""

def pdf(md_path, kind, tag, out_path):
    from markdown_it import MarkdownIt
    from weasyprint import HTML
    text = md_path.read_text()
    body = MarkdownIt("commonmark", {"html": False}).enable("table").render(text)
    vars_ = "--bg:%s;--fg:%s;--accent:%s;--soft:%s;--ink:%s;--th:%s" % (ed["field"], ed["text"], ed["accent"], ed["soft"], ed["ink"], ed["table-header"])
    cover = ('<section class="cover"><div class="label">PCP OS</div><div class="bar"></div><p class="title">PCP OS<br>%s</p><p class="tag">%s</p>'
             '<div class="plate"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 132" width="90pt" height="99pt" role="img" aria-label="PCP emblem">%s</svg></div>'
             '<div class="rule"></div><div class="kind">%s</div><div class="chip">%s</div><div class="who">Kirtus Justus</div><div class="credit">Creator and owner</div>'
             '<div class="notes">Parrot Council Protocol (PCP) 1.0 · PCP OS, build 22 · 13 November 2026<br>Early development. Untested and not peer reviewed. Started only in an emulator. See EVIDENCE_STATUS.md.<br>Evidence before agreement.</div></section>'
             % (kind, tag, bird("core"), kind.upper(), name.upper()))
    html = ('<!doctype html><html lang="en"><head><meta charset="utf-8"><title>PCP OS %s</title><style>%s</style></head><body style="%s">%s'
            '<div class="run runhead">PCP OS %s</div><div class="run runfoot">PCP 1.0 · %s · generated from %s, SHA-256 %s</div>%s</body></html>'
            % (kind, CSS, vars_, cover, kind.upper(), name, md_path.name, sha(md_path.read_bytes())[:16], body))
    HTML(string=html).write_pdf(out_path, pdf_identifier=sha(md_path.read_bytes())[:32].encode(), custom_metadata=False)

def main():
    OUT.mkdir(parents=True, exist_ok=True); (OUT / "backgrounds").mkdir(exist_ok=True)
    spec = (SRC / "PCP_OS_Specification.md").read_text()
    reqs = requirements(spec)
    assert len(reqs) == len(re.findall(r"^\*\*[A-Z]-\d+\.\*\*", spec, re.M)), "a requirement line does not have the form: id, one sentence, status"
    counts = {s: sum(1 for r in reqs if r["status"] == s) for s in ("BUILT", "PARTIAL", "DESIGN")}
    (OUT / "PCP_OS_Requirements.json").write_text(json.dumps({"format": "pcp-os-requirements/1", "generated_from": "PCP_OS_Specification.md", "source_sha256": sha(spec.encode()),
        "keywords": "BCP 14 (RFC 2119, RFC 8174)", "status_counts": counts, "requirements": reqs}, indent=1) + "\n")
    for f in ("PCP_OS_Overview.md", "PCP_OS_Specification.md"):
        (OUT / f).write_text((SRC / f).read_text())
    for key in T["module"]:
        (OUT / "backgrounds" / ("pcp-%s.svg" % key.replace("_", "-"))).write_text(background(key))
    (OUT / "backgrounds" / "README.md").write_text("# Default backgrounds | %s\n\nOne per workspace, in this edition's theme. Each is a vector file with no fixed size: a display draws it at its own resolution, the field fills any shape of screen, and the emblem and words stay whole on every screen from 4:3 to 21:9. On a taller screen, such as a phone held upright, the sides are cut; a layout for that shape does not exist yet. Nothing in them is a bitmap, so nothing is stretched. Generated from the design tokens; the emblem is the one on the covers.\n\n`pcp-core.svg` is the default. The others are used when a workspace has that module in its profile.\n" % name)
    if "--no-pdf" not in sys.argv:
        pdf(SRC / "PCP_OS_Overview.md", "Overview", "What PCP OS is, what exists today, and the ideas it is built on. Plain language.", OUT / "PCP_OS_Overview.pdf")
        pdf(SRC / "PCP_OS_Specification.md", "Specification", "The requirements, each numbered, each marked as built, partly built, or design.", OUT / "PCP_OS_Specification.pdf")
    print(json.dumps({"edition": EDITION, "requirements": len(reqs), "status": counts, "files": sorted(p.relative_to(OUT).as_posix() for p in OUT.rglob("*") if p.is_file())}))

main()
