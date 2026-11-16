#!/usr/bin/env python3
"""Qualify Linux process-group cancellation against unregistered descendants.

Adversarial tests only: a process group is not a container and cannot contain a
descendant that starts a new session. Every launched process is under a
disposable supervisor-created session. No installed guest or real worker is used.
"""

import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

from containment_probe import (cleanup_escaped_child, has_exited, invoke,
                               proc_identity, read_ticks, wait_for)


def scenario(root, cli, delegate, request, mode, node):
    directory = root / mode
    directory.mkdir()
    db = directory / "desk.redb"
    invoke(str(cli), "init", str(db), "0", "1", "1")
    # The adapter itself is a new session leader. Its group is exclusively
    # test-owned, never the GitHub runner's process group.
    adapter = subprocess.Popen(
        [str(delegate), str(db), request, mode, "--",
         sys.executable, str(Path(__file__).with_name("containment_probe.py")),
         mode, str(directory)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        start_new_session=True, close_fds=True,
    )
    observations = {
        "mode": mode,
        "adapter_pid": adapter.pid,
        "test_group": adapter.pid,
    }
    failure = None
    try:
        wait_for(
            lambda: (directory / "leaf.identity.json").exists()
            and (directory / "leaf.pid").exists()
            and read_ticks(directory / "leaf.tick") >= 3,
            f"{mode} child ready",
        )
        ident = json.loads((directory / "leaf.identity.json").read_text(encoding="ascii"))
        pid = int((directory / "leaf.pid").read_text(encoding="ascii"))
        assert pid == ident["pid"] != adapter.pid
        assert proc_identity(pid)["start_tick"] == ident["start_tick"]
        group = os.getpgid(pid)
        assert group == (adapter.pid if mode == "same-group-parent" else pid), (
            mode, pid, group, adapter.pid,
        )
        observations["descendant_pid"] = pid
        observations["descendant_group"] = group
        invoke(str(cli), "cancel", str(db), f"cancel-{mode}", "1")
        out, err = adapter.communicate(timeout=8)
        observations["adapter_exit_code"] = adapter.returncode
        assert adapter.returncode != 0, (out, err)
        status = json.loads(invoke(str(cli), "status", str(db)).stdout)
        assert len(status["nodes"]) == 1 and status["nodes"][0]["state"] == "Cancelled"
        observations["registered_state"] = "Cancelled"
        initial = read_ticks(directory / "leaf.tick")
        time.sleep(0.35)
        before_fence = read_ticks(directory / "leaf.tick")
        observations["descendant_ticks_after_desk_cancel"] = before_fence - initial
        assert before_fence > initial, "adverse control missing: descendant already exited"

        # Test an independently ordered process-group signal, *after* desk
        # cancellation. It is not part of current pcp-delegate's behavior.
        try:
            os.killpg(adapter.pid, signal.SIGTERM)
            observations["test_group_signal_sent"] = True
        except ProcessLookupError:
            observations["test_group_signal_sent"] = False
        time.sleep(0.55)
        after_fence = read_ticks(directory / "leaf.tick")
        observations["ticks_after_group_signal"] = after_fence - before_fence
        observations["descendant_exited_or_reaped"] = has_exited(pid, ident["start_tick"])

        if mode == "same-group-parent":
            assert observations["test_group_signal_sent"], "test group missing before fence"
            assert after_fence == before_fence, "same-group descendant survived group fence"
            assert observations["descendant_exited_or_reaped"], "same-group descendant not exited"
            observations["scope_result"] = "test group signal stops same-group descendant"
        else:
            assert after_fence > before_fence, "detached control did not escape group fence"
            assert not observations["descendant_exited_or_reaped"], "detached control exited early"
            observations["scope_result"] = "detached new-session descendant survives group fence"
    except Exception as exc:
        failure = f"{type(exc).__name__}: {exc}"
        observations["error"] = failure
    finally:
        # Reap/terminate only test-created processes, even after an assertion
        # failure. The detached case has a distinct process group.
        (directory / "release").touch()
        if adapter.poll() is None:
            try:
                os.killpg(adapter.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                adapter.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                adapter.kill()
                adapter.communicate(timeout=3)
        if (directory / "leaf.pid").exists():
            try:
                observations["cleanup"] = cleanup_escaped_child(directory)
            except Exception as exc:
                observations["cleanup_error"] = f"{type(exc).__name__}: {exc}"
                failure = failure or observations["cleanup_error"]
    if failure:
        raise AssertionError(json.dumps(observations, sort_keys=True))
    if not observations.get("cleanup", {}).get("leaf_exited_or_reaped"):
        raise AssertionError("test child not independently confirmed exited/reaped")
    return observations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--delegate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    opts = parser.parse_args()
    if sys.platform != "linux":
        parser.error("Linux /proc and process-group control required")
    report = {
        "scope": "transient GitHub-hosted Linux process groups with disposable Parrots desks",
        "os_level_container_claim": False,
        "pcp_kernel_claim": False,
        "native_supervisor_modified": False,
        "ongoing_runtime_claim": False,
        "cases": {},
    }
    failure = None
    with tempfile.TemporaryDirectory(prefix="pcp-d13-pgroups-") as temp:
        for mode in ("same-group-parent", "rogue-parent"):
            try:
                report["cases"][mode] = scenario(
                    Path(temp), opts.cli.resolve(strict=True),
                    opts.delegate.resolve(strict=True),
                    f"d13-{mode}", mode, 1,
                )
            except Exception as exc:
                report["cases"][mode] = {"error": str(exc)}
                failure = failure or f"{mode}: {exc}"
    report["status"] = "OBSERVED_BOUNDARY" if failure is None else "INCOMPLETE"
    opts.output.parent.mkdir(parents=True, exist_ok=True)
    opts.output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, sort_keys=True))
    if failure:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
