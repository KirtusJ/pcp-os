"""Reconstruct the base editions (the method build's output) from a release's editions by inverting
assemble_release.py's one-for-one text replacements. Proof of correctness: reassembling reproduces the release.
For a release of build 21 or later (the evidence statement gained an item in build 21). An earlier release is rebased with the script in its own source archive."""
import sys, shutil, re, json
from pathlib import Path
X, BASE, OS, ASM = (Path(a) for a in sys.argv[1:5])
src = ASM.read_text()
def inv(t, a, b):
    assert t.count(b) == 1, b[:60]; return t.replace(b, a)
for name in ("Personal", "Commercial", "Enterprise"):
    e = X / f"PCP_1.0_{name}_Edition" / f"PCP_1.0_{name}_Edition"
    d = BASE / f"PCP_1.0_{name}_Edition" / f"PCP_1.0_{name}_Edition"
    d.mkdir(parents=True, exist_ok=True)
    for f in ("ACR_1.0_Implementation.zip", "PCP_1.0_Readers.zip", "EDITION_GUIDE.md", "PRICING.md", "EDITION.json"):
        shutil.copy(e / f, d / f)
    lic = (e / "LICENSE.md").read_text()
    lic = inv(lic, "the ACR 1.0 Implementation packet, the PCP 1.0 Readers packet and the documents supplied with them in this edition.",
              "the ACR 1.0 Implementation packet, the PCP 1.0 Readers packet, the PCP plugins `pcp` and `pcp-os` with the PCP OS host program, the machine form and design tokens, the PCP OS documents and backgrounds, and the documents supplied with them in this edition.")
    lic = inv(lic, "(b) run the programs in the Implementation packet;", "(b) run the programs supplied in the Materials;")
    (d / "LICENSE.md").write_text(lic)
    n = (e / "NOTICE.md").read_text()
    n = inv(n, "the Implementation packet (`ACR_1.0_Implementation.zip`) and the Readers packet (`PCP_1.0_Readers.zip`).",
            "the Implementation packet (`ACR_1.0_Implementation.zip`), the Readers packet (`PCP_1.0_Readers.zip`), the plugins and the PCP OS host program (`PCP_1.0_Plugins/`), the machine form and design tokens (`PCP_1.0_Machine/`), and the PCP OS documents and backgrounds (`PCP_1.0_OS/`).")
    n = inv(n, "That is a manner of delivery, and it grants nothing further.", "The PCP OS host program is delivered compiled, for Linux on x86_64 and on 64-bit ARM. Each is a manner of delivery, and it grants nothing further.")
    n = inv(n, '"Parrot Council Protocol", "PCP" and the bird emblem identify this product,', '"Parrot Council Protocol", "PCP", "PCP OS" and the bird emblem identify this product,')
    n = inv(n, "The programs in the Implementation packet use only the Python standard library. No other third-party code, text or artwork is included.",
            "The programs in the Implementation packet use only the Python standard library. The PCP OS host program is compiled with the Rust standard library and a small number of open-source components, each under the MIT License or the Apache License 2.0; `THIRD_PARTY_LICENSES.md` lists them with their notices. No other third-party code, text or artwork is included.")
    n = inv(n, "The texts were drafted with AI systems", "The texts and programs were drafted with AI systems")
    (d / "NOTICE.md").write_text(n)
    ev = (e / "EVIDENCE_STATUS.md").read_text()
    m = re.search(r"5\. \*\*The plugins in live sessions\.\*\*.*?\n7\. \*\*What the release checks are\.\*\*", ev, re.S)
    print(name, "live sessions phrase:", re.search(r"were run in (.*?) uncontrolled", m.group(0)).group(1))
    ev = ev.replace(m.group(0), "5. **What the release checks are.**")
    ev = inv(ev, "6. **What the cited sources are.**", "8. **What the cited sources are.**")
    (d / "EVIDENCE_STATUS.md").write_text(ev)
    v = (e / "verify_edition.py").read_text()
    # the three patches, read out of assemble_release.py itself
    body = src[src.index("def patch_verifier(src):"):src.index("def edition_evidence")]
    ns = {}
    calls = []
    exec(body.replace("t = rep(t,", "t = cap(t,"), {"cap": lambda t, a, b: (calls.append((a, b)), t)[1]}, ns)
    ns["patch_verifier"]("")
    for a, b in reversed(calls):
        v = inv(v, a, b)
    (d / "verify_edition.py").write_text(v)
    shutil.copytree(e / "PCP_1.0_OS", OS / name, dirs_exist_ok=True)
