#!/usr/bin/env python3
"""Is the catalogue the tree, and is master free of session context?

    python3 build/check_catalog.py            (from the repository root)

Predicates.
 K1 every tracked entry at the top of the repository is a row of CATALOG.md.
 K2 every tracked folder one level down in a folder named in DEEP is a row of CATALOG.md.
 K3 every row of CATALOG.md is a tracked path.
 K4 every tracked file is read, and none outside ALLOWED holds a session's address or a session's
    scratch path. A file that cannot be read fails the check: it has not been scanned. What a
    path should be is taken from git's index: a tracked file replaced by a folder fails; a
    symbolic link is scanned as the path it names and is not followed.
 K6 every relative link in every tracked Markdown file names a tracked path, so that a move which
    breaks a link fails here. Two spellings are read: the inline "[words](target)" and the
    reference definition "[label]: target" on one line. A target in angle brackets is read
    through the closing bracket; a bare one ends at white space. A definition is checked whether
    or not anything uses it. Not read: a definition whose target is on the following line,
    percent-encoded paths (they are not decoded), raw HTML and bare addresses. Links inside code
    fences, of backticks or tildes, are not looked at: assured for a fence at the top level of a
    page, best effort inside a list item or quotation, where a link after the container ends
    can be missed (see outside_fences); links to another site and links within a
    page are counted and reported, not checked.
 K7 every row says what its path is, and its last column is yes, no or not determined.
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
import tempfile

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


PREFIX = re.compile(rb"[ \t>]*")
RUN = re.compile(rb"(`{3,}|~{3,})(.*)")


def placed(line):
    """One line as (quotation marks before it, columns of indentation after the last of them, the
    rest). A tab counts as four columns."""
    prefix = PREFIX.match(line).group(0)
    after = prefix[prefix.rfind(b">") + 1:]
    return prefix.count(b">"), len(after.replace(b"\t", b"    ")), line[len(prefix):]


def outside_fences(data):
    """The text with fenced code taken out, read a line at a time (B3). A fence opens on a line
    that starts, after indentation or quotation marks, with three or more backticks or tildes; a
    backtick fence's opening line holds no further backtick. It closes on a line holding only a
    run of the same character at least as long as the opening one, or at the end of the file.
    It also ends, and the line is read as ordinary text, where the block that held it ends: a
    line with fewer quotation marks, or, for a fence indented four or more columns (which is
    inside a list item, or is an indented code block and no fence at all), a line that is not
    blank and is indented less than the opening one.
    This is not a Markdown parser. What is assured is the fence at the top level of a page.
    Inside a list item or a quotation the handling is best effort and errs both ways: it can
    read code as links, and it can miss a real link after the container ends. The known miss
    (B3): an unclosed fence indented under four columns inside a list item is taken for a
    top-level fence and runs to the end of the file. A control below records that it is missed.
    Full coverage of what a renderer links would need a check that uses a renderer."""
    kept, fence = [], None
    for line in data.split(b"\n"):
        quotes, indent, rest = placed(line)
        run = RUN.match(rest)
        if fence is not None:
            mark, opened_quotes, opened_indent = fence
            if quotes < opened_quotes or (opened_indent >= 4 and rest.strip() and indent < opened_indent):
                fence = None
            else:
                if run and run.group(1)[:1] == mark[:1] and len(run.group(1)) >= len(mark) and not run.group(2).strip():
                    fence = None
                continue
        if run and not (run.group(1)[:1] == b"`" and b"`" in run.group(2)):
            fence = (run.group(1), quotes, indent)
        else:
            kept.append(line)
    return b"\n".join(kept)


# A target is either in angle brackets, read through the closing bracket and so able to hold
# spaces, or bare, ending at the first white space (C3).
LINK = re.compile(rb"\]\((?:<([^<>\n]*)>|([^)\s<][^)\s]*))(?:\s+\"[^\"]*\")?\)")
# A reference definition, "[label]: target" on a line of its own, indented at most three spaces (B3).
DEFINITION = re.compile(rb"^ {0,3}\[[^\]\n]+\]:[ \t]*(?:<([^<>\n]*)>|([^\s<]\S*))", re.M)
ELSEWHERE = re.compile(r"[a-z][a-z0-9+.-]*:|//")


def links(name, data, tracked):
    """The links of one Markdown file, sorted: (broken, to a path in the repository, to another
    site, within the same page). Only the second kind is checked, against the tracked paths; a
    link to a place within another file is checked for the file only."""
    broken, paths, elsewhere, within = [], 0, 0, 0
    text = outside_fences(data)
    for found in [m.group(1) if m.group(1) is not None else m.group(2) for m in list(LINK.finditer(text)) + list(DEFINITION.finditer(text))]:
        link = found.decode("utf-8", "replace")
        if link.startswith("#"):
            within += 1
            continue
        if ELSEWHERE.match(link):
            elsewhere += 1
            continue
        target = link.split("#")[0].split("?")[0]
        if not target:
            within += 1
            continue
        paths += 1
        path = target.lstrip("/") if target.startswith("/") else posixpath.normpath(posixpath.join(posixpath.dirname(name), target))
        if path != "." and path not in tracked:
            broken.append("K6: %s links to %s, which is not tracked" % (name, link))
    return broken, paths, elsewhere, within


ROW = re.compile(r"\| `([^`]+)` \| (.*) \| (.*) \|")
RELEASE = ("yes", "no", "not determined")


def malformed(text):
    """Rows that do not say what the path is, or whose last column is not one of RELEASE (A3)."""
    found = []
    for line in text.splitlines():
        if not line.startswith("| `"):
            continue
        match = ROW.fullmatch(line)
        if not match or not match.group(2).strip() or match.group(3) not in RELEASE:
            found.append("K7: this row has no description, or its last column is not one of %s: %s" % (", ".join(RELEASE), line))
    return found


def problems(files, text, contents):
    rows, tree = listed(text), wanted(files)
    tracked = set(files) | {"/".join(f.split("/")[:n]) for f in files for n in range(1, f.count("/") + 1)}
    found = ["K1/K2: not in the catalogue: " + p for p in sorted(tree - set(rows))]
    found += ["K3: in the catalogue but not tracked: " + p for p in sorted(set(rows) - tracked)]
    found += ["K3: listed twice: " + p for p in sorted({p for p in rows if rows.count(p) > 1})]
    for name, data in contents:
        if data is not None and name.endswith(".md"):
            found += links(name, data, tracked)[0]
    found += malformed(text)
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
    require(problems(files, good.replace("| r | no |", "|  | no |"), [])[:1] != [] and problems(files, good.replace("| r | no |", "|  | no |"), [])[0].startswith("K7"), "a row with no description (A3)")
    require(problems(files, good.replace("| r | no |", "| r | maybe |"), [])[0].startswith("K7"), "a row with an unknown release value (A3)")
    require(links("a.md", b"[a](README.md) [b](https://x.org) [c](#top) [d](README.md#part)", {"README.md"}) == ([], 2, 1, 1), "links are counted by kind")
    reference = b"[Trial instructions][trial-guide]\n\n[trial-guide]: docs/obsolete-guide.md\n"
    require(problems(files, good, [("README.md", reference)]) == ["K6: README.md links to docs/obsolete-guide.md, which is not tracked"], "a reference definition to a path that is gone (B3)")
    require(problems(files, good, [("README.md", reference.replace(b"docs/obsolete-guide.md", b"<examples/a/x.py> \"title\""))]) == [], "a reference definition that holds, in angle brackets with a title")
    tilde = b"~~~md\n[trial]: docs/obsolete.md\n[x](gone.md)\n~~~\n"
    require(links("a.md", tilde, set()) == ([], 0, 0, 0), "nothing inside a tilde fence is a link (B3)")
    require(links("a.md", tilde + b"[trial]: docs/obsolete.md\n", set())[0] == ["K6: a.md links to docs/obsolete.md, which is not tracked"], "and what follows the closed tilde fence is read")
    require(links("a.md", b"[Read][t]\n\n[t]: <os-docs/B3 trial notes.md>\n", {"os-docs/B3 trial notes.md"}) == ([], 1, 0, 0), "B3's tracked name with spaces, in brackets, holds")
    longer = b"~~~~md\n~~~\n[trial]: docs/obsolete.md\n~~~~\n"
    require(links("a.md", longer, set()) == ([], 0, 0, 0), "a four-tilde fence is not closed by three tildes (B3)")
    require(links("a.md", longer + b"[real]: docs/obsolete.md\n", set())[0] == ["K6: a.md links to docs/obsolete.md, which is not tracked"], "and a definition right after it is read")
    require(links("a.md", b"text\n~~~\n[trial]: docs/obsolete.md\n[x](gone.md)\n", set()) == ([], 0, 0, 0), "an unclosed fence is code to the end of the file (B3)")
    require(links("a.md", b"```` md\n```\n[x](gone.md)\n````\n[y](gone.md)\n", set())[0] == ["K6: a.md links to gone.md, which is not tracked"], "the same for backticks: one link read, the one after the fence")
    require(links("a.md", b"```not a fence``` [x](gone.md)\n", set())[0] != [], "three backticks opening and closing on one line are not a fence")
    require(links("a.md", b"- item\n\n      ~~~\n      [x](gone.md)\n      ~~~\n", set()) == ([], 0, 0, 0), "a fence inside a list item, indented")
    after = b"\n[Read][t]\n\n[t]: docs/after.md\n"
    gone = ["K6: a.md links to docs/after.md, which is not tracked"]
    require(links("a.md", b"    ~~~\n" + after, set())[0] == gone, "an indented run of tildes is code, not a fence: the link after the dedent is read (B3)")
    require(links("a.md", b"> ~~~md\n> code\n" + after, set())[0] == gone, "a fence in a quotation ends with the quotation (B3)")
    require(links("a.md", b"- item\n\n      ~~~\n      [x](gone.md)\n\ntext [y](docs/after.md)\n", set())[0] == gone, "an unclosed fence in a list item ends with the item")
    require(links("a.md", b" ~~~\n[x](gone.md)\n\n[t]: gone.md\n", set()) == ([], 0, 0, 0), "a top-level fence indented under four columns still runs to the end of the file")
    # Recorded limit, not a property: this link is real and missing, and is NOT reported (B3).
    require(links("a.md", b"- Example\n  ~~~md\n  code\n" + after, set()) == ([], 0, 0, 0), "the known miss after a list item has changed: update the statement of what K6 assures")
    spaced = b"[guide]: <docs/Trial guide.md>\n[inline](<docs/Trial guide.md>)\n"
    require(links("a.md", spaced, {"docs/Trial guide.md"}) == ([], 2, 0, 0), "a bracketed target with a space, tracked, holds in both spellings (C3)")
    require(links("a.md", spaced, {"docs/Trial"})[0] == ["K6: a.md links to docs/Trial guide.md, which is not tracked"] * 2, "a bracketed target with a space is not cut at the space (C3)")
    require(links("a.md", b"[x]: docs/Trial guide.md\n", {"docs/Trial"}) == ([], 1, 0, 0), "a bare target still ends at white space")
    require(links("a.md", b"   [x]: README.md\n    [y]: gone.md\n[z]: https://x.org\n", {"README.md"}) == ([], 1, 1, 0), "a definition indented four spaces is code, not a definition")
    with tempfile.TemporaryDirectory() as folder:
        require(read(folder, "100644") is None, "a tracked file whose path is a folder is not read (C3)")
        require(read(os.path.join(folder, "absent"), "100644") is None, "a tracked file that is missing is not read")
        require(read(folder, "160000") == folder.encode(), "a submodule entry is scanned as its name")
    # A link is not followed: what it names is scanned, and a session address in the name is found.
    require(problems(files, good, [("README.md", address)]) != [], "a link whose target names a session")


def read(name, mode):
    """The bytes to scan for one tracked path, or None if they cannot be had. What the path should
    be comes from git's index, not from what is found on disk: a submodule entry (mode 160000) has
    no bytes here and is scanned as its name; a symbolic link (120000) is scanned as the path it
    names and never followed; anything else must be a regular file, so a tracked file that has
    been replaced by a folder, or is missing, is not read and fails the check (C3)."""
    try:
        if mode == "160000":
            return name.encode()
        if mode == "120000":
            return os.readlink(name).encode()
        if os.path.islink(name) or not os.path.isfile(name):
            return None
        with open(name, "rb") as handle:
            return handle.read()
    except OSError:
        return None


def main():
    selftest()
    staged = [entry.split("\t", 1) for entry in subprocess.check_output(["git", "ls-files", "-s", "-z"]).decode().split("\0")[:-1]]
    files = [name for _, name in staged]
    contents = [(name, read(name, meta.split()[0])) for meta, name in staged]
    text = open("CATALOG.md", encoding="utf-8").read()
    found = problems(files, text, contents)
    if found:
        sys.exit("\n".join(found))
    allowed = sum(1 for name, data in contents if name.startswith(ALLOWED) and CONTEXT.search(data))
    tracked = set(files) | {"/".join(f.split("/")[:n]) for f in files for n in range(1, f.count("/") + 1)}
    counts = [links(name, data, tracked)[1:] for name, data in contents if name.endswith(".md")]
    paths, elsewhere, within = (sum(c[i] for c in counts) for i in range(3))
    print("catalogue: %d tracked files, every one read, %d rows, all agree" % (len(files), len(listed(text))))
    print("links in Markdown: %d to paths in this repository, every one tracked; not checked: %d to other sites, %d within their own page" % (paths, elsewhere, within))
    print("session context: only in the %d files still allowed" % allowed)


if __name__ == "__main__":
    main()
