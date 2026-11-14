#!/usr/bin/env python3
"""Assemble the release: one archive per edition, each complete by itself, and the release-level index.

    python3 assemble_release.py <baseline editions dir> <plugins out> <machine out> <design dir> <cargo registry src> <release dir> <os documents out>

The method packet and the Readers packet of each edition are taken as the method build produced them
(build/method/). What this adds around them: the two plugins, the machine form, the design tokens, and the
edition documents that name them. Standard library only.
"""
import hashlib, json, os, shutil, sys, zipfile
from pathlib import Path

BASE, PLUGINS, MACHINE, DESIGN, CARGO, REL, OSDOCS = (Path(a) for a in sys.argv[1:8])
HERE = Path(__file__).resolve().parent
BUILD, BUILD_DATE, PRODUCT_VERSION = 21, "2026-11-13", "1.0"
FINGERPRINT = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707"
IMPL_SHA = "c7310f55b34ec7705b48363b95fc0839e7df96dd044c41bfd9b3183edf21abe4"
EDITIONS = {"PERSONAL": "Personal", "COMMERCIAL": "Commercial", "ENTERPRISE": "Enterprise"}
sha = lambda b: hashlib.sha256(b).hexdigest()
# What was observed in live sessions and in Calibrations, written into the evidence statement. It is kept beside this
# script as evidence.json, so that the source archive alone rebuilds the editions: until build 22 it was passed in the
# environment and was nowhere in the source, and a rebuild had to recover it from a released edition.
_EVIDENCE = Path(__file__).resolve().parent / "evidence.json"
LIVE = json.loads(os.environ["PCP_LIVE_SESSIONS"]) if os.environ.get("PCP_LIVE_SESSIONS") else (json.loads(_EVIDENCE.read_text(encoding="utf-8")) if _EVIDENCE.is_file() else {})

def rep(text, a, b, n=1):
    assert text.count(a) == n, (a[:70], text.count(a))
    return text.replace(a, b)

