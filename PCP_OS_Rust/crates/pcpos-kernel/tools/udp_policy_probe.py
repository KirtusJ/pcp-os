"""Replay a pinned guest UDP rule against a private loopback Ethernet peer."""
import argparse
import hashlib
import ipaddress
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time

LOCAL = ("192.0.2.2", 50001, bytes.fromhex("020000000001"))
PEER = ("198.51.100.99", 53, bytes.fromhex("020000000002"))
POLICY = "net_udp=192.0.2.2:50001,198.51.100.99:53,020000000002"


def checksum(data):
    if len(data) % 2:
        data += b"\0"
    total = sum(x[0] for x in struct.iter_unpack("!H", data))
    while total >> 16:
        total = (total & 65535) + (total >> 16)
    return (~total) & 65535


def datagram(source, destination, body, fragment=0x4000):
    src, sport, smac = source
    dst, dport, dmac = destination
    addresses = ipaddress.IPv4Address(src).packed + ipaddress.IPv4Address(dst).packed
    udp = bytearray(struct.pack("!4H", sport, dport, 8 + len(body), 0) + body)
    seed = addresses + struct.pack("!BBH", 0, 17, len(udp))
    udp[6:8] = struct.pack("!H", checksum(seed + udp) or 65535)
    ip = bytearray(struct.pack("!BBHHHBBH", 0x45, 0, 20 + len(udp), 23, fragment, 64, 17, 0) + addresses)
    ip[10:12] = struct.pack("!H", checksum(ip))
    return (dmac + smac + b"\x08\x00" + ip + udp).ljust(60, b"\0")


def changed(frame, offset, value):
    return frame[:offset] + value + frame[offset + len(value):]


def ip_field(frame, offset, value):
    updated = changed(changed(frame, 14 + offset, value), 24, b"\0\0")
    return changed(updated, 24, struct.pack("!H", checksum(updated[14:34])))


def fixtures():
    allowed = datagram(LOCAL, PEER, b"OUTBOUND")
    blocked = [
        datagram(LOCAL, (PEER[0], 31337, PEER[2]), b"BAD-PORT"),
        datagram(("192.0.2.3", LOCAL[1], LOCAL[2]), PEER, b"BAD-SRC"),
        datagram(LOCAL, ("203.0.113.77", PEER[1], PEER[2]), b"BAD-DST"),
        datagram(LOCAL, PEER, b"FIRST", 0x2000),
        datagram(LOCAL, PEER, b"LATER", 1),
        changed(allowed, 12, b"\x81\x00"),  # VLAN
        changed(allowed, 12, b"\x86\xdd"),  # IPv6
        changed(allowed, 12, b"\x88\xb5"),  # kernel management EtherType
        changed(allowed, 12, b"\x08\x06"),  # ARP
        changed(allowed, 0, b"\xff" * 6),
        changed(allowed, 6, b"\x02\x00\x00\x00\x00\xfe"),
        changed(allowed, 14, b"\x46"),       # options/IHL
        ip_field(allowed, 2, b"\x00\xff"),    # truncated IP datagram
        ip_field(allowed, 8, b"\x00"),        # expired TTL
        ip_field(allowed, 9, b"\x06"),        # TCP protocol
        changed(allowed, 24, b"\x00\x00"),   # IP checksum
        changed(allowed, 38, b"\x00\x08"),   # UDP/IP length disagreement
        changed(allowed, 40, b"\x00\x01"),   # UDP checksum
        changed(allowed, 59, b"X"),          # covert nonzero Ethernet padding
    ]
    return allowed, blocked


def compile_probe(root, args, allowed, blocked):
    (root / "permitted.bin").write_bytes(allowed)
    (root / "rejected.bin").write_bytes(b"".join(blocked))
    source = Path(__file__).resolve().parents[1] / "programs/udp-policy/probe.S"
    subprocess.run([args.clang, "--target=x86_64-unknown-linux-gnu", f"-DREJECT_COUNT={len(blocked)}",
                    "-c", str(source), "-o", "probe.o"], cwd=root, check=True)
    subprocess.run([args.objcopy, "-j", ".text", "-O", "binary", "probe.o", "probe.raw"],
                   cwd=root, check=True)
    code = (root / "probe.raw").read_bytes()
    size = 120 + len(code)
    elf = struct.pack("<16sHHIQQQIHHHHHH", b"\x7fELF\2\1\1", 3, 62, 1, 120, 64, 0, 0, 64, 56, 1, 0, 0, 0)
    elf += struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, size, size, 4096) + code
    (root / "raw.elf").write_bytes(elf)
    return hashlib.sha256(elf).hexdigest()


