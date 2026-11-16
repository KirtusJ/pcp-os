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
          measurement: str, request: str, *, fault: bool, hold: bool = False, spoof: bool = False):
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
    if hold:
        args += ["-e", "D12_HOLD_GUEST=1"]
    if spoof:
        args += ["-e", "D12_SPOOF_EXIT33=1"]
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

def process_identity(pid: int):
    # Host-procfs PID+start clock+boot identity, not a name-only oracle.
    try:
        raw = Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
    except FileNotFoundError:
        return None
    parts = raw.rsplit(") ", 1)[1].split()
    return {"pid": pid, "state": parts[0], "start_tick": int(parts[19])}

def qemu_child(name: str):
    try:
        output = invoke(["docker", "top", name, "-eo", "pid,args"], checked=True).stdout
    except RuntimeError:
        return None
    matches = []
    for row in output.splitlines()[1:]:
        col = row.split(maxsplit=1)
        if len(col) == 2 and col[0].isdigit() and "qemu-system-x86_64" in col[1]:
            matches.append(int(col[0]))
    if len(matches) != 1:
        return None
    identity = process_identity(matches[0])
    if not identity or identity["state"] in {"Z", "X", "x"}:
        return None
    identity["boot_id_sha256"] = sha(
        Path("/proc/sys/kernel/random/boot_id").read_bytes()
    )
    return identity

