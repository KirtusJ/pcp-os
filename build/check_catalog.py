#!/usr/bin/env python3
"""Is the catalogue the tree, and is master free of session context?

    python3 build/check_catalog.py            (from the repository root)

Predicates.
 K1 every tracked entry at the top of the repository is a row of CATALOG.md.
 K2 every tracked folder one level down in a folder named in DEEP is a row of CATALOG.md.
 K3 every row of CATALOG.md is a tracked path.
 K4 no tracked text file outside ALLOWED holds a session's address or a session's scratch path.
 K5 the comparison refuses, before the tree is read, a catalogue with a row missing, a row for a
    path that is not there, and a file holding a session's address.
Exit 0 only if all hold. Standard library and git only.
"""
import re
import subprocess
import sys

DEEP = ("PCP_OS_Rust/crates", "apps", "store", "examples", "os-docs", "audit")
# The one place still allowed to hold session context, until those records leave master (issue 368).
ALLOWED = ("audit/director-z3-tasks/records/",)
# This file names the patterns it looks for.
SELF = ("build/check_catalog.py",)
CONTEXT = re.compile(rb"claude\.ai/code/session_[0-9A-Za-z]+|/tmp/claude-[0-9]+/|chatgpt\.com/c/[0-9a-f-]{8,}")


def wanted(files):
    paths = {f.split("/")[0] for f in files}
    for parent in DEEP:
        depth = parent.count("/") + 2
        paths |= {"/".join(f.split("/")[:depth]) for f in files if f.startswith(parent + "/") and f.count("/") >= depth}
    return paths


def listed(text):
    return [m.group(1) for line in text.splitlines() for m in [re.match(r"\| `([^`]+)` \|", line)] if m]


def problems(files, text, contents):
    rows, tree = listed(text), wanted(files)
    tracked = set(files) | {"/".join(f.split("/")[:n]) for f in files for n in range(1, f.count("/") + 1)}
    found = ["K1/K2: not in the catalogue: " + p for p in sorted(tree - set(rows))]
    found += ["K3: in the catalogue but not tracked: " + p for p in sorted(set(rows) - tracked)]
    found += ["K3: listed twice: " + p for p in sorted({p for p in rows if rows.count(p) > 1})]
    for name, data in contents:
        if not name.startswith(ALLOWED) and name not in SELF and CONTEXT.search(data):
            found.append("K4: session context in " + name)
    return found


def selftest():
    files = ["README.md", "examples/a/x.py", "examples/a/y.py", "examples/top.txt", "audit/b/c/d.txt"]
    good = "| `README.md` | r | no |\n| `examples` | e | no |\n| `examples/a` | a | no |\n| `audit` | a | no |\n| `audit/b` | b | no |\n"
    address = b"see https://claude.ai/code/session_" + b"01AbCdEf"
    assert problems(files, good, [("README.md", b"plain")]) == []
    assert problems(files, good.replace("| `examples/a` | a | no |\n", ""), []) == ["K1/K2: not in the catalogue: examples/a"]
    assert problems(files, good + "| `gone` | g | no |\n", []) == ["K3: in the catalogue but not tracked: gone"]
    assert problems(files, good + "| `audit` | again | no |\n", []) == ["K3: listed twice: audit"]
    assert problems(files, good, [("README.md", address)]) == ["K4: session context in README.md"]
    assert problems(files, good, [(ALLOWED[0] + "r.txt", address)]) == []


def main():
    selftest()
    files = subprocess.check_output(["git", "ls-files", "-z"]).decode().split("\0")[:-1]
    contents = []
    for name in files:
        try:
            with open(name, "rb") as handle:
                contents.append((name, handle.read()))
        except OSError:
            pass  # a tracked path that is not a readable file here (a link out of the tree)
    found = problems(files, open("CATALOG.md", encoding="utf-8").read(), contents)
    if found:
        sys.exit("\n".join(found))
    allowed = sum(1 for name, data in contents if name.startswith(ALLOWED) and CONTEXT.search(data))
    print("catalogue: %d tracked files, %d rows, all agree; session context only in the %d files still allowed" % (
        len(files), len(listed(open("CATALOG.md", encoding="utf-8").read())), allowed))


if __name__ == "__main__":
    main()
