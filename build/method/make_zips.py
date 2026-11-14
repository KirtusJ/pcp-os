#!/usr/bin/env python3
"""Deterministic archives: sorted entries, fixed timestamp, no extra attributes."""
import hashlib, sys, zipfile
from pathlib import Path
def build(src, name, out):
    src = Path(src); out = Path(out)
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for p in sorted(q for q in src.rglob("*") if q.is_file() and "__pycache__" not in q.parts):
            zi = zipfile.ZipInfo(name + "/" + p.relative_to(src).as_posix(), date_time=(2026, 11, 11, 12, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED; zi.external_attr = 0o644 << 16
            z.writestr(zi, p.read_bytes())
    return hashlib.sha256(out.read_bytes()).hexdigest()
if __name__ == "__main__":
    print(build(sys.argv[1], sys.argv[2], sys.argv[3]))