def original_process_no_longer_running(original: dict):
    if sha(Path("/proc/sys/kernel/random/boot_id").read_bytes()) != original["boot_id_sha256"]:
        raise AssertionError("host boot identity changed during observation")
    current = process_identity(original["pid"])
    return current is None or current["start_tick"] != original["start_tick"] or current["state"] in {"Z", "X", "x"}

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
        expected_line = (
            f"command line: /kernel.elf32 then=off image={measurement} "
            f"d12_ticket={request} d12_attempt={n}"
        )
        command_lines = [line for line in txt.splitlines() if line.startswith("command line: ")]
        assert command_lines == [expected_line], (
            f"attempt {n}: original guest serial is not bound to this synthetic request/attempt"
        )
        assert "PCP OS kernel" in txt, f"attempt {n} lacks guest kernel banner"
        assert "ALL SELF-TESTS PASS." in txt, f"attempt {n} lacks successful kernel self-tests"
        assert "measured; equal to the value the loader named" in txt
        matches = MEASURE.findall(txt)
        assert matches == [measurement], f"attempt {n} does not have exact measured image"
        heads = HEAD.findall(txt)
        assert len(heads) == 1 and int(heads[0][0]) > 6
        entries.append({
            "attempt": n, "native_kernel_boot": True,
            "native_command_line_bound_to_request": True,
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
    names = ["request.txt", "attention.txt", "busy-same.stderr", "busy-other.stderr"]
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

def interrupted_after_spawn(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # The guest CPU is deliberately held with QEMU -S, so this is proof of
    # actual process spawn, NOT proof that the kernel completed a boot.
    with tempfile.TemporaryDirectory(prefix="d12-qemu-after-spawn-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        name = "d12-after-spawn-" + secrets.token_hex(5)
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False, hold=True)
            await_true(
                lambda: (directory / "start-1.txt").exists()
                and (directory / "qemu-1.serial").exists()
                and inspect(name)["State"]["Running"],
                "START committed and QEMU stdout opened", 20,
            )
            identity = None
            def have_child():
                nonlocal identity
                identity = qemu_child(name)
                return identity is not None
            await_true(have_child, "exact OS-live QEMU child process", 12)
            assert identity and not original_process_no_longer_running(identity)
            assert not list(directory.glob("end-*.txt"))
            # This is a test-owned **operator kill** of a disposable container,
            # not a spontaneous kernel crash and not automatically authorized
            # recovery in a real service.
            invoke(["docker", "kill", "--signal", "KILL", name])
            await_true(lambda: not inspect(name)["State"]["Running"], "container stop after operator SIGKILL", 10)
            await_true(
                lambda: original_process_no_longer_running(identity),
                "exact previously observed QEMU PID stopped/reaped", 12,
            )
            # Restart is explicit, not an inferred effect of ChatGPT or the
            # Docker restart manager. The test controller restarts this same
            # container to assess the journal's meaning.
            invoke(["docker", "start", name])
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                and (directory / "attention.txt").exists(),
                "manually restarted controller held incomplete assignment", 12,
            )
            assert "INDETERMINATE" in (directory / "attention.txt").read_text()
            assert (directory / "start-1.txt").read_text() == f"REQUEST={request};ATTEMPT=1;START\n"
            assert not list(directory.glob("end-*.txt"))
            assert not list(directory.glob("start-2.txt"))
            assert not list(directory.glob("qemu-2.serial"))
            assert len(list(directory.glob("qemu-*.serial"))) == 1
            assert original_process_no_longer_running(identity)
            return {
                "fault_window": "after START and actual OS-live QEMU spawn, before any END",
                "paused_guest_execution": True,
                "original_qemu_os_identity": identity,
                "original_qemu_no_longer_running": True,
                "operator_injected_container_SIGKILL": True,
                "restart_performed_by_test_controller": True,
                "no_automatic_duplicate_guest_launch": True,
                "no_fabricated_completion": True,
                "attention": (directory / "attention.txt").read_text().strip(),
                "requested_policy": policy,
                "guest_boot_completed_before_kill": False,
            }
        finally:
            capture(directory, evidence_dir / "after_spawn", 1)
            invoke(["docker", "rm", "-f", name], checked=False)

def fabricated_exit_33(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # Negative, test-owned evidence: status 33 without the corresponding native
    # kernel serial MUST NOT permit an END or completion claim.
    with tempfile.TemporaryDirectory(prefix="d12-fabricated-exit-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        name = "d12-fabricated-exit-" + secrets.token_hex(5)
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False, spoof=True)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                and (directory / "attention.txt").exists(),
                "spoofed QEMU exit 33 rejected by independent native serial check", 12,
            )
            assert inspect(name)["RestartCount"] == 0
            assert inspect(name)["State"]["ExitCode"] == 0
            serial = (directory / "qemu-1.serial").read_bytes()
            assert serial.startswith(b"SYNTHETIC TEST:") and b"PCP OS kernel" not in serial
            assert (directory / "start-1.txt").read_text() == f"REQUEST={request};ATTEMPT=1;START\n"
            assert not list(directory.glob("end-*.txt"))
            assert not (directory / "qemu-2.serial").exists()
            receipt = (directory / "attention.txt").read_text()
            assert "UNCONFIRMED: QEMU exited 33 but native receipt invalid" in receipt
            return {
                "test_only_spoof": True,
                "falsified_qemu_exit_code": 33,
                "native_kernel_boot_observed": False,
                "synthetic_serial_sha256": sha(serial),
                "no_fabricated_END": True,
                "host_restart_count": 0,
                "attention": receipt.strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "spoof", 1)
            invoke(["docker", "rm", "-f", name], checked=False)

def sparse_future_attempt(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # Adverse journal structure: a valid-looking future attempt 3 must not
    # authorize dispatch of missing attempt 1 or 2.
    with tempfile.TemporaryDirectory(prefix="d12-noncontiguous-journal-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        name = "d12-noncontiguous-" + secrets.token_hex(5)
        (directory / "start-3.txt").write_text(
            f"REQUEST={request};ATTEMPT=3;START\n", encoding="ascii"
        )
        (directory / "end-3.txt").write_text(
            f"REQUEST={request};ATTEMPT=3;QEMU_EXIT=33;END\n", encoding="ascii"
        )
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                and (directory / "attention.txt").exists(),
                "noncontiguous journal causes fail-closed hold", 12,
            )
            info = inspect(name)
            assert info["RestartCount"] == 0 and info["State"]["ExitCode"] == 0
            reason = (directory / "attention.txt").read_text()
            assert "INVALID_JOURNAL: noncontiguous attempt 3" in reason
            assert not (directory / "start-1.txt").exists()
            assert not list(directory.glob("qemu-*.serial"))
            return {
                "test_only_noncontiguous_journal": True,
                "future_marker_attempt": 3,
                "old_preenrolled_future_attempt_not_promoted": True,
                "no_qemu_launched": True,
                "host_restart_count": 0,
                "attention": reason.strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "sparse", 3)
            invoke(["docker", "rm", "-f", name], checked=False)

def invalid_prior_receipt(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # A prior syntactically valid END marker with no real native kernel
    # readback cannot authorize a second QEMU attempt on service restart.
    with tempfile.TemporaryDirectory(prefix="d12-false-prior-END-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        name = "d12-false-prior-" + secrets.token_hex(5)
        (directory / "start-1.txt").write_text(
            f"REQUEST={request};ATTEMPT=1;START\n", encoding="ascii"
        )
        (directory / "end-1.txt").write_text(
            f"REQUEST={request};ATTEMPT=1;QEMU_EXIT=33;END\n", encoding="ascii"
        )
        (directory / "qemu-1.serial").write_bytes(
            b"SYNTHETIC ONLY: no native guest was launched for this END\n"
        )
        (directory / "qemu-1.stderr").write_bytes(b"")
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                and (directory / "attention.txt").exists(),
                "unverified prior END refused before new guest launch", 12,
            )
            info = inspect(name)
            assert info["RestartCount"] == 0 and info["State"]["ExitCode"] == 0
            attention = (directory / "attention.txt").read_text()
            assert "INVALID_JOURNAL: prior attempt 1 END without valid native receipt" in attention
            assert not (directory / "start-2.txt").exists()
            assert not (directory / "qemu-2.serial").exists()
            assert len(list(directory.glob("end-*.txt"))) == 1
            return {
                "false_prior_END_fixture": True,
                "syntactic_prior_END_rejected_without_native_receipt": True,
                "no_new_QEMU_guest_launched": True,
                "host_restart_count": 0,
                "attention": attention.strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "forged_end", 2)
            invoke(["docker", "rm", "-f", name], checked=False)


def replayed_native_guest_receipt(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # Actual byte-for-byte successful QEMU serial from *another* synthetic
    # request, paired here with syntactically valid journal markers for a new
    # request. A good kernel run is not a receipt for this operation.
    original = evidence_dir / "valid" / "qemu-1.serial"
    raw = original.read_bytes()
    txt = raw.decode("utf-8")
    pattern = (
        rf"command line: /kernel\.elf32 then=off image={re.escape(measurement)} "
        rf"d12_ticket=([0-9a-f]{{24}}) d12_attempt=1"
    )
    source_commands = [
        line for line in txt.splitlines() if line.startswith("command line: ")
    ]
    assert len(source_commands) == 1, "source must have one exact kernel command line"
    source_match = re.fullmatch(pattern, source_commands[0])
    assert source_match is not None, "source must be the exact prior native guest attempt 1"
    old_ticket = source_match.group(1)
    assert "ALL SELF-TESTS PASS." in txt and "PCP OS kernel " in txt
    with tempfile.TemporaryDirectory(prefix="d12-native-replay-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        while request == old_ticket:
            request = secrets.token_hex(12)
        name = "d12-native-replay-" + secrets.token_hex(5)
        (directory / "request.txt").write_text(request, encoding="ascii")
        (directory / "start-1.txt").write_text(
            f"REQUEST={request};ATTEMPT=1;START\n", encoding="ascii"
        )
        (directory / "end-1.txt").write_text(
            f"REQUEST={request};ATTEMPT=1;QEMU_EXIT=33;END\n", encoding="ascii"
        )
        (directory / "qemu-1.serial").write_bytes(raw)
        (directory / "qemu-1.stderr").write_bytes(b"")
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                and (directory / "attention.txt").exists(),
                "authentic previous guest serial refused for unrelated request", 12,
            )
            info = inspect(name)
            assert info["RestartCount"] == 0 and info["State"]["ExitCode"] == 0
            reason = (directory / "attention.txt").read_text()
            assert "INVALID_JOURNAL: prior attempt 1 END without valid native receipt" in reason
            assert "native kernel command line does not match the exact request and attempt" in reason
            assert not list(directory.glob("start-2.txt"))
            assert not list(directory.glob("qemu-2.serial"))
            assert (directory / "qemu-1.serial").read_bytes() == raw
            return {
                "source_is_real_completed_kernel_guest_serial": True,
                "source_ticket": old_ticket,
                "new_synthetic_request": request,
                "source_native_serial_sha256": sha(raw),
                "exact_prior_serial_replay_refused": True,
                "no_new_QEMU_guest_launched": True,
                "host_restart_count": 0,
                "attention": reason.strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "native_replay", 2)
            invoke(["docker", "rm", "-f", name], checked=False)

def completed_journal_missing_native_serial(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # A journal that appears to have COMPLETED all three attempts is not
    # sufficient when an independently recorded native serial is gone.
    valid = evidence_dir / "valid"
    request = (valid / "request.txt").read_text(encoding="ascii")
    assert re.fullmatch("[0-9a-f]{24}", request)
    removed_real_serial = (valid / "qemu-2.serial").read_bytes()
    assert b"ALL SELF-TESTS PASS." in removed_real_serial
    with tempfile.TemporaryDirectory(prefix="d12-terminal-missing-receipt-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        name = "d12-terminal-missing-" + secrets.token_hex(5)
        for n in (1, 2, 3):
            for suffix in (f"start-{n}.txt", f"end-{n}.txt", f"qemu-{n}.stderr"):
                shutil.copyfile(valid / suffix, directory / suffix)
            if n != 2:
                shutil.copyfile(valid / f"qemu-{n}.serial", directory / f"qemu-{n}.serial")
        (directory / "request.txt").write_text(request, encoding="ascii")
        assert len(list(directory.glob("end-*.txt"))) == 3
        assert not (directory / "qemu-2.serial").exists()
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False)
            await_true(
                lambda: not inspect(name)["State"]["Running"],
                "terminal-journal controller returned after examining prior runs", 12,
            )
            info = inspect(name)
            assert info["RestartCount"] == 0
            assert info["State"]["ExitCode"] == 0
            attention = directory / "attention.txt"
            assert attention.exists(), (
                "test-first failure: all-complete journal was silently accepted "
                "even though real guest attempt 2 raw serial was missing"
            )
            reason = attention.read_text(encoding="utf-8")
            assert "INVALID_JOURNAL: prior attempt 2 END without valid native receipt" in reason
            assert not (directory / "qemu-2.serial").exists()
            assert len(list(directory.glob("start-*.txt"))) == 3
            return {
                "fixture_copied_real_three_guest_complete_journal": True,
                "omitted_real_attempt": 2,
                "omitted_real_native_serial_sha256": sha(removed_real_serial),
                "terminal_journal_missing_receipt_refused": True,
                "no_new_QEMU_launched": True,
                "attention": reason.strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "terminal_missing", 3)
            invoke(["docker", "rm", "-f", name], checked=False)


def completed_journal_wrong_attempt_serial(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # Both source files are genuine native serial outputs from the same
    # synthetic request, but attempt 1 cannot authenticate attempt 2.
    valid = evidence_dir / "valid"
    request = (valid / "request.txt").read_text(encoding="ascii")
    assert re.fullmatch("[0-9a-f]{24}", request)
    genuine_first = (valid / "qemu-1.serial").read_bytes()
    genuine_second = (valid / "qemu-2.serial").read_bytes()
    assert genuine_first != genuine_second
    with tempfile.TemporaryDirectory(prefix="d12-terminal-wrong-attempt-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        name = "d12-terminal-wrong-attempt-" + secrets.token_hex(5)
        for n in (1, 2, 3):
            for suffix in (f"start-{n}.txt", f"end-{n}.txt",
                           f"qemu-{n}.serial", f"qemu-{n}.stderr"):
                shutil.copyfile(valid / suffix, directory / suffix)
        (directory / "request.txt").write_text(request, encoding="ascii")
        (directory / "qemu-2.serial").write_bytes(genuine_first)
        assert len(list(directory.glob("end-*.txt"))) == 3
        try:
            policy = start(image, name, directory, kernel, measurement, request, fault=False)
            await_true(
                lambda: not inspect(name)["State"]["Running"]
                and (directory / "attention.txt").exists(),
                "completed journal's genuine prior-attempt substitution rejected", 12,
            )
            info = inspect(name)
            assert info["RestartCount"] == 0 and info["State"]["ExitCode"] == 0
            reason = (directory / "attention.txt").read_text()
            assert "INVALID_JOURNAL: prior attempt 2 END without valid native receipt" in reason
            assert "native kernel command line does not match the exact request and attempt" in reason
            assert (directory / "qemu-2.serial").read_bytes() == genuine_first
            assert not (directory / "start-4.txt").exists()
            return {
                "fixture_three_complete_real_guest_journals": True,
                "source_attempt": 1,
                "falsely_assigned_attempt": 2,
                "genuine_source_serial_sha256": sha(genuine_first),
                "original_correct_second_serial_sha256": sha(genuine_second),
                "wrong_attempt_serial_refused_before_completion": True,
                "no_new_QEMU_launched": True,
                "attention": reason.strip(),
                "requested_policy": policy,
            }
        finally:
            capture(directory, evidence_dir / "terminal_wrong_attempt", 3)
            invoke(["docker", "rm", "-f", name], checked=False)


def concurrent_duplicate_admission(image: str, kernel: Path, measurement: str, evidence_dir: Path):
    # Concurrent contenders with the *same or a different* ticket must not
    # infer a crash from a first controller's genuinely live paused QEMU.
    # Linux flock here is cooperative, local, and does not prevent a hostile
    # process with same-user write privileges from replacing the lock inode.
    with tempfile.TemporaryDirectory(prefix="d12-concurrent-request-") as tmp:
        directory = Path(tmp)
        os.chmod(directory, 0o777)
        request = secrets.token_hex(12)
        other_request = secrets.token_hex(12)
        while other_request == request:
            other_request = secrets.token_hex(12)
        name_a = "d12-active-owner-" + secrets.token_hex(5)
        name_b = "d12-second-owner-" + secrets.token_hex(5)
        name_c = "d12-third-owner-" + secrets.token_hex(5)
        first_started = False
        second_started = False
        third_started = False
        try:
            start(image, name_a, directory, kernel, measurement, request, fault=False, hold=True)
            first_started = True
            original = None
            def first_qemu_is_live():
                nonlocal original
                original = qemu_child(name_a)
                return original is not None and (directory / "start-1.txt").exists()
            await_true(first_qemu_is_live, "first real QEMU OS process with START marker", 15)
            assert not original_process_no_longer_running(original)
            assert inspect(name_a)["State"]["Running"]
            assert not (directory / "attention.txt").exists()

            policy_b = start(image, name_b, directory, kernel, measurement, request, fault=False)
            second_started = True
            await_true(lambda: not inspect(name_b)["State"]["Running"], "same-request contender stopped", 12)
            info_b = inspect(name_b)
            assert info_b["State"]["ExitCode"] == 0 and info_b["RestartCount"] == 0
            log_b = invoke(["docker", "logs", name_b])
            observed_b = (log_b.stdout + log_b.stderr).encode("utf-8")
            assert b"D12_BUSY: another same-host controller holds the journal; no dispatch" in observed_b
            (directory / "busy-same.stderr").write_bytes(observed_b)
            assert inspect(name_a)["State"]["Running"]
            assert not original_process_no_longer_running(original)
            assert not (directory / "attention.txt").exists(), (
                "a same-ticket contender falsely quarantined the actually live QEMU assignment"
            )

            policy_c = start(
                image, name_c, directory, kernel, measurement, other_request,
                fault=False,
            )
            third_started = True
            await_true(lambda: not inspect(name_c)["State"]["Running"], "different-request contender stopped", 12)
            info_c = inspect(name_c)
            assert info_c["State"]["ExitCode"] == 0 and info_c["RestartCount"] == 0
            log_c = invoke(["docker", "logs", name_c])
            observed_c = (log_c.stdout + log_c.stderr).encode("utf-8")
            assert b"D12_BUSY: another same-host controller holds the journal; no dispatch" in observed_c
            (directory / "busy-other.stderr").write_bytes(observed_c)

            assert (directory / "request.txt").read_text(encoding="ascii") == request
            assert not (directory / "attention.txt").exists(), (
                "different-ticket contender overwrote/quarantined live owner's journal"
            )
            assert inspect(name_a)["State"]["Running"]
            assert not original_process_no_longer_running(original)
            assert len(list(directory.glob("start-*.txt"))) == 1
            assert not (directory / "start-2.txt").exists()
            assert len(list(directory.glob("qemu-*.serial"))) == 1
            return {
                "duplicate_same_ticket_controller": True,
                "concurrent_distinct_ticket_controller": True,
                "original_ticket": request,
                "blocked_second_ticket": other_request,
                "concurrent_original_qemu_os_identity": original,
                "first_qemu_still_running": True,
                "both_contenders_report_busy_not_completion": True,
                "same_ticket_busy_log_sha256": sha(observed_b),
                "different_ticket_busy_log_sha256": sha(observed_c),
                "contenders_exited_without_automatic_restart": True,
                "contenders_could_not_poison_live_owner": True,
                "no_duplicate_guest_launch": True,
                "same_ticket_contender_policy": policy_b,
                "other_ticket_contender_policy": policy_c,
            }
        finally:
            if third_started:
                invoke(["docker", "rm", "-f", name_c], checked=False)
            if second_started:
                invoke(["docker", "rm", "-f", name_b], checked=False)
            if first_started:
                invoke(["docker", "kill", "--signal", "KILL", name_a], checked=False)
                invoke(["docker", "rm", "-f", name_a], checked=False)
            capture(directory, evidence_dir / "concurrent", 1)


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
        result["post_spawn_interrupted"] = interrupted_after_spawn(a.image, kernel, a.measurement, evidence_dir)
        result["falsified_exit_33"] = fabricated_exit_33(a.image, kernel, a.measurement, evidence_dir)
        result["noncontiguous_journal"] = sparse_future_attempt(a.image, kernel, a.measurement, evidence_dir)
        result["false_prior_END"] = invalid_prior_receipt(a.image, kernel, a.measurement, evidence_dir)
        result["replayed_native_guest_receipt"] = replayed_native_guest_receipt(
            a.image, kernel, a.measurement, evidence_dir
        )
        result["terminal_complete_missing_receipt"] = completed_journal_missing_native_serial(
            a.image, kernel, a.measurement, evidence_dir
        )
        result["terminal_complete_wrong_attempt"] = completed_journal_wrong_attempt_serial(
            a.image, kernel, a.measurement, evidence_dir
        )
        result["simultaneous_same_request"] = concurrent_duplicate_admission(
            a.image, kernel, a.measurement, evidence_dir
        )
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
