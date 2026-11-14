#!/usr/bin/env python3
"""Publisher-side vocabulary gate. The plaintext list of excluded terms lives only here, outside both
packets. `python3 vocabulary_gate.py scan <dir> [allow-file ...]` reports every occurrence in the text
files of a tree; `python3 vocabulary_gate.py digests` prints the SHA-256 set that the Implementation
verifier embeds, so the packet can detect reintroduction without naming any term."""
import hashlib, re, sys
from pathlib import Path

# Single words (lower-case) and two-word phrases. Inflections are listed explicitly; nothing is stemmed.
WORDS = """parrot parrots council councils scribe scribes sentinel sentinels spectator spectators roost roosts
perch perches perched perching gallery galleries autopilot wildcard wildcards motion motions referee referees
advocate advocates advocated advocating proposer proposers grounder grounders chaos chair chairs debate debates
debated debating verdict verdicts theatrical theatre theater squawk squawks chirp chirps feather feathers beak
beaks polly bird birds pcp""".split()
PHRASES = ["storm drill", "storm drills"]
TEXT_SUFFIXES = (".md", ".json", ".csv", ".py", ".txt", ".html")
WORD_RE = re.compile(r"[a-z]+")


def tokens(text):
    ws = WORD_RE.findall(text.lower())
    return ws, [a + " " + b for a, b in zip(ws, ws[1:])]


def digests():
    return sorted(hashlib.sha256(t.encode()).hexdigest() for t in WORDS + PHRASES)


def scan(root, allow=()):
    root = Path(root); hits = []
    for p in sorted(root.rglob("*")):
        if not p.is_file() or p.suffix not in TEXT_SUFFIXES or "__pycache__" in p.parts:
            continue
        rel = p.relative_to(root).as_posix()
        if rel in allow:
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        for n, line in enumerate(text.splitlines(), 1):
            ws, bs = tokens(line)
            found = sorted(set(w for w in ws if w in WORDS) | set(b for b in bs if b in PHRASES))
            if found:
                hits.append((rel, n, found))
    return hits


if __name__ == "__main__":
    if sys.argv[1] == "digests":
        print("\n".join(digests()))
    else:
        hits = scan(sys.argv[2], sys.argv[3:])
        for rel, n, found in hits:
            print("%s:%d: %s" % (rel, n, ", ".join(found)))
        print("%d occurrences" % len(hits)); sys.exit(1 if hits else 0)
