#!/usr/bin/env python3
"""Replay bounded native register handoffs against both independent published anchors."""
from hashlib import sha256
import json
from pathlib import Path
import secrets

from virtio_tx_probe import arguments, boot, check

VECTORS = Path(__file__).resolve().parents[3] / "vectors"
MACS = bytes.fromhex("02000000000202000000000188b5")
PASS = "the isolated peer supplied the verified register extension"
BASE = "3112244aad5d5015c2d9b86a0b96e2317adcc537bc2e641d221f0622b86cb56f"
# Independent expectations for serial application outcomes; the packet carries no trusted head.
ERRORS = {
    "fork": "unexpected target",
    "guard": "guard replay",
    "truncated": "incomplete tail",
    "broken": "invalid extension",
    "old-attempt": "wrong envelope",
    "wrong-nonce": "wrong envelope",
    "extent": "wrong extent",
}


def exchange(tail, nonce, attempt, variant):
    fields = b"PCPR" + bytes([1, attempt]) + nonce
    request = (MACS + fields).ljust(60, b"\x00")
    if variant == "silent":
        return request, None
    if variant == "old-attempt":
        fields = b"PCPR" + bytes([1, 1]) + nonce
    if variant == "wrong-nonce":
        fields = fields[:-1] + bytes([fields[-1] ^ 1])
    if variant == "fork":
        tail = tail.replace(b"55:26Z", b"55:25Z")
    elif variant == "guard":
        tail = tail.replace(b'"findings":1', b'"findings":2')
    elif variant == "broken":
        tail = tail.replace(b'"seq":12', b'"seq":11')
    elif variant == "truncated":
        tail = tail[:-1]
    n = len(tail) + int(variant == "extent")
    reply = MACS[6:12] + MACS[:6] + MACS[12:] + fields + n.to_bytes(2, "big") + tail
    return request, reply


def main():
    args = arguments(__doc__)
    vector = (VECTORS / "register_fphpm_exercise.jsonl").read_bytes().splitlines(keepends=True)
    expected = json.loads((VECTORS / "register_fphpm_exercise.expected.json").read_bytes())
    check("published vector", "event count", len(vector), 13)
    check("published vector", "expected event count", expected["events"], 13)
    check("published vector", "prior head", sha256(vector[-2].removesuffix(b"\n")).hexdigest(), BASE)
    target = expected["head"]
    tail = vector[-1]
    check("published vector", "target head", sha256(tail.removesuffix(b"\n")).hexdigest(), target)

    # Every arm gets a separate boot. The two numbered attempts of each active arm
    # share a guest replica. The old-attempt arm returns the captured first envelope.
    specs = [
        ("fork-then-good", ("fork", "good")),
        ("guard-then-good", ("guard", "good")),
        ("truncated-then-good", ("truncated", "good")),
        ("bad-chain-then-good", ("broken", "good")),
        ("bad-envelope-then-good", ("wrong-nonce", "good")),
        ("good-then-repeat", ("good", "good")),
        ("fork-then-old-envelope", ("fork", "old-attempt")),
        ("two-refusals", ("extent", "guard")),
        ("silent", ("silent",)),
        ("boot-flag-off", ()),
        ("build-feature-off", ()),
    ]
    runs = []
    for name, variants in specs:
        nonce = secrets.token_bytes(8)
        # The first reply in the replay arm must be a valid captured response
        # with an unacceptable target. Send those *identical* bytes a second time.
        exchanges = [exchange(tail, nonce, i, v) for i, v in enumerate(variants, 1)]
        if name == "fork-then-old-envelope":
            exchanges[1] = (exchanges[1][0], exchanges[0][1])
        image = args.default_image if name == "build-feature-off" else args.enabled_image
        run = boot(args, name, image.resolve(), name != "boot-flag-off", True,
                   nonce=nonce, register_exchanges=exchanges)
        runs.append(run)
    (args.output / "receipt.json").write_text(json.dumps(runs, indent=2) + "\n", encoding="utf-8")

    baseline = runs[-1]["tests"]
    if not baseline or PASS in baseline:
        raise SystemExit("invalid default test baseline")
    for run, (name, variants) in zip(runs, specs):
        nonce = bytes.fromhex(run["nonce"])
        installed = False
        serial = [f"register replica: base 12 {BASE}"] if variants else []
        wire, challenges, answers = [], [], []
        for i, variant in enumerate(variants, 1):
            challenge, response = exchange(tail, nonce, i, variant)
            if variant == "old-attempt":
                response = bytes.fromhex(answers[0])
            challenges.append(challenge.hex())
            wire.append(challenge.hex())
            if response is None:
                continue
            answers.append(response.hex())
            wire.append(response.hex())
            if installed and variant == "good":
                outcome = 'Err("stale base")'
            elif variant == "good":
                outcome, installed = "Ok(())", True
            else:
                outcome = f'Err("{ERRORS[variant]}")'
            snapshot = f"13 {target}" if installed else f"12 {BASE}"
            serial.append(f"register replica: attempt {i} {outcome} {snapshot}")
        refused = bool(variants) and not installed
        result = 'Err("RX completion timeout")' if name == "silent" else 'Err("target absent")'
        if installed:
            result = "Ok(())"
        check(name, "replica snapshots", run["replica"], serial)
        check(name, "final status", run["device_result"], ["virtio register fixture: " + result] if variants else [])
        check(name, "guest challenges", run["observed"], challenges)
        check(name, "peer replies", run["peer_sent"], answers)
        check(name, "captured Ethernet frames in order", run["pcap"], wire)
        check(name, "guest exit", run["exit"], 35 if refused else 33)
        check(name, "failed tests", run["failures"], [PASS] if refused else [])
        check(name, "other passed tests", [t for t in run["tests"] if t != PASS], baseline)
        check(name, "register test pass count", run["tests"].count(PASS), int(installed))
        print(f"{name}: {len(challenges)} requests, {len(answers)} responses, "
              f"{len(run['tests'])} passed, {len(run['failures'])} failed, guest exit {run['exit']}")


if __name__ == "__main__":
    main()
