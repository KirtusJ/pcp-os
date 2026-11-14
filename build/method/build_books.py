#!/usr/bin/env python3
"""Publisher-side builder: every PDF in the package from its Markdown source (WeasyPrint, tagged PDF/UA-1)."""
import os, re, sys, html, hashlib, json
from pathlib import Path
from html.parser import HTMLParser
from markdown_it import MarkdownIt
os.environ.setdefault("SOURCE_DATE_EPOCH", "1794441600")  # 2026-11-11T12:00:00Z
from weasyprint import HTML
from weasyprint.text.fonts import FontConfiguration

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else "pkg")
EDITION = sys.argv[2] if len(sys.argv) > 2 else "PERSONAL"
THEME = json.loads(Path(__file__).with_name("themes.json").read_text())[EDITION]
GUIDE = Path(sys.argv[3]) if len(sys.argv) > 3 else None
EVID = "Evidence status: untested and not peer reviewed. See EVIDENCE_STATUS.md."
MD = MarkdownIt("commonmark", {"html": False}).enable("table")
COMPONENT_RE = re.compile(r"\*\*Component:\*\* `(acr\.[a-z_]+)`, Version (\d+\.\d+)")

MODULES = {
 "01_Core": dict(name="Core", label="PCP CORE", title=["Adversarial", "Claim Review"], tag="The epistemological protocol inside PCP: accountable examination and construction.",
   credit="Protocol creator and owner", note="Ordinary authorized work remains the default.",
   bg="#10283F", fg="#F2F7FC", accent="#80D5D0", ink="#187C78", body="#187C78", head="#E2F2ED", beak="#E3A24B", eye="#18324B", wing="#41BAD7", mark=None),
 "02_Mathematics": dict(name="Mathematics", label="PCP MATHEMATICS", title=["Adversarial", "Claim Review", "Mathematics"], tag="A complementary module for mathematical exploration and proof.",
   credit="Protocol creator and owner", note="Requires the governing core in context for its assigned use.",
   bg="#701A32", fg="#F2F7FC", accent="#E3ADBF", ink="#7D2945", body="#A8415F", head="#F3DFE5", beak="#F7D377", eye="#481425", wing="#E3ADBF", mark="arcs", markc="#701A32"),
 "03_Science": dict(name="Science", label="PCP SCIENCE", title=["Adversarial", "Claim Review", "Science"], tag="A complementary module for scientific research.",
   credit="Protocol creator and owner", note="Requires the governing core in context for its assigned use.",
   bg="#A5E7F5", fg="#103844", accent="#1286A1", ink="#006B86", body="#006B86", head="#F4FDFF", beak="#F0A42F", eye="#103844", wing="#103844", mark="ring", markc="#A5E7F5"),
 "04_Engineering": dict(name="Engineering", label="PCP ENGINEERING", title=["Adversarial", "Claim Review", "Engineering"], tag="A complementary module for engineering decisions.",
   credit="Protocol creator and owner", note="Requires the governing core in context for its assigned use.",
   bg="#D05D29", fg="#29180F", accent="#29180F", ink="#9A3E16", body="#7A2E0E", head="#FBE4D5", beak="#29180F", eye="#29180F", wing="#F6C453", mark="chevrons", markc="#29180F"),
 "05_Cryptography": dict(name="Cryptography", label="PCP CRYPTOGRAPHY", title=["Adversarial", "Claim Review", "Cryptography"], tag="A complementary module for cryptographic development and review.",
   credit="Protocol creator and owner", note="Requires the governing core in context for its assigned use.",
   bg="#34214F", fg="#F2F7FC", accent="#C7A8F0", ink="#68429B", body="#7B55B0", head="#EEE6FA", beak="#C7A8F0", eye="#34214F", wing="#1D1330", mark="keyhole", markc="#C7A8F0"),
 "06_Statistics": dict(name="Statistics", label="PCP STATISTICS", title=["Adversarial", "Claim Review", "Statistics"], tag="A complementary module for statistical inference and uncertainty.",
   credit="Protocol creator and research direction", note="Requires the governing core in context for its assigned use.",
   bg="#FFFFFF", fg="#152137", accent="#152137", ink="#152137", body="#152137", head="#DCE6F2", beak="#F0A42F", eye="#152137", wing="#5B8DEF", mark="bars", markc="#FFFFFF"),
 "07_Quality_Assurance": dict(name="Quality Assurance", label="PCP QUALITY ASSURANCE", title=["Adversarial", "Claim Review", "Quality Assurance"], tag="A complementary module for quality objectives and verification.",
   credit="Protocol creator and research direction", note="Requires the governing core in context for its assigned use.",
   bg="#B5D94B", fg="#152137", accent="#3F5A0C", ink="#4E6B12", body="#2F5D0F", head="#F6FBE8", beak="#F0A42F", eye="#183047", wing="#183047", mark="check", markc="#B5D94B"),
}

