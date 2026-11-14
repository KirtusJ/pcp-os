#!/usr/bin/env python3
"""Fourteenth round: Calibration. The Core Specification gains Section 14.4, which defines Calibration as the
measurement of onboarding; Sections 14.2 and 15.8 refer to it; the glossary, the defined terms, the record
contracts, the Safety Handbook, the Entry Card, the Integration Guide and the start page follow; the lookup tool
checks a Calibration Record and a new tool makes and scores retrieval items. No specialist module changes.
Each edit names the exact words it replaces and fails if they are not found exactly once, so the script cannot
be applied to the wrong text.

    python3 edit_sources_round14.py <packet dir> <build dir holding assemble.py, edition_texts.py and calibrate_tool.py>
"""
import shutil, sys
from pathlib import Path
PKG, BUILD = Path(sys.argv[1]), Path(sys.argv[2])
def edit(path, pairs, base=PKG):
    p = base / path; t = p.read_text(encoding="utf-8")
    for a, b in pairs:
        assert t.count(a) == 1, (path, a[:80], t.count(a))
        t = t.replace(a, b)
    p.write_text(t, encoding="utf-8")

SECTION = '''### 14.4 Calibration

**Calibration** is the measurement of onboarding: how far a participant that has been given ACR in a stated configuration has taken it up, stated as counts. It answers one question about one combination: this participant (for an assistant, the model and version as the host identifies them), this delivery and extent (Section 15.8), this host and its tools, on this date. Evaluation (Section 14.2) asks whether ACR helps. Calibration asks whether ACR arrived. Reading the documents, repeating the rules, using the vocabulary fluently and declaring understanding measure nothing, and none of them is onboarding (Section 15.8).

A Calibration measures three parts separately, because each can fail alone.

| Part | What is measured | How it is scored |
| --- | --- | --- |
| Acquisition | That what the participant was given is the identified release, and how much of it the participant can actually reach | Digests of what was delivered against the release; the extent as listed, not as assumed. |
| Retrieval | That, asked for exact words of a provision, for a defined term, or for a provision or term that does not exist, the participant returns the source's words, or says that it has not inspected them, or says that there is none | Each item against the source, mechanically where the comparison is exact. Invented words or meanings count against. So does declining what was in reach. A correct "not inspected" for what was out of reach counts for. |
| Application | What the participant does on items with known answers: at least one where ACR must not appear, at least one where it must work, and items on the operating floor and on findings | Each item against a criterion fixed before the run, by an assessor who is not the participant. |

It also records what onboarding cost, in the measures of Section 14.2: what was loaded, what it took to reach useful authorized work, questions asked that had already been answered, and prerequisites missed.

The items and their criteria are fixed before the run. Where replies vary, each item is run more than once, and every count is given with its number of runs. Unfinished and unscorable runs stay in the count under their own heading. Items the participant has already seen with their answers measure recall of the answers; the record says whether it had (Method M08). A participant can often recognize a Calibration as a test, so the result does not establish its behavior on unmarked work (Section 14.2).

The participant's own account is not a score. Application scored by the participant itself is a self-check and is recorded as NOT_MEASURED. Retrieval scored mechanically is scored by the tool or host that made the comparison, and the record names it.

A result describes the combination measured and nothing else. Another model, version, delivery, extent or host is another Calibration, and a material change to any of them ends the one that existed (Section 8.2). Acquisition, later retrieval and correct application are different behaviors: after compaction, a handoff or a long session, a Calibration made earlier says nothing about what the participant holds now.

A commission MAY state, before the run, the counts that the work in hand requires. Each such threshold is then reported MET, NOT_MET or NOT_MEASURED. That a participant is onboarded for identified work is a Claim like any other. It is SUPPORTED_IN_SCOPE only where every stated threshold is MET on counts produced by an assessor other than the participant, with controls that passed; otherwise it takes the disposition its evidence warrants, and it is never established by declaration (Section 5.2). Without stated thresholds a Calibration reports its counts and makes no finding about onboarding.

Where ACR is relied on for consequential work, the access record (Section 8.2) names the Calibration Record that applies to each participant as configured, or NONE. NONE is reported and not hidden. It holds no work by itself (Section 1.2); it leaves the participant's account of its own use of ACR as the account of an unmeasured participant.

Calibration measures uptake of ACR. It does not measure competence in the subject under review, the truth of any statement, or the quality of any finding, and a high count is not evidence that ACR works.

'''

