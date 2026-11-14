#!/usr/bin/env python3
"""Local release check for the ACR Version 1.0 Implementation packet (Adversarial Claim Review). Standard library only.

Checks the file inventory and hashes against MANIFEST.json and the pins below,
then re-derives the cross-file relationships from the sources: component
identities, the Core pin, the clause index, the kernel extracts, defined terms,
load tiers, the path crosswalk and the probe controls. It also checks that the packet holds only
text files and that no excluded vocabulary has been reintroduced: the excluded terms are held only
as SHA-256 digests of lower-case tokens, so this file names none of them.

A pass is a local fact about these files. It does not authenticate the
publisher, validate meaning, show how any assistant behaves or grant anything.
Anyone able to rewrite the package can rewrite this file: compare the printed
release fingerprint with one obtained from the publisher by a separate channel.
"""
from __future__ import annotations
import csv, hashlib, io, json, re, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
MANIFEST_FORMAT = "acr-release-local-integrity/2.0"
SELF = "TOOLS/verify_release.py"
EXPECTED_PINS = {}  # @@PINS@@
EXCLUDED_TOKEN_SHA256 = frozenset([
    '00355d712afb18f4f2dcb05f06ff58411b3512796ec63d008eb4b8037e970b1b',
    '04d656f6689cbaac3cff425aeded4673a91ef776883f961e4d19c1b5a2f8f3c4',
    '06a8d99fc6930dbf638ca0330c53a435b63c9541957cf9b7a237c336916c5f81',
    '07589d5a8215211f1d85e09303c89e8176a5d6b2c8c9d3b7abf0b6ac3cd52047',
    '1107ee0c41402bc5b8da3cf459b32dea94095e28e3d87201d5754f6935216eba',
    '11e221bd1b7fc99544f3f765b0c39dd2786e636c3422d6bec026d1ed03884cac',
    '1d883ad5a90bb42c11ee0da2200d98849966b5ba08b97fa1c1b9e0b74d15471c',
    '27d92a9db854f475aea98cc6c631a4826542a47190b3bf90753beb6f6be2649b',
    '2b7847b7b705781d7cf21a05e9c1bb37cbf078aea103bc3edcc6aca52ab65453',
    '305238273bb0fac2a73b43256e38d1515fdf61ec1d30ef161446c3588b93a97b',
    '3121771b9c5973e0f089fc009419975232f6f374f1f95a118e8259c98dd5ded4',
    '31a89e3644524440db30f17a7a9676768329cf3f40d1142d327429ea3eb3e6b3',
    '3c054e0895eb388192f4f044f563b0b7b5ce9816ba4f04fe3609db080a266a54',
    '3d7eb3d3dab17ee509a6911e0feb928eb46e956adcada792493614bfb97a7a9c',
    '400f16dd64f8fde979c0405863b6e48fb9222b7263e4bda0894f3bfd1894a313',
    '4482f0a99d90207638fbe31c899304e0520fce94751cfdc58d767f685c89b70c',
    '4488b8b86b1ac061dbe37242297e5827dad889823fd1a5acaed43dec0108d048',
    '46d43b13b63ec7bfce84d59eaf1487549381631bc730c1b23c80c14d38688448',
    '48095a1c70e7fed8de7e6d224a977cf0eaa297149c2968f5cc2e03ffa0c6c6b1',
    '5805801f160b90e1bf7c1908767c4ab1c249b1eeb765c89568db630cd7734f14',
    '5947be9d5aba81ec204c6ef0d413473e61328f3f02a824ef18ca58dc4fc5867c',
    '5d5bddb577102d0a960bcf6fea9050c10fe5e9feddcb5c2170ccab872db9ee87',
    '6525e305a557ea1b3c85112e59be623834767668b7268209292c2c7a5dfa5594',
    '6c719a94030a6c484bc6e29b04ac4c6d26b5fa504efe0dc439e9ae4654421a90',
    '6f7a9000cccc11b5223760ed0b76329581848cd0604509730ca2be54e6d317d8',
    '70dd27e3d4349e0fad4c0356ffbc9b9683def260678092d8bf117180f608c523',
    '7a5179eecc0fe18760ba615f92603372ae3fe302860098a019e15927551fee3b',
    '7f14eadc80600546fd90badfdd93d13f356f9a8a50abf8ce4195f67f062de824',
    '81803dd4e410a92bc5d510ea539a5d38493e3fe26b707dbac6a63b9e3a7b4d90',
    '87fb00d259cbcd67706332f118d16e7068661b5e99ba5bd41f7e9e60f96e6c34',
    '8a71ffb1cb03f587913dff1d1e116ba6c1b79eb7c2458d47e4546a69c3b25fde',
    '8c5ae2da1fb999644157d94d951c76bc2b8e5436ccaa878d439bb4b39fddb71a',
    '8ce12ec2a09b7d1d07a7c4b9bb1d91b33a054386bd47dca8f51199d61b25cfad',
    '916aa376e9fb0904ff1ada9e946f9651ddb4b85eebbb939616d08fd61da5f490',
    '929f9a342442b12741c13baeffee7078db3f00541a323b3e56061bb4a547bcf6',
    '94b4a9f92e452b538dfcb4735ed01f2b8a7fd44ce6f2787f5074242092fbdde3',
    '9bb448f456b812b04c7b9270d35340b168e440af4cb50ace0eeaa31c345650cf',
    'a0c7ec73f6d7f2b4f3e6de3769d290d4f877ff97bad08b675f9abd0c8ee97dff',
    'a5dc8ae9108d0dedee822d292ec367995afde95ee3c9332906f97cb3d743be4a',
    'a6b7efa857f92efe3fe15769cb583efd1e81b0e510fd8587e41ebacf74a47d94',
    'a9e3ea5f5a1cdbe81128ec8d5bf1a4d2e03b2f978b09d198507bbb5d105feaf5',
    'abbbe40618c0898217b2236530024c3b6fd24997b03d8c08a180dcda2d239be0',
    'adfa3d8774f17569423b6ea58ab27815817b4be0a21274afca72b4c9c26dfe3c',
    'b1fbf686a9a64dffbd3917b739b3e81a073a4d14529c69146c04c9d4895dc62b',
    'b52115ea553c55b7f70ceb81a97870163e02cb159d7d4717abb82d4e606e2465',
    'bf89f78e1363c1ffd693196e1212910d3fd429a4438ab9ea4f7cdce8ba0a747c',
    'c415349aa6b5513deafe98172f2f96bb1ba8797797ccddbefad7538d8602fb6f',
    'c430a317993b507d372ba0ae3b418fe1a6a141a125178c89e2ce347bfa22baf8',
    'ce387d5e0a2972dea9e5129a52ac3b8d58a4d180fc9eece5946d926643a3d2c0',
    'd05c34e72352778bfa9070db07730b8b4a061d2f9afeaa8b66d76dcf7275b34d',
    'd65620743e765b499b98679dd86129147db886b57c01ce533faba291a414ef51',
    'e9d9d0f618e05c97b1b5d121c57a45825461f2079ff2feee93f787d835a701d0',
    'ed3c7bc96fed7441c591e20c2ceebaf1b7166df1d707c975d177246ad791bf9c',
    'ee4a4d2d0f10b5f8ac34888ffbb8ec1f638dff5cfdd01a6ae7e5d0e5488c46dc',
    'f176f4ccf681245ecf8c063c0704551aefae187a976ff748d868ce98f6869db7',
    'f99aab15a045ef02a523ebdc8f5b3e884960f410307f8805fd41c60da2d5a848',
    'f9da47b6ea1c414fb0fb0f2516948f296f2fcee5e74b059312311880b766d3aa',
    'fb4eec2e64f95eca4d1893147a5b3b52c0746fdba61f329640e6206cb29a8ef5',
    'ffb301e28a65f3046dd636ef12d04f8d2fc2e2b3b0001b0d88ce5e770dbca2da',
])
COMPONENT_ORDER = ["acr.core", "acr.mathematics", "acr.science", "acr.engineering", "acr.cryptography",
                   "acr.statistics", "acr.quality_assurance"]