def zip_dir(src, name, out):
    """Deterministic archive: sorted entries, fixed timestamp. A file that is executable stays executable."""
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for p in sorted(q for q in src.rglob("*") if q.is_file() and "__pycache__" not in q.parts):
            zi = zipfile.ZipInfo(name + "/" + p.relative_to(src).as_posix(), date_time=(2026, 11, 12, 12, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.external_attr = ((0o100755 if os.access(p, os.X_OK) else 0o100644) << 16)
            z.writestr(zi, p.read_bytes())
    return sha(out.read_bytes())

# ---------------------------------------------------------------------------------------------------------
# Texts
# ---------------------------------------------------------------------------------------------------------

WHO = {
    "PERSONAL": ("one person, for that person's own personal, study and research use", "You bought it once, for US$5.00. There is no subscription.",
                 "You are the only person in every review: you direct it, and the assistant's challenge of its own case is a self-check, which the record says. Rules that need a second person cannot be met in this edition, and nothing pretends otherwise."),
    "COMMERCIAL": ("the organization named in a signed Commercial License Agreement, for its internal business use", "There is no published price: the fee and terms are in your signed Agreement.",
                   "Several people can take part. Name the Director of each review. Whoever administers an installation does not assess under it, and an auditor writes nothing. A standard that requires two different people to authorize a change is available."),
    "ENTERPRISE": ("the organization named in a signed Enterprise License Agreement, including integration into its own systems as that Agreement provides", "There is no published price: the fee and terms are in your signed Agreement.",
                   "The organization assigns roles and groups and may set a floor that every installation meets. An installation may be stricter than the floor and never laxer. Whoever administers does not assess, and an auditor writes nothing."),
}

README = """# Parrot Council Protocol (PCP) 1.0 | {name} Edition

*Evidence before agreement.*

**Parrot Council Protocol (PCP)** is the product. **Adversarial Claim Review (ACR)** is the method inside it: an epistemological protocol that specifies how a claim is examined and what its evidence is permitted to establish.

This is the whole {name} Edition, build {build} of Version 1.0, dated {date}.

## What PCP does

You give an assistant a claim and the thing the claim is about: "this function works for any list", "this study shows sleep improves scores", "this proof is complete". Under ACR the assistant fixes exactly what is being claimed and what would count for and against it before it looks, inspects the thing itself instead of a description of it, runs what can be run, records what it did and did not examine, and reports what the evidence supports. It does not report more certainty than its work reached, and it says so when a conclusion rests on memory.

ACR is used only when you ask for it. At all other times the assistant works normally.

## Why two names

| | PCP | ACR |
| --- | --- | --- |
| What it is | The product: everything in this folder | The method: the rules of a review |
| Where it appears | Covers, licenses, plugin names, PCP OS | The Specifications, the kernel, every review record |
| Varies with the edition | Yes: license, roles, page theme | Never: the same bytes in every edition |
| Identified by | Version 1.0, build {build} | The digest of its release: `{fp}` |

The method is written to stand alone. Its text names only itself, so a review can say exactly which rules it was held to, and those rules do not change with what you paid or which program delivered them. The product is everything that may differ around the method. Asking an assistant for "PCP" or for "ACR" gets the same thing.

## This edition

Licensed to {who}. {price}

{roles}

The three editions hold the same method, the same programs and the same documents. They differ in the license, in who may take part and in which roles, and in the colour of the page.

## What is in the folder

| Item | What it is | For |
| --- | --- | --- |
| `README.md` | This overview | Reading first |
| `EDITION_GUIDE.md` | How to use the method as text in any assistant | People |
| `LICENSE.md`, `NOTICE.md`, `PRICING.md` | What is permitted, who owns it, the fixed price schedule | People |
| `EVIDENCE_STATUS.md` | What has and has not been established | Reading before relying on anything |
| `ACR_1.0_Implementation.zip` | The method: Specifications, Overviews, guides, kernel, tools, probes. Named ACR throughout; no product name, no edition | Assistants and programs |
| `PCP_1.0_Readers.zip` | Typeset reading copies of every document in this edition's theme, and the brand identity | People |
| `PCP_1.0_Plugins/` | Two plugins for Claude: `pcp` (text only) and `pcp-os` (with a recorded register). Install one | Claude Code |
| `PCP_1.0_Machine/` | The release as data: every module and provision by address and digest, the state machines, schemas, design tokens | Programs and integrators |
| `PCP_1.0_OS/` | PCP OS: its Overview and Specification, each as text and as a typeset reading copy, the requirements as data, and the default backgrounds for every workspace | People; programs |
| `THIRD_PARTY_LICENSES.md` | Notices for the open-source components compiled into the PCP OS program | People |
| `EDITION.json`, `MANIFEST.json`, `verify_edition.py` | Identity, inventory and a local check | Programs |

## Three kinds of document, kept apart

| Kind | What it is | Where |
| --- | --- | --- |
| **Specification** | The rules. Precise and normative; the only text that binds | `MODULES/*/SOURCE/Specification.md` in the Implementation packet; as data in `PCP_1.0_Machine/pcp.machine.json` |
| **Overview** | Plain language, start to finish. Explains; requires nothing | `MODULES/*/SOURCE/Overview.md`; this file, for the product |
| **Guide** | How to do a particular thing | `GUIDES/` in the Implementation packet; `EDITION_GUIDE.md`; each plugin's `README.md` |

The Specifications, Overviews and guides of the method are the same in every edition. What is written per edition is this overview, the edition guide, the license and `EDITION.json`.

## The modules

Core applies whenever ACR is in use. A specialist module applies only when you select it; nothing selects one for you.

| Module | Use it for | Colour |
| --- | --- | --- |
{module_rows}

Each module has a colour profile, used wherever a workspace of that module is shown. `PCP_1.0_Machine/design/` holds the profiles; the Readers packet's `RELEASE/Brand_And_Book_Identity.md` explains the cover design.

## Starting

**With Claude Code.** Add the plugin folder as a marketplace and install one plugin:

```
/plugin marketplace add <path to this folder>/PCP_1.0_Plugins
/plugin install pcp-os@pcp-{lower}
```

Then ask for a PCP review in your own words, or type `/pcp-os:review <what> "<the claim>"`. For a science project, type `/pcp-os:workspace science` first. `pcp` is the lighter plugin: the same method as text, with no program and no record. `PCP_1.0_Plugins/README.md` says what each does on your computer. Extract this archive with a tool that keeps file permissions (`unzip` does); `verify_edition.py` tells you if the program lost its permission to run.

**With any other assistant.** Follow `EDITION_GUIDE.md`: extract the Implementation packet and give the assistant `KERNEL.md`.

## Checking this folder

`python3 verify_edition.py` checks every file against `MANIFEST.json`, that the packets and plugins are the ones `EDITION.json` names, that the method text inside each plugin is the Implementation packet's own, file for file, and runs each packet's own verifier. A pass is a local fact about these files. It authenticates nobody and certifies nothing.

## Status

Proprietary; all rights reserved; supplied under license only. Copyright (c) 2026 Kirtus Justus.

Not peer reviewed, not independently reviewed, not evaluated in use. `EVIDENCE_STATUS.md` is the full statement, and nothing here may be described as validated, proven or certified. This is a pre-publication build. The price schedule is fixed.
"""

MODULE_USE = {
    "core": "Any claim, change or piece of work. Always applies",
    "mathematics": "Proofs, derivations, computations claimed to be exact",
    "science": "Studies, experiments, measurements and what they are said to show",
    "engineering": "Designs, requirements, hazards and whether a system meets them",
    "cryptography": "Security claims about schemes, protocols and their implementations",
    "statistics": "Estimates, tests, models and what the numbers support",
    "quality_assurance": "Whether a product or process was checked against what it was meant to meet",
}

PLUGINS_README = """# Parrot Council Protocol (PCP) 1.0 | {name} Edition | Plugins for Claude

**Parrot Council Protocol (PCP)** is the product. **Adversarial Claim Review (ACR)** is the method inside it. This folder is a Claude Code marketplace named `pcp-{lower}` that holds the two PCP plugins. Version {pv}: PCP {v}, build {build}.

## Two plugins. Install one.

| Plugin | What it is | Where it runs |
| --- | --- | --- |
| `pcp` | The method as skills, subagents and the pinned method text. Text only: no program, no hook, no tool server. Nothing enforces it; the assistant follows it or does not. | Claude Code. On claude.ai and in Cowork, skills load and themes do not; not tried there. |
| `pcp-os` | The same method over the native core of PCP OS: a hash-chained register whose guards check every event in a program outside the assistant's context, module workspaces, a task view, and hooks that record what the host itself observes. | Claude Code on a computer where the program can run. The programs supplied are for Linux, on x86_64 and on 64-bit ARM; on macOS and on Windows use `pcp`. A local program does not run on claude.ai. |

Both carry the ACR 1.0 Implementation packet unchanged (manifest `{fp}`), the same bytes as `ACR_1.0_Implementation.zip` in the {name} Edition and in every other edition.

## Installing

Extract the archive with a tool that keeps file permissions (`unzip` does): `pcp-os` starts a program, and a program that has lost its permission to run does not start. Then, in Claude Code:

```
/plugin marketplace add <path to this folder>
/plugin install pcp-os@pcp-{lower}
```

or `pcp@pcp-{lower}` for the lighter plugin. Claude Code reads the plugins from this folder where it stands, so keep the folder. To move to a later build, replace the folder and run `/plugin marketplace update pcp-{lower}`. Then ask for a PCP review in your own words, or type `/pcp-os:review` or `/pcp:review`. Each plugin's own `README.md` lists its commands and says exactly what it does on your computer.

## How the edition is kept apart

Inside each plugin, the folder `edition/` is the edition: `EDITION.json` for programs and `edition.md` for the assistant. Every other file of a plugin is the same bytes in every edition, and the build refuses to finish otherwise. A plugin therefore cannot behave differently by edition except through that one folder, and the method cannot differ at all.

## Versions

The product is Version {v}. Each build of it has a number, and the plugins carry both as `{pv}`, so that Claude Code can tell a corrected build from the one it replaces. Two builds are the same only if their digests are the same.

## What the license covers

`LICENSE.md`, supplied beside this folder, names these plugins and the PCP OS program among the licensed Materials. The plugins are part of the {name} Edition whether they reach you inside the edition's archive or in the plugins archive `PCP_1.0_Plugins_{name}_Edition.zip`; the plugins archive is not a separate product and carries no separate license. Nothing in this folder is legal advice.
"""

MACHINE_README = """# PCP 1.0 | The release as data

For programs and integrators. Nothing here is needed to use PCP, and nothing here restates a rule: the Markdown Specifications in the Implementation packet govern.

| File | What it is | Standard it follows |
| --- | --- | --- |
| `pcp.machine.json` | Every module (identifier, version, digest, what it requires, how it is selected), every numbered provision as an address with the SHA-256 of its exact text and its line range, the defined terms, the load tiers, the eight state machines as tables, the kinds of event, the exit codes, and which clause each guard cites | JSON (RFC 8259). Key words counted as BCP 14 defines them (RFC 2119, RFC 8174) |
| `schemas/register-event.schema.json` | The envelope of one line of a PCP OS register | JSON Schema 2020-12 |
| `schemas/edition-profile.schema.json` | `edition/EDITION.json` inside each plugin | JSON Schema 2020-12 |
| `design/pcp.tokens.json` | The design tokens: three edition themes, seven module profiles, and the 21 workspaces they make | Design Tokens Community Group format |
| `design/pcp.tokens.css` | The same as CSS custom properties | CSS |
| `design/pcp.tokens.audit.json` | Every colour pairing measured: contrast, and separation under simulated colour-vision deficiency | WCAG 2.2 (1.4.1, 1.4.3, 1.4.11) |
| `design/pcp.terminal.tsv` | Module plates as terminal colour codes | ECMA-48 SGR, 24-bit |
| `design/themes/` | Claude Code terminal themes, one per module, dark and light | Claude Code theme files |

## Why provisions are addresses and not rule records

A rule record would need, for each requirement, an identifier, a level and the condition under which it applies. Two of the seven Specifications (Statistics, Quality Assurance) number their requirements. The other five number headings only, many requirement sentences carry no key word, and conditions are written in prose. Turning those into records would mean someone deciding what each sentence requires, which is redrafting the method. So a provision here is its module, number and title, with the digest of its exact text: a program fetches the governing words from the pinned file, and can tell at once whether they are the words a record was made under.

## Design in one paragraph

Two codes, kept apart. The **edition** sets the page: a light, a middle and a dark field, told apart by lightness first, so that they survive greyscale and colour-vision deficiency. The **module** sets the hue: seven plates, each with a wing mark and a name. A **workspace** is a module shown on an edition's page: the module's hue, moved in lightness only as far as needed to reach 4.5 to 1 against that field. Hue alone does not separate every pair of modules for every viewer; the audit file gives the figures. That is why every colour is paired with a word or a mark and no colour is the only code. Colour decides nothing in a review.
"""

def third_party():
    out = ["# Third-party components\n",
           "The PCP OS host program (`PCP_1.0_Plugins/plugins/pcp-os/servers/`, one file for Linux on x86_64 and one for Linux on 64-bit ARM) is compiled from the Licensor's own source together with the Rust standard library (MIT License or Apache License 2.0) and the open-source components below. Each is used unchanged. Their notices follow, as their licenses require. These licenses apply to those components only; they do not change the license of PCP.\n",
           "The PDF reading copies embed subsets of the typefaces Inter (SIL Open Font License 1.1) and DejaVu Sans Mono (Bitstream Vera license); see `NOTICE.md`, section 7.\n"]
    lock = (HERE.parent / "src/os-rust/Cargo.lock").read_text()
    crates = []
    cur = {}
    for line in lock.splitlines():
        if line.startswith("name = "):
            cur = {"name": line.split('"')[1]}
        elif line.startswith("version = ") and cur:
            cur["version"] = line.split('"')[1]; crates.append(cur); cur = {}
    table = ["| Component | Version | License |", "| --- | --- | --- |"]
    texts = []
    for c in crates:
        if c["name"].startswith("pcpos-"):
            continue
        d = next(iter(sorted(CARGO.glob("*/%s-%s" % (c["name"], c["version"])))), None)
        # Two programs are supplied, so every component in the lock file is compiled into one of them. Its notice must be at hand.
        assert d is not None, "the source of %s %s is not in the cargo registry; run cargo fetch" % (c["name"], c["version"])
        lic = next((l.split('"')[1] for l in (d / "Cargo.toml").read_text().splitlines() if l.startswith("license = ")), "see its files")
        table.append("| %s | %s | %s |" % (c["name"], c["version"], lic))
        for f in sorted(d.glob("LICENSE*")):
            if "APACHE" in f.name.upper():
                continue    # the Apache License text is given once below
            texts.append("## %s %s: %s\n\n```\n%s\n```\n" % (c["name"], c["version"], f.name, f.read_text(errors="replace").strip()))
    apache = next(iter(sorted(CARGO.glob("*/sha2-*/LICENSE-APACHE"))))
    return "\n".join(out + table + [""] + texts + ["## Apache License, Version 2.0 (the alternative license of the components above)\n\n```\n%s\n```\n" % apache.read_text().strip()])

def patch_verifier(src):
    t = src
    t = rep(t, '''Checks the inventory and digests of this folder against MANIFEST.json; that the
two packets are the ones EDITION.json names;''', '''Checks the inventory and digests of this folder against MANIFEST.json; that the
two packets are the ones EDITION.json names; that each plugin is whole, states
this edition and carries the method's text, each file the Implementation packet's
own; that the PCP OS programs are the ones named and may be run; that the machine
form describes that packet;''')
    t = rep(t, '''"behavioral_validation": "NOT_PERFORMED", "observational_evidence": "NONE_OF_EFFECT"}, "evidence record")''',
            '''"behavioral_validation": "NOT_PERFORMED", "observational_evidence": "NONE_OF_EFFECT",
                                "plugins_in_live_sessions": "UNCONTROLLED_SESSIONS_ONLY"}, "evidence record")''')
    t = rep(t, '''        if not quick:
            for cmd in''', '''        # Plugins: whole, of this edition, and carrying this Implementation packet and no other
        impl_files = {e["path"]: e["sha256"] for e in json.loads(im)["files"]}
        market = json.loads(text("PCP_1.0_Plugins/.claude-plugin/marketplace.json"))
        need(market.get("name") == "pcp-" + edition.lower() and [p["name"] for p in market["plugins"]] == ["pcp", "pcp-os"], "marketplace identity")
        for plugin in ("pcp", "pcp-os"):
            root = HERE / "PCP_1.0_Plugins" / "plugins" / plugin
            pm = json.loads((root / "MANIFEST.json").read_text(encoding="utf-8"))
            for e in pm["files"]:
                need(sha((root / e["path"]).read_bytes()) == e["sha256"], "plugin %s: %s differs from its manifest" % (plugin, e["path"]))
            listed = {e["path"] for e in pm["files"]}
            on_disk = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p.name != "MANIFEST.json" and "__pycache__" not in p.parts}
            need(listed == on_disk, "plugin %s: inventory differs from its manifest" % plugin)
            prof = json.loads((root / "edition" / "EDITION.json").read_text(encoding="utf-8"))
            need(prof.get("edition") == edition and prof.get("method_release", {}).get("manifest_sha256") == sha(im), "plugin %s states another edition or method release" % plugin)
            need(sha((root / "method" / "MANIFEST.json").read_bytes()) == sha(im), "plugin %s carries another method release" % plugin)
            # A plugin carries the method's text: every file of the packet outside its programs and probes, each the packet's own bytes
            held = {p.relative_to(root / "method").as_posix() for p in (root / "method").rglob("*") if p.is_file()} - {"MANIFEST.json"}
            need(held <= set(impl_files) and not any(p.startswith("INTEGRATION/") for p in held), "plugin %s: its method folder holds a file that does not belong there" % plugin)
            need(all(p in held for p in impl_files if not p.startswith(("INTEGRATION/", "TOOLS/"))), "plugin %s: a file of the method's text is missing" % plugin)
            for path in held:
                need(sha((root / "method" / path).read_bytes()) == impl_files[path], "plugin %s: method file %s is not the packet's" % (plugin, path))
            pj = json.loads((root / ".claude-plugin" / "plugin.json").read_text(encoding="utf-8"))
            need(pj.get("name") == plugin and pj.get("version") == ed.get("plugin_version") and pj.get("author", {}).get("name") == "Kirtus Justus", "plugin %s identity" % plugin)
        import os
        for g in ed["components"]["plugins"]["pcp-os"]["programs"]:
            need(sha((HERE / "PCP_1.0_Plugins" / g["file"]).read_bytes()) == g["sha256"], "a PCP OS program is not the one EDITION.json names: %s" % g["file"])
        if os.name == "posix":
            for f in ["PCP_1.0_Plugins/plugins/pcp-os/servers/pcpos"] + ["PCP_1.0_Plugins/" + g["file"] for g in ed["components"]["plugins"]["pcp-os"]["programs"]]:
                need(os.access(HERE / f, os.X_OK), "%s is not executable: the archive was extracted without its permissions. Run: chmod +x '%s'" % (f, HERE / f))
        osreq = json.loads(text("PCP_1.0_OS/PCP_OS_Requirements.json"))
        need(osreq["source_sha256"] == sha((HERE / "PCP_1.0_OS" / "PCP_OS_Specification.md").read_bytes()), "the PCP OS requirements were not generated from the Specification beside them")
        need(all((HERE / "PCP_1.0_OS" / f).is_file() for f in ("PCP_OS_Overview.pdf", "PCP_OS_Specification.pdf", "backgrounds/pcp-core.svg")), "PCP OS reading copies or backgrounds are missing")
        machine = json.loads(text("PCP_1.0_Machine/pcp.machine.json"))
        need(machine["method_release"]["manifest_sha256"] == sha(im), "the machine form describes another method release")
        for m in machine["modules"]:
            need(impl_files.get(m["specification"]["path"]) == m["specification"]["sha256"], "machine form: %s is not the packet's Specification" % m["id"])
        if not quick:
            for cmd in''')
    return t

def edition_evidence(base_text):
    # Since the thirteenth round the method build writes items 1 to 4 and the date as they stand; what is added here is the plugins.
    t = base_text
    n = LIVE.get("sessions", "a small number of")
    t = rep(t, "5. **What the release checks are.**",
            "5. **The plugins in live sessions.** On 12 and 13 November 2026 the two plugins were run in %s uncontrolled sessions of Claude Code with two models of one provider, by the party that wrote them, on tasks it chose. In those sessions the plugins loaded, the hooks fired, the host recorded typed commands, and the assistant completed recorded reviews that verified. This shows that the packages work as programs and can be followed. It is not an evaluation: there was no comparison arm, no blinding and no pre-set criterion, and it supports no rate and no conclusion about effect.%s\n6. **Calibration of the deliveries.** %s\n7. **What the release checks are.**" % (n, LIVE.get("matrix", ""), LIVE.get("calibration", "No Calibration (Core Section 14.4) of any delivery of this build has been made: the entry is NONE.")))
    t = rep(t, "6. **What the cited sources are.**", "8. **What the cited sources are.**")
    return t

def build_edition(key, name, out_root):
    src = BASE / ("PCP_1.0_%s_Edition" % name) / ("PCP_1.0_%s_Edition" % name)
    dst = out_root / ("PCP_1.0_%s_Edition" % name)
    if dst.exists():
        shutil.rmtree(dst)
    dst.mkdir(parents=True)
    for f in ("ACR_1.0_Implementation.zip", "PCP_1.0_Readers.zip", "EDITION_GUIDE.md", "PRICING.md"):
        shutil.copy(src / f, dst / f)                                   # unchanged bytes
    assert sha((dst / "ACR_1.0_Implementation.zip").read_bytes()) == IMPL_SHA
    # License: the Materials now name what is supplied. No grant, restriction, fee or term is otherwise changed.
    lic = (src / "LICENSE.md").read_text()
    lic = rep(lic, "the ACR 1.0 Implementation packet, the PCP 1.0 Readers packet and the documents supplied with them in this edition.",
              "the ACR 1.0 Implementation packet, the PCP 1.0 Readers packet, the PCP plugins `pcp` and `pcp-os` with the PCP OS host program, the machine form and design tokens, the PCP OS documents and backgrounds, and the documents supplied with them in this edition.")
    lic = rep(lic, "(b) run the programs in the Implementation packet;", "(b) run the programs supplied in the Materials;")
    (dst / "LICENSE.md").write_text(lic)
    notice = (src / "NOTICE.md").read_text()
    notice = rep(notice, "the Implementation packet (`ACR_1.0_Implementation.zip`) and the Readers packet (`PCP_1.0_Readers.zip`).",
                 "the Implementation packet (`ACR_1.0_Implementation.zip`), the Readers packet (`PCP_1.0_Readers.zip`), the plugins and the PCP OS host program (`PCP_1.0_Plugins/`), the machine form and design tokens (`PCP_1.0_Machine/`), and the PCP OS documents and backgrounds (`PCP_1.0_OS/`).")
    notice = rep(notice, "That is a manner of delivery, and it grants nothing further.",
                 "The PCP OS host program is delivered compiled, for Linux on x86_64 and on 64-bit ARM. Each is a manner of delivery, and it grants nothing further.")
    notice = rep(notice, '"Parrot Council Protocol", "PCP" and the bird emblem identify this product,', '"Parrot Council Protocol", "PCP", "PCP OS" and the bird emblem identify this product,')
    notice = rep(notice, "The programs in the Implementation packet use only the Python standard library. No other third-party code, text or artwork is included.",
                 "The programs in the Implementation packet use only the Python standard library. The PCP OS host program is compiled with the Rust standard library and a small number of open-source components, each under the MIT License or the Apache License 2.0; `THIRD_PARTY_LICENSES.md` lists them with their notices. No other third-party code, text or artwork is included.")
    notice = rep(notice, "The texts were drafted with AI systems", "The texts and programs were drafted with AI systems")
    (dst / "NOTICE.md").write_text(notice)
    (dst / "EVIDENCE_STATUS.md").write_text(edition_evidence((src / "EVIDENCE_STATUS.md").read_text()))
    (dst / "THIRD_PARTY_LICENSES.md").write_text(third_party())
    # Plugins and the machine form
    shutil.copytree(PLUGINS / name, dst / "PCP_1.0_Plugins", symlinks=False)
    pv = json.loads((dst / "PCP_1.0_Plugins/plugins/pcp/.claude-plugin/plugin.json").read_text())["version"]
    (dst / "PCP_1.0_Plugins/README.md").write_text(PLUGINS_README.format(name=name, lower=name.lower(), pv=pv, v=PRODUCT_VERSION, build=BUILD, fp=FINGERPRINT))
    m = dst / "PCP_1.0_Machine"
    shutil.copytree(MACHINE, m)
    shutil.copytree(DESIGN, m / "design")
    (m / "README.md").write_text(MACHINE_README)
    shutil.copytree(OSDOCS / name, dst / "PCP_1.0_OS")
    tokens = json.loads((DESIGN / "pcp.tokens.json").read_text())
    machine = json.loads((MACHINE / "pcp.machine.json").read_text())
    rows = []
    for mod in machine["modules"]:
        tk = tokens["module"].get(mod["key"], {})
        rows.append("| %s | %s | %s `%s`, %s |" % (tk.get("name", {}).get("$value", mod["key"]), MODULE_USE.get(mod["key"], "As its Overview states"),
                                                 {"core": "Navy", "mathematics": "Burgundy", "science": "Sky blue", "engineering": "Orange", "cryptography": "Violet", "statistics": "White", "quality_assurance": "Lime"}.get(mod["key"], "Plate"),
                                                 tk.get("plate", {}).get("$value", ""), tk.get("mark", {}).get("$value", "")))
    who, price, roles = WHO[key]
    (dst / "README.md").write_text(README.format(name=name, lower=name.lower(), build=BUILD, date="13 November 2026", fp=FINGERPRINT, who=who, price=price, roles=roles, module_rows="\n".join(rows)))
    ed = json.loads((src / "EDITION.json").read_text())
    ed["build"] = BUILD; ed["build_date"] = BUILD_DATE; ed["plugin_version"] = pv
    ed["evidence"]["plugins_in_live_sessions"] = "UNCONTROLLED_SESSIONS_ONLY"
    ed["method_release"] = {"manifest_sha256": FINGERPRINT, "revised": "2026-11-13", "predecessor_manifest_sha256": "7cb5d12758feb3b3171d5acb1386c651feae022c5907b28885275f1f845952a4", "identical_in_every_edition": True}
    ed["components"] = {
        "plugins": {"folder": "PCP_1.0_Plugins", "marketplace": "pcp-" + name.lower(),
                    "pcp": {"manifest_sha256": sha((dst / "PCP_1.0_Plugins/plugins/pcp/MANIFEST.json").read_bytes())},
                    "pcp-os": {"manifest_sha256": sha((dst / "PCP_1.0_Plugins/plugins/pcp-os/MANIFEST.json").read_bytes()),
                               "program": {"file": "plugins/pcp-os/servers/linux-x86_64/pcpos", "sha256": sha((dst / "PCP_1.0_Plugins/plugins/pcp-os/servers/linux-x86_64/pcpos").read_bytes()), "system": "linux-x86_64", "signed": False},
                               "programs": [{"file": "plugins/pcp-os/servers/%s/pcpos" % sysname, "sha256": sha((dst / "PCP_1.0_Plugins/plugins/pcp-os/servers" / sysname / "pcpos").read_bytes()), "system": sysname,
                                             "linking": "STATIC", "signed": False, "tried": tried}
                                            for sysname, tried in (("linux-x86_64", "EVERY_TEST_AND_SCENARIO_OF_THE_RELEASE"), ("linux-aarch64", "SCENARIOS_UNDER_QEMU_USER_EMULATION_ONLY"))]}},
        "machine_form": {"folder": "PCP_1.0_Machine", "sha256": sha((m / "pcp.machine.json").read_bytes())},
        "os_documents": {"folder": "PCP_1.0_OS", "specification_sha256": sha((dst / "PCP_1.0_OS/PCP_OS_Specification.md").read_bytes()),
                         "overview_sha256": sha((dst / "PCP_1.0_OS/PCP_OS_Overview.md").read_bytes()), "requirements": json.loads((dst / "PCP_1.0_OS/PCP_OS_Requirements.json").read_text())["status_counts"]},
        "design_tokens": {"file": "PCP_1.0_Machine/design/pcp.tokens.json", "sha256": sha((m / "design/pcp.tokens.json").read_bytes())},
    }
    (dst / "EDITION.json").write_text(json.dumps(ed, indent=2) + "\n")
    (dst / "verify_edition.py").write_text(patch_verifier((src / "verify_edition.py").read_text()))
    files = [{"path": p.relative_to(dst).as_posix(), "bytes": p.stat().st_size, "sha256": sha(p.read_bytes())}
             for p in sorted(q for q in dst.rglob("*") if q.is_file()) if p.name != "MANIFEST.json"]
    (dst / "MANIFEST.json").write_text(json.dumps({"format": "pcp-edition-local-integrity/1.0", "product": "Parrot Council Protocol", "product_version": PRODUCT_VERSION,
                                                   "build": BUILD, "edition": key, "payload_count": len(files), "files": files}, indent=2) + "\n")
    return dst

PLUGINS_ARCHIVE_README = """# Parrot Council Protocol (PCP) 1.0 | {name} Edition | Plugins archive

*Evidence before agreement.*

This archive holds the two PCP plugins for Claude of the {name} Edition, build {build} of Version 1.0, dated {date}, with the documents that govern them. It is a part of the {name} Edition, cut out for a licensee who wants the plugins on a computer without the rest: the folder `PCP_1.0_Plugins/` here is the same bytes as the folder of that name inside `PCP_1.0_{name}_Edition.zip`. It is not a separate product and carries no separate license.

## What is here

| Item | What it is |
| --- | --- |
| `PCP_1.0_Plugins/` | A Claude Code marketplace named `pcp-{lower}` with the plugins `pcp` (text only) and `pcp-os` (with a recorded register). Install one. Its `README.md` says what each does |
| `LICENSE.md`, `NOTICE.md`, `PRICING.md` | The {name} Edition's own, unchanged. The license lists all the Materials of the edition, including items that are not in this archive |
| `EVIDENCE_STATUS.md` | What has and has not been established. Read it before relying on anything |
| `THIRD_PARTY_LICENSES.md` | Notices for the open-source components compiled into the PCP OS program |
| `PLUGINS.json`, `MANIFEST.json`, `verify_plugins.py` | Identity, inventory and a local check |

Not here: the reading copies, `ACR_1.0_Implementation.zip` as a separate packet, the machine form and the PCP OS documents. They are in the edition's archive. The method's text is here, inside each plugin's `method/` folder, each file the packet's own bytes (manifest `{fp}`); the packet's Python programs and its probes are not.

## Installing

Extract with a tool that keeps file permissions (`unzip` does). Then, in Claude Code:

```
/plugin marketplace add <path to this folder>/PCP_1.0_Plugins
/plugin install pcp-os@pcp-{lower}
```

or `pcp@pcp-{lower}` for the lighter plugin. Then ask for a PCP review in your own words, or type `/pcp-os:review <what> "<the claim>"`. The programs supplied with `pcp-os` are for Linux, on x86_64 and on 64-bit ARM; on macOS and on Windows install `pcp`.

## Checking

`python3 verify_plugins.py` checks every file against `MANIFEST.json`, that each plugin is whole and of this edition, that the method inside each is the release named in `PLUGINS.json`, and that the program may be run. A pass is a local fact about these files. It authenticates nobody and certifies nothing.

## Status

Proprietary; all rights reserved; supplied under license only. Copyright (c) 2026 Kirtus Justus.

Not peer reviewed, not independently reviewed, not evaluated in use. `EVIDENCE_STATUS.md` is the full statement, and nothing here may be described as validated, proven or certified. This is a pre-publication build.
"""

def build_plugins_archive(key, name, edition_dir, out_root):
    """The plugins of one edition with the documents that govern them. Every file but the four written here is copied from the edition."""
    dst = out_root / ("PCP_1.0_Plugins_%s_Edition" % name)
    if dst.exists():
        shutil.rmtree(dst)
    dst.mkdir(parents=True)
    shutil.copytree(edition_dir / "PCP_1.0_Plugins", dst / "PCP_1.0_Plugins", symlinks=False)     # copytree keeps the permission to run
    for f in ("LICENSE.md", "NOTICE.md", "PRICING.md", "EVIDENCE_STATUS.md", "THIRD_PARTY_LICENSES.md"):
        shutil.copy(edition_dir / f, dst / f)
    ed = json.loads((edition_dir / "EDITION.json").read_text())
    comp = ed["components"]["plugins"]
    ident = {"format": "pcp-plugins-archive/1", "product": "Parrot Council Protocol", "abbreviation": "PCP", "method": "Adversarial Claim Review", "method_abbreviation": "ACR",
             "version": PRODUCT_VERSION, "build": BUILD, "build_date": BUILD_DATE, "creator": "Kirtus Justus", "edition": key, "edition_name": name + " Edition",
             "part_of": {"archive": "PCP_1.0_%s_Edition.zip" % name, "edition_fingerprint": sha((edition_dir / "MANIFEST.json").read_bytes()),
                         "statement": "PCP_1.0_Plugins/ here is the same bytes as PCP_1.0_Plugins/ in that edition. Not a separate product; no separate license."},
             "licensing": ed["licensing"], "license_model": ed["license_model"], "evidence": ed["evidence"],
             "plugin_version": ed["plugin_version"], "marketplace": comp["marketplace"], "method_release": {"manifest_sha256": FINGERPRINT},
             "plugins": {"pcp": comp["pcp"], "pcp-os": comp["pcp-os"]}}
    (dst / "PLUGINS.json").write_text(json.dumps(ident, indent=2) + "\n")
    (dst / "README.md").write_text(PLUGINS_ARCHIVE_README.format(name=name, lower=name.lower(), build=BUILD, date="13 November 2026", fp=FINGERPRINT))
    (dst / "verify_plugins.py").write_text((HERE / "verify_plugins.template.py").read_text())
    for p in sorted(q for q in (edition_dir / "PCP_1.0_Plugins").rglob("*") if q.is_file()):
        q = dst / "PCP_1.0_Plugins" / p.relative_to(edition_dir / "PCP_1.0_Plugins")
        assert q.read_bytes() == p.read_bytes() and os.access(q, os.X_OK) == os.access(p, os.X_OK), p
    files = [{"path": p.relative_to(dst).as_posix(), "bytes": p.stat().st_size, "sha256": sha(p.read_bytes())}
             for p in sorted(q for q in dst.rglob("*") if q.is_file()) if p.relative_to(dst).as_posix() != "MANIFEST.json"]
    (dst / "MANIFEST.json").write_text(json.dumps({"format": "pcp-plugins-local-integrity/2", "product": "Parrot Council Protocol", "product_version": PRODUCT_VERSION,
                                                   "build": BUILD, "edition": key, "payload_count": len(files), "files": files}, indent=2) + "\n")
    return dst

def check_archive(z, executables):
    """What was packed is what unpacks: every name once, and the files that must run are marked to run."""
    with zipfile.ZipFile(z) as a:
        names = a.namelist()
        assert len(names) == len(set(names)) and a.testzip() is None, z
        modes = {i.filename.split("/", 1)[1]: (i.external_attr >> 16) & 0o777 for i in a.infolist()}
    for f in executables:
        assert modes[f] == 0o755, (z, f, oct(modes[f]))
    assert all(m == 0o644 for f, m in modes.items() if f not in executables), z

def main():
    REL.mkdir(parents=True, exist_ok=True)
    work = REL / "_work"
    index = {}
    for key, name in EDITIONS.items():
        d = build_edition(key, name, work)
        z = REL / ("PCP_1.0_%s_Edition.zip" % name)
        index[key] = {"file": z.name, "sha256": zip_dir(d, d.name, z), "files": sum(1 for p in d.rglob("*") if p.is_file()),
                      "edition_fingerprint": sha((d / "MANIFEST.json").read_bytes())}
        run = ["plugins/pcp-os/servers/pcpos", "plugins/pcp-os/servers/linux-x86_64/pcpos", "plugins/pcp-os/servers/linux-aarch64/pcpos"]
        check_archive(z, ["PCP_1.0_Plugins/" + f for f in run])
        pd = build_plugins_archive(key, name, d, work)
        pz = REL / ("PCP_1.0_Plugins_%s_Edition.zip" % name)
        index[key]["plugins_archive"] = {"file": pz.name, "sha256": zip_dir(pd, pd.name, pz), "files": sum(1 for p in pd.rglob("*") if p.is_file()),
                                         "fingerprint": sha((pd / "MANIFEST.json").read_bytes()),
                                         "marketplace_folder": "the same bytes as PCP_1.0_Plugins/ in " + z.name}
        check_archive(pz, ["PCP_1.0_Plugins/" + f for f in run])
    print(json.dumps(index, indent=1))
    (REL / "_work" / "editions.json").write_text(json.dumps(index, indent=1) + "\n")

main()