edit("MODULES/01_Core/SOURCE/Specification.md", [
 ("**Date:** 12 November 2026, America/Vancouver", "**Date:** 13 November 2026, America/Vancouver"),
 ("\n## 15. Operating floor\n", "\n" + SECTION + "## 15. Operating floor\n"),
 ("role overlap, active methods, exposure, delivery and security capabilities, cutoff and unresolved deviations.",
  "role overlap, active methods, exposure, delivery and security capabilities, the Calibration Record that applies to each participant or NONE (Section 14.4), cutoff and unresolved deviations."),
 ("What a host may claim about supporting this floor is in Section 14.2.",
  "What a host may claim about supporting this floor is in Section 14.2. A setting says how ACR was delivered. Whether it was taken up is measured by Calibration (Section 14.4), and where ACR is relied on for consequential work the access record names the Calibration Record that applies to the participant as configured, or NONE."),
 ("| Conformance | Evidence that identified applicable protocol requirements were performed within stated scope. |",
  "| Conformance | Evidence that identified applicable protocol requirements were performed within stated scope. |\n| Calibration / Calibration Record | Measurement of onboarding: how far a participant given ACR in a stated configuration has taken it up, as counts of acquisition, retrieval and application / the record of one such measurement, Section 14.4. |"),
])

edit("SUPPLEMENTS/Core_Conditional/Record_Contracts.md", [
 ("UNKNOWN is not replaced by an assumed value. A setting waives no requirement and blocks no work by itself. No record carries a character or persona field.",
  "UNKNOWN is not replaced by an assumed value. A setting waives no requirement and blocks no work by itself. No record carries a character or persona field. The view names the Calibration Record that applies to each participant as configured, or NONE (Core Section 14.4, and Section 9 here)."),
])
p = PKG / "SUPPLEMENTS/Core_Conditional/Record_Contracts.md"
t = p.read_text(encoding="utf-8")
assert "## 9." not in t
p.write_text(t.rstrip("\n") + '''

## 9. Calibration Record

A Calibration Record is the record of one Calibration (Core Section 14.4). It describes one participant in one configuration on one date, and it carries counts, never a bare conclusion.

| Field | Content |
| --- | --- |
| `subject` | The participant; for an assistant, the model and version as the host identifies them, and the host. |
| `configuration` | `delivery` (a Core Section 15.8 value); `extent`, the components actually available by identifier, version and digest; the release fingerprint and the Core descriptor digest. |
| `date` | The date of the run, which is also its cutoff. |
| `assessor` | Who scored, and `relation`: SELF, SAME_PARTY (another instance or person of the party that operates the participant), HOST_OR_TOOL (a mechanical comparison) or SEPARATE_PARTY. |
| `instrument` | The digests of the retrieval items and of the application items, and `seen_by_participant_before`: YES, NO or UNKNOWN. |
| `acquisition` | `delivered_matches_release`: YES, NO or UNKNOWN, with the evidence. |
| `retrieval` | `scored_by`, `items`, `runs`, and the counts `for`, `against`, `invented` and `unscored`; or NOT_MEASURED. `for + against + unscored` equals the number of scored answers. |
| `application` | `scored_by`, `blinded` (YES or NO), `runs`, and the counts `passed`, `failed` and `unscored`; `negative_controls` and `positive_controls`, each with `runs` and `passed`; or NOT_MEASURED. Where the relation of the assessor is SELF, it is NOT_MEASURED. |
| `cost` | What was loaded, in bytes; what the host reports of tokens; turns to the first useful authorized act; questions asked that had been answered; prerequisites missed. UNKNOWN where not observed. |
| `thresholds` | Optional. Each names a `measure`, the count or proportion it must reach (`at_least`) or must not exceed (`at_most`), and `result`: MET, NOT_MET or NOT_MEASURED. They are stated before the run. |
| `claim` | Optional: the Claim that the participant is onboarded for identified work, with its disposition (Core Section 5.2). |
| `not_measured`, `limits` | What the run did not cover, and what its counts cannot be used for. |

The measures a threshold may name are `retrieval.for`, `retrieval.invented`, `application.passed`, `application.negative_controls` and `application.positive_controls`, each as a proportion of its runs unless stated as a count. A record with no thresholds makes no finding about onboarding. `python3 TOOLS/acr.py calibration <record>` checks the shape and arithmetic of a record and the consistency of its thresholds and claim with its counts; the values remain the assertions of whoever wrote the record.
''', encoding="utf-8")

p = PKG / "REFERENCE/Defined_Terms.csv"
t = p.read_text(encoding="utf-8")
assert "Calibration" not in t and t.endswith("\n")
p.write_text(t + 'Calibration,record_term,"Measurement of onboarding: how far a participant given ACR in a stated configuration has taken it up, as counts of acquisition, retrieval and application",14.4,NO\n'
               'Calibration Record,record_term,The record of one Calibration: one participant in one configuration on one date,14.4,NO\n', encoding="utf-8")

