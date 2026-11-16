#!/usr/bin/env python3
"""Check bounded Docker-managed reruns of a *real* PCP OS QEMU guest.

The Linux host provides the OCI runtime and restart policy. The PCP OS kernel
runs only INSIDE QEMU, never as the container's own host kernel. This script
is a test oracle, not a production permission or process supervisor.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import tempfile
import time

HEAD = re.compile(r"kernel register:\s+(\d+) events, head ([0-9a-f]{64})")
MEASURE = re.compile(r"running image .*?SHA-256: ([0-9a-f]{64}) \(measured; equal")

def invoke(args: list[str], *, timeout: int = 35, checked: bool = True):
    v = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    if checked and v.returncode:
        raise RuntimeError(
            f"{args[0]} returned {v.returncode}: stderr={v.stderr[-800:]} stdout={v.stdout[-350:]}"
        )
    return v

def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()

def await_true(f, description: str, seconds: float):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if f():
            return
        time.sleep(0.25)
    raise AssertionError("timeout waiting for " + description)

def inspect(name: str):
    return json.loads(invoke(["docker", "inspect", name]).stdout)[0]

def start(image: str, name: str, directory: Path, kernel: Path,
          measurement: str, request: str, *, fault: bool):
    args = [
        "docker", "run", "-d", "--name", name,
        "--restart", "on-failure:2",
        "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
        "--user", "65534:65534", "--init",
        "--pids-limit", "64", "--memory", "1g", "--cpus", "2",
        "--tmpfs", "/tmp:rw,nosuid,nodev,size=33554432",
        "--mount", f"type=bind,source={directory},target=/state",
        "--mount", f"type=bind,source={kernel},target=/kernel.elf32,readonly",
        "-e", f"D12_MEASUREMENT={measurement}",
        "-e", f"D12_REQUEST={request}",
    ]
    if fault:
        args += ["-e", "D12_FAULT_PRESPAWN=1"]
    args += [image]
    invoke(args)
    obj = inspect(name)
    policy = obj["HostConfig"]
    assert policy["NetworkMode"] == "none"
    assert policy["ReadonlyRootfs"] is True
    assert policy["RestartPolicy"]["Name"] == "on-failure"
    assert policy["RestartPolicy"]["MaximumRetryCount"] == 2
    assert policy["PidsLimit"] == 64
    assert policy["Memory"] == 1024**3
    assert "ALL" in (policy["CapDrop"] or [])
    assert any("no-new-privileges" in x for x in policy["SecurityOpt"] or [])
    assert obj["Config"]["User"] == "65534:65534"
    return {
        "image": obj["Image"],
        "network_disabled": True,
        "read_only_root": True,
        "requested_pid_limit": 64,
        "requested_memory_bytes": 1024**3,
        "no_new_privileges_configured": True,
        "drop_all_capabilities_configured": True,
        "synthetic_request": request,
    }

def evidence(directory: Path, request: str, measurement: str, attempts: int) -> list[dict]:
    expected = set()
    entries = []
    for n in range(1, attempts + 1):
        start_path = directory / f"start-{n}.txt"
        end_path = directory / f"end-{n}.txt"
        serial_path = directory / f"qemu-{n}.serial"
        stderr_path = directory / f"qemu-{n}.stderr"
        expected |= {start_path.name, end_path.name, serial_path.name, stderr_path.name}
        assert start_path.read_text() == f"REQUEST={request};ATTEMPT={n};START\n"
        assert end_path.read_text() == f"REQUEST={request};ATTEMPT={n};QEMU_EXIT=33;END\n"
        serial = serial_path.read_bytes()
        stderr = stderr_path.read_bytes()
        txt = serial.decode("utf-8")
        assert "PCP OS kernel" in txt, f"attempt {n} lacks guest kernel banner"
        assert "ALL SELF-TESTS PASS." in txt, f"attempt {n} lacks successful kernel self-tests"
        assert "measured; equal to the value the loader named" in txt
        matches = MEASURE.findall(txt)
        assert matches == [measurement], f"attempt {n} does not have exact measured image"
        heads = HEAD.findall(txt)
        assert len(heads) == 1 and int(heads[0][0]) > 6
        entries.append({
            "attempt": n, "native_kernel_boot": True,
            "qemu_reported_exit_code": 33,
            "serial_bytes": len(serial), "serial_sha256": sha(serial),
            "stderr_sha256": sha(stderr),
            "kernel_register_events": int(heads[0][0]),
            "kernel_register_head": heads[0][1],
        })
    actual = {p.name for p in directory.glob("start-*.txt")}
    assert actual == {f"start-{n}.txt" for n in range(1, attempts + 1)}
    assert {p.name for p in directory.glob("end-*.txt")} == {
        f"end-{n}.txt" for n in range(1, attempts + 1)
    }
    assert {p.name for p in directory.glob("qemu-*.serial")} == {
        f"qemu-{n}.serial" for n in range(1, attempts + 1)
    }
    return entries

def capture(directory: Path, destination: Path, attempts: int) -> None:
    """Positive allowlist only: this fixture has no secrets or real task data."""
    destination.mkdir(parents=True, exist_ok=True)
    names = ["request.txt", "attention.txt"]
    for n in range(1, attempts + 1):
        names.extend([
            f"start-{n}.txt", f"end-{n}.txt",
            f"qemu-{n}.serial", f"qemu-{n}.stderr",
        ])
    for name in names:
        path = directory / name
        if path.is_file() and not path.is_symlink():
            shutil.copyfile(path, destination / name)


def qualifying_run(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    with tempfile.TemporaryDirectory(prefix="d12-qemu-restart-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        name = "d12-qemu-restart-" + secrets.token_hex(5)
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                    and inspect(name)["RestartCount"] >= 2
                    and (directory / "end-3.txt").exists(),
                "three separate boots followed by successful wrapper exit", 160,
            )
            info = inspect(name)
            assert info["RestartCount"] == 2, info["RestartCount"]
            assert info["State"]["ExitCode"] == 0, info["State"]
            assert not (directory / "attention.txt").exists()
            entries = evidence(directory, request, measurement, 3)
            assert (directory / "request.txt").read_text() == request
            # A fresh operation identity must not be silently adopted by
            # replay of this unrelated retained journal.
            other = "d12-qemu-mismatch-" + secrets.token_hex(5)
            try:
                start(image, other, directory, kernel, measurement, secrets.token_hex(12), fault=False)
                await_true(
                    lambda: not inspect(other)["State"]["Running"]
                        and (directory / "attention.txt").exists(),
                    "cross-request stale-state refusal", 20,
                )
                assert inspect(other)["RestartCount"] == 0
                assert inspect(other)["State"]["ExitCode"] == 0
                assert "REQUEST_MISMATCH" in (directory / "attention.txt").read_text()
                assert len(list(directory.glob("qemu-*.serial"))) == 3
            finally:
                invoke(["docker", "rm", "-f", other], checked=False)
            return {
                "actual_guest_boots": entries,
                "docker_managed_restart_count": info["RestartCount"],
                "requested_policy": policy,
                "bounded_successful_final_exit": info["State"]["ExitCode"],
                "distinct_request_refused_without_guest_boot": True,
                "semantic_task_resumption_proven": False,
            }
        finally:
            capture(directory, evidence_dir / "valid", 3)
            invoke(["docker", "rm", "-f", name], checked=False)

def interrupted_run(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    with tempfile.TemporaryDirectory(prefix="d12-qemu-interrupted-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        name = "d12-qemu-interrupted-" + secrets.token_hex(5)
        request = secrets.token_hex(12)
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=True)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                    and (directory / "attention.txt").exists(),
                "interrupted START receiving fail-closed attention after Docker restart", 40,
            )
            info = inspect(name)
            assert info["RestartCount"] == 1, info["RestartCount"]
            assert info["State"]["ExitCode"] == 0, info["State"]
            assert (directory / "start-1.txt").read_text() == (
                f"REQUEST={request};ATTEMPT=1;START\n"
            )
            assert "INDETERMINATE" in (directory / "attention.txt").read_text()
            assert not list(directory.glob("end-*.txt"))
            assert not list(directory.glob("qemu-*.serial"))
            return {
                "after_start_before_guest_fault": True,
                "docker_restarted_controller_once": info["RestartCount"] == 1,
                "prior_work_not_silently_marked_finished": True,
                "duplicate_guest_not_launched": True,
                "attention_recorded": (directory / "attention.txt").read_text().strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "fault", 1)
            invoke(["docker", "rm", "-f", name], checked=False)

def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--image", required=True)
    p.add_argument("--kernel", type=Path, required=True)
    p.add_argument("--measurement", required=True)
    p.add_argument("--source-head", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--evidence-dir", type=Path, required=True)
    a = p.parse_args()
    kernel = a.kernel.resolve(strict=True)
    assert re.fullmatch("[0-9a-f]{64}", a.measurement)
    out = a.out
    out.parent.mkdir(parents=True, exist_ok=True)
    evidence_dir = a.evidence_dir
    evidence_dir.mkdir(parents=True, exist_ok=True)
    result = {
        "scope": "bounded Linux OCI Docker restart of actual PCP OS x86_64 QEMU guests",
        "source_head": a.source_head,
        "kernel_file_sha256": sha(kernel.read_bytes()),
        "externally_named_code_rodata_sha256": a.measurement,
        "host_kernel_is_pcp_os": False,
        "qemu_guest_kernel_is_pcp_os": True,
        "independent_resident_service_after_ci": False,
        "qwen_present": False,
        "product_binary_or_iso_modified": False,
        "status": "INCOMPLETE",
    }
    try:
        result["docker_engine_version"] = invoke(
            ["docker", "version", "--format", "{{.Server.Version}}"]
        ).stdout.strip()
        result["image_id"] = invoke([
            "docker", "image", "inspect", a.image, "--format", "{{.Id}}"
        ]).stdout.strip()
        result["qualification"] = qualifying_run(a.image, kernel, a.measurement, evidence_dir)
        result["interrupted"] = interrupted_run(a.image, kernel, a.measurement, evidence_dir)
        result["status"] = "OBSERVED_IN_TEST_SCOPE"
        return 0
    except Exception as e:
        result["status"] = "FAILED_OR_INCOMPLETE"
        result["failure"] = f"{type(e).__name__}: {e}"
        raise
    finally:
        result["captured_evidence"] = [
            {
                "path": str(p.relative_to(evidence_dir)),
                "bytes": p.stat().st_size,
                "sha256": sha(p.read_bytes()),
            }
            for p in sorted(evidence_dir.rglob("*"))
            if p.is_file()
        ]
        out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")

if __name__ == "__main__":
    raise SystemExit(main())
