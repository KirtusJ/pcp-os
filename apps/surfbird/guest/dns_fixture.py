"""Local authoritative UDP fixture for the reserved name fixture.test."""

from contextlib import contextmanager
import secrets
import socket
import socketserver
import struct
import threading

QUESTION = b"\x07fixture\x04test\x00\x00\x01\x00\x01"
DNS_ARMS = ("allowed", "alternate", "retry", "nxdomain", "truncated", "mismatch", "malformed")
DNS_ERRORS = {
    "nxdomain": "DNS name does not exist",
    "truncated": "DNS truncated response",
    "mismatch": "DNS timeout",
    "malformed": "DNS timeout",
}


@contextmanager
def resolver(scenario, port):
    # Disjoint ranges force two different answers for the same ELF and hostname.
    last = (140 if scenario == "alternate" else 40) + secrets.randbelow(40)
    address = f"10.0.2.{last}"
    seen = []

    class Handler(socketserver.BaseRequestHandler):
        def handle(self):
            data, sock = self.request
            if (
                len(data) != 12 + len(QUESTION)
                or data[2:12] != bytes.fromhex("0100 0001 0000 0000 0000")
                or data[12:] != QUESTION
            ):
                return
            seen.append(data[:2])
            if scenario == "retry" and len(seen) == 1:
                return
            ident = int.from_bytes(data[:2], "big")
            record = bytes.fromhex("c00c 0001 0001 0000003c 0004") + socket.inet_aton(address)

            def reply(flags, question=QUESTION, body=b"", txid=ident, sender=sock, ns=0, ar=0):
                head = struct.pack("!6H", txid, flags, 1, int(bool(body)), ns, ar)
                sender.sendto(head + question + body, self.client_address)

            if scenario == "nxdomain":
                reply(0x8503)
            elif scenario == "truncated":
                reply(0x8700, body=record)
            elif scenario == "malformed":
                reply(0x8500, body=record, ns=1)
                reply(0x8500, body=record, ar=1)
                reply(0x8500, body=record + b"\xff")
            elif scenario == "mismatch":
                reply(0x8500, body=record, txid=ident ^ 1)
                reply(0x8503, question=QUESTION.replace(b"fixture", b"another"))
                reply(0x8500, body=b"\x07another\x04test\x00" + record[2:])
                with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as wrong_port:
                    wrong_port.bind(("127.0.0.1", 0))
                    reply(0x8500, body=record, sender=wrong_port)
            else:
                reply(0x8500, body=record)

    with socketserver.UDPServer(("127.0.0.1", port), Handler) as server:
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            yield (server.server_address[1], address)
        finally:
            server.shutdown()
            worker.join()
