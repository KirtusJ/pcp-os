#!/usr/bin/env python3
"""Boot two kernel images and compare serial, QEMU PCAP and one loopback peer."""
import argparse
import json
from pathlib import Path
import socket
import struct
import subprocess
from hashlib import sha256

from measure import measure


FRAME = bytes.fromhex("02000000000202000000000188b5") + b"PCP-OS virtio TX v1!" + bytes(26)
PASS = "the isolated virtio-net TX queue completed its single frame"


def packets(path):
    if path.stat().st_size > 4096:
        raise ValueError("unexpected traffic volume")
    raw = path.read_bytes()
    endian = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">"}.get(raw[:4])
    if endian is None or len(raw) < 24 or struct.unpack_from(endian + "I", raw, 20)[0] != 1:
        raise ValueError("expected an Ethernet PCAP")
    offset = 24
    result = []
    while offset < len(raw):
        _, _, size, wire = struct.unpack_from(endian + "4I", raw, offset)
        offset += 16
        if size != wire or size > len(raw) - offset:
            raise ValueError("truncated packet")
        result.append(raw[offset:offset + size])
        offset += size
    return result


def boot(args, name, image, enabled, nic, mac="02:00:00:00:00:01"):
    directory = args.output / name
    directory.mkdir()
    image_bytes = image.read_bytes()
    integrity = measure(image_bytes)
    opts = ["-machine", "pc,accel=tcg", "-icount", "shift=4,sleep=off", "-cpu", "max", "-m", "128M", "-vga", "std",
            "-display", "none", "-nic", "none", "-no-reboot", "-kernel", str(image),
            "-append", "then=off image=" + integrity + (" test=virtio-tx" if enabled else ""),
            "-monitor", "none", "-serial", "stdio", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
    if args.firmware:
        opts.extend(["-L", str(args.firmware)])
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as peer:
        peer.bind(("127.0.0.1", 0))
        peer.settimeout(0.15)
        if nic:
            # Both endpoints are literal loopback addresses. The guest has no
            # other NIC; no TAP, user network, bridge, or forwarding is set up.
            port = peer.getsockname()[1]
            opts.extend(["-netdev", f"socket,id=lab,udp=127.0.0.1:{port},localaddr=127.0.0.1:0",
                         "-device", "virtio-net-pci,netdev=lab,id=probe_nic,addr=0x06,"
                         "disable-modern=on,vectors=0,tx_queue_size=256,mac=" + mac,
                         "-object", "filter-dump,id=wire,netdev=lab,file=frame.pcap"])
        with (directory / "serial.txt").open("wb") as serial, (directory / "stderr.txt").open("wb") as errors:
            proc = subprocess.run([args.qemu, *opts], cwd=directory, stdout=serial, stderr=errors,
                                  timeout=35, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        seen = []
        for _ in range(2):
            try:
                data, address = peer.recvfrom(4096)
            except socket.timeout:
                break
            if address[0] != "127.0.0.1":
                raise ValueError("unexpected peer address")
            seen.append(data)
    capture = packets(directory / "frame.pcap") if nic else []
    lines = (directory / "serial.txt").read_text(encoding="utf-8").splitlines()
    tests = [line[7:] for line in lines if line.startswith("  pass ")]
    failures = [line[7:] for line in lines if line.startswith("  FAIL ")]
    return dict(name=name, image_sha256=sha256(image_bytes).hexdigest(), measured=integrity,
                command=opts, exit=proc.returncode, tests=tests, failures=failures,
                device_result=[s for s in lines if s.startswith("virtio TX fixture:")],
                guest_pci=[s for s in lines if s.startswith("device on the bus:") and "class 02.00" in s],
                pcap=[packet.hex() for packet in capture], observed=[packet.hex() for packet in seen])


def check(arm, field, actual, expected):
    if actual != expected:
        raise SystemExit(f"{arm}: {field} mismatch\nexpected {expected!r}\nobserved {actual!r}")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("enabled_image", type=Path)
    p.add_argument("default_image", type=Path)
    p.add_argument("output", type=Path, help="a new directory for all guest logs, captures and the receipt")
    p.add_argument("--qemu", default="qemu-system-x86_64")
    p.add_argument("--firmware", type=Path, help="QEMU firmware directory if needed by this installation")
    args = p.parse_args()
    args.output = args.output.resolve()
    args.output.mkdir()
    enabled, default = args.enabled_image.resolve(), args.default_image.resolve()
    runs = [boot(args, "transmit", enabled, True, True),
            boot(args, "boot-flag-off", enabled, False, True),
            boot(args, "build-feature-off", default, True, True),
            boot(args, "device-absent", enabled, True, False),
            boot(args, "wrong-fixture-mac", enabled, True, True, "02:00:00:00:00:03")]
    (args.output / "receipt.json").write_text(json.dumps(runs, indent=2) + "\n", encoding="utf-8")
    baseline = runs[2]["tests"]
    if not baseline or PASS in baseline:
        raise SystemExit("build-feature-off: missing or contaminated baseline self-tests")
    diagnostics = {
        "transmit": ["virtio TX fixture: Ok(256)"],
        "boot-flag-off": [],
        "build-feature-off": [],
        "device-absent": ['virtio TX fixture: Err("no legacy virtio-net PCI function")'],
        "wrong-fixture-mac": ['virtio TX fixture: Err("fixture MAC mismatch")'],
    }
    device = "device on the bus: 00:06.0 maker 1af4 device 1000 class 02.00"
    for run in runs:
        arm = run["name"]
        positive = arm == "transmit"
        refused = arm in ("device-absent", "wrong-fixture-mac")
        expected = [FRAME.hex()] if positive else []
        check(arm, "guest diagnostic", run["device_result"], diagnostics[arm])
        check(arm, "Ethernet capture", run["pcap"], expected)
        check(arm, "loopback peer", run["observed"], expected)
        check(arm, "guest exit", run["exit"], 35 if refused else 33)
        check(arm, "failed tests", run["failures"], [PASS] if refused else [])
        check(arm, "other passed tests", [t for t in run["tests"] if t != PASS], baseline)
        check(arm, "TX test pass count", run["tests"].count(PASS), int(positive))
        pci = [line.partition(" line ")[0] for line in run["guest_pci"]]
        check(arm, "enumerated Ethernet devices", pci, [] if arm == "device-absent" else [device])
        print(f"{run['name']}: {len(run['tests'])} passed, {len(run['failures'])} failed, "
              f"{len(run['observed'])} frames, guest exit {run['exit']}")
    print("One 60-byte frame matched the capture and peer exactly.")


if __name__ == "__main__":
    main()
