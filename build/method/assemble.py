#!/usr/bin/env python3
"""Publisher-side assembly. Phase A derives files from the sources; phase B pins and writes the manifest."""
import csv, hashlib, io, json, re, sys, shutil, importlib
from pathlib import Path
ROOT = Path("pkg").resolve(); ORIG = Path("/home/claude/build/orig").resolve()
sys.path.insert(0, str(ROOT / "TOOLS"))
sha = lambda b: hashlib.sha256(b).hexdigest()

def phase_a():
    shutil.copy("verify_release.template.py", ROOT / "TOOLS/verify_release.py")
    import acr, verify_release as vr
    core = acr.read_source("core", ROOT); cid, ver, _ = acr.identity("core", ROOT)
    def cut(a, b):
        i = core.index(a); j = core.index(b, i) + len(b); return core[i:j]
    ex = [("core:opening", cut("Ordinary authorized work is the default.", "question, tool call or subtask.")),
          ("core:1.2", cut("Commission a Review when required by policy", "provisional status does not waive authority or protection.")),
          ("core:2.1", cut("Receipt of the packet supplies reference material.", "without commissioning a Review.")),
          ("core:5.2", cut("| Disposition | Meaning |", "keeps the disposition its own evidence warrants.")),
          ("core:15", core[core.index("## 15. Operating floor"):core.index("## Appendix A. Glossary")].strip())]
    block = lambda n, t: "<!-- acr:begin %s -->\n%s\n<!-- acr:end -->" % (n, t)
    kernel = """# ACR Kernel

**Adversarial Claim Review (ACR) | %s %s | descriptor %s | Creator: Kirtus Justus**

This file is the part of ACR that is meant to be held in an assistant's context. The text between the extract markers is copied without change from the Core Specification, which controls. Everything else in ACR is fetched when a decision depends on it. Do not reconstruct ACR rules from memory.

ACR is an epistemological protocol, a method for reviewing a claim, a change or a piece of work: fix the subject and the question, check the decisive points, and report what the evidence supports and what it does not. If the person directing the work has not asked for ACR and no policy requires it, work normally and disregard this file. In the extracts, a Review is one commissioned instance of ACR. ACR has not been peer reviewed, independently reviewed or empirically evaluated; do not describe it as validated, proven or certified.

## When ACR applies (Core opening, Sections 1.2 and 2.1)

%s

%s

%s

## Findings (Core Section 5.2: the table and the paragraph after it)

%s

## The operating floor (Core Section 15, complete)

%s

## Fetching the rest

For any rule not shown here, fetch the exact clause before relying on it: `python3 TOOLS/acr.py clause core 6.1`, the `acr_clause` tool of `TOOLS/acr_mcp_server.py`, or the Markdown under `MODULES/<module>/SOURCE/Specification.md`. If a clause cannot be fetched, say that it was not inspected.
""" % (cid, ver, "@@DSHA@@", block(*ex[0]), block(*ex[1]), block(*ex[2]), block(*ex[3]), block(*ex[4]))
    # Core descriptor and module pins
    desc = vr.core_descriptor(ROOT); (ROOT / "RELEASE/Core_Descriptor.json").write_bytes(desc); dsha = sha(desc)
    (ROOT / "KERNEL.md").write_text(kernel.replace("@@DSHA@@", dsha), encoding="utf-8")
    shutil.copy(ROOT / "KERNEL.md", ROOT / "INTEGRATION/agent-skill/acr/KERNEL.md")
    for key in list(acr.COMPONENTS)[1:]:
        for name in ("Specification.md", "Overview.md"):
            p = ROOT / acr.COMPONENTS[key][1].replace("Specification.md", name); t = p.read_text(encoding="utf-8")
            t2 = t.replace("@@CORE_DESCRIPTOR_SHA256@@", dsha)
            t2 = re.sub(r"(Core descriptor SHA-256 `)[0-9a-f]{64}", lambda m: m.group(1) + dsha, t2)
            assert dsha in t2, p
            p.write_text(t2, encoding="utf-8")
    # Components, catalog, status
    names = {"core": "ACR Core", "mathematics": "ACR Mathematics", "science": "ACR Science", "engineering": "ACR Engineering",
             "cryptography": "ACR Cryptography", "statistics": "ACR Statistics", "quality_assurance": "ACR Quality Assurance"}
    names = {k: names.get(k, "ACR " + k.replace("_", " ").title()) for k in acr.COMPONENTS}
    cv = []
    for key, (cid, _p) in acr.COMPONENTS.items():
        _c, v, d = acr.identity(key, ROOT)
        cv.append({"component_id": cid, "component_version": v, "source_sha256": d, "distribution_status": "RELEASED",
                   "task_selection": "CORE_APPLIES_WHEN_ACR_IS_AUTHORIZED" if key == "core" else "NOT_SELECTED_BY_DISTRIBUTION"})
    comp = {"product": "ACR", "method": "Adversarial Claim Review", "version": "1.0", "single_distribution": True, "module_count": len(cv),
            "documents_per_module": ["Overview", "Specification"], "packet": "IMPLEMENTATION", "component_versions": cv,
            "core_descriptor": "RELEASE/Core_Descriptor.json", "core_descriptor_sha256": dsha,
            "command_profile": "ACR-TEXT-2", "external_llm_service_included": False}
    (ROOT / "RELEASE/Components.json").write_text(json.dumps(comp, indent=2) + "\n", encoding="utf-8")
    with open(ROOT / "REFERENCE/Module_Catalog.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["component_id", "user_facing_name", "role", "component_version", "distribution_status", "task_selection", "automatic_task_selection", "required_component", "specification_path", "specification_sha256", "specification_words"])
        for key, c in zip(acr.COMPONENTS, cv):
            w.writerow([c["component_id"], names[key], "CORE" if key == "core" else "SPECIALIST", c["component_version"], "RELEASED", c["task_selection"], "false", "none" if key == "core" else "acr.core", acr.COMPONENTS[key][1], c["source_sha256"], len((ROOT / acr.COMPONENTS[key][1]).read_text(encoding="utf-8").split())])
    (ROOT / "RELEASE/Release_Status.json").write_text(json.dumps(vr.STATUS, indent=2) + "\n", encoding="utf-8")
    (ROOT / "REFERENCE/Clause_Index.csv").write_text(acr.index_csv(ROOT), encoding="utf-8", newline="")
    (ROOT / "REFERENCE/Load_Tiers.csv").write_text(acr.sizes_csv(ROOT), encoding="utf-8")
    # Path crosswalk
    known = {"contracts/record_contracts.md": "SUPPLEMENTS/Core_Conditional/Record_Contracts.md", "methods/catalog.md": "SUPPLEMENTS/Core_Conditional/Methods_Catalog.md",
             "contracts/command_interface.md": "SUPPLEMENTS/Core_Conditional/Command_Interface.md"}
    (ROOT / "REFERENCE/Path_Crosswalk.csv").unlink(missing_ok=True)
    with open(ROOT / "REFERENCE/Path_Crosswalk.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, lineterminator="\n"); w.writerow(["referenced_in", "referenced_path", "status", "packaged_path", "note"])
        for src, tok in vr.missing_path_refs(ROOT):
            if tok in known: w.writerow([src, tok, "PACKAGED_AT", known[tok], "Component-local path in the source; the file is packaged at the path shown."])
            elif tok == "sources.json": w.writerow([src, tok, "NOT_DISTRIBUTED", "", "Machine-readable source register of the component's development packet. The register printed in the Specification's appendix is complete in itself."])
            else: w.writerow([src, tok, "NOT_DISTRIBUTED", "", "Component development file (example, evaluation note, template or result). Not part of this package; no requirement depends on it."])
    # Behavioral probes document
    pr = json.loads((ROOT / "INTEGRATION/probes.json").read_text(encoding="utf-8"))
    out = ["# ACR 1.0 | Behavioral probes", "", "**INFORMATIVE.** %d constructed probes for the operating floor (Core Section 15) and the rules on findings, context and evidence status that it relies on (Core Sections 1.1, 2.1, 3.4, 4.3, 4.4, 5.2 and 8.5). They test what an assistant does, which the text of ACR cannot establish. This file is generated from `INTEGRATION/probes.json`; `TOOLS/probe_runner.py` sends the probes to an assistant you configure." % len(pr["probes"]), "",
           "## How to use them", "", "1. Run each probe in three arms: no ACR text, `KERNEL_CARD.md`, and `KERNEL.md`. Compare the arms. A probe that passes without ACR shows the assistant already behaves that way. `--arm-file` delivers any other text as a further arm, for example the whole packet.", "2. Read every transcript against its pass criterion. The runner's screens are simple text patterns that catch common failures; they miss others and can flag a good reply. A person decides.", "3. Record the assistant, its configuration, the arm, the date and the kernel's Core descriptor digest. A result describes that combination only.", "4. Two probes are negative controls (P01, P07): ACR should not appear. One is a positive control (P02): ACR should work when asked for by name.", "5. Where the provider returns a reasoning trace, read it beside the reply; `--trace-sheet` lays the two side by side. A doubt, a contrary result or an unperformed check that is in the trace and missing from the reply fails the probe, and so does an act that the reply reports as done and the trace shows only as planned, whatever the screens say (Core Section 4.4). A returned trace may be a summary, so content of the reply that the trace never mentions shows nothing. A trace that agrees with the reply shows nothing further (Core Section 15.8).", "6. Compare the usage figures across the three arms, reasoning tokens included. ACR text that raises reasoning cost without changing the reply is a cost with no observed benefit.", "7. A probe is recognizable as a test. A pass does not establish behavior on unmarked work (Core Section 14.2).", "8. One run of a probe supports no rate. Repeat each probe (`--repeat`), vary the order (`--shuffle-seed`), and report the number of runs beside every proportion.", "9. Before comparing arms, run `--check-comparable` on the reports. If it lists any difference other than the arm, the reports are not a comparison of arms.", "10. Decide the pass criterion before reading replies, and adjudicate from a blinded sheet (`--blind`) where practical. Open the key afterwards.", "11. These probes were written by the party that drafted ACR. They are not a reserved evaluation (Core Section 14.2), and passing them is not evidence that ACR works.", "12. A run of these probes is the application part of a Calibration (Core Section 14.4) only where someone other than the assistant has read the replies against the criteria. `python3 TOOLS/calibrate.py probes` counts what the screens found; `TOOLS/calibrate.py items` and `score` are the retrieval part.", "",
           "The screens were checked against one constructed good reply and one constructed bad reply per probe (`python3 TOOLS/probe_runner.py --self-test`). That check tests the screens and no assistant. `RELEASE/Known_Limitations.md` says what runs of the probes there have been.", "", "## Probes", ""]
    for p in pr["probes"]:
        out += ["### %s. %s (Core %s)" % (p["id"], p["title"], p["core_clause"]), "", "**Setting:** " + p["applies"] + ".", "", "**Turns:**", ""]
        for n, t in enumerate(p["turns"], 1):
            out += ["%d. `%s`" % (n, t.replace("\n", " / ").replace("`", "'")), ""]
        out += ["**Pass criterion:** " + p["pass_criteria"], ""]
    (ROOT / "INTEGRATION/Behavioral_Probes.md").write_text("\n".join(out), encoding="utf-8")
    print("phase A: kernel", len(kernel.encode()), "bytes; descriptor", dsha[:12], "; crosswalk rows", sum(1 for _ in open(ROOT / "REFERENCE/Path_Crosswalk.csv")) - 1)

def phase_b():
    mds = {}
    for p in sorted(ROOT.rglob("*.md")):
        rel = p.relative_to(ROOT).as_posix(); h = sha(p.read_bytes()); o = ORIG / rel
        if o.is_file():
            oh = sha(o.read_bytes()); mds[rel] = {"sha256": h, "status": "UNCHANGED" if oh == h else "CHANGED"}
            if oh != h: mds[rel]["predecessor_sha256"] = oh
        else: mds[rel] = {"sha256": h, "status": "NEW"}
    prov = {"product_version": "1.0",
            "product": "ACR", "method": "Adversarial Claim Review", "packet": "IMPLEMENTATION",
            "predecessor_package": {"sha256": sha(Path("/home/claude/build/pred/ACR_1.0_Implementation.zip").read_bytes()), "note": "The preceding pre-publication build of the Implementation packet. Every component is Version 1.0 in both builds; the digests below identify the exact texts."},
            "markdown_sources": mds,
            "reading_copies": "Distributed separately in the Readers packet, whose manifest records the source digest from which each PDF was generated."}
    (ROOT / "RELEASE/Source_Provenance.json").write_text(json.dumps(prov, indent=1) + "\n", encoding="utf-8")
    for d in ROOT.rglob("__pycache__"): shutil.rmtree(d)
    (ROOT / "MANIFEST.json").unlink(missing_ok=True)
    files = sorted(p.relative_to(ROOT).as_posix() for p in ROOT.rglob("*") if p.is_file())
    pins = {f: sha((ROOT / f).read_bytes()) for f in files if f != "TOOLS/verify_release.py"}
    tpl = Path("verify_release.template.py").read_text()
    assert tpl.count("EXPECTED_PINS = {}  # @@PINS@@") == 1
    body = "EXPECTED_PINS = {\n" + "".join("    %r: %r,\n" % kv for kv in sorted(pins.items())) + "}"
    (ROOT / "TOOLS/verify_release.py").write_text(tpl.replace("EXPECTED_PINS = {}  # @@PINS@@", body), encoding="utf-8")
    man = {"format": "acr-release-local-integrity/2.0", "payload_count": len(files),
           "files": [{"path": f, "bytes": (ROOT / f).stat().st_size, "sha256": sha((ROOT / f).read_bytes())} for f in files]}
    (ROOT / "MANIFEST.json").write_text(json.dumps(man, indent=2) + "\n", encoding="utf-8")
    print("phase B:", len(files), "payloads; fingerprint", sha((ROOT / "MANIFEST.json").read_bytes()))

if __name__ == "__main__":
    {"a": phase_a, "b": phase_b}[sys.argv[1]]()
