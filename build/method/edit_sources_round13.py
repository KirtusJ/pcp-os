#!/usr/bin/env python3
"""Thirteenth round: the method's lookup tool and its evidence statement. No Specification, Overview, guide,
kernel or probe changes. Each edit names the exact words it replaces and fails if they are not found exactly
once, so the script cannot be applied to the wrong text.

    python3 edit_sources_round13.py <packet dir> <build dir holding assemble.py and edition_texts.py>
"""
import sys
from pathlib import Path
PKG, BUILD = Path(sys.argv[1]), Path(sys.argv[2])
def edit(path, pairs, base=PKG):
    p = base / path; t = p.read_text(encoding="utf-8")
    for a, b in pairs:
        assert t.count(a) == 1, (path, a[:80], t.count(a))
        t = t.replace(a, b)
    p.write_text(t, encoding="utf-8")

# 1. The lookup tool. Two faults, both reachable only in a packet someone has altered or extended:
#    a version was cut to its first two numbers (2.4.1-rc.2 was read as 2.4), and where two folders declared
#    one identifier the first in folder order was served. The PCP OS host program was corrected for both in
#    build 17; the packet's own tool now follows the same two rules.
edit("TOOLS/acr.py", [
 ('''VERSION_RE = re.compile(r"\\*\\*Component:\\*\\* `(acr\\.[a-z_]+)`, Version (\\d+\\.\\d+)")


def discover(root=ROOT):
    """The modules of a packet, read from the header of each Specification under MODULES, in folder order.
    Adding a module is adding a folder: nothing here lists them."""
    out = {}
    for spec in sorted((Path(root) / "MODULES").glob("*/SOURCE/Specification.md")):
        m = VERSION_RE.search(spec.read_text(encoding="utf-8")[:6000])
        if m and m.group(1)[4:] not in out:
            out[m.group(1)[4:]] = (m.group(1), spec.relative_to(root).as_posix())
    return out


COMPONENTS = discover()
''', '''COMPONENT_RE = re.compile(r"^\\*\\*Component:\\*\\* `(acr\\.[a-z_]+)`, Version (\\S+)", re.M)
VERSION_OK = re.compile(r"[0-9][0-9A-Za-z.+-]{0,63}")


def header(text):
    """(identifier, version) from the component line in a Specification's first 60 lines, or None.
    The version is the whole field as written, so that 1.0.1 and 2.4.1-rc.2 are not read as shorter ones."""
    m = COMPONENT_RE.search("\\n".join(text.splitlines()[:60]))
    if not m:
        return None
    version = m.group(2).rstrip(".,;")
    return (m.group(1), version) if VERSION_OK.fullmatch(version) else None


def discover(root=ROOT, ambiguous=None):
    """The modules of a packet, read from the header of each Specification under MODULES, in folder order.
    Adding a module is adding a folder: nothing here lists them. An identifier that two folders declare is
    served from neither, because which text applies must never depend on the order of folder names; the
    folders are put in `ambiguous` under the module's key."""
    out, twice = {}, {}
    for spec in sorted((Path(root) / "MODULES").glob("*/SOURCE/Specification.md")):
        h = header(spec.read_text(encoding="utf-8"))
        if not h:
            continue
        key, rel = h[0][4:], spec.relative_to(root).as_posix()
        if key in twice:
            twice[key].append(rel)
        elif key in out:
            twice[key] = [out.pop(key)[1], rel]
        else:
            out[key] = (h[0], rel)
    if ambiguous is not None:
        ambiguous.update(twice)
    return out


AMBIGUOUS = {}
COMPONENTS = discover(ambiguous=AMBIGUOUS)
'''),
 ('''    key = ALIASES.get(key, key)
    if key not in COMPONENTS and key not in CONDITIONAL:''', '''    key = ALIASES.get(key, key)
    if key in AMBIGUOUS:
        raise AcrError("%r is declared by more than one folder (%s); the identifier is ambiguous and none of them is used"
                       % (name, ", ".join(AMBIGUOUS[key])))
    if key not in COMPONENTS and key not in CONDITIONAL:'''),
 ('''    m = VERSION_RE.search("\\n".join(text.splitlines()[:12]))
    return m.group(2) if m else None''', '''    h = header(text)
    return h[1] if h else None'''),
 ('''            import command_interface
''', '''            try:
                import command_interface
            except ImportError:
                raise AcrError("TOOLS/command_interface.py is not beside this tool; nothing was parsed")
'''),
])
edit("TOOLS/test_acr_tools.py", [
 ('''import json, subprocess, sys, unittest
''', '''import json, subprocess, sys, tempfile, unittest
'''),
 ('''    def test_fidelity(self):
''', '''    def test_module_headers(self):
        self.ok(not acr.AMBIGUOUS and len(acr.COMPONENTS) == 7 and all(acr.identity(k)[1] == "1.0" for k in acr.COMPONENTS))
        line = "**Component:** `acr.%s`, Version %s"
        for written, read in (("1.0  ", "1.0"), ("2.4.1-rc.2+build.4", "2.4.1-rc.2+build.4"), ("1.0.1.", "1.0.1"), ("3", "3")):
            self.ok(acr.header("# T\\n\\n" + line % ("sample", written) + "\\n") == ("acr.sample", read), written)
        for written in ("v1", "", "1.0/2", "1" * 65):
            self.ok(acr.header("# T\\n\\n" + line % ("sample", written) + "\\n") is None, written)
        self.ok(acr.header("\\n" * 60 + line % ("sample", "1.0")) is None, "a component line below the header was read")
        with tempfile.TemporaryDirectory() as d:
            for folder, key, version in (("01_one", "sample", "2.4.1-rc.2"), ("02_two", "sample", "2.5.0"), ("03_three", "other", "1.0"),
                                         ("04_four", "third", "1.0"), ("05_five", "third", "1.0"), ("06_six", "third", "1.0")):
                p = Path(d) / "MODULES" / folder / "SOURCE"
                p.mkdir(parents=True)
                (p / "Specification.md").write_text("# T\\n\\n" + line % (key, version) + "\\n", encoding="utf-8")
            twice = {}
            found = acr.discover(Path(d), twice)
            self.ok(list(found) == ["other"], "an identifier two folders declare was served: %s" % list(found))
            self.ok(sorted(twice) == ["sample", "third"] and len(twice["sample"]) == 2 and len(twice["third"]) == 3, twice)

    def test_fidelity(self):
'''),
])

