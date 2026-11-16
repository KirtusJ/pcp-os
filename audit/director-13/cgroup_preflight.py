#!/usr/bin/env python3
"""Read-only Linux containment capability preflight, not an enforcement test.

Reports only capability/permission observations in the transient test runner.
No cgroup is created, no process is moved, no policy file is written and no
capability is elevated. A plausible configuration does not establish a fence.
"""

import argparse
import json
import os
from pathlib import Path
import sys


def read_words(path):
    try:
        return path.read_text(encoding="utf-8").split()
    except (OSError, UnicodeDecodeError):
        return None


def current_membership():
    try:
        lines = Path("/proc/self/cgroup").read_text(encoding="utf-8").splitlines()
    except OSError:
        return None
    # In a unified v2 hierarchy, the 0:: entry is the cgroup path as seen
    # within the cgroup namespace. Never record its possible identity tokens.
    for line in lines:
        if line.startswith("0::"):
            return line[3:]
    return None


def mount_v2():
    try:
        lines = Path("/proc/self/mountinfo").read_text(encoding="utf-8").splitlines()
    except OSError:
        return None
    for line in lines:
        fields = line.split(" - ", 1)
        if len(fields) != 2:
            continue
        right = fields[1].split()
        left = fields[0].split()
        if len(right) >= 1 and right[0] == "cgroup2" and len(left) >= 5:
            return {"mount": left[4], "flags": left[5] if len(left) > 5 else ""}
    return None


def characterize():
    result = {
        "scope": "read-only runtime preflight on the transient Linux Actions job",
        "cgroup_enforcement_tested": False,
        "container_created": False,
        "supervisor_installed": False,
        "delegation_authorized_or_proven": False,
        "model_residency_proven": False,
        "linux": sys.platform == "linux",
    }
    if sys.platform != "linux":
        result["status"] = "NOT_LINUX"
        return result

    mount = mount_v2()
    membership = current_membership()
    result["cgroup2_mount_found"] = mount is not None
    result["unified_membership_found"] = membership is not None
    result["membership_depth"] = (
        None if membership is None else len([x for x in membership.split("/") if x])
    )
    if mount is None:
        result["status"] = "CGROUP2_NOT_EXPOSED"
        return result

    root = Path(mount["mount"])
    result["mount_read_only_by_mountinfo"] = "ro" in mount["flags"].split(",")
    try:
        result["mount_read_only_by_statvfs"] = bool(os.statvfs(root).f_flag & os.ST_RDONLY)
    except OSError:
        result["mount_read_only_by_statvfs"] = None
    # Namespace-relative membership paths can refer to a mount root that is
    # already scoped to this container. Never infer a delegated path from text.
    candidate = root / ((membership or "").lstrip("/"))
    here = candidate if candidate.is_dir() else root
    result["membership_directory_resolved"] = candidate.is_dir()
    result["self_cgroup_directory_writable_by_access"] = os.access(here, os.W_OK)
    result["subtree_control_writable_by_access"] = os.access(
        here / "cgroup.subtree_control", os.W_OK
    )
    result["available_controllers"] = sorted(
        read_words(here / "cgroup.controllers") or []
    )
    result["enabled_child_controllers"] = sorted(
        read_words(here / "cgroup.subtree_control") or []
    )
    result["control_files_present"] = {
        name: (here / name).is_file()
        for name in ("cgroup.procs", "cgroup.events", "cgroup.kill",
                     "cgroup.freeze", "pids.max", "memory.max", "cpu.max")
    }
    result["can_open_self_pidfd"] = False
    if hasattr(os, "pidfd_open"):
        try:
            descriptor = os.pidfd_open(os.getpid(), 0)
            try:
                result["can_open_self_pidfd"] = True
            finally:
                os.close(descriptor)
        except OSError:
            pass
    result["status"] = "READ_ONLY_CAPABILITY_INSPECTION"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = characterize()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n",
                           encoding="utf-8")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
