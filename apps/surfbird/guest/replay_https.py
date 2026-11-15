"""Run one opt-in guest HTTPS request using ordinary QEMU NAT and preserve the encrypted trace."""

import argparse
import hashlib
import ipaddress
import json
from pathlib import Path
import secrets
import shutil
import struct
import subprocess
import sys
from packet_trace import GUEST_IP, GUEST_MAC, frames


def require(ok, reason):
    if not ok:
        raise RuntimeError(reason)


def dns_name(message, start):
    labels, visited = [], set()
    at, end = start, None
    while True:
        require(at < len(message) and at not in visited, "invalid DNS name")
        visited.add(at)
        n = message[at]
        if n & 0xC0 == 0xC0:
            require(at + 1 < len(message), "truncated DNS pointer")
            if end is None:
                end = at + 2
            at = ((n & 63) << 8) | message[at + 1]
            continue
        require(n <= 63 and at + 1 + n <= len(message), "invalid DNS label")
        at += 1
        if n == 0:
            return b".".join(labels).lower(), end if end is not None else at
        labels.append(message[at : at + n])
        at += n


def dns_addresses(message, question):
    flags, questions, answers = struct.unpack_from("!HHH", message, 2)
    require(flags & 0x800F == 0x8000 and questions == 1, "invalid DNS response")
    name, at = dns_name(message, 12)
    require(
        name == question and message[at : at + 4] == b"\0\1\0\1", "wrong DNS question"
    )
    at += 4
    addresses, aliases = {}, {}
    for _ in range(answers):
        owner, at = dns_name(message, at)
        kind, klass, _, length = struct.unpack_from("!HHIH", message, at)
        at += 10
        require(at + length <= len(message), "truncated DNS answer")
        if kind == 1 and klass == 1 and length == 4:
            addresses.setdefault(owner, set()).add(message[at : at + 4])
        elif kind == 5 and klass == 1:
            alias, terminal = dns_name(message, at)
            require(terminal == at + length, "wrong CNAME length")
            aliases[owner] = alias
        at += length
    seen = set()
    while name in aliases:
        require(name not in seen, "CNAME loop")
        seen.add(name)
        name = aliases[name]
    return addresses.get(name, set())


def packet_evidence(path):
    resolver = bytes([10, 0, 2, 3])
    question = b"www.youtube.com"
    pending, answers, syn = set(), set(), set()
    received = sent = 0
    for packet in frames(path.read_bytes()):
        if packet[12:14] != b"\x08\x00" or len(packet) < 34:
            continue
        ip = packet[14:]
        payload = ip[(ip[0] & 15) * 4 : int.from_bytes(ip[2:4], "big")]
        src_ip, dst_ip = ip[12:16], ip[16:20]
        if ip[9] == 17 and len(payload) >= 20:
            sport, dport, size, _ = struct.unpack_from("!HHHH", payload)
            require(size == len(payload), "UDP length mismatch")
            msg = payload[8:]
            if (src_ip, dst_ip, dport) == (GUEST_IP, resolver, 53):
                require(packet[6:12] == GUEST_MAC, "unexpected DNS source MAC")
                name, at = dns_name(msg, 12)
                require(
                    name == question
                    and msg[at : at + 4] == b"\0\1\0\1"
                    and msg[2] & 128 == 0,
                    "wrong outgoing DNS question",
                )
                pending.add((sport, msg[:2]))
            elif (src_ip, dst_ip, sport) == (resolver, GUEST_IP, 53):
                require((dport, msg[:2]) in pending, "unmatched DNS answer")
                answers.update(dns_addresses(msg, question))
        if ip[9] == 6 and len(payload) >= 20:
            src, dst = struct.unpack_from("!HH", payload)
            data = payload[(payload[12] >> 4) * 4 :]
            if dst == 443:
                require(
                    src_ip == GUEST_IP
                    and packet[6:12] == GUEST_MAC
                    and dst_ip in answers,
                    "TCP connection did not follow its own DNS answer",
                )
                if payload[13] & 2:
                    syn.add(str(ipaddress.ip_address(ip[16:20])))
                sent += len(data)
            if src == 443:
                require(
                    dst_ip == GUEST_IP
                    and packet[:6] == GUEST_MAC
                    and str(ipaddress.ip_address(src_ip)) in syn,
                    "unexpected response flow",
                )
                received += len(data)
    require(
        pending
        and len(syn) == 1
        and all(ipaddress.ip_address(a).is_global for a in syn),
        "guest public DNS/TCP path missing",
    )
    require(sent > 100 and received > 100, "TLS exchange missing")
    return {
        "public_destination": syn.pop(),
        "outgoing_tcp_payload_bytes_including_retransmission": sent,
        "incoming_tcp_payload_bytes_including_retransmission": received,
    }