CORE_FILES = ["MODULES/01_Core/SOURCE/Specification.md",
              "SUPPLEMENTS/Core_Conditional/Command_Interface.md", "SUPPLEMENTS/Core_Conditional/Methods_Catalog.md",
              "SUPPLEMENTS/Core_Conditional/Record_Contracts.md"]
STATUS = {"product": "ACR", "method": "Adversarial Claim Review", "product_version": "1.0", "publication_state": "PRE_PUBLICATION",
          "licensing": "PROPRIETARY_ALL_RIGHTS_RESERVED", "rights_granted_by_possession": "NONE",
          "host_effect_authorization": "NOT_GRANTED_BY_THIS_PACKAGE",
          "professional_or_safety_certification": "NOT_ESTABLISHED", "third_party_llm_service_included": False,
          "peer_review": "NOT_PERFORMED", "independent_review": "NOT_PERFORMED", "behavioral_validation": "NOT_PERFORMED",
          "empirical_evaluation": "NOT_PERFORMED", "observational_evidence": "NONE_OF_EFFECT",
          "packet": "IMPLEMENTATION"}
PATH_TOKEN = re.compile(r"(?<![\w/.-])((?:[\w-]+/)*[\w-]+\.(?:md|py|json|csv))(?![\w-])")
PIN_RE = re.compile(r"Core descriptor SHA-256 `([0-9a-f]{64})`")


