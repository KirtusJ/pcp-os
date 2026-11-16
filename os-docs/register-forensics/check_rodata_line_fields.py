#!/usr/bin/env python3
"""Bounded source-location hypothesis check of no-feature .rodata diffs.

Reads only the two controlled extracted .rodata sections from the same CI
runner. Reports exact differing byte offsets and whether every changed
4-byte word moves +1 when a single source line is inserted. It cannot, by
itself, prove Rust Location type/layout or semantic equivalence.
"""
from pathlib import Path
import json
import sys

CASES=("base_1","base_2","blank_before_arm","comment_before_arm","cfg_original_arm","cfg_inline_arm")


def inspect(d):
    raw={k:(d/(k+".rodata.bin")).read_bytes() for k in CASES}
    base=raw["base_1"]
    if not base or len(set(map(len,raw.values())))!=1:
        raise ValueError("empty or nonidentically sized .rodata")
    same_meta={
      "AA_binary_sections_identical":raw["base_1"]==raw["base_2"],
      "blank_equals_comment":raw["blank_before_arm"]==raw["comment_before_arm"],
      "blank_equals_extra_cfg_line":raw["blank_before_arm"]==raw["cfg_original_arm"],
      "same_line_cfg_equals_unmodified":raw["cfg_inline_arm"]==base,
    }
    modified=raw["blank_before_arm"]
    offsets=[i for i,(a,b) in enumerate(zip(base,modified)) if a!=b]
    rows=[]
    for at in offsets:
        if at+4>len(base):
            raise ValueError("out-of-range changed offset")
        x=int.from_bytes(base[at:at+4],"little")
        y=int.from_bytes(modified[at:at+4],"little")
        rows.append({
           "offset":at,
           "old_byte":base[at], "new_byte":modified[at],
           "alignment_mod_8":at%8,
           "old_u32_le":x,"new_u32_le":y,
           "u32_plus_one":y==x+1,
           "unchanged_neighbors_3bytes":base[at+1:at+4]==modified[at+1:at+4],
        })
    all_one=bool(rows) and all(r["u32_plus_one"] and r["alignment_mod_8"]==0
                               and r["unchanged_neighbors_3bytes"] for r in rows)
    return {
        "purpose":"discriminate source line-number encoding from feature-enabled code",
        "case_equality_controls":same_meta,
        "length_bytes":len(base),
        "differing_byte_count":len(offsets),
        "first_64_differing_word_summaries":rows[:64],
        "every_changed_8_aligned_u32_is_plus_one":all_one,
        "inference":("consistent with 4-byte little-endian line-number fields shifted by one inserted source line"
                     if all_one else "cannot attribute differences to identical +1 u32 changes"),
        "type_identity_proven":False,
        "executable_text_or_product_semantics_tested_here":False,
        "external_trust_anchor_established":False
    }


def main():
    data=Path(sys.argv[1])
    o=inspect(data)
    (data/"rodata-line-field-readback.json").write_text(json.dumps(o,indent=2,sort_keys=True)+"\n")
    print(json.dumps(o,sort_keys=True))
    if not all(o["case_equality_controls"].values()):
        return 1
    # A failure to show +1 is a negative result, not a test crash. Report 0
    # after publishing it so the hypothesis itself can be falsified.
    return 0


if __name__=="__main__":
    raise SystemExit(main())
