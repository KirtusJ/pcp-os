#!/usr/bin/env python3
"""Read-only comparison of section bytes for source line/cfg experiments."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
import sys
from prepare_rodata_cases import CASES


def sha(b):
    return hashlib.sha256(b).hexdigest()


def changed_regions(a,b,maximum=16):
    if len(a)!=len(b):
        return [],None
    count=0
    runs=[]
    active=None
    for i,(x,y) in enumerate(zip(a,b)):
        if x!=y:
            count+=1
            if active is None:
                active=i
        elif active is not None:
            if len(runs)<maximum:
                runs.append({"begin":active,"end_exclusive":i,"length":i-active,
                             "old_first8_hex":a[active:min(i,active+8)].hex(),
                             "new_first8_hex":b[active:min(i,active+8)].hex()})
            active=None
    if active is not None and len(runs)<maximum:
        i=len(a)
        runs.append({"begin":active,"end_exclusive":i,"length":i-active,
                     "old_first8_hex":a[active:min(i,active+8)].hex(),
                     "new_first8_hex":b[active:min(i,active+8)].hex()})
    return runs,count


def summarize(folder):
    blobs={name:(folder/(name+".rodata.bin")).read_bytes() for name in CASES}
    text={name:(folder/(name+".text_sha256")).read_text().strip() for name in CASES}
    elf={name:(folder/(name+".elf_sha256")).read_text().strip() for name in CASES}
    measured={name:(folder/(name+".measurement")).read_text().strip() for name in CASES}
    if len(set(map(len,blobs.values())))!=1:
        raise ValueError("rodata sizes differ: byte offset comparisons invalid")
    first=blobs["base_1"]
    controls={
      "A_A_base_rodata_same":blobs["base_1"]==blobs["base_2"],
      "A_A_base_text_same":text["base_1"]==text["base_2"],
      "A_A_base_elf_same":elf["base_1"]==elf["base_2"],
      "A_A_base_measured_same":measured["base_1"]==measured["base_2"],
    }
    cases={}
    for name in CASES:
        regions,count=changed_regions(first,blobs[name])
        cases[name]={
          "rodata_sha256":sha(blobs[name]),
          "rodata_bytes":len(blobs[name]),
          "equal_to_base_rodata":blobs[name]==first,
          "text_sha256":text[name],
          "equal_to_base_text":text[name]==text["base_1"],
          "measured_sha256":measured[name],
          "elf_sha256":elf[name],
          "different_rodata_byte_count":count,
          "first_contiguous_differences":regions
        }
    return {
       "scope":"same path, same compiler, each case uses D10 Cargo feature declaration but does NOT enable it",
       "controls":controls,
       "cases":cases,
       "source_cfg_attribute_line_position_hypothesis_only":True,
       "blank_line_equals_cfg_rodata":blobs["blank_before_arm"]==blobs["cfg_original_arm"],
       "comment_line_equals_cfg_rodata":blobs["comment_before_arm"]==blobs["cfg_original_arm"],
       "same_line_cfg_equals_baseline_rodata":blobs["cfg_inline_arm"]==first,
       "first_record_line_continuity_not_proven":True,
       "conformance_or_release_safety_proven":False
    }


def main():
    folder=Path(sys.argv[1])
    report=summarize(folder)
    (folder/"rodata-differential.json").write_text(json.dumps(report,sort_keys=True,indent=2)+"\n")
    print(json.dumps({k:v for k,v in report.items() if k!="cases"}|{"case_summary":{k:{
      "rodata_sha256":v["rodata_sha256"],
      "equal_to_base_rodata":v["equal_to_base_rodata"],
      "equal_to_base_text":v["equal_to_base_text"],
      "different_rodata_byte_count":v["different_rodata_byte_count"],
      "first_diff_offsets":[x["begin"] for x in v["first_contiguous_differences"]]
    } for k,v in report["cases"].items()}},sort_keys=True))
    return 0 if all(report["controls"].values()) else 1


if __name__=="__main__":
    raise SystemExit(main())
