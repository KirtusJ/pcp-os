#!/usr/bin/env python3
"""Adversarial bound: one admitted task can spawn multiple unregistered OS workers.

Only synthetic, disposable Linux Python children under the real Rust Parrots
adapter. This deliberately measures how an admission quota differs from an
enforced process quota. It does not start Qwen, a real PCP kernel or a service.
"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from containment_probe import (
    cleanup_escaped_child, cleanup_registered_actor, has_exited,
    invoke, proc_identity, read_actor_identity, read_ticks, wait_for,
)

UNREGISTERED_CHILDREN = 3


def identity():
    item = proc_identity(os.getpid())
    assert item is not None and item["state"] != "Z"
    return {"pid": os.getpid(), "start_tick": item["start_tick"],
            "process_group": os.getpgrp()}


def leaf(directory):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    item = identity()
    assert item["pid"] == item["process_group"], "leaf must own an isolated group"
    (directory / "leaf.identity.json").write_text(json.dumps(item), encoding="ascii")
    (directory / "leaf.pid").write_text(str(os.getpid()), encoding="ascii")
    end = time.monotonic() + 22.0
    while time.monotonic() < end and not (directory / "release").exists() \
            and not (directory.parent / "release").exists():
        with (directory / "leaf.tick").open("ab") as out:
            out.write(b"x")
        time.sleep(0.065)
    (directory / "leaf.exited").write_text("worker exited", encoding="ascii")


def parent(root):
    root = Path(root)
    (root / "budget-parent.identity.json").write_text(
        json.dumps(identity()), encoding="ascii"
    )
    for i in range(UNREGISTERED_CHILDREN):
        directory = root / f"unregistered-{i}"
        directory.mkdir()
        subprocess.Popen(
            [sys.executable, __file__, "--leaf", str(directory)],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, close_fds=True, start_new_session=True
        )
    wait_for(
        lambda: all(
            (root / f"unregistered-{i}" / "leaf.identity.json").exists()
            and read_ticks(root / f"unregistered-{i}" / "leaf.tick") >= 2
            for i in range(UNREGISTERED_CHILDREN)
        ),
        "all detached children ready", 7,
    )
    end = time.monotonic() + 22.0
    while time.monotonic() < end and not (root / "release").exists():
        with (root / "parent.tick").open("ab") as out:
            out.write(b"x")
        time.sleep(0.065)


def noop(path):
    Path(path).write_text("DISPATCH WAS NOT BLOCKED", encoding="ascii")


def run(cli, delegate, output):
    report = {
        "scope": "transient Linux processes, one disposable native Rust Parrots desk",
        "product_code_modified": False,
        "container_enforced": False,
        "logical_admission_is_os_process_quota": False,
        "persistent_service_established": False,
        "result": "INCOMPLETE",
        "controls": {},
    }
    problem = None
    with tempfile.TemporaryDirectory(prefix="pcp-d13-admission-bound-") as tmp:
        root = Path(tmp)
        database = root / "desk.redb"
        invoke(str(cli), "init", str(database), "0", "1", "10")
        adapter = subprocess.Popen([
            str(delegate), str(database), "actual-root", "one authorized root", "--",
            sys.executable, __file__, "--parent", str(root),
        ], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
           text=True, close_fds=True, start_new_session=True)
        try:
            wait_for(
                lambda: (root / "budget-parent.identity.json").exists()
                and read_ticks(root / "parent.tick") >= 2
                and all((root / f"unregistered-{i}" / "leaf.identity.json").exists()
                        and read_ticks(root / f"unregistered-{i}" / "leaf.tick") >= 3
                        for i in range(UNREGISTERED_CHILDREN)),
                "parent and three escaped processes active", 9,
            )
            parent_id = read_actor_identity(root, "budget-parent")
            assert not has_exited(parent_id["pid"], parent_id["start_tick"])
            children = []
            for i in range(UNREGISTERED_CHILDREN):
                directory = root / f"unregistered-{i}"
                item = read_actor_identity(directory, "leaf")
                assert item["process_group"] == item["pid"]
                assert not has_exited(item["pid"], item["start_tick"])
                children.append((directory, item))
            assert len({x["pid"] for _, x in children}) == UNREGISTERED_CHILDREN
            status = json.loads(invoke(str(cli), "status", str(database)).stdout)
            assert status["max_live"] == 1 and status["max_total"] == 10
            assert status["revision"] == 1
            assert len(status["nodes"]) == 1 and status["nodes"][0]["state"] == "Active"

            # Valid positive admission check: attempting an additional ordinary
            # admitted root must be refused while the first occupies its slot.
            forbidden = root / "legitimate-dispatch-marker"
            attempted = subprocess.run([
                str(delegate), str(database), "legitimate-extra",
                "another legitimate root", "--",
                sys.executable, __file__, "--noop", str(forbidden)
            ], text=True, capture_output=True, timeout=10)
            assert attempted.returncode != 0
            assert "live node limit reached" in attempted.stderr
            assert not forbidden.exists(), "refused task was still dispatched"
            status = json.loads(invoke(str(cli), "status", str(database)).stdout)
            assert status["revision"] == 1 and len(status["nodes"]) == 1
            report["controls"]["admission_guard"] = {
                "max_live": 1,
                "max_total": 10,
                "ordinary_second_root_refused": True,
                "denial_reason": "live node limit reached",
                "refused_worker_not_launched": True,
                "revision_unchanged": True,
                "actual_parent_os_alive": True,
                "unregistered_os_children_alive": UNREGISTERED_CHILDREN,
            }

            invoke(str(cli), "cancel", str(database), "cancel-root", "1")
            out, err = adapter.communicate(timeout=10)
            assert adapter.returncode != 0, (out, err)
            wait_for(
                lambda: has_exited(parent_id["pid"], parent_id["start_tick"]),
                "direct parent's OS process exit", 3,
            )
            state = json.loads(invoke(str(cli), "status", str(database)).stdout)
            assert state["revision"] == 2
            assert len(state["nodes"]) == 1 and state["nodes"][0]["state"] == "Cancelled"
            before = [read_ticks(d / "leaf.tick") for d, _ in children]
            time.sleep(0.45)
            deltas = [read_ticks(d / "leaf.tick") - start
                      for (d, _), start in zip(children, before)]
            assert all(delta > 0 for delta in deltas)
            assert all(not has_exited(item["pid"], item["start_tick"])
                       for _, item in children)
            report["controls"]["adverse_bypass"] = {
                "desk_cancelled": True,
                "registered_parent_os_exited": True,
                "registered_nodes": 1,
                "independently_live_unregistered_children_after_cancel": UNREGISTERED_CHILDREN,
                "post_cancel_heartbeat_increments": deltas,
                "none_of_the_detached_children_had_an_admission": True,
            }
            report["result"] = "CONFIRMED_ADVERSE_BOUNDARY"
        except Exception as exc:
            problem = f"{type(exc).__name__}: {exc}"
            report["error"] = problem
        finally:
            (root / "release").touch()
            try:
                report["cleanup_parent"] = cleanup_registered_actor(
                    adapter, root, "budget-parent"
                )
            except Exception as exc:
                report["parent_cleanup_error"] = f"{type(exc).__name__}: {exc}"
                problem = problem or report["parent_cleanup_error"]
            report["cleanup_children"] = []
            for i in range(UNREGISTERED_CHILDREN):
                subdir = root / f"unregistered-{i}"
                if (subdir / "leaf.pid").exists() and \
                        (subdir / "leaf.identity.json").exists():
                    try:
                        report["cleanup_children"].append(
                            cleanup_escaped_child(subdir))
                    except Exception as exc:
                        report["cleanup_children"].append(
                            {"error": f"{type(exc).__name__}: {exc}"})
                        problem = problem or "child cleanup incomplete"
                else:
                    # No arbitrary PID is signalled from an ambiguous partial
                    # creation. The child also has a bounded life and a release.
                    report["cleanup_children"].append({
                        "created_and_identified": False
                    })
            if problem:
                report["result"] = "INCOMPLETE"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n",
                      encoding="utf-8")
    print(json.dumps(report, sort_keys=True))
    if problem:
        raise SystemExit(1)


def main():
    if len(sys.argv) > 2 and sys.argv[1] == "--parent":
        parent(sys.argv[2])
        return
    if len(sys.argv) > 2 and sys.argv[1] == "--leaf":
        leaf(sys.argv[2])
        return
    if len(sys.argv) > 2 and sys.argv[1] == "--noop":
        noop(sys.argv[2])
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--delegate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    if sys.platform != "linux":
        parser.error("Linux /proc is required")
    run(options.cli.resolve(strict=True), options.delegate.resolve(strict=True),
        options.output)


if __name__ == "__main__":
    main()