def bird_svg(m, size=120):
    mark = ""
    c = m.get("markc")
    k = m.get("mark")
    if k == "arcs":
        mark = '<path d="M32 82 A17 17 0 0 1 66 82" fill="none" stroke="%s" stroke-width="4.5" stroke-linecap="round"/><path d="M41 82 A8 8 0 0 1 57 82" fill="none" stroke="%s" stroke-width="4.5" stroke-linecap="round"/>' % (c, c)
    elif k == "ring":
        mark = '<circle cx="49" cy="72" r="12" fill="none" stroke="%s" stroke-width="4.5"/><circle cx="49" cy="72" r="4" fill="%s"/>' % (c, c)
    elif k == "chevrons":
        mark = '<polyline points="36,72 49,59 62,72" fill="none" stroke="%s" stroke-width="5" stroke-linejoin="round" stroke-linecap="round"/><polyline points="36,87 49,74 62,87" fill="none" stroke="%s" stroke-width="5" stroke-linejoin="round" stroke-linecap="round"/>' % (c, c)
    elif k == "keyhole":
        mark = '<circle cx="49" cy="64" r="8" fill="%s"/><polygon points="45,68 53,68 57,88 41,88" fill="%s"/>' % (c, c)
    elif k == "bars":
        mark = '<rect x="35" y="76" width="7" height="12" fill="%s"/><rect x="45.5" y="66" width="7" height="22" fill="%s"/><rect x="56" y="56" width="7" height="32" fill="%s"/>' % (c, c, c)
    elif k == "check":
        mark = '<polyline points="35,72 45,84 63,58" fill="none" stroke="%s" stroke-width="6" stroke-linejoin="round" stroke-linecap="round"/>' % c
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 132" width="%dpt" height="%dpt" role="img" aria-label="PCP %s emblem">'
            '<ellipse cx="54.7" cy="61.1" rx="41" ry="48.7" fill="%s"/><circle cx="67" cy="29.8" r="27.7" fill="%s"/>'
            '<polygon points="88.5,19.5 117.2,33.9 88.5,42.1" fill="%s"/><circle cx="73.1" cy="23.6" r="4.4" fill="%s"/>'
            '<ellipse cx="49" cy="71.3" rx="27.15" ry="30.2" fill="%s"/>%s'
            '<polygon points="23.9,94.4 3.4,130.2 44.4,105.6" fill="%s"/></svg>') % (
        size, round(size * 1.1), m["name"], m["body"], m["head"], m["beak"], m["eye"], m["wing"], mark, m["body"])

