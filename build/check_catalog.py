#!/usr/bin/env python3
"""Is the catalogue the tree, and is master free of session context?

    python3 build/check_catalog.py            (from the repository root)

Predicates.
 K1 every tracked entry at the top of the repository is a row of CATALOG.md.
 K2 every tracked folder one level down in a folder named in DEEP is a row of CATALOG.md.
 K3 every row of CATALOG.md is a tracked path.
 K4 every tracked file is read, and none outside ALLOWED holds a session's address or a session's
    scratch path. A file that cannot be read fails the check: it has not been scanned. A symbolic
    link is scanned as the path it names and is not followed.
 K6 every relative link in every tracked Markdown file names a tracked path, so that a move which
    breaks a link fails here. Links inside code fences, links to another site and links within a
    page are not looked at.
 K5 the comparison refuses, before the tree is read, a catalogue with a row missing, a row for a
    path that is not there, a file holding a session's address, a file that cannot be read, and a
    Markdown link to a path that is not there.
    These controls are not asserts, so they also run under python -O.
Exit 0 only if all hold. Standard library and git only.
"""
import os
import posixpath
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


FENCE = re.compile(rb"```.*?```", re.S)
LINK = re.compile(rb"\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
ELSEWHERE = re.compile(r"[a-z][a-z0-9+.-]*:|#|//")


def broken_links(name, data, tracked):
    """The relative links of one Markdown file that do not name a tracked path."""
    found = []
    for match in LINK.finditer(FENCE.sub(b"", data)):
        link = match.group(1).decode("utf-8", "replace")
        if ELSEWHERE.match(link):
            continue
        target = link.split("#")[0].split("?")[0]
        if not target:
            continue
        path = target.lstrip("/") if target.startswith("/") else posixpath.normpath(posixpath.join(posixpath.dirname(name), target))
        if path != "." and path not in tracked:
            found.append("K6: %s links to %s, which is not tracked" % (name, link))
    return found


def problems(files, text, contents):
    rows, tree = listed(text), wanted(files)
    tracked = set(files) | {"/".join(f.split("/")[:n]) for f in files for n in range(1, f.count("/") + 1)}
    found = ["K1/K2: not in the catalogue: " + p for p in sorted(tree - set(rows))]
    found += ["K3: in the catalogue but not tracked: " + p for p in sorted(set(rows) - tracked)]
    found += ["K3: listed twice: " + p for p in sorted({p for p in rows if rows.count(p) > 1})]
    for name, data in contents:
        if data is not None and name.endswith(".md"):
            found += broken_links(name, data, tracked)
    for name, data in contents:
        if data is None:
            found.append("K4: could not be read, so not scanned: " + name)
        elif not name.startswith(ALLOWED) and name not in SELF and CONTEXT.search(data):
            found.append("K4: session context in " + name)
    return found


def require(condition, what):
    """A control that must hold. Not `assert`: Python run with -O drops asserts, and these must run."""
    if not condition:
        sys.exit("K5: the check's own control failed: " + what)


def selftest():
    files = ["README.md", "examples/a/x.py", "examples/a/y.py", "examples/top.txt", "audit/b/c/d.txt"]
    good = "| `README.md` | r | no |\n| `examples` | e | no |\n| `examples/a` | a | no |\n| `audit` | a | no |\n| `audit/b` | b | no |\n"
    address = b"see https://claude.ai/code/session_" + b"01AbCdEf"
    require(problems(files, good, [("README.md", b"plain")]) == [], "a true catalogue passes")
    require(problems(files, good.replace("| `examples/a` | a | no |\n", ""), []) == ["K1/K2: not in the catalogue: examples/a"], "a missing row")
    require(problems(files, good + "| `gone` | g | no |\n", []) == ["K3: in the catalogue but not tracked: gone"], "a row for nothing")
    require(problems(files, good + "| `audit` | again | no |\n", []) == ["K3: listed twice: audit"], "a row twice")
    require(problems(files, good, [("README.md", address)]) == ["K4: session context in README.md"], "a session address")
    require(problems(files, good, [(ALLOWED[0] + "r.txt", address)]) == [], "the allowed place")
    require(problems(files, good, [("README.md", None)]) == ["K4: could not be read, so not scanned: README.md"], "an unreadable file (C3)")
    page = b"[a](examples/a/x.py) [b](../README.md#top) [c](https://example.org/x) [d](#here)\n```\n[e](gone.md)\n```\n"
    require(problems(files, good, [("audit/b/n.md", page.replace(b"examples/a", b"../../examples/a").replace(b"../README", b"../../README"))]) == [], "links that hold")
    require(problems(files, good, [("README.md", b"see [the plan](os-docs/moved.md)")]) == ["K6: README.md links to os-docs/moved.md, which is not tracked"], "a link to a path that is gone")
    # A link is not followed: what it names is scanned, and a session address in the name is found.
    require(problems(files, good, [("README.md", address)]) != [], "a link whose target names a session")


def read(name):
    """The bytes to scan for one tracked path, or None if they cannot be had. A symbolic link is
    scanned as the path it names, never followed; a submodule entry has no bytes here and is
    scanned as its name."""
    try:
        if os.path.islink(name):
            return os.readlink(name).encode()
        if os.path.isdir(name):
            return name.encode()
        with open(name, "rb") as handle:
            return handle.read()
    except OSError:
        return None


def main():
    selftest()
    files = subprocess.check_output(["git", "ls-files", "-z"]).decode().split("\0")[:-1]
    contents = [(name, read(name)) for name in files]
    text = open("CATALOG.md", encoding="utf-8").read()
    found = problems(files, text, contents)
    if found:
        sys.exit("\n".join(found))
    allowed = sum(1 for name, data in contents if name.startswith(ALLOWED) and CONTEXT.search(data))
    links = sum(len(LINK.findall(FENCE.sub(b"", data))) for name, data in contents if name.endswith(".md"))
    print("catalogue: %d tracked files, every one read, %d rows, all agree; %d Markdown links looked at, none broken; session context only in the %d files still allowed" % (
        len(files), len(listed(text)), links, allowed))


if __name__ == "__main__":
    main()
