"""Check QEMU's classic Ethernet pcap independently of the guest and HTTP fixture logs."""

import hashlib
import json
import re
import socket
import struct
from pathlib import Path

GUEST_IP = socket.inet_aton("10.0.2.15")
HOST_IP = socket.inet_aton("10.0.2.2")
GUEST_MAC = bytes.fromhex("020000000001")


def check(ok, reason):
    if not ok:
        raise ValueError(f"guest packet trace: {reason}")


def frames(raw):
    endian = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">"}[raw[:4]]
    check(struct.unpack_from(endian + "I", raw, 20)[0] == 1, "expected Ethernet pcap")
    at = 24
    while at < len(raw):
        captured, length = struct.unpack_from(endian + "II", raw, at + 8)
        at += 16
        check(14 <= captured == length <= len(raw) - at, "truncated frame")
        yield raw[at : at + length]
        at += length


def stream_bytes(segments, initial_seq):
    stream = bytearray()
    for seq, data in sorted(segments, key=lambda p: (p[0] - initial_seq) % 2**32):
        start = (seq - initial_seq) % 2**32
        check(start <= len(stream), "gap in TCP stream")
        overlap = min(len(data), len(stream) - start)
        check(stream[start : start + overlap] == data[:overlap], "conflicting retransmission")
        stream.extend(data[overlap:])
    return bytes(stream)


def verify(capture, port, expected_paths, scenario, bodies, host_ip=HOST_IP):
    raw = Path(capture).read_bytes()
    packets = list(frames(raw))
    arp = []
    tcp = []
    for frame in packets:
        src, kind = frame[6:12], frame[12:14]
        if kind == b"\x08\x06" and len(frame) >= 42:
            arp.append((src, int.from_bytes(frame[20:22], "big"), frame[28:32], frame[38:42]))
        if kind != b"\x08\x00":
            continue
        ip = frame[14:]
        if len(ip) < 20 or ip[9] != 6:
            continue
        header, total = 4 * (ip[0] & 15), int.from_bytes(ip[2:4], "big")
        check(ip[0] >> 4 == 4 and 20 <= header <= total <= len(ip), "invalid IPv4 bounds")
        s = ip[header:total]
        check(len(s) >= 20 and 20 <= 4 * (s[12] >> 4) <= len(s), "invalid TCP bounds")
        sp, dp, seq, ack = struct.unpack_from("!HHII", s)
        outgoing = src == GUEST_MAC and ip[12:20] == GUEST_IP + host_ip and dp == port
        incoming = frame[:6] == GUEST_MAC and ip[12:20] == host_ip + GUEST_IP and sp == port
        if outgoing or incoming:
            tcp.append((outgoing, sp if outgoing else dp, seq, ack, s[13], s[4 * (s[12] >> 4) :]))
    evidence = {"frames": len(packets), "pcap_sha256": hashlib.sha256(raw).hexdigest()}
    if scenario == "denied":
        # The isolated denial fixture has no background traffic. A raw writer chooses its MAC.
        check(not packets, "expected an empty capture on the denied arm")
        return evidence
    check(
        (GUEST_MAC, 1, GUEST_IP, host_ip) in arp
        and any(op == 2 and sip == host_ip and tip == GUEST_IP for _, op, sip, tip in arp),
        "missing ARP exchange",
    )
    check(tcp and tcp[0][0] and tcp[0][4] & 0x17 == 2, "first TCP segment must be guest SYN")
    _, local, syn, _, _, _ = tcp[0]
    check(all(p[1] == local for p in tcp), "more than one client socket")
    evidence.update({"guest": f"10.0.2.15:{local}", "destination": f"{socket.inet_ntoa(host_ip)}:{port}"})
    if scenario == "refused":
        reset = False
        for out, _, seq, ack, flags, _ in tcp:
            if out:
                check(seq == syn and flags == 2, "unexpected segment after SYN on closed port")
            else:
                check(
                    flags & 0x17 == 0x14 and ack == (syn + 1) % 2**32,
                    "invalid reset in response to SYN on closed port",
                )
                reset = True
        check(not any(p[-1] for p in tcp), "unexpected TCP payload on refused port")
        return dict(evidence, reset=reset, syn_attempts=sum(p[0] and p[4] == 2 for p in tcp))
    reply = next(
        (
            i
            for i, p in enumerate(tcp)
            if not p[0] and p[4] & 0x17 == 0x12 and p[3] == (syn + 1) % 2**32
        ),
        None,
    )
    check(reply is not None, "missing matching SYN-ACK")
    server_seq = (tcp[reply][2] + 1) % 2**32
    established = next(
        (
            i
            for i in range(reply + 1, len(tcp))
            if tcp[i][0]
            and tcp[i][4] & 0x16 == 0x10
            and tcp[i][2] == (syn + 1) % 2**32
            and tcp[i][3] == server_seq
        ),
        None,
    )
    check(established is not None, "missing final handshake ACK")

    def payload(out, initial):
        return stream_bytes(
            [
                (seq, part)
                for direct, _, seq, _, _, part in tcp[established:]
                if direct == out and part
            ],
            initial,
        )

    requests = payload(True, (syn + 1) % 2**32)
    responses = payload(False, server_seq)
    paths = [
        p.decode("ascii") for p in re.findall(rb"GET (/[^ \r\n#]*) HTTP/1\.1\r\n", requests)
    ]
    check(paths == expected_paths, f"GET paths {paths!r}")
    expected_stream = b"".join(
        f"GET {path} HTTP/1.1\r\nHost: fixture.test\r\nAccept: text/html\r\n\r\n".encode("ascii")
        for path in expected_paths
    )
    check(requests == expected_stream, "extra or malformed HTTP request bytes")
    if scenario in ("allowed", "fragment"):
        # Match each entire body with its length and maintain response order.
        rest = responses
        for path in paths:
            head, sep, rest = rest.partition(b"\r\n\r\n")
            body = bodies[path].encode("ascii")
            check(
                sep
                and head.startswith(b"HTTP/1.1 200 OK\r\n")
                and f"Content-Length: {len(body)}".encode() in head.split(b"\r\n")
                and rest.startswith(body),
                "response body mismatch",
            )
            rest = rest[len(body) :]
        check(not rest, "unexpected response bytes")
        check(any(out and flags & 1 for out, _, _, _, flags, _ in tcp), "guest did not send FIN")
    evidence.update(
        {"handshake": "SYN/SYN-ACK/ACK", "get_paths": paths, "http_reply_bytes": len(responses)}
    )
    return evidence


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(
        description="Verify a saved navigation capture against its fixture."
    )
    parser.add_argument("capture", type=Path)
    parser.add_argument("fixture", type=Path)
    args = parser.parse_args()
    expected = json.loads(args.fixture.read_text(encoding="utf-8"))
    report = verify(args.capture, expected["port"], expected["paths"], "allowed", expected["pages"])
    check(
        report["pcap_sha256"] == expected["pcap_sha256"],
        "capture digest differs from fixture record",
    )
    print(json.dumps(report, indent=2))