def run(a):
    work = a.output.resolve()
    work.mkdir(parents=True, exist_ok=False)
    for source, name in [(a.guest, "surf.elf"), (a.kernel, "kernel.elf32")]:
        shutil.copyfile(source, work / name)
    pin = hashlib.sha256(a.guest.read_bytes()).hexdigest()
    tool = (
        Path(__file__).resolve().parents[3]
        / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
    )
    measure = subprocess.check_output(
        [sys.executable, str(tool), str(a.kernel)], text=True
    ).strip()
    (work / "net.cfg").write_bytes((443).to_bytes(2, "big") + secrets.token_bytes(10))
    (work / "tls.cfg").write_bytes(
        bytes([["watch", "wrong-host", "untrusted"].index(a.arm)])
    )
    cmd = [a.qemu]
    if a.firmware:
        cmd += ["-L", str(a.firmware.resolve())]
    cmd += [
        "-cpu",
        "max,vendor=GenuineIntel,rdrand=on",
        "-accel",
        "tcg",
        "-m",
        "256M",
        "-display",
        "none",
        "-no-reboot",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-device",
        "AC97,audiodev=silent",
        "-audiodev",
        "none,id=silent",
        "-serial",
        "file:serial.log",
        "-monitor",
        "none",
        "-netdev",
        "user,id=public,ipv6=off",
        "-device",
        "virtio-net-pci,netdev=public,disable-modern=on,vectors=0,mac=02:00:00:00:00:01",
        "-object",
        "filter-dump,id=trace,netdev=public,file=guest.pcap",
        "-kernel",
        "kernel.elf32",
        "-initrd",
        "surf.elf,net.cfg,tls.cfg",
        "-append",
        f"auto=surf.elf then=off image={measure} net_image={pin}",
    ]
    with (work / "stderr.log").open("wb") as err:
        result = subprocess.run(
            cmd, cwd=work, stdout=subprocess.DEVNULL, stderr=err, timeout=150
        )
    serial = (work / "serial.log").read_text(errors="replace")
    require(
        "it is in SAFE" not in serial,
        "kernel refused to start its guest after a self-test failure",
    )
    require("starting surf.elf" in serial, "guest did not launch")
    pkt = packet_evidence(work / "guest.pcap")
    lines = [s for s in serial.splitlines() if s.startswith("SURFBIRD ")]
    print(
        json.dumps({"guest": pin, "arm": a.arm, "observed": lines, **pkt}), flush=True
    )
    require(
        "kernel's register of its own start, verification and stop verifies" in serial,
        "boot register",
    )
    if a.arm == "watch":
        require(
            any(line.startswith("SURFBIRD HTTPS HTTP/1.1 200 ") for line in lines),
            "expected authenticated HTTP 200",
        )
        require(
            "SURFBIRD certificate chain and hostname verified" in lines
            and "SURFBIRD authenticated HTML body prefix received" in lines,
            "missing authenticated HTML",
        )
        require(
            "surf.elf ended DONE: exit 0" in serial and result.returncode == 33,
            "guest exit",
        )
    else:
        reason = (
            "TLS hostname mismatch" if a.arm == "wrong-host" else "TLS unknown issuer"
        )
        require(
            "SURFBIRD error " + reason in lines
            and "surf.elf ended FAILED: exit 1" in serial,
            "wrong certificate failure",
        )
        require(not any("HTTPS HTTP/" in s for s in lines), "unauthenticated response")
    receipt = {
        "guest_sha256": pin,
        "kernel_measure": measure,
        "arm": a.arm,
        "serial": lines,
        "pcap_sha256": hashlib.sha256((work / "guest.pcap").read_bytes()).hexdigest(),
        **pkt,
    }
    (work / "result.json").write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("kernel", type=Path)
    p.add_argument("guest", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--qemu", default="qemu-system-x86_64")
    p.add_argument("--firmware", type=Path)
    p.add_argument(
        "--arm", choices=("watch", "wrong-host", "untrusted"), default="watch"
    )
    run(p.parse_args())