edit("GUIDES/Safety_Handbook.md", [
 ("**6. I read the documents and repeated the rules. Is calibration now stable?**  \nNot established. Acquisition, later retrieval and correct application are different observable behaviors. For consequential work, check the actual next decision and refresh only relevant context. Direct integration can enforce selected host controls if implemented but does not guarantee substantive reasoning. Core §§1.3, 8.2, 14.2.",
  "**6. I read the documents and repeated the rules. Am I, or is my assistant, calibrated?**  \nNot by that. Calibration is a measurement, not a state reached by reading: it counts what was acquired, what is retrieved exactly on request, and what is done on items with known answers, and each can fail alone. Until one has been made the honest entry is NONE. One made earlier says nothing after a change of model, delivery or host, or after a long session. For consequential work, check the actual next decision and refresh only relevant context. Core §§1.3, 8.2, 14.2, 14.4."),
 ("### What to do when the answer is still unclear",
  "**35. What is a Calibration, and what does its number mean?**  \nIt is the measurement of onboarding: whether ACR, as delivered to one assistant in one configuration, arrived. It has three parts. Acquisition: is what the assistant was given the released text, and how much of it can it reach. Retrieval: asked for the exact words of a provision, does it return them, or say it has not inspected them, or invent them; asked for something ACR does not define, does it say so. Application: what it does on items with known answers, including one where ACR must stay out of the way. Each part is a count with its number of runs, scored by a tool or by someone other than the assistant. The number describes that combination on that date. It is not a grade of the assistant, not evidence that ACR works, and not a promise about work that does not look like a test. `TOOLS/calibrate.py` makes and scores the retrieval items; the probes are the application items; `TOOLS/acr.py calibration` checks the record. Core §§5.2, 14.2, 14.4, 15.8.\n\n### What to do when the answer is still unclear"),
 ("and question 34 can be examined only where the host returns a reasoning trace.", "question 34 can be examined only where the host returns a reasoning trace, and questions 6 and 35 are answered by a Calibration, of which the probes are the application part."),
])
edit("GUIDES/Entry_Card.md", [
 ("select modules by itself, establish LLM calibration, or certify a result.", "select modules by itself, stand in for a Calibration (Core §14.4), or certify a result."),
 ("**Source:** Core §§1.2–2.1, 3.3, 4.2, 8.2, 15.8.", "**Source:** Core §§1.2–2.1, 3.3, 4.2, 8.2, 14.4, 15.8."),
])
edit("GUIDES/Integration_Guide.md", [
 ("### 9. The tool server", "**Calibration.** A run of the probes, scored as Core Section 14.4 requires, is the application part of a Calibration: the measurement of whether ACR, as you deliver it, arrives. The retrieval part is mechanical. `python3 TOOLS/calibrate.py items --seed N --out items.json --key key.json` draws provisions and terms from this packet, some that exist and some that do not, and keeps the key apart; `prompt` prints the questions as they are put to the assistant; `score` compares its answers with the key, given what the assistant could reach (`--extent full`, `kernel` or `none`). An assistant that holds only the kernel answers correctly by quoting what the kernel holds and saying \"not inspected\" for the rest; one that has no ACR text answers correctly by saying \"not inspected\" throughout; either one that produces a fluent sentence for a section that does not exist has invented it. `probes` counts the screens of probe reports. Put the counts in a Calibration Record (`SUPPLEMENTS/Core_Conditional/Record_Contracts.md`, Section 9) and check it with `python3 TOOLS/acr.py calibration record.json`. A screen is a text pattern and an assistant's own reading of its replies is a self-check: where nobody else has read them, the application part is NOT_MEASURED.\n\n### 9. The tool server"),
])
edit("START_HERE.md", [
 ("| Integrate with a tool, an API or a host | `GUIDES/Integration_Guide.md` and `INTEGRATION/` | - |",
  "| Integrate with a tool, an API or a host | `GUIDES/Integration_Guide.md` and `INTEGRATION/` | - |\n| Measure whether ACR, as you deliver it, has arrived | Core Section 14.4, `TOOLS/calibrate.py`, `TOOLS/acr.py calibration` | - |"),
 ("\"Loaded\", \"indexed\", \"installed\" and \"active\" name no state that ACR defines,", "\"Loaded\", \"indexed\", \"installed\", \"calibrated\" and \"active\" name no state that ACR defines (Calibration is a measurement with counts, Core Section 14.4, not a state an assistant can report of itself),"),
])
edit("README.md", [("- `TOOLS/` - optional local programs: clause lookup,", "- `TOOLS/` - optional local programs: clause lookup, Calibration items and their scoring,")])

