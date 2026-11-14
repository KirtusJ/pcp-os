#!/usr/bin/env python3
"""Local check for the PCP 1.0 plugins of one edition, supplied without the rest of the edition.
Standard library only.

  python3 verify_plugins.py

Checks the inventory and digests of this folder against MANIFEST.json; that the
marketplace is this edition's and lists the two plugins; that each plugin is
whole against its own manifest, states this edition, and carries the ACR
Implementation packet that PLUGINS.json names, file for file; and that the
PCP OS program is the one named and may be run. A pass is a local fact about
these files. It authenticates nobody, validates no meaning, and is not legal
advice or a statement that the product has been tested.
"""
import hashlib, json, os, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent
M = HERE / "PCP_1.0_Plugins"


class Fail(Exception):
    pass


def need(cond, msg):
    if not cond:
        raise Fail(msg)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def verify():
    man = json.loads((HERE / "MANIFEST.json").read_text(encoding="utf-8"))
    ident = json.loads((HERE / "PLUGINS.json").read_text(encoding="utf-8"))
    need(man.get("format") == "pcp-plugins-local-integrity/2" and ident.get("format") == "pcp-plugins-archive/1", "unsupported format")
    edition = ident.get("edition")
    need(edition in ("PERSONAL", "COMMERCIAL", "ENTERPRISE") and man.get("edition") == edition, "edition identity")
    files = {e["path"]: e for e in man["files"]}
    actual = sorted(p.relative_to(HERE).as_posix() for p in HERE.rglob("*")
                    if p.is_file() and p.relative_to(HERE).as_posix() != "MANIFEST.json" and "__pycache__" not in p.parts)
    need(actual == sorted(files) and man.get("payload_count") == len(files), "inventory differs from manifest: %s" % sorted(set(actual) ^ set(files))[:5])
    for p, e in files.items():
        data = (HERE / p).read_bytes()
        need(len(data) == e["bytes"] and sha(data) == e["sha256"], "content differs from manifest: %s" % p)
    for f in ("LICENSE.md", "NOTICE.md", "PRICING.md", "EVIDENCE_STATUS.md", "THIRD_PARTY_LICENSES.md", "README.md"):
        need((HERE / f).is_file(), "%s is missing" % f)
    need("All rights reserved" in (HERE / "NOTICE.md").read_text(encoding="utf-8"), "the notice does not carry its controlling statement")
    market = json.loads((M / ".claude-plugin" / "marketplace.json").read_text(encoding="utf-8"))
    need(market.get("name") == "pcp-" + edition.lower() and [p["name"] for p in market["plugins"]] == ["pcp", "pcp-os"], "marketplace identity")
    fp = ident["method_release"]["manifest_sha256"]
    for plugin in ("pcp", "pcp-os"):
        root = M / "plugins" / plugin
        need(sha((root / "MANIFEST.json").read_bytes()) == ident["plugins"][plugin]["manifest_sha256"], "plugin %s is not the one PLUGINS.json names" % plugin)
        pm = json.loads((root / "MANIFEST.json").read_text(encoding="utf-8"))
        for e in pm["files"]:
            need(sha((root / e["path"]).read_bytes()) == e["sha256"], "plugin %s: %s differs from its manifest" % (plugin, e["path"]))
        listed = {e["path"] for e in pm["files"]}
        on_disk = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p.name != "MANIFEST.json" and "__pycache__" not in p.parts}   # method/MANIFEST.json is checked against the pinned release below
        need(listed == on_disk, "plugin %s: inventory differs from its manifest" % plugin)
        prof = json.loads((root / "edition" / "EDITION.json").read_text(encoding="utf-8"))
        need(prof.get("edition") == edition and prof.get("method_release", {}).get("manifest_sha256") == fp, "plugin %s states another edition or method release" % plugin)
        need(sha((root / "method" / "MANIFEST.json").read_bytes()) == fp, "plugin %s carries another method release" % plugin)
        # A plugin carries the method's text: every file of the packet outside its programs and probes, each the packet's own bytes
        packet = {e["path"]: e["sha256"] for e in json.loads((root / "method" / "MANIFEST.json").read_text(encoding="utf-8"))["files"]}
        held = {p.relative_to(root / "method").as_posix() for p in (root / "method").rglob("*") if p.is_file()} - {"MANIFEST.json"}
        need(held <= set(packet) and not any(p.startswith("INTEGRATION/") for p in held), "plugin %s: its method folder holds a file that does not belong there" % plugin)
        need(all(p in held for p in packet if not p.startswith(("INTEGRATION/", "TOOLS/"))), "plugin %s: a file of the method's text is missing" % plugin)
        for path in held:
            need(sha((root / "method" / path).read_bytes()) == packet[path], "plugin %s: method file %s is not the packet's" % (plugin, path))
        pj = json.loads((root / ".claude-plugin" / "plugin.json").read_text(encoding="utf-8"))
        need(pj.get("name") == plugin and pj.get("version") == ident.get("plugin_version") and pj.get("author", {}).get("name") == "Kirtus Justus", "plugin %s identity" % plugin)
    progs = ident["plugins"]["pcp-os"]["programs"]
    for prog in progs:
        need(sha((M / prog["file"]).read_bytes()) == prog["sha256"], "a PCP OS program is not the one PLUGINS.json names: %s" % prog["file"])
    if os.name == "posix":
        for f in ["plugins/pcp-os/servers/pcpos"] + [prog["file"] for prog in progs]:
            need(os.access(M / f, os.X_OK), "PCP_1.0_Plugins/%s is not executable: the archive was extracted without its permissions. Run: chmod +x \"%s\"" % (f, M / f))
    return edition, len(files)


def main():
    try:
        edition, n = verify()
    except Fail as e:
        print("FAIL_PLUGINS_CHECK: %s" % e)
        return 1
    except (OSError, ValueError, KeyError) as e:
        print("FAIL_PLUGINS_CHECK: %s: %s" % (type(e).__name__, e))
        return 1
    print("PASS_PLUGINS_CHECK %s edition, %d files (not authenticated, not certified, not peer reviewed, behavior not tested)" % (edition, n))
    return 0


if __name__ == "__main__":
    sys.exit(main())