CSS = """
@page { size: A4; margin: 25mm 20mm 22mm 20mm;
  @top-left { content: string(runhead); font: 700 6.6pt Inter; letter-spacing: .06em; color: var(--ink); }
  @top-right { content: string(runright); font: 400 6.6pt Inter; color: #5B6B7A; }
  @bottom-left { content: string(runfoot); font: 400 6.6pt Inter; color: #5B6B7A; }
  @bottom-right { content: counter(page); font: 400 6.6pt Inter; color: #5B6B7A; } }
@page cover { margin: 0; @top-left { content: none } @top-right { content: none } @bottom-left { content: none } @bottom-right { content: none } }
html { font: 400 9.4pt/1.5 Inter, 'DejaVu Sans', sans-serif; color: #18222D; font-variant-ligatures: none; hyphens: manual; }
body { margin: 0; }
.run { position: absolute; width: 0; height: 0; overflow: hidden; }
.runhead { string-set: runhead content(); } .runright { string-set: runright content(); } .runfoot { string-set: runfoot content(); }
.cover { page: cover; position: relative; width: 210mm; height: 297mm; background: var(--bg); color: var(--fg); break-after: page; overflow: hidden; }
.cover .label { position: absolute; left: 48pt; top: 56pt; font: 700 12pt Inter; letter-spacing: .02em; }
.cover .bar { position: absolute; left: 48pt; top: 81pt; width: 54pt; height: 5pt; background: var(--accent); }
.cover .title { position: absolute; left: 48pt; top: 136pt; font: 700 37pt/52pt Inter; margin: 0; }
.cover .tag { position: absolute; left: 48pt; width: 330pt; font: 400 16pt/25pt Inter; }
.cover .plate { position: absolute; right: 48pt; top: 118pt; width: 128pt; height: 138pt; border-radius: 18pt; border: 2pt solid var(--accent); }
.cover .plate svg { position: absolute; left: 19pt; top: 19.5pt; }
.cover .chip { position: absolute; right: 48pt; top: 569pt; font: 700 8.5pt Inter; letter-spacing: .09em; color: var(--accent); border: 1.2pt solid var(--accent); border-radius: 10pt; padding: 3pt 9pt; }
.cover .rule { position: absolute; left: 48pt; right: 48pt; top: 556pt; border-top: 1pt solid var(--accent); }
.cover .kind { position: absolute; left: 48pt; top: 573pt; font: 700 12pt Inter; color: var(--accent); }
.cover .who { position: absolute; left: 48pt; top: 604pt; font: 700 13pt Inter; }
.cover .credit { position: absolute; left: 48pt; top: 628pt; font: 400 10pt Inter; color: var(--soft); }
.cover .meta { position: absolute; left: 48pt; top: 672pt; font: 400 9pt/18pt Inter; color: var(--soft); }
.cover .notes { position: absolute; left: 48pt; top: 728pt; font: 400 9.5pt/15pt Inter; color: var(--soft); }
.cover .notes b { color: var(--fg); font-weight: 700; }
.toc { break-after: page; } .toc h2 { font: 700 20pt Inter; margin: 0 0 4pt; color: #18222D; }
.toc .sub { font-size: 8pt; color: #5B6B7A; margin-bottom: 18pt; }
.toc a { display: block; color: #18222D; text-decoration: none; padding: 5.5pt 0; border-bottom: .4pt solid #E3E8EE; }
.toc a::after { content: target-counter(attr(href), page); float: right; color: #5B6B7A; }
.plainhead { border-bottom: 1.2pt solid var(--ink); margin-bottom: 10pt; padding-bottom: 5pt; font: 700 7pt Inter; letter-spacing: .06em; color: var(--ink); }
h1 { font: 700 19pt/1.25 Inter; margin: 0 0 8pt; color: #18222D; }
h2 { font: 700 13pt/1.3 Inter; margin: 18pt 0 6pt; color: var(--ink); break-after: avoid; }
h3 { font: 700 10.6pt/1.3 Inter; margin: 13pt 0 4pt; color: var(--ink); break-after: avoid; }
p { margin: 0 0 7pt; } ul, ol { margin: 0 0 7pt; padding-left: 16pt; } li { margin-bottom: 2.5pt; }
a { color: var(--ink); word-break: break-word; }
code { font: 400 8.3pt 'DejaVu Sans Mono', monospace; background: #F1F4F7; padding: 0 2pt; border-radius: 2pt; word-break: break-word; }
pre { background: #F1F4F7; padding: 7pt; border-radius: 3pt; white-space: pre-wrap; } pre code { background: none; padding: 0; }
blockquote { margin: 0 0 7pt; padding: 2pt 0 2pt 10pt; border-left: 2.5pt solid var(--ink); color: #2A3540; }
table { width: 100%; border-collapse: collapse; margin: 4pt 0 10pt; font-size: 8.5pt; line-height: 1.4; }
th { background: var(--th); color: #fff; text-align: left; font-weight: 700; padding: 5pt 7pt; }
td { padding: 5pt 7pt; border-bottom: .4pt solid #DDE3EA; vertical-align: top; } tr:nth-child(even) td { background: #F7F9FB; }
thead { display: table-row-group; } tr { break-inside: avoid; }
"""

class Text(HTMLParser):
    """Plain text of the rendered HTML, with list markers as a PDF text extraction shows them."""
    def __init__(self): super().__init__(); self.out = []; self.stack = []; self.bullets = 0; self.numbers = []
    def handle_starttag(self, tag, attrs):
        if tag == "ul": self.stack.append(None)
        elif tag == "ol": self.stack.append(int(dict(attrs).get("start", 1)))
        elif tag == "li" and self.stack:
            if self.stack[-1] is None: self.bullets += 1
            else: self.numbers.append("%d." % self.stack[-1]); self.stack[-1] += 1
    def handle_endtag(self, tag):
        if tag in ("ul", "ol") and self.stack: self.stack.pop()
    def handle_data(self, d): self.out.append(d)