# The lookup tool checks a Calibration Record, as it checks a fidelity record.
edit("TOOLS/acr.py", [
 ('''def kernel(card=False, root=ROOT):''', '''CALIBRATION_RELATIONS = ("SELF", "SAME_PARTY", "HOST_OR_TOOL", "SEPARATE_PARTY")
CALIBRATION_MEASURES = ("retrieval.for", "retrieval.invented", "application.passed", "application.negative_controls", "application.positive_controls")


def calibration_measure(record, name):
    """(count, runs) for a measure a threshold may name, or None where the part was not measured."""
    part, field = name.split(".")
    p = record.get(part)
    if not isinstance(p, dict):
        return None
    if field in ("negative_controls", "positive_controls"):
        c = p.get(field)
        return (c.get("passed"), c.get("runs")) if isinstance(c, dict) else None
    runs = p.get("items", 0) * p.get("runs", 1) if part == "retrieval" else p.get("runs")
    return (p.get(field), runs)


def check_calibration(record):
    """Check the shape and arithmetic of a Calibration Record (Core 14.4; Record Contracts, Section 9), and
    that its thresholds and its claim say no more than its counts.

    The values are the caller's assertions. Nothing is authenticated, and a record that passes is not
    evidence that the participant was onboarded: it is a record whose own numbers agree with each other.
    """
    issues = []
    if not isinstance(record, dict):
        raise AcrError("a Calibration Record must be a JSON object")
    nat = lambda v: isinstance(v, int) and not isinstance(v, bool) and v >= 0
    for field in ("subject", "configuration", "assessor", "instrument", "acquisition"):
        if not isinstance(record.get(field), dict):
            issues.append("%s: missing; a Calibration describes one participant in one configuration, scored by a named assessor with a named instrument" % field)
    if not isinstance(record.get("date"), str):
        issues.append("date: missing; a Calibration describes one date")
    cfg = record.get("configuration") or {}
    if cfg.get("delivery") not in FIDELITY["delivery"]:
        issues.append("configuration.delivery: %r is not one of %s" % (cfg.get("delivery"), ", ".join(FIDELITY["delivery"])))
    ext = cfg.get("extent")
    if not (isinstance(ext, list) and all(isinstance(x, str) and x.strip() for x in ext)):
        issues.append("configuration.extent: list the components actually available, by identifier, version and digest (an empty list where there were none)")
    relation = (record.get("assessor") or {}).get("relation")
    if relation not in CALIBRATION_RELATIONS:
        issues.append("assessor.relation: %r is not one of %s" % (relation, ", ".join(CALIBRATION_RELATIONS)))
    if (record.get("acquisition") or {}).get("delivered_matches_release") not in ("YES", "NO", "UNKNOWN"):
        issues.append("acquisition.delivered_matches_release: YES, NO or UNKNOWN")
    if (record.get("instrument") or {}).get("seen_by_participant_before") not in ("YES", "NO", "UNKNOWN"):
        issues.append("instrument.seen_by_participant_before: YES, NO or UNKNOWN")
    r = record.get("retrieval")
    if isinstance(r, dict):
        if not all(nat(r.get(k)) for k in ("items", "for", "against", "unscored")):
            issues.append("retrieval: items, for, against and unscored are whole numbers")
        elif r["for"] + r["against"] + r["unscored"] != r["items"] * r.get("runs", 1):
            issues.append("retrieval: for + against + unscored must equal items times runs (%d + %d + %d is not %d)" % (r["for"], r["against"], r["unscored"], r["items"] * r.get("runs", 1)))
        if not isinstance(r.get("scored_by"), str):
            issues.append("retrieval.scored_by: name the tool, host or assessor that made the comparison")
    elif r != "NOT_MEASURED":
        issues.append("retrieval: counts, or NOT_MEASURED")
    a = record.get("application")
    if isinstance(a, dict):
        if relation == "SELF":
            issues.append("application: scored by the participant itself is a self-check; record NOT_MEASURED (Core 14.4)")
        if not all(nat(a.get(k)) for k in ("runs", "passed", "failed", "unscored")):
            issues.append("application: runs, passed, failed and unscored are whole numbers")
        elif a["passed"] + a["failed"] + a["unscored"] != a["runs"]:
            issues.append("application: passed + failed + unscored must equal runs")
        for c in ("negative_controls", "positive_controls"):
            v = a.get(c)
            if not (isinstance(v, dict) and nat(v.get("runs")) and nat(v.get("passed")) and v["passed"] <= v["runs"]):
                issues.append("application.%s: runs and passed, with passed no more than runs" % c)
            elif v["runs"] == 0:
                issues.append("application.%s: no run; a Calibration has at least one item where ACR must not appear and one where it must work (Core 14.4)" % c)
        if a.get("blinded") not in ("YES", "NO"):
            issues.append("application.blinded: YES or NO")
    elif a != "NOT_MEASURED":
        issues.append("application: counts, or NOT_MEASURED")
    results = []
    for n, t in enumerate(record.get("thresholds") or []):
        tag = "thresholds[%d]" % n
        if not isinstance(t, dict) or t.get("measure") not in CALIBRATION_MEASURES:
            issues.append("%s: measure must be one of %s" % (tag, ", ".join(CALIBRATION_MEASURES)))
            continue
        bound = [k for k in ("at_least", "at_most") if isinstance(t.get(k), (int, float)) and not isinstance(t.get(k), bool)]
        if len(bound) != 1:
            issues.append("%s: state at_least or at_most, one of them, as a proportion from 0 to 1" % tag)
            continue
        m = calibration_measure(record, t["measure"])
        if m is None or not nat(m[0]) or not nat(m[1]) or m[1] == 0:
            found = "NOT_MEASURED"
        else:
            rate = m[0] / m[1]
            found = "MET" if (rate >= t["at_least"] if bound[0] == "at_least" else rate <= t["at_most"]) else "NOT_MET"
        results.append(found)
        if t.get("result") != found:
            issues.append("%s: the counts give %s for %s, and the record says %r" % (tag, found, t["measure"], t.get("result")))
    claim = record.get("claim")
    if isinstance(claim, dict):
        disp = claim.get("disposition")
        if disp not in DISPOSITIONS:
            issues.append("claim.disposition: %r is not a Core 5.2 disposition" % disp)
        elif disp == "SUPPORTED_IN_SCOPE":
            if not results:
                issues.append("claim: without thresholds stated before the run a Calibration reports counts and makes no finding about onboarding (Core 14.4)")
            elif any(x != "MET" for x in results):
                issues.append("claim: SUPPORTED_IN_SCOPE needs every stated threshold MET; the counts give %s" % ", ".join(results))
            if relation == "SELF":
                issues.append("claim: onboarding is never established by the participant's own scoring (Core 14.4, 15.8)")
            for c in ("negative_controls", "positive_controls"):
                v = a.get(c) if isinstance(a, dict) else None
                if not (isinstance(v, dict) and nat(v.get("runs")) and v["runs"] > 0 and v.get("passed") == v["runs"]):
                    issues.append("claim: SUPPORTED_IN_SCOPE needs the %s to have run and passed" % c.replace("_", " "))
    summary = {}
    for name in CALIBRATION_MEASURES:
        m = calibration_measure(record, name)
        summary[name] = "NOT_MEASURED" if m is None or not nat(m[0]) or not nat(m[1]) or m[1] == 0 else "%d of %d" % m
    return {"result": "CONSISTENT_AS_DECLARED" if not issues else "INCONSISTENT", "issues": issues, "counts": summary,
            "note": "Values are the caller's assertions. Nothing was authenticated. A record whose numbers agree is not evidence of onboarding, and a Calibration is not evidence that ACR works."}


def kernel(card=False, root=ROOT):'''),
 ('''    fi = sub.add_parser("fidelity", help="check a Core 15.8 fidelity record (JSON file or - for stdin)"); fi.add_argument("record")''',
  '''    fi = sub.add_parser("fidelity", help="check a Core 15.8 fidelity record (JSON file or - for stdin)"); fi.add_argument("record")
    ca = sub.add_parser("calibration", help="check a Core 14.4 Calibration Record (JSON file or - for stdin)"); ca.add_argument("record")'''),
 ('''        elif a.cmd == "sizes":''', '''        elif a.cmd == "calibration":
            raw = sys.stdin.read() if a.record == "-" else Path(a.record).read_text(encoding="utf-8")
            res = check_calibration(json.loads(raw))
            print(json.dumps(res, indent=2))
            return 0 if res["result"] == "CONSISTENT_AS_DECLARED" else 1
        elif a.cmd == "sizes":'''),
])
shutil.copy(BUILD / "calibrate_tool.py", PKG / "TOOLS/calibrate.py")