# 2. The evidence statement. Item 3 said the probes had been run on no assistant. That was true on
#    11 November 2026 and stopped being true the next day.
ITEM_3_OLD = "No assistant, model or host has been tested against it: the behavioral probes %s have been run on no assistant."
ITEM_3_NEW = ("No assistant, model or host has been tested against ACR under conditions that support a conclusion. The behavioral probes %s "
              "were run on 12 November 2026 in uncontrolled sessions only: by the party that drafted ACR, with assistants of one provider, most probes "
              "once or twice, with screens that are text patterns and replies read by that same party. Those runs show that the probes can be delivered "
              "and screened. They support no rate and no conclusion about effect, and they are not the evaluation this item says is missing.")
edit("RELEASE/Known_Limitations.md", [
 ("As of 11 November 2026:", "As of 12 November 2026:"),
 (ITEM_3_OLD % "in this packet", ITEM_3_NEW % "in this packet"),
])
edit("assemble.py", [
 ("No assistant was tested in preparing this release.", "That check tests the screens and no assistant. `RELEASE/Known_Limitations.md` says what runs of the probes there have been."),
], base=BUILD)
edit("edition_texts.py", [
 ('DATE = "11 November 2026"', 'DATE = "12 November 2026"'),
 (ITEM_3_OLD % "supplied in the Implementation packet", ITEM_3_NEW % "supplied in the Implementation packet"),
 ("The Implementation packet carries the same statement for the method (`RELEASE/Known_Limitations.md`).",
  "The Implementation packet carries the method's own statement (`RELEASE/Known_Limitations.md`), which says the same of the method and does not speak of the product's plugins."),
], base=BUILD)
edit("assemble_editions.py", [('"build_date": "2026-11-11"', '"build_date": "2026-11-12"')], base=BUILD)
print("round 13 edits applied")
