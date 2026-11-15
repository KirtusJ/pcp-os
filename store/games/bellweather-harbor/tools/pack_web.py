#!/usr/bin/env python3
"""Embed the release WASM and harbor interface assets in one offline HTML file."""

import argparse
import base64
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("wasm", type=Path)
parser.add_argument("template", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()

html = args.template.read_text(encoding="utf-8")
wasm = args.wasm.read_bytes()
if not wasm.startswith(b"\0asm\1\0\0\0"):
    parser.error(f"{args.wasm} is not a WebAssembly 1.0 binary")

replacements = {"__BELLWEATHER_WASM_BASE64__": base64.b64encode(wasm).decode("ascii")}
for marker, filename in [
    ("/*__BELLWEATHER_MARKS__*/", "marks.js"),
    ("/*__BELLWEATHER_QUAY__*/", "quay.js"),
    ("/*__BELLWEATHER_TEA__*/", "tea.js"),
    ("/*__BELLWEATHER_TEA_CSS__*/", "tea.css"),
    ("/*__BELLWEATHER_QUAY_CSS__*/", "quay.css"),
    ("/*__BELLWEATHER_QUAY_SOUND__*/", "quay-sound.js"),
    ("/*__BELLWEATHER_FREIGHT_ART__*/", "freight-art.js"),
    ("/*__BELLWEATHER_EXCHANGE__*/", "exchange.js"),
    ("/*__BELLWEATHER_EXCHANGE_CSS__*/", "exchange.css"),
    ("/*__BELLWEATHER_YARD__*/", "yard.js"),
    ("/*__BELLWEATHER_YARD_CSS__*/", "yard.css"),
    ("/*__BELLWEATHER_WORKS__*/", "works.js"),
    ("/*__BELLWEATHER_WORKS_CSS__*/", "works.css"),
    ("/*__BELLWEATHER_WORKS_ART__*/", "works-art.js"),
    ("/*__BELLWEATHER_TICKET__*/", "ticket.js"),
    ("/*__BELLWEATHER_TICKET_CSS__*/", "ticket.css"),
    ("/*__BELLWEATHER_MANIFEST__*/", "manifest.js"),
    ("/*__BELLWEATHER_MANIFEST_CSS__*/", "manifest.css"),
    ("/*__BELLWEATHER_MANIFEST_ART__*/", "manifest-art.js"),
]:
    replacements[marker] = args.template.with_name(filename).read_text(encoding="utf-8")

for marker, content in replacements.items():
    if html.count(marker) != 1:
        parser.error(f"expected one occurrence of {marker!r} in {args.template}")
    html = html.replace(marker, content)

args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(html, encoding="utf-8")
print(f"Built {args.output} ({args.output.stat().st_size:,} bytes)")