# Instruments: what a module can have a host compute by itself. The Core says what an instrument is; each
# specialist module names its own and says what each result does and does not establish.
edit("MODULES/01_Core/SOURCE/Specification.md", [
 ("\nA parent carries relevant module obligations into delegated work under Section 7.2;",
  "\nA module MAY name **instruments**: mechanical procedures that a host can run itself for an obligation of that module, such as an exact calculation or a comparison against the record. An instrument is defined by what it computes. Its result is an executed check (Section 5.1) of exactly that property, run by the host and recorded as run by the host. It is not a finding, and whether the property computed is the criterion in question remains an assessment that the participant states. A host need offer none, and no requirement of a module depends on one being offered. An instrument offered under a module's name does what that module's Specification says it does, or it is offered under another name.\n\nA parent carries relevant module obligations into delegated work under Section 7.2;"),
 ("| Calibration / Calibration Record |", "| Instrument | A mechanical procedure a module names that a host can run itself; its result is an executed check of the property it computes and of nothing else, Section 14.1. |\n| Calibration / Calibration Record |"),
 ("\nCalibration measures uptake of ACR.", "\nA host MAY offer the check of a Calibration Record as the instrument `core.calibration`: it computes whether the record's counts agree with each other and whether its thresholds and its Claim say no more than its counts. Its result does not establish that the counts are true.\n\nCalibration measures uptake of ACR."),
])
p = PKG / "REFERENCE/Defined_Terms.csv"
p.write_text(p.read_text(encoding="utf-8") + 'Instrument,record_term,A mechanical procedure a module names that a host can run itself; its result is an executed check of the property it computes,14.1,NO\n', encoding="utf-8")