class Fail(Exception):
    pass


def need(cond, msg):
    if not cond:
        raise Fail(msg)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def strict_json(data):
    def pairs(items):
        d = {}
        for k, v in items:
            need(k not in d, "duplicate JSON key: %s" % k)
            d[k] = v
        return d

    def bad(_):
        raise Fail("non-finite JSON number")
    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=pairs, parse_constant=bad)
    except (UnicodeDecodeError, json.JSONDecodeError) as e:
        raise Fail("invalid JSON: %s" % e)


def core_descriptor(root):
    files = {p: sha((root / p).read_bytes()) for p in CORE_FILES}
    text = (root / CORE_FILES[0]).read_text(encoding="utf-8")
    import acr
    obj = {"component_id": "acr.core", "version": acr.version_of(text), "files": files}
    return (json.dumps(obj, indent=1, sort_keys=True) + "\n").encode("utf-8")


def missing_path_refs(root):
    """Path-like tokens in the inherited sources that do not resolve to a file in the package."""
    have = [p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()]
    names = {h.rsplit("/", 1)[-1] for h in have}
    out = set()
    for md in sorted(list((root / "MODULES").glob("*/SOURCE/*.md")) + list((root / "SUPPLEMENTS").rglob("*.md"))):
        text = re.sub(r"https?://\S+", " ", md.read_text(encoding="utf-8"))
        for tok in set(PATH_TOKEN.findall(text)):
            if tok in have or any(h.endswith("/" + tok) for h in have) or ("/" not in tok and tok in names):
                continue
            out.add((md.relative_to(root).as_posix(), tok))
    return sorted(out)


def kernel_extracts(text):
    return re.findall(r"<!-- acr:begin ([\w.:-]+) -->\n(.*?)\n<!-- acr:end -->", text, flags=re.S)