def frames(path):
    raw = path.read_bytes()
    order = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">"}[raw[:4]]
    if struct.unpack_from(order + "I", raw, 20)[0] != 1:
        raise ValueError("capture must contain Ethernet")
    i, result = 24, []
    while i < len(raw):
        _, _, captured, wire = struct.unpack_from(order + "4I", raw, i)
        i += 16
        if wire != captured or i + captured > len(raw):
            raise ValueError("truncated capture")
        result.append(raw[i:i+captured])
        i += captured
    return result


def run_arm(root, args, arm, option, pin, allowed, blocked):
    early = datagram(PEER, LOCAL, b"PRIORUDP")
    replies = [datagram(("203.0.113.77", 44111, PEER[2]), LOCAL, b"NO-FLOW"),
               datagram(PEER, LOCAL, b"FRAGMENT", 0x2000), datagram(PEER, LOCAL, b"VALIDUDP")]
    replies.append(replies[-1])
    peer_frames = []
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind(("127.0.0.1", 0))
        sock.settimeout(.05)
        stop = threading.Event()
        log, capture = root / (arm+".serial"), root / (arm+".pcap")
        guest_port = 0
        if arm == "restricted":
            # The peer must address QEMU before it receives the first guest frame.
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as reserve:
                reserve.bind(("127.0.0.1", 0))
                guest_port = reserve.getsockname()[1]

        def peer():
            queued = False
            while not stop.is_set():
                if guest_port and not queued and log.exists() and b"UDP PRE READY" in log.read_bytes():
                    sock.sendto(early, ("127.0.0.1", guest_port))
                    queued = True
                try:
                    message, address = sock.recvfrom(2048)
                except TimeoutError:
                    continue
                peer_frames.append(message)
                if message == allowed:
                    for packet in replies:
                        sock.sendto(packet, address)
                        time.sleep(.025)

        worker = threading.Thread(target=peer)
        worker.start()
        binary = [args.qemu] + (["-L", str(args.bios_dir.resolve())] if args.bios_dir else [])
        cmd = binary + ["-machine","pc,i8042=off","-accel","tcg","-cpu","max","-m","256M","-display","none","-vga","none",
                        "-monitor","none","-no-reboot","-kernel",str(args.kernel.resolve()),"-initrd","raw.elf",
                        "-append",f"auto=raw.elf then=off net_image={pin} {option}",
                        "-netdev",f"socket,id=lane,udp=127.0.0.1:{sock.getsockname()[1]},localaddr=127.0.0.1:{guest_port}",
                        "-device","virtio-net-pci,netdev=lane,disable-modern=on,vectors=0,mac=02:00:00:00:00:01",
                        "-object",f"filter-dump,id=trace,netdev=lane,file={capture.name}",
                        "-serial","file:"+str(log),"-device","isa-debug-exit,iobase=0xf4,iosize=0x04"]
        try:
            run = subprocess.run(cmd, cwd=root, capture_output=True, timeout=args.timeout,
                                 creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        finally:
            stop.set()
            worker.join()

    lines = log.read_text(errors="replace").splitlines()
    if arm == "raw":
        marker, outgoing, wire = "UDP RAW PASSED", [blocked[0]], [blocked[0]]
    elif arm == "restricted":
        marker, outgoing, wire = "UDP FILTERED OK!", [allowed], [early, allowed] + replies
        expected = f"guest UDP counts: tx-pass=1 tx-deny={len(blocked)} rx-pass=1 rx-drop=4"
        if expected not in lines:
            raise RuntimeError(f"{arm}: counters disagree; inspect {log}")
    else:
        marker, outgoing, wire = "UDP GRANT DENIED", [], []
    if run.returncode != 33 or marker not in lines or peer_frames != outgoing or frames(capture) != wire:
        raise RuntimeError(f"{arm}: guest, peer or capture disagreement; inspect {log} and {capture}")
    print(f"{arm}: {marker}; {len(wire)} captured frame(s)", flush=True)


def replay(root, args):
    root.mkdir(parents=True, exist_ok=False)
    allowed, blocked = fixtures()
    pin = compile_probe(root, args, allowed, blocked)
    for arm, option, image in [("raw", "", pin), ("restricted", POLICY, pin),
                               ("malformed", "net_udp=bogus", pin), ("duplicate", POLICY+" "+POLICY, pin),
                               ("wrong-image", POLICY, "0"*64)]:
        run_arm(root, args, arm, option, image, allowed, blocked)
    print("kernel SHA-256", hashlib.sha256(args.kernel.read_bytes()).hexdigest())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, required=True)
    parser.add_argument("--out", type=Path, help="new directory to retain serial logs and captures")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--bios-dir", type=Path)
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--objcopy", default="llvm-objcopy")
    parser.add_argument("--timeout", type=int, default=90)
    args = parser.parse_args()
    if args.out:
        replay(args.out.resolve(), args)
    else:
        with tempfile.TemporaryDirectory() as scratch:
            replay(Path(scratch) / "run", args)
