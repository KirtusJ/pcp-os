#!/usr/bin/env python3
"""Verify the recorded evidence for one completed resident-worker round."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
from collections import deque
from uuid import UUID

from supervise import PARENTS, SERIAL_LIMIT, check_start, parse_oracle, require, sha

NODES = range(1, len(PARENTS))


def integer(value, label, minimum=0):
    require(type(value) is int and value >= minimum, f"invalid {label}")
    return value


def node_id(event):
    node = integer(event["node"], "node", 1)
    require(node in NODES, "node outside the requested tree")
    return node


def read_prefix(run, name, limit, node):
    """Hash bytes on disk, retaining only the boot and most recent challenge."""
    digest = hashlib.sha256()
    pending = b""
    boot = bytearray()
    ready = f"READY {node}\n".encode()
    started = False
    last = deque(maxlen=6)
    boot_end = 0
    with (run / name).open("rb") as serial:
        while limit:
            chunk = serial.read(min(limit, 64 * 1024))
            require(chunk, f"{name}: truncated serial prefix")
            digest.update(chunk)
            limit -= len(chunk)
            lines = (pending + chunk).split(b"\n")
            pending = lines.pop()
            require(len(pending) <= SERIAL_LIMIT, f"{name}: oversized serial line")
            for raw in lines:
                line = raw.removesuffix(b"\r") + b"\n"
                if not started:
                    boot.extend(line)
                    require(len(boot) <= SERIAL_LIMIT, f"{name}: oversized boot record")
                    boot_end += len(raw) + 1
                    started = line == ready
                else:
                    last.append((line, len(raw) + 1))
    require(started and not pending, f"{name}: incomplete serial prefix")
    final_lines = list(last)[-3:]
    last_size = sum(size for _, size in final_lines)
    reply = b"".join(line for line, _ in final_lines).decode("ascii")
    if len(last) == 6:
        require(last[0][0] != last[3][0], f"{name}: repeated consecutive challenge")
    return digest.hexdigest(), boot.decode("ascii"), reply, boot_end, last_size


def check_row(run, row, node, active, config, oracle, previous):
    a = active[node]
    parent = PARENTS[node]
    require(node_id(row) == node and integer(row["parent"], "parent") == parent and
            row["uuid"] == a["uuid"], f"node {node}: wrong round identity or parent")
    name = f"{node}-{a['attempt']}.serial"
    require(row.get("serial_file") == name and "serial_bytes" in row,
            f"node {node}: missing or inconsistent per-attempt serial evidence")
    prompt = integer(row["prompt"], "prompt")
    require(prompt < len(oracle) and row["tokens"] == oracle[prompt] and
            all(type(token) is int for token in row["tokens"]), f"node {node}: oracle mismatch")
    end = integer(row["serial_bytes"], "serial length", 1)
    previous_end = previous.get(row["uuid"], 0)
    digest, boot, reply, boot_end, reply_size = read_prefix(run, name, end, node)
    require(end - reply_size >= max(boot_end, previous_end), f"node {node}: reused serial prefix")
    require(digest == row["serial_sha256"], f"{name}: serial digest mismatch")
    incoming = active[parent]["ticket"] if parent else config["root_input"]
    require(integer(a["input"], "incoming ticket", 1) == incoming, f"{name}: wrong incoming ticket")
    ticket, children = check_start(node, boot, parent, incoming, config["salt"],
                                   config["measurement"], config["worker_sha256"], oracle)
    require(integer(a["ticket"], "ticket") == ticket and a["children"] == children and
            all(type(child) is int for child in a["children"]), f"{name}: dispatch mismatch")
    tokens = " ".join(map(str, oracle[prompt]))
    require(reply == f"CHALLENGE {prompt}\nLM tokens {tokens}\nREADY {node}\n", f"{name}: wrong reply")


def verify(run, oracle_path, cycle):
    integer(cycle, "cycle", 1)
    oracle_bytes = oracle_path.read_bytes()
    oracle = parse_oracle(oracle_bytes)
    active, attempts, previous = {}, {}, {}
    seen = set()
    prior = repairs = 0
    with (run / "events.jsonl").open("rb") as events:
        first = events.readline()
        require(first.endswith(b"\n"), "missing complete input record")
        config = json.loads(first)
        require(config["event"] == "inputs" and config["oracle_sha256"] == sha(oracle_bytes),
                "input record or oracle digest mismatch")
        for key in ("measurement", "worker_sha256", "kernel_sha256"):
            require(re.fullmatch("[0-9a-f]{64}", config[key]), f"invalid {key}")
        require(integer(config["salt"], "salt") < 16, "invalid salt")
        integer(config["root_input"], "root input", 1)
        for line in events:
            if not line.endswith(b"\n"):
                break
            event = json.loads(line)
            kind = event["event"]
            if kind == "interrupted":
                node = node_id(event)
                if node in active:
                    active[node]["interrupted"] = True
                repairs += 1
            elif kind == "retired":
                node = node_id(event)
                require(node in active and active[node]["uuid"] == event["uuid"] and
                        not any(PARENTS[child] == node for child in active),
                        f"node {node}: invalid retirement")
                del active[node]
            elif kind == "admitted":
                node = node_id(event)
                parent = PARENTS[node]
                require(node not in active and integer(event["parent"], "parent") == parent and
                        (not parent or parent in active and not active[parent].get("interrupted")),
                        f"node {node}: invalid admission order")
                uid = event["uuid"]
                require(isinstance(uid, str) and str(UUID(uid)) == uid and uid not in seen,
                        f"node {node}: invalid or reused UUID")
                seen.add(uid)
                attempt = integer(event["attempt"], "attempt", 1)
                require(attempt > attempts.get(node, 0), f"node {node}: reused attempt")
                attempts[node] = attempt
                active[node] = dict(event, interrupted=False)
            elif kind == "readback":
                require(isinstance(event.get("record"), dict), "full historical round record unavailable")
                record = event["record"]
                require(integer(event["cycle"], "cycle") == prior + 1 and
                        integer(record["cycle"], "record cycle") == event["cycle"], "round sequence mismatch")
                require(integer(record["repairs"], "repairs") == repairs, "repair count mismatch")
                require(len(active) == len(NODES) and not any(a.get("interrupted") for a in active.values()),
                        "incomplete cohort at readback")
                digest = sha((json.dumps(record, indent=2) + "\n").encode())
                require(digest == event["sha256"], "round digest mismatch")
                rows = record["nodes"]
                require(isinstance(rows, list) and len(rows) == len(NODES), "incomplete round")
                for node, row in zip(NODES, rows):
                    require(node_id(row) == node and row["uuid"] == active[node]["uuid"],
                            f"node {node}: recorded identity mismatch")
                    if event["cycle"] == cycle:
                        check_row(run, row, node, active, config, oracle, previous)
                    previous[row["uuid"]] = integer(row["serial_bytes"], "serial length", 1)
                if event["cycle"] == cycle:
                    return {"cycle": cycle, "time": record["time"], "sha256": digest,
                            "root_uuid": active[1]["uuid"], "guests": len(rows),
                            "edges": len(rows) - 1, "repairs": repairs}
                prior += 1
            elif kind in ("finished", "failed", "stopped"):
                break
            else:
                raise ValueError(f"unexpected event {kind!r}")
    raise ValueError(f"completed round {cycle} unavailable")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--oracle", type=Path, required=True)
    parser.add_argument("--cycle", type=int, required=True)
    args = parser.parse_args()
    try:
        result = verify(args.run, args.oracle, args.cycle)
    except (OSError, KeyError, ValueError, TypeError, RuntimeError) as error:
        print(f"readback verification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
