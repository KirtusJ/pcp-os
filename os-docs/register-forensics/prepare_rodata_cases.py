#!/usr/bin/env python3
"""One-factor source line/attribute controls, never modifying pinned checkouts."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from prepare_identity_variants import prep, R_ARM

CASES=("base_1","base_2","blank_before_arm","comment_before_arm","cfg_inline_arm","cfg_original_arm")
CFG='            #[cfg(not(feature = "register-view-diagnostic"))]'
COMMENT='            // Test-only empty source-position control'
INLINE='            #[cfg(not(feature = "register-view-diagnostic"))] (0x13, false) => register_view(rec),'


def prepare(base,d10,work,name):
    if name not in CASES:
        raise ValueError("unknown controlled case")
    inherited="cfg_original_arm" if name=="cfg_original_arm" else "cargo_only"
    parent=prep(base,d10,work,inherited)
    f=work/"PCP_OS_Rust/crates/pcpos-kernel/src/session.rs"
    s=f.read_text()
    if name not in ("base_1","base_2","cfg_original_arm"):
        if s.count(R_ARM)!=1:
            raise ValueError("original source arm missing or repeated")
        arm={
          "blank_before_arm":"\n"+R_ARM,
          "comment_before_arm":COMMENT+"\n"+R_ARM,
          "cfg_inline_arm":INLINE
        }[name]
        s=s.replace(R_ARM,arm,1)
        f.write_text(s)
    return {
        "case":name,
        "inherited_manifest_is_D10_feature_declaration":True,
        "base_hash":parent["session_source_sha256"],
        "actual_session_source_sha256":hashlib.sha256(f.read_bytes()).hexdigest(),
        "number_of_source_line_insertions": 1 if name in ("blank_before_arm","comment_before_arm","cfg_original_arm") else 0,
        "normal_feature_set":True
    }


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--baseline",type=Path,required=True)
    ap.add_argument("--diagnostic",type=Path,required=True)
    ap.add_argument("--target",type=Path,required=True)
    ap.add_argument("--case",choices=CASES,required=True)
    args=ap.parse_args()
    print(json.dumps(prepare(args.baseline,args.diagnostic,args.target,args.case),sort_keys=True))


if __name__=="__main__":
    main()