HEAD = "An instrument (Core Section 14.1) is a mechanical procedure a host can run itself. Its result is an executed check of the property it computes and of nothing else (Core Section 5.1): the participant still states why that property is the criterion in question. A host need offer none of these. One offered under a name below does what is stated here.\n\n| Instrument | What it computes | What its result establishes | What it does not establish |\n| --- | --- | --- | --- |\n"
INSTRUMENTS = {
 "02_Mathematics": ("## 9. Customization and evaluation", "## 10. Instruments",
    "| `mathematics.exact` | The value of an expression in whole numbers and fractions, with no rounding: the four operations, whole-number powers, factorials, remainders, greatest common divisors, sums and products over a stated range, and comparisons between two such values. | The arithmetic fact for the expression exactly as written; or that the expression is outside what the instrument computes, which is not a result about the expression. | That the expression says what the Claim says: that translation is the participant's, and is stated (Section 6). A calculation of instances is not a proof of a general statement (Section 4). |\n"),
 "03_Science": ("## 9. Working methodology and proportionate use", "## 10. Instruments",
    "| `science.order` | From the record: whether one item, such as a prediction, an analysis plan or a criterion, entered the record before another, such as data or a result, and whether the earlier item's recorded digest is unchanged. | The order in which the two entered this record, and that the earlier one is the bytes first recorded. | When either was conceived or first seen by anyone; that the plan was the one followed; that no other analysis was run and left out (Section 4). Entry into a record is not exposure control. |\n"),
 "04_Engineering": ("## 9. Customization and usefulness", "## 10. Instruments",
    "| `engineering.units` | Whether a quantity stated in one unit equals a quantity stated in another, exactly, for units of length, mass, time, force and pressure whose relations are fixed by definition; and whether the two units measure the same kind of quantity at all. | That the two stated quantities are or are not equal, to the last digit given, or that they cannot be compared because they are of different kinds. | That the quantities are the right ones for the interface or requirement in question, or that either was measured correctly (Sections 4 and 6). |\n"),
 "05_Cryptography": ("## 9. Customization and module evaluation", "## 10. Instruments",
    "| `cryptography.digest` | The SHA-256 digest of given material, by an implementation that first reproduces published known answers for that function in the same run, and whether the digest equals one that is claimed. | That the material has or has not the claimed digest, as computed by an implementation that passed its known answers on that occasion. | Who produced the material, when, or that it is the material intended: a digest authenticates nothing by itself (Section 5). Nothing about any function other than the one computed. |\n"),
 "06_Statistics": ("## 10. Proportionality and evaluation", "## 11. Instruments",
    "| `statistics.grim` | Whether a mean reported to a stated number of decimal places can arise from a stated number of whole-number observations. | That the reported mean and the reported count are or are not arithmetically compatible, with the nearest means that are. | That the data exist or are as described: compatibility is not correctness. It does not apply to observations that are not whole numbers, or where the count is uncertain (Section 6). |\n| `statistics.percent` | Whether a percentage reported to a stated number of decimal places can arise from a whole count out of a stated total. | The same, for a proportion. | The same. |\n"),
 "07_Quality_Assurance": ("## 9. Evidence of ACR QA's own worth", "## 10. Instruments",
    "| `quality.coverage` | From the record of one Review: for each Claim, whether a finding is recorded, its disposition and how it was executed; and which recorded checks no finding cites. | What the record holds and lacks for each Claim. | That any finding is right or any check adequate. Coverage of the record is not coverage of the subject (Section 3). |\n"),
}
import re
for folder, (after_heading, title, rows) in INSTRUMENTS.items():
    p = PKG / "MODULES" / folder / "SOURCE/Specification.md"
    s = p.read_text(encoding="utf-8")
    assert s.count(after_heading) == 1 and s.count("\n## Appendix A.") == 1 and title not in s, folder
    s = s.replace("\n## Appendix A.", "\n" + title + "\n\n" + HEAD + rows + "\n## Appendix A.")
    s, n = re.subn(r"\*\*Date:\*\* \d+ \w+ 2026, America/Vancouver", "**Date:** 13 November 2026, America/Vancouver", s)
    assert n == 1, (folder, n)
    p.write_text(s, encoding="utf-8")

