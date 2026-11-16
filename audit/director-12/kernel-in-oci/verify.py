#!/usr/bin/env python3
"""Transient QEMU guest inside unprivileged Docker: evidence, not a resident PCP kernel."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

HEAD = re.compile(r"kernel register:\s+(\d+) events, head ([0-9a-f]{64})")
HEX64 = re.compile(r"^[0-9a-f]{64}$")

def shell(args: list[str], timeout: float = 45.0, check: bool = True) -> subprocess.CompletedProcess:
    done = subprocess.run(args, text=True, capture_output=True, timeout=timeout)
    if check and done.returncode != 0:
        raise RuntimeError(f"{args[0]} exited {done.returncode}: {done.stderr[-1200:]} {done.stdout[-500:]}")
    return done

def sha256(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()

def run_guest(image: str, kernel: Path, pin: str | None, cutoff: int, probe: Path | None = None) -> tuple[subprocess.CompletedProcess, float]:
    args = [
        "docker", "run", "--rm",
        "--network", "none", "--read-only",
        "--pids-limit", "64", "--memory", "1g", "--cpus", "2",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
        "--user", "65534:65534", "--init",
        "--tmpfs", "/tmp:rw,nosuid,nodev,size=33554432",
        "--mount", f"type=bind,source={kernel},target=/kernel.elf32,readonly",
        image,
        "-accel", "tcg",
        "-cpu", "max",
        "-m", "128M",
        "-display", "none", "-serial", "stdio", "-monitor", "none",
        "-no-reboot",
        "-kernel", "/kernel.elf32",
        "-append", "then=off" + (f" image={pin}" if pin is not None else "") + (" auto=probe.elf grant=none" if probe is not None else ""),
        "-net", "none",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]
    if probe is not None:
        # The loader obtains an exact external ELF, not a built-in program.
        args[args.index(image):args.index(image)] = [
            "--mount", f"type=bind,source={probe},target=/probe.elf,readonly"
        ]
        args.extend(["-initrd", "/probe.elf"])
    start = time.monotonic()
    result = shell(args, timeout=cutoff, check=False)
    return result, time.monotonic() - start

def record_arm(name: str, result: subprocess.CompletedProcess, elapsed: float, directory: Path) -> dict:
    serial = result.stdout.encode("utf-8")
    diagnostic = result.stderr.encode("utf-8")
    (directory / f"{name}.serial").write_bytes(serial)
    (directory / f"{name}.stderr").write_bytes(diagnostic)
    head = HEAD.findall(result.stdout)
    return {
        "exit_code": result.returncode,
        "serial_bytes": len(serial),
        "serial_sha256": sha256(serial),
        "stderr_bytes": len(diagnostic),
        "stderr_sha256": sha256(diagnostic),
        "elapsed_seconds": round(elapsed, 3),
        "registered_event_counts_and_heads": [
            {"event_count": int(n), "head": h} for n, h in head
        ],
        "self_tests_pass_banner": "ALL SELF-TESTS PASS." in result.stdout,
        "self_tests_failed_banner": bool(re.search(r"\d+ SELF-TEST\(S\) FAILED", result.stdout)),
        "image_was_compared_successfully": "measured; equal to the value the loader named" in result.stdout,
        "image_was_rejected": "measured; NOT the value the loader named" in result.stdout,
        "image_was_unchecked": "loader named no value to compare it with" in result.stdout,
        "built_in_programs_not_run": "the processes built into it are not run" in result.stdout,
        "external_probe_zero_failures": "probe: 0 failure(s)" in result.stdout,
    }

def validate_positive(record: dict, serial: str, expected_measurement: str) -> None:
    assert record["exit_code"] == 33, f"expected ISA exit 33, got {record['exit_code']}"
    assert "PCP OS kernel" in serial, "native kernel banner absent"
    assert record["self_tests_pass_banner"] and not record["self_tests_failed_banner"]
    assert record["image_was_compared_successfully"] and not record["image_was_rejected"]
    assert re.search(
        rf"running image .*SHA-256: {re.escape(expected_measurement)} \(measured; equal", serial
    ), "kernel measured image differs from the independent file measurement"
    assert len(record["registered_event_counts_and_heads"]) == 1, "missing/unexpected register head"
    assert record["registered_event_counts_and_heads"][0]["event_count"] > 1

def validate_bad_pin(record: dict, serial: str, bad: str) -> None:
    assert record["exit_code"] != 33, "invalid externally named image was accepted"
    assert "PCP OS kernel" in serial, "cannot attribute failure to kernel"
    assert f"the loader named, which is {bad}" in serial, "kernel did not report wrong pin"
    assert record["image_was_rejected"] and record["self_tests_failed_banner"]
    assert record["built_in_programs_not_run"], "invalid image appears to run its programs"
    assert not record["self_tests_pass_banner"], "failed image claimed all tests passed"
    assert len(record["registered_event_counts_and_heads"]) == 1, "invalid image register not observed"

def validate_unpinned(record: dict, serial: str) -> None:
    assert record["exit_code"] == 33, "unlocked image control did not boot"
    assert record["image_was_unchecked"] and not record["image_was_compared_successfully"]
    assert record["self_tests_pass_banner"], "unlocked image did not finish its independent self tests"
    assert len(record["registered_event_counts_and_heads"]) == 1

def validate_probe(record: dict, serial: str, expected_measurement: str) -> None:
    validate_positive(record, serial, expected_measurement)
    assert "probe: run with no grant but the console" in serial, (
        "the externally loaded ELF did not execute in no-grant mode"
    )
    assert "probe: 0 failure(s)" in serial, (
        "the externally loaded ELF did not complete all of its own checks"
    )
    assert record["external_probe_zero_failures"]


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--image", required=True)
    p.add_argument("--kernel", required=True, type=Path)
    p.add_argument("--measurement", required=True)
    p.add_argument("--source-head", required=True)
    p.add_argument("--probe", type=Path, help="optional externally loaded kernel guest ELF")
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--guest-timeout", type=int, default=160)
    args = p.parse_args()
    kernel = args.kernel.resolve(strict=True)
    probe = args.probe.resolve(strict=True) if args.probe else None
    if not HEX64.fullmatch(args.measurement):
        p.error("measurement must be 64 lowercase hexadecimal characters")
    if not args.source_head.strip():
        p.error("source head required")
    if args.guest_timeout < 20 or args.guest_timeout > 300:
        p.error("guest-timeout must be 20..300 seconds")

    # This compares actual guest boots under explicit launch conditions.
    # Docker's host kernel is Linux, not PCP OS.
    report = {
        "kind": "pcp-os-in-qemu-in-oci/test-only",
        "producer": "Director 12 diagnostic",
        "source_checkout_head": args.source_head,
        "expected_kernel_measurement": args.measurement,
        "kernel_file_sha256": sha256(kernel.read_bytes()),
        "kernel_file_bytes": kernel.stat().st_size,
        "guest_probe_sha256": sha256(probe.read_bytes()) if probe else None,
        "external_guest_probe_verified": False,
        "guest_is_actually_booted_pcp_kernel": False,
        "host_is_linux_docker_not_pcp_kernel": True,
        "runtime_is_ephemeral": True,
        "host_enforced_acr_claim": False,
        "qwen_inference_observed": False,
        "pcp_iso_built_or_released": False,
        "continuous_post_job_operation_established": False,
        "status": "INCOMPLETE",
        "arms": {},
    }
    args.out.mkdir(parents=True, exist_ok=True)
    summary_path = args.out / "result.json"
    try:
        report["docker_server"] = shell(["docker", "version", "--format", "{{.Server.Version}}"]).stdout.strip()
        report["image_id"] = shell(["docker", "image", "inspect", args.image, "--format", "{{.Id}}"]).stdout.strip()
        report["qemu_version"] = shell([
            "docker", "run", "--rm", "--network", "none", "--read-only",
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
            args.image, "--version",
        ]).stdout.splitlines()[0]
        for arm, pin in [
            ("valid_pin", args.measurement),
            ("wrong_pin", "0" * 64),
            ("no_pin", None),
            *((("probe_no_grant", args.measurement),) if probe is not None else ()),
        ]:
            assert pin != args.measurement or arm != "wrong_pin"
            q, elapsed = run_guest(
                args.image, kernel, pin, args.guest_timeout,
                probe if arm == "probe_no_grant" else None,
            )
            evidence = record_arm(arm, q, elapsed, args.out)
            report["arms"][arm] = evidence
            if arm == "valid_pin":
                validate_positive(evidence, q.stdout, args.measurement)
                report["guest_is_actually_booted_pcp_kernel"] = True
            elif arm == "wrong_pin":
                validate_bad_pin(evidence, q.stdout, "0" * 64)
            elif arm == "no_pin":
                validate_unpinned(evidence, q.stdout)
            elif arm == "probe_no_grant":
                validate_probe(evidence, q.stdout, args.measurement)
                report["external_guest_probe_verified"] = True
        report["status"] = "OBSERVED_IN_BOUNDED_CI"
        return 0
    except Exception as exc:
        report["status"] = "FAILED_OR_INCOMPLETE"
        report["error"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        summary_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")

if __name__ == "__main__":
    raise SystemExit(main())
