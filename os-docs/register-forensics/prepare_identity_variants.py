#!/usr/bin/env python3
"""Make exact, isolated no-feature source interventions for D10 #240 diagnosis.

All images are compiled later at the same pathname and staging location.
This tool never edits the pinned checkouts, any default Git branch, or ISO.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import shutil

BASE="74c4d4a9934ad68e1b5be380971bf3377a464c6d"
D10="9fa9819103a67b0f2b124991a2d0d88b4c64b755"
CASES=("base_1","base_2","cargo_only","cfg_original_arm","cfg_double_arm","cfg_double_arm_inert_fn","d10_default")

R_ARM="            (0x13, false) => register_view(rec),"
R_CFG='            #[cfg(not(feature = "register-view-diagnostic"))]\n'+R_ARM
R_TWO='            #[cfg(feature = "register-view-diagnostic")]\n'+R_ARM+'\n'+R_CFG
INERT='\n#[cfg(feature = "register-view-diagnostic")]\nfn d11_unreachable_in_default_build() { crate::log(format_args!("DIAGNOSTIC-ONLY")); }\n'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prep(baseline, d10, target, name):
    if name not in CASES:
        raise ValueError(f"unexpected case {name}")
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    # run.sh itself copies crates+vectors to a fixed build stage.
    shutil.copytree(baseline/"PCP_OS_Rust",target/"PCP_OS_Rust",
        ignore=shutil.ignore_patterns("target","__pycache__",".git"))
    rel="PCP_OS_Rust/crates/pcpos-kernel"
    session=target/rel/"src/session.rs"
    manifest=target/rel/"Cargo.toml"
    original=session.read_text()
    if original.count(R_ARM)!=1:
        raise ValueError("pinned #233 original R arm no longer unique")
    if name not in ("base_1","base_2"):
        manifest.write_bytes((d10/rel/"Cargo.toml").read_bytes())
    if name in ("cfg_original_arm","cfg_double_arm","cfg_double_arm_inert_fn"):
        arm=R_CFG if name=="cfg_original_arm" else R_TWO
        changed=original.replace(R_ARM,arm,1)
        if name=="cfg_double_arm_inert_fn":
            changed+=INERT
        session.write_text(changed)
    if name=="d10_default":
        session.write_bytes((d10/rel/"src/session.rs").read_bytes())
    return {
        "case":name,
        "baseline_commit":BASE,
        "diagnostic_commit":D10,
        "source_intent":"no-feature build for every case",
        "session_source_sha256":digest(session),
        "crate_manifest_sha256":digest(manifest),
        "expected_control":"A/A same bytes" if name.startswith("base_") else "controlled default-source perturbation"
    }


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--baseline",type=Path,required=True)
    ap.add_argument("--diagnostic",type=Path,required=True)
    ap.add_argument("--target",type=Path,required=True)
    ap.add_argument("--case",choices=CASES,required=True)
    args=ap.parse_args()
    import json
    print(json.dumps(prep(args.baseline,args.diagnostic,args.target,args.case),sort_keys=True))


if __name__=="__main__":
    main()