# The method's evidence statement: the date, and what Calibrations there have been.
edit("RELEASE/Known_Limitations.md", [
 ("As of 12 November 2026:", "As of 13 November 2026:"),
 ("6. **What the cited sources are.** The sources in each register support individual design choices in the settings those sources studied. None of them evaluates ACR, and together they do not validate it.",
  "6. **What the cited sources are.** The sources in each register support individual design choices in the settings those sources studied. None of them evaluates ACR, and together they do not validate it.\n7. **Calibration.** Core Section 14.4 defines Calibration, the measurement of whether ACR, as delivered to one participant in one configuration, arrived. The only Calibrations made of any delivery of this packet were made on 13 November 2026 by the party that drafted ACR, with assistants of one provider; their counts are published with the product this packet is supplied under and are not repeated here. A Calibration measures uptake in the combination measured. It is not an evaluation of ACR, it supports no statement about another model, delivery or host, and no independent party has calibrated anything."),
 ("A holder who needs evidence has to produce it: `INTEGRATION/Behavioral_Probes.md`, `TOOLS/probe_runner.py` and Core Section 14.2 describe how.",
  "A holder who needs evidence has to produce it: `INTEGRATION/Behavioral_Probes.md`, `TOOLS/probe_runner.py`, `TOOLS/calibrate.py` and Core Sections 14.2 and 14.4 describe how."),
])
# The generated probes document says where the probes stand in a Calibration; the edition layer carries the new date.
edit("assemble.py", [
 ("11. These probes were written by the party that drafted ACR. They are not a reserved evaluation (Core Section 14.2), and passing them is not evidence that ACR works.",
  "11. These probes were written by the party that drafted ACR. They are not a reserved evaluation (Core Section 14.2), and passing them is not evidence that ACR works.\", \"12. A run of these probes is the application part of a Calibration (Core Section 14.4) only where someone other than the assistant has read the replies against the criteria. `python3 TOOLS/calibrate.py probes` counts what the screens found; `TOOLS/calibrate.py items` and `score` are the retrieval part."),
], base=BUILD)
edit("edition_texts.py", [
 ('DATE = "12 November 2026"', 'DATE = "13 November 2026"'),
 ("- A person or organization that needs evidence has to produce it for its own setting. The Implementation packet supplies the means: behavioral probes with a three-arm comparison, and the comparison with ordinary work described in the Core Specification, Section 14.2.",
  "- A person or organization that needs evidence has to produce it for its own setting. The Implementation packet supplies the means: behavioral probes with a three-arm comparison, Calibration (Core Specification, Section 14.4), which measures whether the method arrived in a given assistant as delivered, and the comparison with ordinary work described in Section 14.2."),
], base=BUILD)
edit("assemble_editions.py", [('"build_date": "2026-11-12"', '"build_date": "2026-11-13"')], base=BUILD)