def verify(root):
    import acr
    # 1. Manifest and inventory
    mpath = root / "MANIFEST.json"
    need(mpath.is_file(), "MANIFEST.json missing")
    mbytes = mpath.read_bytes()
    man = strict_json(mbytes)
    need(isinstance(man, dict) and set(man) == {"format", "payload_count", "files"}, "manifest keys")
    need(man["format"] == MANIFEST_FORMAT, "unsupported manifest format")
    files = man["files"]
    need(isinstance(files, list) and type(man["payload_count"]) is int and man["payload_count"] == len(files), "payload count")
    seen = {}
    for e in files:
        need(isinstance(e, dict) and set(e) == {"path", "bytes", "sha256"}, "manifest entry keys")
        p = e["path"]
        need(isinstance(p, str) and p and not p.startswith("/") and ".." not in p.split("/") and "\\" not in p, "unsafe path")
        need(p not in seen, "duplicate path %s" % p)
        need(type(e["bytes"]) is int and e["bytes"] >= 0, "byte count must be an integer: %s" % p)
        need(isinstance(e["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", e["sha256"]), "digest format: %s" % p)
        seen[p] = e
    actual = sorted(q.relative_to(root).as_posix() for q in root.rglob("*")
                    if q.is_file() and q.name != "MANIFEST.json" and "__pycache__" not in q.parts)
    need(actual == sorted(seen), "inventory differs from manifest: %s" % sorted(set(actual) ^ set(seen))[:5])
    for p, e in seen.items():
        data = (root / p).read_bytes()
        need(len(data) == e["bytes"] and sha(data) == e["sha256"], "content differs from manifest: %s" % p)
    # 2. Pinned identities
    need(set(EXPECTED_PINS) | {SELF} == set(seen), "file set differs from pinned release: %s"
         % sorted((set(EXPECTED_PINS) | {SELF}) ^ set(seen))[:5])
    for p, h in EXPECTED_PINS.items():
        need(seen[p]["sha256"] == h, "content differs from pinned release: %s" % p)
    # 3. Status and component identity
    need(strict_json((root / "RELEASE/Release_Status.json").read_bytes()) == STATUS, "release status record")
    comp = strict_json((root / "RELEASE/Components.json").read_bytes())
    need(comp.get("product") == "ACR" and comp.get("version") == "1.0", "product identity")
    cv = comp.get("component_versions")
    need(isinstance(cv, list) and [c.get("component_id") for c in cv] == COMPONENT_ORDER, "component order")
    keys = list(acr.COMPONENTS)
    for c, key in zip(cv, keys):
        spec = acr.read_source(key, root)
        over = (root / acr.COMPONENTS[key][1].replace("Specification.md", "Overview.md")).read_text(encoding="utf-8")
        need(c["component_version"] == "1.0" == acr.version_of(spec) == acr.version_of(over), "version mismatch for %s" % key)
        need(c["component_id"] == acr.COMPONENTS[key][0] and c["source_sha256"] == acr.identity(key, root)[2], "identity for %s" % key)
        need(("**Component:** `%s`, Version 1.0" % c["component_id"]) in spec and ("**Component:** `%s`, Version 1.0" % c["component_id"]) in over, "component line for %s" % key)
        want = "CORE_APPLIES_WHEN_ACR_IS_AUTHORIZED" if key == "core" else "NOT_SELECTED_BY_DISTRIBUTION"
        need(c.get("task_selection") == want and c.get("distribution_status") == "RELEASED", "selection rule for %s" % key)
    with open(root / "REFERENCE/Module_Catalog.csv", newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))
    need(len(rows) == len(acr.COMPONENTS), "catalog rows")
    for r, c in zip(rows, cv):
        need(r["component_id"] == c["component_id"] and r["component_version"] == c["component_version"]
             and r["task_selection"] == c["task_selection"] and r["automatic_task_selection"] == "false"
             and r["distribution_status"] == "RELEASED"
             and r["required_component"] == ("none" if c["component_id"] == "acr.core" else "acr.core"), "catalog row %s" % r["component_id"])
    # 4. Core pin
    desc = core_descriptor(root)
    need((root / "RELEASE/Core_Descriptor.json").read_bytes() == desc, "Core descriptor is not derived from the Core files")
    dsha = sha(desc)
    need(comp.get("core_descriptor_sha256") == dsha, "Components.json Core descriptor digest")
    core_line = "**Required core:** `acr.core`, Version 1.0; Core descriptor SHA-256 `%s`" % dsha
    for key in keys[1:]:
        for name in ("Specification.md", "Overview.md"):
            t = (root / acr.COMPONENTS[key][1].replace("Specification.md", name)).read_text(encoding="utf-8")
            need(PIN_RE.findall(t) == [dsha] and core_line in t, "Core pin in %s %s" % (key, name))
    for p in CORE_FILES[1:]:
        t = (root / p).read_text(encoding="utf-8")
        need("Conditional attachment to `acr.core`, Version 1.0" in t, "conditional contract identity: %s" % p)
    # No revision or release tags anywhere: the version and the digests are the only identity.
    tag = re.compile(r"\b[A-Z]{2,5}-\d{4}-\d{2}-\d{2}-(?:[A-Z0-9]+-)*R\d+\b|\b(?:Core|Science|Engineering|Mathematics|Cryptography|Statistics) R\d+\b|\bRevision R\d+\b")
    for q in root.rglob("*"):
        if q.is_file() and q.suffix in (".md", ".json", ".csv", ".py") and "__pycache__" not in q.parts:
            need(not tag.search(q.read_text(encoding="utf-8", errors="replace")), "revision tag found in %s" % q.relative_to(root).as_posix())
    # 5. Clause index and internal section references
    need((root / "REFERENCE/Clause_Index.csv").read_bytes().decode("utf-8") == acr.index_csv(root), "clause index is not derived from the sources")
    core = acr.read_source("core", root)
    provs = {r[0] for r in acr.index_source(core)}
    normative = core[:core.index("## Appendix C.")]
    for m in re.finditer(r"Sections? ((?:\d+(?:\.\d+)?)(?:(?:, | and | to |-)\d+(?:\.\d+)?)*)(?![\d.]*\d)", normative):
        for ref in re.findall(r"\d+(?:\.\d+)?", m.group(1)):
            need(ref in provs, "Core refers to a section that does not exist: %s" % ref)
    allprov = {k: {r[0] for r in acr.index_source(acr.read_source(k, root))} for k in acr.COMPONENTS}
    label = {"Core": "core", "Mathematics": "mathematics", "Science": "science", "Engineering": "engineering",
             "Cryptography": "cryptography", "Statistics": "statistics", "QA": "quality_assurance", "Quality Assurance": "quality_assurance"}
    xref = re.compile(r"\b(Core|Mathematics|Science|Engineering|Cryptography|Statistics|QA|Quality Assurance)(?: Specification)?,? "
                      r"(?:Sections?|\u00a7+) ?((?:\d+(?:\.\d+)?)(?:(?:, | and | to |-|\u2013)\d+(?:\.\d+)?)*)")
    texts = [q for q in root.rglob("*.md") if "__pycache__" not in q.parts]
    for q in texts:
        for m in xref.finditer(q.read_text(encoding="utf-8")):
            for ref in re.findall(r"\d+(?:\.\d+)?", m.group(2)):
                need(ref in allprov[label[m.group(1)]], "%s refers to %s %s, which does not exist" % (q.relative_to(root).as_posix(), m.group(1), ref))
    # 6. Kernel
    kernel = (root / "KERNEL.md").read_text(encoding="utf-8")
    ex = kernel_extracts(kernel)
    need(len(ex) >= 4, "kernel extracts")
    for name, block in ex:
        need(block in core, "kernel extract is not verbatim Core text: %s" % name)
    a, b = core.index("## 15. Operating floor"), core.index("## Appendix A. Glossary")
    need(core[a:b].strip() in dict(ex).values(), "kernel must contain Core Section 15 complete")
    need(("acr.core 1.0 | descriptor %s" % dsha) in kernel, "kernel names the Core identity")
    need((root / "INTEGRATION/agent-skill/acr/KERNEL.md").read_text(encoding="utf-8") == kernel, "skill kernel replica")
    skill_dir = root / "INTEGRATION/agent-skill/acr"
    fm = re.match(r"---\n(.*?)\n---\n", (skill_dir / "SKILL.md").read_text(encoding="utf-8"), flags=re.S)
    need(fm is not None, "skill front matter")
    sname = re.search(r"^name: (.+)$", fm.group(1), flags=re.M); sdesc = re.search(r"^description: (.+)$", fm.group(1), flags=re.M)
    need(sname and sname.group(1) == skill_dir.name and re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", sname.group(1)) and len(sname.group(1)) <= 64, "skill name")
    need(sdesc and 0 < len(sdesc.group(1)) <= 1024, "skill description length")
    need("core-component: acr.core 1.0" in fm.group(1), "skill names the Core component")
    card = (root / "KERNEL_CARD.md").read_text(encoding="utf-8")
    need(len(card.encode("utf-8")) <= 1500 and "Core 1.0" in card, "kernel card size or identity")
    for ref in re.findall(r"\((\d+\.\d+)\)", card):
        need(ref in provs, "kernel card cites a missing section: %s" % ref)
    # 7. Defined terms and vocabulary
    terms = acr.terms(root)
    names = [t["defined_term"] for t in terms]
    need(len(names) == len(set(names)), "duplicate defined-term entry")
    duties = re.findall(r"^\| ([A-Z][\w ]+?) \| ", core[core.index("| Duty | Function |"):core.index("### 3.2 ")], flags=re.M)
    duties = [d for d in duties if d != "Duty"]
    need(len(duties) == 14 and all(d in names for d in duties), "every Core duty must be listed in Defined_Terms.csv")
    need(sorted(t["defined_term"] for t in terms if t["class"] == "duty") == sorted(duties), "duty rows in Defined_Terms.csv")
    need(all(t["persona_permitted"] == "NO" for t in terms), "persona flag")
    codes = re.search(r"Role codes are ([A-Z_, ]+?) and ([A-Z_]+)\.", (root / CORE_FILES[3]).read_text(encoding="utf-8"))
    need(codes and sorted(c.strip() for c in (codes.group(1) + ", " + codes.group(2)).split(",")) == sorted(d.upper().replace(" ", "_") for d in duties), "record role codes match the Core duties")
    # Excluded vocabulary: no text file may contain a token whose digest is in the excluded set.
    tok = re.compile(r"[a-z]+")
    for q in root.rglob("*"):
        if not q.is_file() or q.suffix not in (".md", ".json", ".csv", ".py") or "__pycache__" in q.parts:
            continue
        ws = tok.findall(q.read_text(encoding="utf-8", errors="replace").lower())
        for w in ws + [a + " " + b for a, b in zip(ws, ws[1:])]:
            need(sha(w.encode("utf-8")) not in EXCLUDED_TOKEN_SHA256, "excluded vocabulary in %s" % q.relative_to(root).as_posix())
    # 8. Derived reference files
    need((root / "REFERENCE/Load_Tiers.csv").read_text(encoding="utf-8") == acr.sizes_csv(root), "load tiers are not measured from the files")
    with open(root / "REFERENCE/Path_Crosswalk.csv", newline="", encoding="utf-8") as f:
        cw = list(csv.DictReader(f))
    listed_refs = {(r["referenced_in"], r["referenced_path"]) for r in cw}
    need(set(missing_path_refs(root)) == listed_refs, "path crosswalk differs from the references found in the sources")
    for r in cw:
        need(r["status"] in ("PACKAGED_AT", "NOT_DISTRIBUTED"), "crosswalk status")
        if r["status"] == "PACKAGED_AT":
            need((root / r["packaged_path"]).is_file(), "crosswalk target missing: %s" % r["packaged_path"])
    # 8a. Notice and evidence status
    notice = (root / "NOTICE.md").read_text(encoding="utf-8")
    need("**Proprietary material. All rights reserved.**" in notice and "Copyright (c) 2026 Kirtus Justus. All rights reserved" in notice
         and "Holding a copy grants nothing." in notice and "supplied only under license" in notice, "proprietary notice")
    kl = (root / "RELEASE/Known_Limitations.md").read_text(encoding="utf-8")
    for phrase in ("**Not peer reviewed.**", "**Not independently reviewed.**", "**Not empirically evaluated.**", "**No observational evidence of effect.**"):
        need(phrase in kl, "evidence status statement: %s" % phrase)
    # The evidence statement has one home (the limits file), one rule (Core 1.1) and one pointer at each entry point.
    need("A participant MUST NOT describe ACR as validated, proven, certified or peer reviewed." in core, "Core 1.1 evidence rule")
    for entry in ("README.md", "START_HERE.md"):
        need("RELEASE/Known_Limitations.md" in (root / entry).read_text(encoding="utf-8"), "evidence status pointer in %s" % entry)
    need("not peer reviewed" in (root / "KERNEL_CARD.md").read_text(encoding="utf-8"), "kernel card evidence status")
    need("has not been peer reviewed, independently reviewed or empirically evaluated" in kernel, "kernel evidence status")
    # 9. Text-only packet: the typeset reading copies are distributed separately
    for p in seen:
        need(p.rsplit(".", 1)[-1] in ("md", "json", "csv", "py"), "non-text payload in the Implementation packet: %s" % p)
    # 10. Probes
    probes = strict_json((root / "INTEGRATION/probes.json").read_bytes())
    ids = [p["id"] for p in probes["probes"]]
    need(len(ids) == len(set(ids)) and len(ids) >= 26, "probe identifiers")
    for p in probes["probes"]:
        for ref in re.findall(r"\d+\.\d+", p["core_clause"]):
            need(ref in provs, "probe cites a missing Core section: %s" % ref)
    import probe_runner
    need(probe_runner.self_test() == [], "probe screen controls")
    # 11. Tool self-tests
    import io as _io, unittest
    suite = unittest.defaultTestLoader.loadTestsFromNames(["test_command_interface", "test_acr_tools", "test_acr_host"])
    result = unittest.TextTestRunner(stream=_io.StringIO(), verbosity=0).run(suite)
    need(result.testsRun >= 40 and result.wasSuccessful(), "tool self-tests failed (%d failures, %d errors)" % (len(result.failures), len(result.errors)))
    return len(files), sha(mbytes)


def main(argv=None):
    root = Path(argv[0]).resolve() if argv else HERE.parent
    try:
        n, fp = verify(root)
    except Fail as e:
        print("FAIL_LOCAL_RELEASE_CHECK: %s" % e)
        return 1
    except (OSError, KeyError, ValueError, TypeError, IndexError, re.error) as e:
        print("FAIL_LOCAL_RELEASE_CHECK: %s: %s" % (type(e).__name__, e))
        return 1
    print("PASS_LOCAL_STRUCTURAL_AND_DERIVED_CHECKS %d payloads, ACR Version 1.0 Implementation packet / ACR Core 1.0 (not authenticated, not certified, behavior not tested)" % n)
    print("release fingerprint (SHA-256 of MANIFEST.json): %s" % fp)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
