#!/usr/bin/env python3
"""Check one native virtio challenge/reply using guest serial, a peer and Ethernet capture."""
import json
import secrets

from virtio_tx_probe import FRAME, arguments, boot, check


REPLY = FRAME[6:12] + FRAME[:6] + FRAME[12:14] + b"PCP-OS virtio RX v1!" + bytes(26)
PASS = "the isolated virtio-net RX queue received the exact reply"


def main():
    args = arguments(__doc__)
    enabled, default = args.enabled_image.resolve(), args.default_image.resolve()
    # Every started guest receives a fresh host-selected 64-bit challenge. The peer
    # answers only a byte-exact request; its answer echoes the challenge on the wire.
    cases = []
    runs = []
    for name, image, flag, outcome in [
        ("reply", enabled, True, "Ok(())"),
        ("boot-flag-off", enabled, False, None),
        ("build-feature-off", default, True, None),
        ("silent-peer", enabled, True, 'Err("RX completion timeout")'),
        ("wrong-reply", enabled, True, 'Err("RX payload mismatch")'),
        ("wrong-nonce", enabled, True, 'Err("RX nonce mismatch")'),
        ("long-reply", enabled, True, 'Err("RX used length")'),
    ]:
        nonce = secrets.token_bytes(8)
        challenge = FRAME[:-8] + nonce
        reply = REPLY[:-8] + nonce
        if name == "silent-peer":
            reply = None
        elif name == "wrong-reply":
            reply = reply[:51] + bytes([reply[51] ^ 1]) + reply[52:]
        elif name == "wrong-nonce":
            reply = reply[:-1] + bytes([reply[-1] ^ 1])
        elif name == "long-reply":
            reply += b"\x01"
        cases.append((name, challenge, reply, outcome))
        runs.append(boot(args, name, image, flag, True, reply_probe=True, reply=reply, nonce=nonce))
    (args.output / "receipt.json").write_text(json.dumps(runs, indent=2) + "\n", encoding="utf-8")
    baseline = next(run["tests"] for run in runs if run["name"] == "build-feature-off")
    if not baseline or PASS in baseline:
        raise SystemExit("build-feature-off: missing or contaminated baseline self-tests")
    device = "device on the bus: 00:06.0 maker 1af4 device 1000 class 02.00"
    for run, (name, challenge, reply, outcome) in zip(runs, cases):
        started = outcome is not None
        success = name == "reply"
        refused = started and not success
        expected_out = [challenge.hex()] if started else []
        expected_in = [reply.hex()] if started and reply is not None else []
        check(name, "guest diagnostic", run["device_result"],
              ["virtio reply fixture: " + outcome] if started else [])
        check(name, "challenge observed by peer", run["observed"], expected_out)
        check(name, "answer sent by peer", run["peer_sent"], expected_in)
        check(name, "Ethernet capture in wire order", run["pcap"], expected_out + expected_in)
        check(name, "guest exit", run["exit"], 35 if refused else 33)
        check(name, "failed tests", run["failures"], [PASS] if refused else [])
        check(name, "other passed tests", [test for test in run["tests"] if test != PASS], baseline)
        check(name, "RX test pass count", run["tests"].count(PASS), int(success))
        check(name, "enumerated Ethernet devices",
              [line.partition(" line ")[0] for line in run["guest_pci"]], [device])
        print(f"{name}: {len(run['tests'])} passed, {len(run['failures'])} failed, "
              f"{len(run['pcap'])} Ethernet frames, guest exit {run['exit']}")


if __name__ == "__main__":
    main()
