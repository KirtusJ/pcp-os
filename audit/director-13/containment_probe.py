#!/usr/bin/env python3
"""Bounded Linux test of pcp-delegate's process-containment boundary.

This is an adverse *qualification* test, not a hardened launcher. It deliberately
creates one non-cooperating grandchild in a new session, observes whether it is
still doing work after the registered parent is cancelled, and cleans it up.
No real PCP register, installed guest, model, network or privileged service is used.
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


def wait_for(check, what, seconds=8.0):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if check():
            return
        time.sleep(0.05)
    raise AssertionError(f"timed out waiting for {what}")


def read_ticks(path):
    try:
        return path.stat().st_size
    except FileNotFoundError:
        return 0


def proc_identity(pid):
    try:
        # Split after the parenthesized process name, which may contain spaces.
        rest = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8").rsplit(") ", 1)[1].split()
        return {"state": rest[0], "start_tick": int(rest[19])}
    except FileNotFoundError:
        return None


def actor(mode, directory):
    root = Path(directory)
    identity = proc_identity(os.getpid())
    assert identity is not None
    (root / f"{mode}.identity.json").write_text(
        json.dumps({"pid": os.getpid(), "start_tick": identity["start_tick"],
                    "process_group": os.getpgrp()}), encoding="ascii"
    )
    heartbeat = root / ("leaf.tick" if mode == "leaf" else "parent.tick")
    if mode in {"rogue-parent", "same-group-parent"}:
        # Both descendants bypass pcp-delegate. The original 'rogue-parent'
        # escapes to a new session; 'same-group-parent' stays in the test's
        # own session, to distinguish a process-group fence from a cgroup.
        child = subprocess.Popen(
            [sys.executable, __file__, "leaf", str(root)],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=(mode == "rogue-parent"), close_fds=True,
        )
        (root / "leaf.pid").write_text(str(child.pid), encoding="ascii")
    deadline = time.monotonic() + 25.0
    while time.monotonic() < deadline and not (root / "release").exists():
        with heartbeat.open("ab") as out:
            out.write(b"X")
        time.sleep(0.07)
    (root / f"{mode}.exited").write_text("exit requested", encoding="utf-8")


def invoke(*args, timeout=12.0):
    return subprocess.run(args, text=True, capture_output=True, timeout=timeout, check=True)


def has_exited(pid, original_start_tick):
    # Linux's /proc marks orphans as zombies before PID 1 can reap them.
    # A changed start tick means the old process exited and the PID was reused.
    current = proc_identity(pid)
    return (current is None or current["state"] == "Z"
            or current["start_tick"] != original_start_tick)


def cleanup_escaped_child(root, expected_group=None):
    pidfile = root / "leaf.pid"
    if not pidfile.exists():
        return {"leaf_created": False}
    pid = int(pidfile.read_text(encoding="ascii"))
    identity = json.loads((root / "leaf.identity.json").read_text(encoding="ascii"))
    assert identity["pid"] == pid
    start_tick = identity["start_tick"]
    approved_group = pid if expected_group is None else expected_group
    (root / "release").touch()
    if not has_exited(pid, start_tick):
        wait_for(lambda: (root / "leaf.exited").exists() or has_exited(pid, start_tick),
                 "escaped worker's exit notice", 2.5)
    if not has_exited(pid, start_tick):
        # The leaf identity is still the same process; check group ownership.
        # Never accidentally signal the runner's or a foreign process group.
        if os.getpgid(pid) != approved_group or (
            expected_group is None and identity["process_group"] != pid
        ):
            raise AssertionError("detached leaf escaped its test-owned cleanup group")
        os.killpg(approved_group, signal.SIGTERM)
        try:
            wait_for(lambda: has_exited(pid, start_tick), "escaped worker cleanup", 2.0)
        except AssertionError:
            if has_exited(pid, start_tick):
                pass
            elif os.getpgid(pid) == approved_group:
                os.killpg(approved_group, signal.SIGKILL)
                wait_for(lambda: has_exited(pid, start_tick),
                         "forced escaped worker cleanup", 2.0)
            else:
                raise AssertionError("cleanup group identity changed")
    return {"leaf_created": True, "leaf_pid": pid, "leaf_exited_or_reaped": has_exited(pid, start_tick)}


def read_actor_identity(root, mode):
    identity = json.loads((root / f"{mode}.identity.json").read_text(encoding="ascii"))
    assert isinstance(identity["pid"], int) and identity["pid"] > 0
    assert isinstance(identity["start_tick"], int) and identity["start_tick"] > 0
    return identity


def silent_liveness_oracle(root):
    """Negative control: absence of heartbeats does not prove an OS process exited."""
    sleeper = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(10)"],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL, start_new_session=True, close_fds=True,
    )
    try:
        wait_for(lambda: proc_identity(sleeper.pid) is not None, "silent worker to start")
        identity = proc_identity(sleeper.pid)
        assert identity is not None and identity["state"] != "Z"
        marker = root / "silent.tick"
        before = read_ticks(marker)
        time.sleep(0.20)
        after = read_ticks(marker)
        assert before == after == 0
        assert not has_exited(sleeper.pid, identity["start_tick"])
        return {"no_heartbeat": True, "process_still_alive": True,
                "positive_liveness_oracle": "PID and /proc start tick"}
    finally:
        if sleeper.poll() is None:
            sleeper.terminate()
        sleeper.wait(timeout=5.0)


def cleanup_registered_actor(adapter, root, mode):
    """Terminate only test-created adapter/actor identities after any outcome."""
    if adapter.poll() is None:
        if os.getpgid(adapter.pid) != adapter.pid:
            raise AssertionError("test adapter did not retain its isolated group")
        os.killpg(adapter.pid, signal.SIGTERM)
    try:
        adapter.communicate(timeout=3)
    except subprocess.TimeoutExpired:
        adapter.kill()
        adapter.communicate(timeout=3)
    identity_path = root / f"{mode}.identity.json"
    if not identity_path.exists():
        return {"actor_identity_observed": False}
    identity = read_actor_identity(root, mode)
    pid, tick = identity["pid"], identity["start_tick"]
    if not has_exited(pid, tick):
        # The source test actor PID and start tick must both match /proc.
        os.kill(pid, signal.SIGTERM)
        try:
            wait_for(lambda: has_exited(pid, tick), "test actor to terminate", 2.0)
        except AssertionError:
            if not has_exited(pid, tick):
                os.kill(pid, signal.SIGKILL)
                wait_for(lambda: has_exited(pid, tick),
                         "forced test actor termination", 2.0)
    return {"actor_identity_observed": True,
            "actor_exited_or_reaped": has_exited(pid, tick)}


def probe(cli, delegate, output):
    report = {
        "scope": "Linux transient process tree and a disposable Parrots redb desk only",
        "product_code_changed": False,
        "release_authorized": False,
        "container_enforced": False,
        "resident_runtime_proven": False,
        "controls": {},
    }
    failure = None
    with tempfile.TemporaryDirectory(prefix="pcp-d13-containment-") as tmp:
        root = Path(tmp)
        db = root / "probe.redb"
        invoke(str(cli), "init", str(db), "0", "1", "4")
        try:
            report["controls"]["silent_oracle"] = silent_liveness_oracle(root)
            # Positive: the only child is the directly supervised process.
            direct_dir = root / "direct"
            direct_dir.mkdir()
            direct = subprocess.Popen([
                str(delegate), str(db), "direct-01", "direct worker", "--",
                sys.executable, __file__, "direct-parent", str(direct_dir),
            ], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
               stderr=subprocess.PIPE, text=True, start_new_session=True,
               close_fds=True)
            try:
                wait_for(lambda: read_ticks(direct_dir / "parent.tick") >= 2
                         and (direct_dir / "direct-parent.identity.json").exists(),
                         "direct worker to start")
                direct_identity = read_actor_identity(direct_dir, "direct-parent")
                assert not has_exited(direct_identity["pid"], direct_identity["start_tick"])
                invoke(str(cli), "cancel", str(db), "cancel-direct", "1")
                out, err = direct.communicate(timeout=8.0)
                assert direct.returncode != 0, (out, err)
                wait_for(lambda: has_exited(direct_identity["pid"],
                                              direct_identity["start_tick"]),
                         "direct worker to exit after cancellation", 2.0)
                n = read_ticks(direct_dir / "parent.tick")
                time.sleep(0.40)
                assert read_ticks(direct_dir / "parent.tick") == n
                recorded = json.loads(invoke(str(cli), "status", str(db)).stdout)
                assert len(recorded["nodes"]) == 1 and recorded["nodes"][0]["state"] == "Cancelled"
                report["controls"]["direct"] = {
                    "adapter_exit_code": direct.returncode,
                    "direct_child_stopped_after_cancel": True,
                    "direct_child_os_exited_or_reaped": True,
                    "recorded_state": recorded["nodes"][0]["state"],
                }
            finally:
                report["controls"]["direct_cleanup"] = cleanup_registered_actor(
                    direct, direct_dir, "direct-parent"
                )

            # Negative: a descendant escapes the registered parent's process
            # tree and continues writing after the desk has closed its parent.
            rogue_dir = root / "rogue"
            rogue_dir.mkdir()
            rogue = subprocess.Popen([
                str(delegate), str(db), "rogue-01", "unregistered child probe", "--",
                sys.executable, __file__, "rogue-parent", str(rogue_dir),
            ], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
               stderr=subprocess.PIPE, text=True, start_new_session=True,
               close_fds=True)
            try:
                wait_for(lambda: (rogue_dir / "leaf.pid").exists()
                         and (rogue_dir / "rogue-parent.identity.json").exists()
                         and read_ticks(rogue_dir / "leaf.tick") >= 2,
                         "unregistered detached descendant")
                rogue_identity = read_actor_identity(rogue_dir, "rogue-parent")
                assert not has_exited(rogue_identity["pid"], rogue_identity["start_tick"])
                leaf_identity = read_actor_identity(rogue_dir, "leaf")
                assert not has_exited(leaf_identity["pid"], leaf_identity["start_tick"])
                invoke(str(cli), "cancel", str(db), "cancel-rogue", "2")
                out, err = rogue.communicate(timeout=8.0)
                assert rogue.returncode != 0, (out, err)
                wait_for(lambda: has_exited(rogue_identity["pid"],
                                              rogue_identity["start_tick"]),
                         "registered parent worker OS exit", 2.0)
                before = read_ticks(rogue_dir / "leaf.tick")
                time.sleep(0.50)
                after = read_ticks(rogue_dir / "leaf.tick")
                recorded = json.loads(invoke(str(cli), "status", str(db)).stdout)
                assert [x["state"] for x in recorded["nodes"]] == ["Cancelled", "Cancelled"]
                report["controls"]["unregistered_descendant"] = {
                    "adapter_exit_code": rogue.returncode,
                    "recorded_state": recorded["nodes"][1]["state"],
                    "ticks_after_parent_cancel": after - before,
                    "continued_after_parent_cancel": after > before,
                    "tracked_as_separate_desk_node": len(recorded["nodes"]) > 2,
                    "registered_parent_os_exited_or_reaped": True,
                    "detached_child_os_alive": True,
                }
                assert not has_exited(leaf_identity["pid"], leaf_identity["start_tick"]), (
                    "detached child exited before a live continuation could be established"
                )
                assert after > before, (
                    "negative control did not expose the known group escape; "
                    "check whether the supervisor was hardened, or the probe is stale"
                )
            finally:
                report["controls"]["rogue_parent_cleanup"] = cleanup_registered_actor(
                    rogue, rogue_dir, "rogue-parent"
                )
                report["cleanup"] = cleanup_escaped_child(rogue_dir)
            report["scope_result"] = (
                "PASS: direct cancellation observed; detached unregistered "
                "descendant remained active, so cancellation is not containment"
            )
        except Exception as exc:
            failure = f"{type(exc).__name__}: {exc}"
            report["error"] = failure
            report["scope_result"] = "INCOMPLETE: boundary probe did not finish"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, sort_keys=True))
    if failure:
        raise SystemExit(1)


def main():
    if len(sys.argv) >= 3 and sys.argv[1] in {"direct-parent", "rogue-parent", "same-group-parent", "leaf"}:
        actor(sys.argv[1], sys.argv[2])
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--delegate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    opts = parser.parse_args()
    if sys.platform != "linux":
        parser.error("this probe requires Linux /proc and process groups")
    probe(opts.cli.resolve(strict=True), opts.delegate.resolve(strict=True), opts.output)


if __name__ == "__main__":
    main()