def squash(s): return re.sub(r"\s+", "", s)

def slug_headings(body):
    toc = []; n = [0]
    def rep(m):
        n[0] += 1; hid = "h%d" % n[0]
        if m.group(1) == "2": toc.append((hid, re.sub(r"<[^>]+>", "", m.group(2))))
        return '<h%s id="%s">%s</h%s>' % (m.group(1), hid, m.group(2), m.group(1))
    return re.sub(r"<h([23])>(.*?)</h\1>", rep, body, flags=re.S), toc

def render(md_path, pdf_path, module=None, kind=None):
    src = md_path.read_text(encoding="utf-8")
    body = MD.render(src)
    body, toc = slug_headings(body)
    t = Text(); t.feed(body); source_text = squash("".join(t.out))
    h1 = re.search(r"<h1>(.*?)</h1>", body, flags=re.S)
    doc_title = html.unescape(re.sub(r"<[^>]+>", "", h1.group(1))) if h1 else md_path.stem
    comp = COMPONENT_RE.search("\n".join(src.splitlines()[:12]))
    if module:
        m = MODULES[module]
        title = "PCP 1.0 %s | ACR %s %s" % (THEME["name"], m["name"], kind)
        vars_ = "--bg:%s;--fg:%s;--accent:%s;--ink:%s;--th:%s;--soft:%s" % (THEME["field"], THEME["text"], THEME["sig"], THEME["ink"], THEME["th"], THEME["soft"])
        tag_top = 136 + 52 * len(m["title"]) + 22
        note2 = "An informative guide; the Specification governs requirements." if kind == "Overview" else "Normative requirements and conditional contracts."
        cover = ('<section class="cover"><div class="label">PARROT COUNCIL PROTOCOL (PCP)&nbsp; /&nbsp; VERSION 1.0</div><div class="bar"></div>'
                 '<p class="title">%s</p><div class="plate" style="background:%s">%s</div>'
                 '<div class="tag" style="top:%dpt">Evidence before agreement.<br>%s</div><div class="rule"></div>'
                 '<div class="kind">%s %s</div><div class="chip">%s</div><div class="who">Kirtus Justus</div><div class="credit">%s</div>'
                 '<div class="meta">11 November 2026&nbsp; |&nbsp; America/Vancouver<br>Component %s&nbsp; |&nbsp; Version %s</div>'
                 '<div class="notes">%s<br>%s<br>%s</div></section>') % (
            "<br>".join(m["title"]), m["bg"], bird_svg(m, 90), tag_top, m["tag"], m["name"], kind, THEME["name"].upper(), m["credit"], comp.group(1), comp.group(2), m["note"], note2, EVID)
        tocs = '<section class="toc"><h2>Contents</h2><div class="sub">ACR %s %s / PCP 1.0 / %s</div>%s</section>' % (
            m["name"], kind, THEME["name"], "".join('<a href="#%s">%s</a>' % (i, t_) for i, t_ in toc))
        runs = ('<div class="run runhead">PCP / ACR %s</div><div class="run runright">1.0 | %s</div>'
                '<div class="run runfoot">Kirtus Justus / %s %s / %s</div>') % (m["name"].upper(), kind, comp.group(1), comp.group(2), THEME["name"])
        front = cover + tocs
    else:
        title = "PCP 1.0 %s | %s" % (THEME["name"], md_path.stem.replace("_", " ").title())
        vars_ = "--bg:#fff;--fg:#18222D;--accent:%s;--ink:%s;--th:%s;--soft:#5B6B7A" % (THEME["ink"], THEME["ink"], THEME["th"])
        runs = ('<div class="run runhead">PCP / ADVERSARIAL CLAIM REVIEW</div><div class="run runright">1.0 | %s</div>'
                '<div class="run runfoot">Kirtus Justus / %s</div>') % (html.escape(md_path.stem.replace("_", " ")), THEME["name"])
        front = '<div class="plainhead">PCP&nbsp; /&nbsp; ADVERSARIAL CLAIM REVIEW&nbsp; /&nbsp; VERSION 1.0&nbsp; /&nbsp; %s</div>' % THEME["name"].upper()
    doc = ('<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>%s</title><meta name="author" content="Kirtus Justus">'
           '<meta name="description" content="Generated from %s"><style>:root{%s}%s</style></head><body>%s%s<main>%s</main></body></html>') % (
        html.escape(title), md_path.name, vars_, CSS, runs, front, body)
    ident = hashlib.sha256(doc.encode()).digest()[:16]
    pdf_path.parent.mkdir(parents=True, exist_ok=True)
    HTML(string=doc, base_url=str(md_path.parent)).write_pdf(str(pdf_path), font_config=FontConfiguration(), pdf_variant="pdf/ua-1", pdf_identifier=ident)
    if not module: source_text = squash("PCP / ADVERSARIAL CLAIM REVIEW / VERSION 1.0 / " + THEME["name"].upper()) + source_text
    return (source_text, t.bullets, "".join(t.numbers)), 2 if module else 0

