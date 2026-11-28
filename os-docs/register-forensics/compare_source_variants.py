#!/usr/bin/env python3
"""Report which no-feature source intervention first changes measured image bytes.

This is a diagnostic matrix, not a conformance verdict or a compiler proof.
"""
from __future__ import annotations
import json
from pathlib import Path
import re
import sys
from prepare_identity_variants import CASES

FIELDS=("elf_sha256","measurement","text_sha256","rodata_sha256","text_size","rodata_size")
HEX=re.compile(r"[0-9a-f]{64}\Z")


def collect(folder: Path):
    results={}
    for case in CASES:
        v={}
        for field in FIELDS:
            path=folder/(case+"."+field)
            if not path.exists():
                raise ValueError(f"missing source observation: {path}")
            raw=path.read_text().strip()
            if field.endswith("size"):
                n=int(raw)
                if n<=0:
                    raise ValueError("empty measured section")
                v[field]=n
            else:
                if not HEX.fullmatch(raw):
                    raise ValueError("invalid SHA256 for "+case+"/"+field)
                v[field]=raw
        results[case]=v
    return results


def judge(results):
    base=results["base_1"]
    reproducible=results["base_1"]==results["base_2"]
    cases={}
    for name in CASES:
        value=results[name]
        flags={k:value[k]==base[k] for k in FIELDS}
        cases[name]={"observed":value,"equal_to_base":flags}
    first_measured_change=next((name for name in CASES[2:] if results[name]["measurement"]!=base["measurement"]),None)
    first_elf_change=next((name for name in CASES[2:] if results[name]["elf_sha256"]!=base["elf_sha256"]),None)
    return {
      "kind":"NO_FEATURE_SOURCE_INTERVENTION_MATRIX",
      "base":"74c4d4a9934ad68e1b5be380971bf3377a464c6d",
      "diagnostic":"9fa9819103a67b0f2b124991a2d0d88b4c64b755",
      "same_rust_compiler_stage_and_working_path":True,
      "cases":cases,
      "a_a_source_build_reproducible":reproducible,
      "first_candidate_variant_with_distinct_measured_code":first_measured_change,
      "first_candidate_variant_with_distinct_ELF":first_elf_change,
      "disposition": "A_A_CONTROL_PASSED_DIFFERENCES_MEASURED" if reproducible else "INCOMPARABLE_UNSTABLE_A_A",
      "source_text_mutation_proves_semantic_regression":False,
      "host_enforced_runtime_or_shipping_ISO_tested":False
    }


def main():
    folder=Path(sys.argv[1])
    result=judge(collect(folder))
    (folder/"source-factors-result.json").write_text(json.dumps(result,sort_keys=True,indent=2)+"\n")
    print(json.dumps(result,sort_keys=True))
    return 0 if result["a_a_source_build_reproducible"] else 1


if __name__=="__main__":
    raise SystemExit(main())
