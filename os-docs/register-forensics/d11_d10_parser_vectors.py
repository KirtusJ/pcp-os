#!/usr/bin/env python3
"""D11: bounded independently specified D10/Rust ledger grammar controls.

All vectors are synthetic, single-session values. A D10 read_register result is
not live Record authentication. A passing curated corpus is NOT exhaustive
implementation equivalence, model/OS readiness, or a license to ship serial
record exports. The Rust side runs unmodified pinned pcpos-core::ledger::verify.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

PIN_BASE = "74c4d4a9934ad68e1b5be380971bf3377a464c6d"
PIN_D10 = "f1b7430a9c1aaabc5587f19b16e0c1e76646ef81"
ZERO = "0" * 64


def checked_ref(directory, sha):
    observed = subprocess.check_output(
        ["git", "-C", str(directory), "rev-parse", "HEAD"], text=True
    ).strip()
    if observed != sha:
        raise ValueError(f"source ref mismatch {directory}: {observed} != {sha}")
    return observed


def load_d10(path):
    spec = importlib.util.spec_from_file_location("d11_pinned_d10_reader", str(path.resolve()))
    if spec is None or spec.loader is None:
        raise ValueError("D10 Python source unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def event(body=None, **changes):
    obj = {
        "actor": "host", "auth": "HOST",
        "body": {"workspace": "acr:workspace:kernel", "label": "kernel"} if body is None else body,
        "kind": "WORKSPACE_CREATED", "prev": ZERO, "seq": 0,
        "ts": "2026-11-16T21:00:00Z",
    }
    obj.update(changes)
    return obj


def line(obj):
    return json.dumps(obj, sort_keys=True, ensure_ascii=True,
                      separators=(",", ":")).encode("ascii")


def chained_two():
    first = line(event())
    second = line(event(
        body={"outcome": "DONE"}, actor="kernel", kind="TASK_ENDED",
        seq=1, prev=hashlib.sha256(first).hexdigest()
    ))
    return first + b"\n" + second + b"\n"


def all_cases():
    raw = line(event()) + b"\n"
    sl = line(event(actor="/")) + b"\n"
    valid = [
        ("ascii_single", raw, True, True),
        ("two_canonical_events", chained_two(), True, True),
        ("unicode_surrogate_pair", line(event(actor="\U0001f426")) + b"\n", True, True),
        ("escaped_del", line(event(actor="\x7f")) + b"\n", True, True),
        ("int64_min", line(event(body={"num": -(1 << 63)})) + b"\n", True, True),
        ("int64_max", line(event(body={"num": (1 << 63)-1})) + b"\n", True, True),
        ("declared_auth", line(event(auth="DECLARED")) + b"\n", True, True),
        # No kernel workspace constraint exists in Rust ledger::verify or D10
        # read_register. This is only a D11 RAM candidate LOCATOR constraint.
        ("other_workspace_valid_ledger", line(event(body={
            "workspace": "acr:workspace:other", "label": "kernel"
        })) + b"\n", True, True),
        # Rust ledger::verify accepts the empty chain and returns ZERO;
        # D10's diagnostic reader purposefully requires a nonempty line.
        ("empty_chain_scope_difference", b"", False, True),
    ]
    nested = 0
    for _ in range(34):
        nested = [nested]
    invalid = [
        ("unpaired_high_surrogate", line(event(actor="\ud800")) + b"\n", False, False),
        ("unpaired_low_surrogate", line(event(actor="\udc00")) + b"\n", False, False),
        ("uint_overflow", line(event(body={"num": 1 << 63})) + b"\n", False, False),
        ("nested_too_deep", line(event(body={"deep": nested})) + b"\n", False, False),
        ("duplicate_actor", raw.replace(b'"actor":"host",', b'"actor":"host","actor":"host",', 1), False, False),
        ("nonnormalized_slash", sl.replace(b'"actor":"/"', b'"actor":"\\u002f"', 1), False, False),
        ("float", raw.replace(b'"seq":0', b'"seq":0.0', 1), False, False),
        ("wrong_predecessor", line(event(prev="a"*64)) + b"\n", False, False),
        ("wrong_sequence", line(event(seq=1)) + b"\n", False, False),
        ("bool_sequence", line(event(seq=True)) + b"\n", False, False),
        ("bad_timestamp_shape", line(event(ts="2026-11-16T00:00:0Z")) + b"\n", False, False),
        ("wrong_authority", line(event(auth="NOT_HOST")) + b"\n", False, False),
        ("body_is_not_map", line(event(body=[])) + b"\n", False, False),
        ("unexpected_event_field", line(event(extra=True)) + b"\n", False, False),
        ("missing_final_newline", raw[:-1], False, False),
        ("extra_final_newline", raw + b"\n", False, False),
    ]
    return valid + invalid


def run(source, baseline, root):
    observed = {"base": checked_ref(baseline, PIN_BASE),
                "diagnostic": checked_ref(source, PIN_D10)}
    d10 = load_d10(source/"os-docs/display-acceptance/register/d10_diagnostic_native.py")
    vectors = root/"vectors"
    vectors.mkdir(parents=True, exist_ok=False)
    rows = []
    entry_lines = []
    for name, raw, expected_py, expected_rust in all_cases():
        success = True
        actual_head = None
        n = None
        try:
            n, actual_head = d10.read_register(raw)
        except (ValueError, TypeError, UnicodeError, json.JSONDecodeError):
            success = False
        if success != expected_py:
            raise AssertionError(f"{name}: D10 acceptance was {success}; expected {expected_py}")
        if success and not (n >= 1 and len(actual_head) == 64):
            raise AssertionError(f"{name}: D10 returned inconsistent head/count")
        # This emitted Rust head is sourced from independently hashed final
        # canonical event bytes, not from a native screen or a D10 serializer.
        rust_head = (hashlib.sha256(raw[:-1].split(b"\n")[-1]).hexdigest()
                     if expected_rust and raw else ZERO if expected_rust else "-")
        if success and actual_head != rust_head:
            raise AssertionError(f"{name}: D10 digest differs from independent last-line hash")
        (vectors / (name+".bin")).write_bytes(raw)
        entry_lines.append(f"{name}\t{int(expected_rust)}\t{rust_head}\n")
        rows.append({
            "case": name, "D10_accept": success, "Rust_expected_accept": expected_rust,
            "deliberate_scope_difference": success != expected_rust,
            "raw_size_bytes": len(raw),
        })
    (vectors/"index.tsv").write_text("".join(entry_lines))
    summary = {
        "source_commits": observed,
        "source_scope": "D10 Python reader vs separately executed pinned Rust pcpos-core::ledger::verify",
        "synthetic_only": True, "real_record_authenticated": False,
        "rust_executed": False,
        "exhaustive_equivalence": False,
        "case_count": len(rows),
        "cases": rows
    }
    (root/"summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True)+"\n")
    print(json.dumps({
        "case_count": len(rows),
        "D10_python_assertions_passed": len(rows),
        "deliberate_empty_chain_scope_difference": True,
        "rust_verified_separately": "PENDING"
    }))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--diagnostic", type=Path, required=True)
    ap.add_argument("--baseline", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    run(args.diagnostic, args.baseline, args.output)