def jobs():
    out = []
    for d in sorted((ROOT / "MODULES").iterdir()):
        for kind in ("Overview", "Specification"):
            out.append((d / "SOURCE" / (kind + ".md"), d / (kind + ".pdf"), d.name, kind))
    for p in sorted((ROOT / "SUPPLEMENTS/Core_Conditional").glob("*.md")) + sorted((ROOT / "GUIDES").glob("*.md")) + [ROOT / "START_HERE.md"]:
        out.append((p, p.with_suffix(".pdf"), None, None))
    if GUIDE: out.append((GUIDE, ROOT / "EDITION" / "Edition_Guide.pdf", None, None))
    return out

def check(pdf_path, source_text, skip):
    import pymupdf
    d = pymupdf.open(str(pdf_path))
    clip = None
    parts = []
    for i, page in enumerate(d):
        if i < skip: continue
        r = page.rect
        parts.append(page.get_text("text", clip=pymupdf.Rect(0, 60, r.width, r.height - 55)))
    cat = d.pdf_catalog(); tagged = "StructTreeRoot" in d.xref_object(cat)
    got = squash("".join(parts)).replace("\u00a0", "")
    source_text, bullets, numbers = source_text
    # List markers are extracted out of reading order; compare the text without them and count them separately.
    ok_bullets = got.count("\u2022") == bullets
    got = got.replace("\u2022", "")
    # List markers are extracted out of reading order. Align the two texts, allowing a run of markers to be
    # skipped in the extracted text; where a run begins with the digit the source expects next, both readings
    # are tried (backtracking), and the skipped runs must equal the source's markers in order.
    num = re.compile(r"(?:\d{1,2}\.)+")
    def cands(k):
        m = num.match(got, k); out = []
        if m:
            run = m.group(0)
            while run: out.append(run); run = re.sub(r"\d{1,2}\.$", "", run)
        return out
    i = j = 0; skipped = []; stack = []; far = (0, 0); done = False
    while True:
        if (i, j) > far: far = (i, j)
        if i == len(source_text) and j == len(got) and "".join(skipped) == numbers: done = True; break
        if i < len(source_text) and j < len(got) and source_text[i] == got[j]:
            c = cands(j)
            if c: stack.append((i, j, len(skipped), c))
            i += 1; j += 1; continue
        c = cands(j) if j < len(got) else []
        took = False
        while True:
            if c:
                run = c.pop(0)
                if c: stack.append((i, j, len(skipped), c))
                skipped.append(run); j += len(run); took = True; break
            if not stack: break
            i, j, n, c = stack.pop(); del skipped[n:]
        if not took: break
    same = done and ok_bullets
    i, j = far
    if not same: got = got[max(0, j - 60):j + 60] + " <<< expected >>> " + source_text[max(0, i - 60):i + 60]
    return same, d.page_count, tagged, d.metadata.get("title"), got


if __name__ == "__main__":
    only = sys.argv[4:]
    report = []
    for mdp, pdfp, module, kind in jobs():
        if only and not any(o in str(pdfp) for o in only): continue
        st, skip = render(mdp, pdfp, module, kind)
        ok, pages, tagged, title, got = check(pdfp, st, skip)
        if not ok: print("  MISMATCH", got)
        report.append({"pdf": pdfp.relative_to(ROOT).as_posix(), "source": mdp.relative_to(ROOT).as_posix() if ROOT in mdp.parents else "EDITION_GUIDE.md", "pages": pages,
                       "body_text_equals_source": ok, "tagged": tagged, "title": title})
        print(("OK  " if ok else "DIFF"), pages, tagged, pdfp.relative_to(ROOT))
    Path("book_build_report_%s.json" % EDITION).write_text(json.dumps(report, indent=1))
