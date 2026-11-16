#!/usr/bin/env python3
"""Bounded OCI worker process-boundary probe, not a deployment or model test."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import time


def cmd(*args: str, check: bool = True, timeout: float = 35.0) -> subprocess.CompletedProcess:
    result = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    if check and result.returncode != 0:
        raise RuntimeError(
            f"command {args[0]} failed ({result.returncode}): "
            f"{result.stderr[-1000:]} {result.stdout[-500:]}"
        )
    return result


def wait_for(predicate, description: str, seconds: float = 15.0) -> None:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise AssertionError(f"timeout: {description}")


def size(path: Path) -> int:
    try:
        return path.stat().st_size
    except FileNotFoundError:
        return 0


def state(name: str) -> dict:
    return json.loads(cmd("docker", "inspect", name).stdout)[0]


def started_policy(check: dict) -> dict:
    settings = check["HostConfig"]
    facts = {
        "network_mode": settings["NetworkMode"],
        "read_only_rootfs": settings["ReadonlyRootfs"],
        "pids_limit_requested": settings["PidsLimit"],
        "memory_bytes_requested": settings["Memory"],
        "nano_cpus_requested": settings["NanoCpus"],
        "capabilities_dropped": settings["CapDrop"],
        "security_opt": settings["SecurityOpt"],
        "user": check["Config"]["User"],
    }
    assert facts["network_mode"] == "none", facts
    assert facts["read_only_rootfs"], facts
    assert facts["pids_limit_requested"] == 16, facts
    assert facts["memory_bytes_requested"] == 96 * 1024 * 1024, facts
    assert facts["nano_cpus_requested"] == 500_000_000, facts
    assert "ALL" in (facts["capabilities_dropped"] or []), facts
    assert any("no-new-privileges" in s for s in (facts["security_opt"] or [])), facts
    assert facts["user"] == "65534:65534", facts
    return facts


def launch(image: str, name: str, directory: Path, role: str, *, restart: bool) -> None:
    flags = [
        "docker", "run", "--detach", "--name", name,
        "--network", "none",
        "--read-only",
        "--pids-limit", "16",
        "--memory", "96m",
        "--cpus", "0.5",
        "--cap-drop", "ALL",
        "--security-opt", "no-new-privileges",
        "--user", "65534:65534",
        "--init",
        "--mount", f"type=bind,source={directory},target=/state",
    ]
    if restart:
        flags += ["--restart", "on-failure:2"]
    flags += [image, role]
    cmd(*flags)


def run_test(image: str, result: dict) -> None:
    result["docker_server_version"] = cmd(
        "docker", "version", "--format", "{{.Server.Version}}"
    ).stdout.strip()
    result["image_id"] = cmd("docker", "image", "inspect", image, "--format", "{{.Id}}").stdout.strip()

    # First witness: a child independently changes Unix session before its
    # container is stopped. The host, not a model or the worker, controls stop.
    with tempfile.TemporaryDirectory(prefix="pcp-d12-oci-stop-") as d:
        directory = Path(d)
        os.chmod(directory, 0o777)  # fixture volume, contains no secrets
        name = "pcp-d12-stop-" + secrets.token_hex(5)
        try:
            launch(image, name, directory, "parent", restart=False)
            configured = started_policy(state(name))
            wait_for(
                lambda: (directory / "grandchild-setsid").exists()
                and size(directory / "grandchild.tick") >= 3
                and size(directory / "parent.tick") >= 3,
                "actual parent and setsid-grandchild heartbeats", 18,
            )
            alive_before = state(name)["State"]["Running"]
            assert alive_before
            assert size(directory / "grandchild.tick") >= 3
            cmd("docker", "kill", "--signal", "KILL", name)
            wait_for(lambda: not state(name)["State"]["Running"], "Docker state stopped", 10)
            time.sleep(0.3)
            before = size(directory / "grandchild.tick")
            time.sleep(0.6)
            after = size(directory / "grandchild.tick")
            assert after == before, f"grandchild still writes after container stop: {before}->{after}"
            result["stop_boundary"] = {
                "observed": True,
                "worker_sets_new_session": (directory / "grandchild-setsid").exists(),
                "worker_heartbeat_before_stop": before,
                "worker_heartbeat_after_observation": after,
                "container_running_after_stop": state(name)["State"]["Running"],
                "requested_container_policy": configured,
            }
        finally:
            cmd("docker", "rm", "--force", name, check=False)

    # Second witness: a Docker host restart manager, not ChatGPT, creates a
    # successor process when the static worker exits nonzero after >10 seconds.
    with tempfile.TemporaryDirectory(prefix="pcp-d12-oci-restart-") as d:
        directory = Path(d)
        os.chmod(directory, 0o777)
        name = "pcp-d12-restart-" + secrets.token_hex(5)
        try:
            launch(image, name, directory, "restart", restart=True)
            configured = started_policy(state(name))
            wait_for(lambda: (directory / "restart-first").exists(), "first start marker", 10)
            wait_for(
                lambda: (directory / "restart-second").exists()
                and int(state(name)["RestartCount"]) >= 1
                and state(name)["State"]["Running"],
                "Docker-managed restart and new worker process", 32,
            )
            first = size(directory / "restart-first.tick")
            second_before = size(directory / "restart-second.tick")
            time.sleep(0.5)
            second_after = size(directory / "restart-second.tick")
            assert first > 0 and second_after > second_before
            restart_count = int(state(name)["RestartCount"])
            assert restart_count >= 1
            cmd("docker", "stop", "--time", "2", name, timeout=12)
            wait_for(lambda: not state(name)["State"]["Running"], "manual stop", 10)
            result["restart_boundary"] = {
                "observed": True,
                "first_process_had_heartbeat": first > 0,
                "second_process_heartbeat_advanced": second_after > second_before,
                "docker_restart_count": restart_count,
                "stopped_after_operator_action": not state(name)["State"]["Running"],
                "requested_container_policy": configured,
            }
        finally:
            cmd("docker", "rm", "--force", name, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = {
        "purpose": "bounded Docker runtime containment and restart observation",
        "product_code_modified": False,
        "real_pcp_os_kernel_executed": False,
        "qwen_executed": False,
        "independent_resident_service_after_ci": False,
        "host_enforced_acr_claim": False,
        "status": "INCOMPLETE",
        "rust_worker_sha256": hashlib.sha256(args.worker.read_bytes()).hexdigest(),
    }
    try:
        run_test(args.image, report)
        report["status"] = "SUCCESS_IN_TEST_SCOPE"
        return 0
    except Exception as exc:
        report["status"] = "FAILED_OR_INCOMPLETE"
        report["failure"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
