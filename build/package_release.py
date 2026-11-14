#!/usr/bin/env python3
"""Finish the release folder: the source archive, the record archive, the index and the README.

    python3 package_release.py <release dir> <src dir> <build dir> <record dir> <dir holding the supplied archives>
"""
import hashlib, json, os, shutil, sys, tempfile, zipfile
from pathlib import Path
REL, SRC, BUILD, RECORD, SUPPLIED = (Path(a) for a in sys.argv[1:6])
sha = lambda b: hashlib.sha256(b).hexdigest()
sys.path.insert(0, str(BUILD))

def zip_dir(src, name, out):
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for p in sorted(q for q in src.rglob("*") if q.is_file() and "__pycache__" not in q.parts and "target" not in q.relative_to(src).parts):
            zi = zipfile.ZipInfo(name + "/" + p.relative_to(src).as_posix(), date_time=(2026, 11, 12, 12, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.external_attr = ((0o100755 if os.access(p, os.X_OK) else 0o100644) << 16)
            z.writestr(zi, p.read_bytes())
    return {"file": out.name, "sha256": sha(out.read_bytes()), "bytes": out.stat().st_size}

README = """# Parrot Council Protocol (PCP) 1.0 | Release of 13 November 2026 (build 22)

*Evidence before agreement.*

**Parrot Council Protocol (PCP)** is the product. **Adversarial Claim Review (ACR)** is the method inside it: an epistemological protocol that specifies how a claim is examined and what its evidence is permitted to establish. Creator: Kirtus Justus.

This folder is the whole release: the initial, preliminary release of PCP 1.0, put forward for use and for review. It replaces every earlier build.

## What to give to whom

| File | Give it to | What it is |
| --- | --- | --- |
| `PCP_1.0_Personal_Edition.zip` | A person who bought the Personal Edition (US$5.00, once) | Complete by itself |
| `PCP_1.0_Commercial_Edition.zip` | An organization with a signed Commercial License Agreement | Complete by itself |
| `PCP_1.0_Enterprise_Edition.zip` | An organization with a signed Enterprise License Agreement | Complete by itself |
| `PCP_1.0_Plugins_Personal_Edition.zip`, `PCP_1.0_Plugins_Commercial_Edition.zip`, `PCP_1.0_Plugins_Enterprise_Edition.zip` | Someone who already holds that edition's license and wants only the plugins on a computer | The edition's plugin folder, the same bytes, with its license, notices and a local check. Not a separate product |
| `PCP_1.0_Source.zip` | Nobody outside. Yours | PCP OS in Rust and Python, the design generator, every build script, the audit harnesses |
| `PCP_1.0_Release_Record.zip` | Nobody outside, unless you choose | The audits of this release (start with the thirteenth), the two separate reviews, the live-session transcripts, the records of the method revisions, the earlier records. A reviewer should be given this |
| `RELEASE.json` | Programs | Every file here by digest, what each edition holds, what was checked, what is superseded |

## What every edition holds

The same method, the same programs and the same documents. An edition differs in its license, in who may take part in a review, and in the colour of the page.

| Inside an edition | What it is |
| --- | --- |
| `README.md` | The overview: what PCP does, why it has two names, what is in the folder, how to start |
| `ACR_1.0_Implementation.zip` | The method, as revised on 13 November 2026 (the fifteenth round). The same bytes in every edition |
| `PCP_1.0_Readers.zip` | Typeset reading copies in the edition's theme |
| `PCP_1.0_Plugins/` | Two plugins for Claude Code: `pcp` (text only; runs wherever Claude Code does) and `pcp-os` (with a recorded register; its programs are for Linux, on x86_64 and on 64-bit ARM). Install one. The same folder is in the edition's plugins archive |
| `PCP_1.0_Machine/` | The release as data, the schemas, and the design profiles for every module and edition |
| `PCP_1.0_OS/` | PCP OS Overview and Specification, as text and as typeset reading copies; its requirements as data; default backgrounds |
| `LICENSE.md`, `NOTICE.md`, `PRICING.md`, `EVIDENCE_STATUS.md`, `THIRD_PARTY_LICENSES.md` | Rights, price, what has been established |
| `verify_edition.py` | A local check of the whole folder |

## PCP and ACR, and why they are separate

PCP is everything supplied under license: the plugins, PCP OS, the reading copies, the design. ACR is the set of rules a review is held to. The method's text names only itself and is identified by a digest, so a review can say exactly which rules applied, and those rules are the same whatever was paid and whichever program delivered them. Everything that may differ, editions, licenses, programs, colours, is the product. A person can ask an assistant for either name and gets the same thing.

## What changed in build 22

| Change | Why |
| --- | --- |
| The **Calibration of build 21 is withdrawn for the two plugins and was taken again.** Its harness had kept the plugins from their own files, so most `pcp-os` reviews could not be begun. The new one hides the probes from the sessions measured without confining their tools, and has every conversation read twice. Four of ten configurations meet the thresholds; the evidence statement of every edition gives both | Found when the maker's own Calibration sent it back to read build 21's conversations |
| The method was revised (**the fifteenth round**): a Calibration measures the configuration as it is used and its record states what the measurement changed; a reading can be wrong, so some runs are read twice; nothing is asked after a halt where the review was the whole task; four probe criteria say what the Core says; a tool gives the probes to an assistant in its own session without their criteria | Each is a fault of the first Calibration made into a rule |
| The `pcp-os` program serves the kernel text and the list of event bodies itself (`acr_kernel`, `acr_events`), and its skill about PCP OS no longer says the kernel cannot load a program | A session that cannot open the plugin's folder now loses nothing; the skill had been out of date since build 21 |
| The PCP OS **kernel** image is the same file from whatever folder it is built in; the kernel compares its measurement of itself with a value named when it is started, which a tool computes from the image file; a kernel that fails verification starts no program | The first makes the second possible; the third was found while testing the second |
| Build 21 was **rebuilt on a second machine** from its source archive: the six edition archives and Doom came out the same bytes, and the kernel's tests passed there | To see whether the build can be repeated. It also showed what the source archive lacked, which is now in it |
| **The maker's own Calibration**: retrieval 22 of 22, application 25 of 26 on two readings, the Claim recorded CONDITIONAL | The first instruction of this work |

The thirteenth audit, in the release record, gives each of these with what was tested and what remains open.

## What changed in build 21

| Change | Why |
| --- | --- |
| The method defines **Calibration** (Core Section 14.4, the fourteenth round): the measurement of onboarding, as counts of what an assistant holds of ACR, what it retrieves exactly, and what it does on items with known answers. It has a record, a tool that makes and scores retrieval items, and a rule that nobody scores themselves | The rights holder's direction: calibration is the analysis and quantification of successful onboarding or the lack of it, and it must be part of the method |
| The method's modules name **instruments**, mechanical procedures a host can run itself, and the PCP OS program runs eight of them: exact arithmetic, two arithmetic tests of reported statistics, exact unit conversion, a digest, two readings of the register, and the check of a Calibration Record | The rights holder's direction that the modules be given substance and that PCP OS be where their potential is tried |
| The PCP OS **kernel** loads ordinary programs from the loader's files, runs them with a display, a keyboard, sound and music, and shows a menu of its own. It runs **Doom**, as a separate program under its own licence | The rights holder's direction that it run Doom, well, and tested |
| A Calibration was made of five ways of delivering the method to two models of one provider, and its counts are in every edition's evidence statement | So that the new section is used and not only written |

The twelfth audit gives the changes of build 21, the eleventh those of build 20, the tenth those of build 19.

## Checking

Extract an edition with a tool that keeps file permissions (`unzip` does) and run `python3 verify_edition.py` in it; in a plugins archive, `python3 verify_plugins.py`. Compare the archive's SHA-256 with `RELEASE.json`. A pass is a fact about files. It authenticates nobody and certifies nothing.

## Status

Proprietary; all rights reserved; supplied under license only. Copyright (c) 2026 Kirtus Justus.

Not peer reviewed, not independently reviewed, not evaluated in use. The plugins were run in uncontrolled sessions and in two Calibrations by their maker, which shows that they work as programs and how far the method arrives in two models; it supports no conclusion about effect. The license and notice wording was drafted by an AI system and has had no legal review. A pre-publication build.
"""

def main():
    work = REL / "_work"
    editions = json.loads((work / "editions.json").read_text())
    with tempfile.TemporaryDirectory() as td:
        s = Path(td) / "PCP_1.0_Source"
        shutil.copytree(SRC / "os-rust", s / "PCP_OS_Rust", ignore=shutil.ignore_patterns("target", "guard_cases.jsonl"))   # the 6.3 MB case file is generated: see vectors/README.md
        shutil.copytree(SRC / "os-python", s / "PCP_OS_Python", ignore=shutil.ignore_patterns("__pycache__"))
        shutil.copytree(BUILD, s / "build", ignore=shutil.ignore_patterns("__pycache__"))
        shutil.copytree(SRC / "os-docs", s / "os-docs")
        shutil.copytree(SRC.parent / "audit", s / "audit", ignore=shutil.ignore_patterns("__pycache__"))
        (s / "README.md").write_text("# PCP 1.0 | Source\n\nThe rights holder's own files. Not part of any edition.\n\n| Folder | What it is |\n| --- | --- |\n| `PCP_OS_Rust/` | The native core, the host program `pcpos`, the kernel, the published vectors. `cargo run --release --bin pcpos-vectors -- vectors` first (it writes the 6.3 MB case file, which is generated and not kept here), then `cargo test`; `crates/pcpos-kernel/run.sh` (it compiles the kernel in one fixed folder, so the image is the same file wherever this archive is unpacked); `crates/pcpos-kernel/programs/probe/build.sh` and `programs/doom/build.sh` for the programs the kernel's tests load. The programs supplied are built for the targets `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, as `build/release.sh` does. Without that first step three of the Python tests are skipped, and say why |\n| `PCP_OS_Python/` | The second implementation, tested against the Rust vectors. Place `ACR_1.0_Implementation.zip` beside this folder and run `python3 -m unittest discover -s TESTS` in it |\n| `os-docs/` | The PCP OS Overview and Specification as written; `build/build_os_docs.py` makes the reading copies, the requirements data and the backgrounds |\n| `build/` | `release.sh` runs the whole build in order: tests, design tokens, plugins, the documented events through each packaged program, validation, the machine form, the three editions and their verifiers |\n| `audit/` | The harnesses of the eighth to thirteenth audits: `sim/sim.py` drives the packaged program through constructed scenarios, and both implementations read every register a scenario leaves; `live/matrix.py` runs the method's behavioural probes in each arm of delivery; `live17/run.sh` to `live20/run.sh` hold the live conversations on the plugins of those builds; `calibration/` holds the Calibration: `run.py` (five deliveries to two models, every conversation read twice), `hidden.sh` (runs a measured session where the probes and their criteria are not on the disk it sees) and `reexamine21.py` (the second look at build 21's conversations) |\n\nThe method packet is not here: it is `ACR_1.0_Implementation.zip` inside every edition, and the build reads it extracted at `src/method`. `build/method/` holds the scripts that make it, as they stand after the fifteenth round, with the edits of the twelfth to fifteenth rounds. `build/evidence.json` holds what each edition's evidence statement says of live sessions and Calibrations, so that this archive and a release's base editions rebuild the editions.\n\nProprietary; all rights reserved. Copyright (c) 2026 Kirtus Justus.\n")
        source = zip_dir(s, "PCP_1.0_Source", REL / "PCP_1.0_Source.zip")
        r = Path(td) / "PCP_1.0_Release_Record"
        shutil.copytree(RECORD, r)
        (r / "earlier_records").mkdir(exist_ok=True)
        superseded = {}
        earlier = {}            # what the build this one replaces had itself replaced
        for z in sorted(SUPPLIED.rglob("PCP_1.0_Release*.zip")):
            with zipfile.ZipFile(z) as a:
                sup = json.loads(a.read("PCP_1.0_Release/RELEASE.json"))["superseded"]
                earlier = dict(sup.get("superseded_by_earlier_builds", {}), **{"build 21 superseded/" + k: v for k, v in sup["files"].items()})
        consulted = {z.relative_to(SUPPLIED).as_posix(): sha(z.read_bytes()) for z in sorted(SUPPLIED.rglob("*")) if z.is_file() and "consulted" in z.parts}
        for z in sorted(SUPPLIED.rglob("*")):
            if z.is_file() and z.suffix in (".zip", ".md", ".json") and "consulted" not in z.parts:
                superseded[z.relative_to(SUPPLIED).as_posix()] = sha(z.read_bytes())
                if "Maintenance_Record" in z.name or z.name.startswith("ACR_OS_Audit") or z.name.startswith("ACR_Case_Study") or z.name.startswith("PCP_ACR_Baseline"):
                    shutil.copy(z, r / "earlier_records" / z.name)
        record = zip_dir(r, "PCP_1.0_Release_Record", REL / "PCP_1.0_Release_Record.zip")
    ed0 = work / "PCP_1.0_Personal_Edition"
    edj = json.loads((ed0 / "EDITION.json").read_text())
    index = {
        "format": "pcp-release-index/1", "product": "Parrot Council Protocol", "abbreviation": "PCP", "method": "Adversarial Claim Review", "method_abbreviation": "ACR",
        "creator": "Kirtus Justus", "version": "1.0", "build": 22, "build_date": "2026-11-13", "publication_state": "PRE_PUBLICATION",
        "statement": "The initial, preliminary release of PCP 1.0, built whole on 13 November 2026 and put forward for use and for review: the method packet as revised in the fifteenth round, the reading copies generated from it, and everything built on that method release. Build 19 follows a second AI system's audits of builds 17 and 18. The method's lookup tool serves an identifier that two folders declare from neither and reads a version whole; the method's evidence statement says what runs of its probes there have been; a plugin carries the method's text without the packet's probes and Python programs; the PCP OS program is a static file for Linux on x86_64 and on 64-bit ARM; a check is recorded only from a command the host saw run after the review was commissioned, and never from hook input that is not the host's report; a finding that rests only on a reported check is marked UNKNOWN; and material that reaches the record during a review is recorded by how it came. Build 20 follows that system's recheck of build 19, which reproduced those changes and found two defects left in the host program: a report of a command is now kept only if it says, as true or false, whether the command was interrupted, and a report of failure that names exit status 0 is not kept; and a file counts as material a review was given only if its last write is in an earlier second than the review's commissioning. The method packet and the second implementation are the same bytes as in build 19. Build 21 follows the rights holder's direction on three things. The method defines Calibration, the measurement of onboarding (Core Section 14.4), and its modules name instruments that a host can run itself; the PCP OS program runs eight instruments and records each result as a check it ran; and the PCP OS kernel loads programs from the loader's files and runs them with a display, a keyboard, sound and music, among them Doom, which is a separate program under its own licence and is in the source archive as a build script and the six small files that connect it to the kernel, not as a binary. Build 22 follows the rights holder's direction to go on hardening it and, first, to attend to the maker's own onboarding. That measurement sent the maker back to the conversations of build 21's Calibration, which was found to have kept the two plugins from their own files: it is withdrawn for them and was taken again, with the probes hidden from the sessions measured and every conversation read twice. The method's fifteenth round makes rules of what went wrong; the pcp-os program serves the kernel text and the event bodies itself; the kernel image is the same file from any folder, the kernel compares its own measurement with a value named when it is started, and a kernel that fails verification starts no program. Build 21 was rebuilt on a second machine to the same bytes. It supersedes every earlier build.",
        "method_release": {"state": "REVISED_2026-11-13_ROUND_15", "predecessor_manifest_sha256": "7cb5d12758feb3b3171d5acb1386c651feae022c5907b28885275f1f845952a4", "archive": "ACR_1.0_Implementation.zip", "archive_sha256": edj["packets"]["implementation"]["sha256"], "manifest_sha256": edj["packets"]["implementation"]["manifest_sha256"], "files": 58, "identical_in_every_edition": True},
        "editions": editions,
        "plugins": {"version": edj["plugin_version"], "names": ["pcp", "pcp-os"], "edition_overlay": "edition/ inside each plugin; every other file is the same bytes in every edition",
                    "archives": "one per edition, under editions.<EDITION>.plugins_archive: the edition's PCP_1.0_Plugins/ folder, the same bytes, with the edition's license, notices and a local check",
                    "program": edj["components"]["plugins"]["pcp-os"]["program"], "programs": edj["components"]["plugins"]["pcp-os"]["programs"]},
        "source": source, "record": record,
        "checks": json.loads(os.environ.get("PCP_CHECKS", "{}")),
        "superseded": {"note": "Replaced by this build: build 21 of 13 November 2026, supplied with the instruction as PCP_1.0_Release7.zip. Everything build 21 and the builds before it superseded stays superseded.", "files": superseded, "superseded_by_earlier_builds": earlier},
        "consulted": {"note": "Supplied with the instruction and checked against this build; not part of it and not superseded by it: Audit 12 of build 21, supplied with the instruction. What was consulted for builds 19 and 20 stays in the record under audit10/supplied and audit11/supplied.", "files": consulted},
        "not_in_this_release": ["an independent review of any of it", "an evaluation of effect", "Doom, its source or any game data (the Source folder holds a build script and six connecting files)", "a start of the kernel on a real computer or with a real display, keyboard or sound device", "a Calibration taken by anyone but the maker, or read by a person", "a person's judgement of how the game plays", "a program for macOS or Windows", "a run of the 64-bit ARM program on an ARM machine (it was run under emulation)", "a signed program", "the source of the PCP OS program (it is the rights holder's)", "legal review of the amended license and notice", "reading copies of the PCP OS documents inside the Readers packet (they are beside it, in PCP_1.0_OS)"],
    }
    (REL / "RELEASE.json").write_text(json.dumps(index, indent=1) + "\n")
    (REL / "README.md").write_text(README)
    shutil.rmtree(work)
    # One packet: the folder as a single archive beside it.
    packet = REL.parent / "PCP_1.0_Release.zip"
    one = zip_dir(REL, "PCP_1.0_Release", packet)
    print("PACKET", packet, one["bytes"], one["sha256"])
    for p in sorted(REL.iterdir()):
        print("%-48s %10d  %s" % (p.name, p.stat().st_size, sha(p.read_bytes())[:16]))

main()
