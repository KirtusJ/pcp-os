"""Collect each native written response on the existing guest control channel."""
import json
import time

def collect(machines, run, reference_path, cycle):
    cases = json.loads(reference_path.read_bytes())
    rows = []
    for node in range(1, 25):
        guest = machines[node][0]
        guest.live()
        start = guest.position
        if guest.serial.stat().st_size != start:
            raise RuntimeError(f"unsolicited serial before written challenge for node {node}")
        guest.command("send-key", keys=[{"type": "qcode", "data": "e"}], **{"hold-time": 25})
        reply = guest.through_ready(time.monotonic() + guest.response_timeout)
        lines = reply.splitlines()
        if len(lines) != 5 or lines[-1] != f"READY {node}":
            raise RuntimeError(f"incomplete written response from node {node}")
        case = cases[0 if node == 1 else 1 if node <= 4 else 2 if node <= 12 else 3]
        inp = "INPUT " + " ".join(map(str, case["input_ids"]))
        output = "OUTPUT " + " ".join(map(str, case["output_ids"]))
        if lines[:4] != ["EXCERPT " + case["prompt"], inp, output, "STOP " + case["stop"]]:
            raise RuntimeError(f"node {node}: written frame disagrees with independent reference")
        rows.append(dict(node=node, uuid=guest.uuid, serial_file=guest.serial.name, start=start,
                         end=guest.position, sha256=guest.serial_digest.hexdigest(), **case))
    for guest, _, _ in machines.values():
        guest.live()
    (run / "written-round.json").write_text(json.dumps(dict(cycle=cycle, nodes=rows), indent=2) + "\n")