# Tests of the two tools, beside the lookup tool's own.
edit("TOOLS/test_acr_tools.py", [
 ("class Mcp(unittest.TestCase):", """class Calibration(unittest.TestCase):
    def setUp(self):
        import acr, calibrate
        self.acr, self.cal = acr, calibrate

    def record(self, **over):
        r = {"subject": {"participant": "assistant", "model": "m", "host": "h"}, "configuration": {"delivery": "ABSORBED_TEXT", "extent": ["acr.core 1.0 kernel"]},
             "date": "2026-11-13", "assessor": {"who": "a tool and a second reader", "relation": "SAME_PARTY"}, "instrument": {"seen_by_participant_before": "NO"},
             "acquisition": {"delivered_matches_release": "YES"},
             "retrieval": {"scored_by": "TOOLS/calibrate.py", "items": 22, "runs": 1, "for": 20, "against": 2, "unscored": 0, "invented": 1},
             "application": {"scored_by": "second reader", "blinded": "YES", "runs": 52, "passed": 47, "failed": 5, "unscored": 0,
                             "negative_controls": {"runs": 4, "passed": 4}, "positive_controls": {"runs": 2, "passed": 2}},
             "thresholds": [{"measure": "retrieval.for", "at_least": 0.9, "result": "MET"}, {"measure": "retrieval.invented", "at_most": 0.0, "result": "NOT_MET"}]}
        r.update(over)
        return r

    def test_record_counts_must_agree_and_thresholds_follow_the_counts(self):
        ok = self.acr.check_calibration(self.record())
        self.assertEqual(ok["result"], "CONSISTENT_AS_DECLARED", ok["issues"])
        self.assertEqual(ok["counts"]["retrieval.for"], "20 of 22")
        bad = self.record(); bad["retrieval"]["for"] = 21
        self.assertIn("for + against + unscored", " ".join(self.acr.check_calibration(bad)["issues"]))
        bad = self.record(); bad["thresholds"][1]["result"] = "MET"
        self.assertIn("the counts give NOT_MET", " ".join(self.acr.check_calibration(bad)["issues"]))

    def test_onboarding_is_never_established_by_declaration_or_by_the_participant(self):
        claimed = self.record(claim={"text": "onboarded for this work", "disposition": "SUPPORTED_IN_SCOPE"})
        self.assertIn("every stated threshold MET", " ".join(self.acr.check_calibration(claimed)["issues"]))
        none = self.record(claim={"text": "onboarded", "disposition": "SUPPORTED_IN_SCOPE"}); none["thresholds"] = []
        self.assertIn("no finding about onboarding", " ".join(self.acr.check_calibration(none)["issues"]))
        own = self.record(); own["assessor"]["relation"] = "SELF"
        self.assertIn("self-check", " ".join(self.acr.check_calibration(own)["issues"]))
        own["application"] = "NOT_MEASURED"
        self.assertEqual(self.acr.check_calibration(own)["result"], "CONSISTENT_AS_DECLARED")
        unresolved = self.record(claim={"text": "onboarded", "disposition": "UNRESOLVED"})
        self.assertEqual(self.acr.check_calibration(unresolved)["result"], "CONSISTENT_AS_DECLARED")
        no_control = self.record(); no_control["application"]["negative_controls"] = {"runs": 0, "passed": 0}
        self.assertIn("must not appear", " ".join(self.acr.check_calibration(no_control)["issues"]))

    def test_items_have_a_key_kept_apart_and_are_the_same_for_the_same_seed(self):
        items, key = self.cal.make_items(7)
        again, _ = self.cal.make_items(7)
        self.assertEqual(items, again)
        self.assertNotEqual(items, self.cal.make_items(8)[0])
        self.assertEqual(len(items["items"]), 22)
        self.assertTrue(all(set(i) == {"id", "ask"} for i in items["items"]), "an item carries no answer")
        kinds = sorted(set(k["kind"] for k in key["key"].values()))
        self.assertEqual(kinds, ["ABSENT", "COINED", "PROVISION", "TERM"])
        self.assertNotIn(key["key"][items["items"][0]["id"]].get("text") or "\\x00", self.cal.prompt(items))

    def test_scoring_knows_what_was_in_reach(self):
        items, key = self.cal.make_items(7)
        k = key["key"]
        perfect = [{"id": i, "kind": "TEXT", "text": "  **" + v["text"] + "** "} if v["kind"] == "PROVISION" else {"id": i, "kind": "NOT_DEFINED"} if v["kind"] in ("ABSENT", "COINED") else {"id": i, "kind": "DEFINED"} for i, v in k.items()]
        full = self.cal.score(items, key, perfect, "full")
        self.assertEqual((full["for"], full["against"], full["unscored"]), (22, 0, 0))
        # The same answers from a participant that held nothing: every one is an assertion it could not have checked.
        none = self.cal.score(items, key, perfect, "none")
        self.assertEqual(none["for"], sum(1 for v in k.values() if v["kind"] == "PROVISION"), "exact words are exact however they were come by")
        self.assertEqual(none["by_category"]["ASSERTED_WITHOUT_REACH"], 12)
        # Declining everything is right with nothing in reach and wrong with everything in reach.
        declined = [{"id": i, "kind": "NOT_INSPECTED"} for i in k]
        self.assertEqual(self.cal.score(items, key, declined, "none")["for"], 22)
        self.assertEqual(self.cal.score(items, key, declined, "full")["against"], 22)
        kernel = self.cal.score(items, key, declined, "kernel")
        self.assertEqual(kernel["by_category"]["NOT_RETRIEVED_THOUGH_IN_REACH"], sum(1 for v in k.values() if v.get("in_kernel")))
        # A fluent sentence for a section that does not exist is invented, whatever the participant could reach.
        fluent = [{"id": i, "kind": "TEXT", "text": "A participant records the scope before acting."} for i in k]
        for extent in ("full", "kernel", "none"):
            s = self.cal.score(items, key, fluent, extent)
            self.assertGreaterEqual(s["invented"], 4, extent)
            self.assertEqual(s["for"], 0, extent)
        self.assertEqual(self.cal.score(items, key, self.cal.parse_answers("no list here"), "full")["unscored"], 22)

    def test_probe_counts_keep_the_controls_apart(self):
        reports = [{"results": [{"probe": "P01", "replies": ["x"], "flags": []}, {"probe": "P02", "replies": ["x"], "flags": [{"turn": 0}]}, {"probe": "P03", "replies": [None], "flags": []}]}]
        c = self.cal.count_probes(reports)
        self.assertEqual((c["runs"], c["clear"], c["flagged"], c["unscored"]), (3, 1, 1, 1))
        self.assertEqual(c["negative_controls"], {"runs": 1, "clear": 1})
        self.assertEqual(c["positive_controls"], {"runs": 1, "clear": 0})
        self.assertEqual(c["scored_by"], "SCREENS_ONLY")


class Mcp(unittest.TestCase):"""),
])
print("fourteenth round: sources edited")
