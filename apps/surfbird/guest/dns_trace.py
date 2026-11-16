"""Independently verify the DNS exchange and its ordering relative to the HTTP socket."""

import hashlib
import json
import socket
import struct
from pathlib import Path

from packet_trace import GUEST_IP, HOST_IP, check, frames, verify


def verify_dns(capture, resolver_port, destination, arm, paths, pages):
    raw = Path(capture).read_bytes()
    address = socket.inet_aton(destination)
    question = b"\x07fixture\x04test\0" + struct.pack("!HH", 1, 1)
    answer = struct.pack("!HHHIH", 0xC00C, 1, 1, 60, 4) + address
    key = None
    queries = 0
    replies = []
    terminal_reply = None
    guest_tcp = 0
    packets = list(frames(raw))
    for frame in packets:
        if frame[12:14] != b"\x08\x00":
            continue
        ip = frame[14:]
        src, dst = ip[12:16], ip[16:20]
        payload = ip[4 * (ip[0] & 15) : int.from_bytes(ip[2:4], "big")]
        if ip[9] == 6:
            check(terminal_reply == "A", "TCP traffic preceded a matching DNS answer")
            if (src, dst) == (GUEST_IP, address):
                check(payload[2:4] == b"\0P", "unexpected HTTP destination port")
                guest_tcp += 1
            else:
                check(
                    (src, dst) == (address, GUEST_IP) and payload[:2] == b"\0P",
                    "unexpected TCP flow on the isolated link",
                )
        if ip[9] != 17:
            continue
        sport, dport, size, _ = struct.unpack_from("!4H", payload)
        check(size == len(payload) and size >= 20, "invalid fixture UDP length")
        ident, flags, qd, an, ns, ar = struct.unpack_from("!6H", payload, 8)
        records = payload[20:]
        if src == GUEST_IP:
            check(terminal_reply is None, "unexpected query after terminal DNS reply")
            check(
                dst == HOST_IP and dport == resolver_port and sport >= 49152, "wrong DNS endpoint"
            )
            check(
                (flags, qd, an, ns, ar) == (0x0100, 1, 0, 0, 0) and records == question,
                "guest did not ask for fixture.test A IN",
            )
            if key is None:
                key = (ident, sport)
            check(key == (ident, sport), "query changed during retransmission")
            queries += 1
        elif dst == GUEST_IP:
            check(key and src == HOST_IP and dport == key[1], "unsolicited DNS datagram")
            if sport != resolver_port:
                replies.append("wrong port")
            elif ident != key[0]:
                replies.append("wrong ID")
            elif not records.startswith(question):
                replies.append("wrong question")
            elif (flags, qd, an, ns, ar) == (0x8503, 1, 0, 0, 0) and records == question:
                terminal_reply = "NXDOMAIN"
                replies.append("NXDOMAIN")
            elif (flags, qd, an, ns, ar) == (0x8700, 1, 1, 0, 0) and records == question + answer:
                terminal_reply = "truncated"
                replies.append("truncated")
            elif (flags, qd, an, ns, ar) == (0x8500, 1, 1, 0, 0) and records == question + answer:
                check(arm != "retry" or queries >= 2, "DNS answer arrived before the retry")
                terminal_reply = "A"
                replies.append("A")
            elif (
                (flags, qd, an) == (0x8500, 1, 1)
                and records == question + answer
                and (ns, ar) in ((1, 0), (0, 1))
            ):
                replies.append("missing authority" if ns else "missing additional")
            elif (flags, qd, an, ns, ar) == (
                0x8500,
                1,
                1,
                0,
                0,
            ) and records == question + answer + b"\xff":
                replies.append("undeclared suffix")
            else:
                wrong_owner = b"\x07another\x04test\0" + answer[2:]
                check(
                    (flags, qd, an, ns, ar) == (0x8500, 1, 1, 0, 0)
                    and records == question + wrong_owner,
                    "unrecognized fixture DNS answer",
                )
                replies.append("wrong owner")

    check(queries > 0, "guest emitted no DNS query")
    report = {
        "frames": len(packets),
        "resolver_port": resolver_port,
        "queries": queries,
        "replies": replies,
        "name": "fixture.test",
        "pcap_sha256": hashlib.sha256(raw).hexdigest(),
    }
    if arm in ("allowed", "alternate", "retry"):
        check(
            terminal_reply == "A" and guest_tcp and replies == ["A"],
            "missing DNS-to-HTTP navigation",
        )
        report.update(verify(capture, 80, paths, "allowed", pages, address))
    else:
        wanted = {
            "nxdomain": {"NXDOMAIN"},
            "truncated": {"truncated"},
            "mismatch": {"wrong ID", "wrong question", "wrong owner", "wrong port"},
            "malformed": {"missing authority", "missing additional", "undeclared suffix"},
        }[arm]
        check(
            not guest_tcp and terminal_reply != "A" and set(replies) == wanted,
            "negative DNS case differs",
        )
    return report


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("fixture", type=Path)
    args = parser.parse_args()
    case = json.loads(args.fixture.read_text(encoding="utf-8"))
    report = verify_dns(
        args.capture,
        case["resolver_port"],
        case["address"],
        case["arm"],
        case["paths"],
        case["pages"],
    )
    check(report["pcap_sha256"] == case["pcap_sha256"], "capture digest differs from fixture")
    print(json.dumps(report, indent=2))
