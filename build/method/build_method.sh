#!/bin/bash
# The method build: the Implementation packet, the reading copies and the three base editions.
# The successor cryptography edit and its evidence replay are documented in crypto_predicate_evidence.md.
# The scripts are as they stand after the fifteenth round (in the source archive of build 21 they were, by an oversight,
# as they stood before the fourteenth); they work in the folder
# /home/claude/build, as they always have. Needs WeasyPrint 70.0, markdown-it-py, pymupdf, and the fonts Inter
# and DejaVu Sans Mono.
#   build_method.sh <the preceding Implementation packet, extracted> <its archive>
# 1. A fresh copy of the preceding packet is edited:   python3 edit_sources_round12.py pkg .
#                                                      python3 edit_editions_round12.py .
#    For the thirteenth round:                         python3 edit_sources_round13.py pkg .
#    For the fourteenth and fifteenth:                 python3 edit_sources_round14.py pkg . ; python3 edit_sources_round15.py pkg .
#    Each round's script applies to the packet and to the build scripts as the round before left them. The fifteenth
#    edits no build script; the fourteenth's edits to assemble.py, edition_texts.py and assemble_editions.py are
#    already in the copies here.
#    That script also edits assemble.py, edition_texts.py and assemble_editions.py, so it applies to those scripts as the
#    twelfth round left them (the source archive of build 18), not to the ones here, which already carry its edits.
# 2. Derived files and pins:                           python3 assemble.py a && python3 assemble.py b
# 3. Checks:                                           python3 pkg/TOOLS/verify_release.py   (also with -O)
#                                                      the three tool test files; vocabulary_gate.py scan pkg; mutation_tests.py
# 4. Reading copies and base editions:                 python3 assemble_editions.py
# With no edits applied, steps 2 and 4 reproduce the build of 11 November 2026 byte for byte
# (Personal Edition 5bb9492c...), which is how this pipeline was checked before it was used.
set -euo pipefail
B=/home/claude/build; mkdir -p $B/pred $B/out; cp "$(dirname "$0")"/*.py "$(dirname "$0")"/themes.json $B/
rm -rf $B/pkg $B/orig; cp -r "$1" $B/pkg; cp -r "$1" $B/orig; cp "$2" $B/pred/ACR_1.0_Implementation.zip
echo "apply the round's edits to the copies of assemble.py, verify_release.template.py and edition_texts.py from the preceding round, then run steps 2 to 4 in $B"
