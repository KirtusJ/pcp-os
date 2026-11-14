#!/usr/bin/env python3
"""Run two native guests on a direct QEMU loopback link and check both Ethernet captures."""

import argparse
from contextlib import ExitStack
from hashlib import sha256
import json
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import time

from measure import measure
from virtio_tx_probe import check, packets


CASES = ("run-a", "run-b", "unknown", "wrong-image", "missing-worker", "boot-flag-off", "build-feature-off")
DIRECTOR = bytes.fromhex("020000000001")
WORKER = bytes.fromhex("020000000002")
READY_MARK = "worker wire: director RX ready"
PASS = {"director": "the native worker director verified two bound outcomes",
        "receiver": "the native worker receiver finished its bounded dispatch"}
CHILD = re.compile(r"worker wire: child pcp-fib pid=(\d+) task=(\d+) input=(\d+) "
                   r"exit=(\d+) calls=(\d+) released=(\d+) grants=none")


def input_number(nonce):
    return 12 + int.from_bytes(nonce[:2], "big") % 25


def fibonacci(n):
    # Arbitrary-precision reference, with one reduction after all additions.
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a % 251


def frame(src, dst, kind, job=0, cached=False, nonce=bytes(8), n=0, value=0):
    return (dst + src + b"\x88\xb5PCPW" + bytes((1, kind, job, int(cached)))
            + nonce + bytes((n,)) + value.to_bytes(4, "big") + bytes(25))


def udp_port_pair():
    # Reserve two distinct ephemeral ports during selection. Only the QEMU processes
    # bind or use them for the link; the harness never sends or relays a frame.
    with ExitStack() as stack:
        sockets = [stack.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM)) for _ in range(2)]
        for sock in sockets:
            sock.bind(("127.0.0.1", 0))
        return [sock.getsockname()[1] for sock in sockets]


def wait_ready(process, path, deadline):
    while time.monotonic() < deadline:
        if READY_MARK in path.read_text(encoding="utf-8", errors="replace").splitlines():
            return
        if process.poll() is not None:
            raise RuntimeError(f"director exited {process.returncode} before RX readiness; see {path}")
        time.sleep(0.015)
    raise TimeoutError(f"missing director RX readiness; see {path}")


def stop(process):
    if process.poll() is None:
        process.kill()
    process.wait()


def launch(stack, args, image, image_measure, role, words, directory, ports, child):
    directory.mkdir()
    # Multiboot includes the kernel filename in its bounded command line. Keep
    # that name short so a long checkout path cannot truncate the image pin.
    shutil.copyfile(image, directory / "kernel")
    local, remote = ports
    mac = (DIRECTOR if role == "director" else WORKER).hex(":")
    cmd = [args.qemu, "-machine", "pc,accel=tcg", "-icount", "shift=4,sleep=off",
           "-cpu", "max", "-m", "128M", "-vga", "std", "-display", "none", "-nic", "none",
           "-no-reboot", "-kernel", "kernel", "-append", " ".join(["then=off", "image=" + image_measure, *words]),
           "-monitor", "none", "-serial", "stdio", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
           "-netdev", f"socket,id=pair,udp=127.0.0.1:{remote},localaddr=127.0.0.1:{local}",
           "-device", f"virtio-net-pci,netdev=pair,addr=0x06,disable-modern=on,vectors=0,tx_queue_size=256,mac={mac}",
           "-object", "filter-dump,id=wire,netdev=pair,file=frame.pcap"]
    if args.firmware:
        cmd.extend(["-L", str(args.firmware)])
    if child:
        cmd.extend(["-initrd", child.as_posix()])
    serial = stack.enter_context((directory / "serial.txt").open("wb"))
    errors = stack.enter_context((directory / "stderr.txt").open("wb"))
    process = subprocess.Popen(cmd, cwd=directory, stdout=serial, stderr=errors,
                               creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    stack.callback(stop, process)
    return process, cmd


def read_evidence(directory, cmd, code):
    lines = (directory / "serial.txt").read_text(encoding="utf-8").splitlines()
    return {"command": cmd, "exit": code, "pcap": [p.hex() for p in packets(directory / "frame.pcap")],
            "passed": [s[7:] for s in lines if s.startswith("  pass ")],
            "failed": [s[7:] for s in lines if s.startswith("  FAIL ")],
            "protocol": [s for s in lines if s.startswith(("worker wire:", "virtio worker "))]}


def run_case(args, name, nonce, measurements, child_hash):
    directory = args.output / name
    directory.mkdir()
    ports = udp_port_pair()
    active = name not in ("boot-flag-off", "build-feature-off")
    words = name != "boot-flag-off"
    image = args.default if name == "build-feature-off" else args.enabled
    variant = name if name in ("unknown", "wrong-image") else "run"
    pin = ("0" if child_hash[0] != "0" else "1") + child_hash[1:] if name == "wrong-image" else child_hash
    director_words = ["test=worker-director", "reply_nonce=" + nonce.hex(), "worker_case=" + variant] if words else []
    receiver_words = ["test=worker-receiver", "worker_image=" + pin] if words else []
    results = {}
    with ExitStack() as stack:
        dp, dc = launch(stack, args, image, measurements[image], "director", director_words,
                        directory / "director", ports, None)
        deadline = time.monotonic() + args.timeout
        if active:
            wait_ready(dp, directory / "director" / "serial.txt", deadline)
        procs = {"director": (dp, dc)}
        if name != "missing-worker":
            receiver_child = args.challenge_child if name == "altered-result" else args.child
            procs["receiver"] = launch(stack, args, image, measurements[image], "receiver", receiver_words,
                                       directory / "receiver", ports[::-1], receiver_child)
        for role, (process, cmd) in procs.items():
            code = process.wait(timeout=max(0.01, deadline - time.monotonic()))
            results[role] = read_evidence(directory / role, cmd, code)
    return {"case": name, "nonce": nonce.hex(), "guests": results}


def verify(run):
    name, guests = run["case"], run["guests"]
    nonce = bytes.fromhex(run["nonce"])
    n = input_number(nonce)
    disabled = name in ("boot-flag-off", "build-feature-off")
    missing = name == "missing-worker"
    executed = name in ("run-a", "run-b")
    job = 0xfe if name == "unknown" else 1
    value = fibonacci(n) if executed else (1 if name == "unknown" else 2)
    kind = 3 if executed else 4
    expected = []
    if not disabled and not missing:
        req = frame(DIRECTOR, WORKER, 2, job, False, nonce, n)
        expected = [frame(WORKER, DIRECTOR, 1), req, frame(WORKER, DIRECTOR, kind, job, False, nonce, n, value),
                    req, frame(WORKER, DIRECTOR, kind, job, True, nonce, n, value)]
    for role, receipt in guests.items():
        check(name, role + " Ethernet frames", receipt["pcap"], [p.hex() for p in expected])
        check(name, role + " process status", receipt["exit"], 35 if missing else 33)
        check(name, role + " failed self-tests", receipt["failed"], [PASS[role]] if missing else [])
        check(name, role + " passing dispatch tests", receipt["passed"].count(PASS[role]), int(not disabled and not missing))
        trace = receipt["protocol"]
        if disabled:
            check(name, role + " traffic/dispatch traces", trace, [])
            continue
        diagnostics = [s for s in trace if s.startswith("virtio worker ")]
        outcome = 'Err("worker RX timeout")' if missing else "Ok(())"
        check(name, role + " kernel outcome", diagnostics, [f"virtio worker {role}: {outcome}"])
        child_lines = [s for s in trace if s.startswith("worker wire: child ")]
        if role == "director":
            check(name, "director child launches", child_lines, [])
            verdict = [] if missing else [f"worker wire: director verified input={n} kind={kind} value={value} duplicate=1"]
            check(name, "director checked result", [s for s in trace if s.startswith("worker wire: director verified")], verdict)
        else:
            check(name, "receiver launch count", [s for s in trace if s.startswith("worker wire: receiver launches=")],
                  [f"worker wire: receiver launches={int(executed)}"])
            check(name, "child records", len(child_lines), int(executed))
            if executed:
                match = CHILD.fullmatch(child_lines[0])
                if match is None:
                    raise ValueError(f"unrecognized child record: {child_lines[0]}")
                pid, task, argument, result, calls, released = map(int, match.groups())
                check(name, "child input, exit and syscall count", (argument, result, calls), (n, value, 1))
                if min(pid, task, released) <= 0:
                    raise ValueError(f"missing process execution or resource release: {child_lines[0]}")

def verify_altered(run):
    # A locally pinned control executable adds one to its valid exit residue. Nothing
    # on the host sends this result to either guest. The director must stop before retry.
    name = run["case"]
    nonce = bytes.fromhex(run["nonce"])
    n = input_number(nonce)
    altered = (fibonacci(n) + 1) % 251
    expected = [frame(WORKER, DIRECTOR, 1), frame(DIRECTOR, WORKER, 2, 1, False, nonce, n),
                frame(WORKER, DIRECTOR, 3, 1, False, nonce, n, altered)]
    for role, receipt in run["guests"].items():
        check(name, role + " Ethernet frames", receipt["pcap"], [f.hex() for f in expected])
        check(name, role + " process status", receipt["exit"], 35)
        check(name, role + " failed tests", receipt["failed"], [PASS[role]])
        check(name, role + " false passes", receipt["passed"].count(PASS[role]), 0)
        trace = receipt["protocol"]
        error = "worker result" if role == "director" else "worker RX timeout"
        check(name, role + " kernel outcome", [s for s in trace if s.startswith("virtio worker ")],
              [f'virtio worker {role}: Err("{error}")'])
        children = [s for s in trace if s.startswith("worker wire: child ")]
        check(name, role + " child launches", len(children), int(role == "receiver"))
        if children:
            match = CHILD.fullmatch(children[0])
            if match is None:
                raise ValueError("missing observed alternate worker")
            pid, task, argument, result, calls, released = map(int, match.groups())
            check(name, "alternate child's exit", (argument, result, calls), (n, altered, 1))
            if min(pid, task, released) <= 0:
                raise ValueError("missing alternate process or resource release")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("enabled", type=Path)
    parser.add_argument("default", type=Path)
    parser.add_argument("child", type=Path, help="locally installed static Rust ELF, named pcp-fib")
    parser.add_argument("output", type=Path, help="new directory for serial logs, packet captures and receipt")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=350.0, help="maximum host seconds per arm (the absent-peer control can take several minutes)")
    parser.add_argument("--only", choices=(*CASES, "altered-result"), help="run just one arm")
    parser.add_argument("--challenge-child", type=Path, help="optional local pcp-fib ELF returning (F(n)+1) modulo 251")
    args = parser.parse_args()
    args.enabled, args.default, args.child, args.output = [p.resolve() for p in (args.enabled, args.default, args.child, args.output)]
    if args.child.name != "pcp-fib" or args.timeout <= 0:
        parser.error("the child filename must be pcp-fib and the timeout must be positive")
    if args.challenge_child:
        args.challenge_child = args.challenge_child.resolve()
        if args.challenge_child.name != "pcp-fib" or args.challenge_child.read_bytes() == args.child.read_bytes():
            parser.error("the alternate child must be a distinct ELF named pcp-fib")
    if args.only == "altered-result" and args.challenge_child is None:
        parser.error("altered-result requires --challenge-child")
    if args.firmware:
        args.firmware = args.firmware.resolve()
    images = {path: path.read_bytes() for path in (args.enabled, args.default)}
    if images[args.enabled] == images[args.default]:
        parser.error("the enabled and default reference kernels must be distinct builds")
    measurements = {path: measure(data) for path, data in images.items()}
    child_hash = sha256(args.child.read_bytes()).hexdigest()
    args.output.mkdir()
    report = {"images": {str(path): sha256(data).hexdigest() for path, data in images.items()},
              "child_sha256": child_hash, "runs": []}
    runs = report["runs"]
    cases = list(CASES)
    if args.challenge_child:
        report["challenge_child_sha256"] = sha256(args.challenge_child.read_bytes()).hexdigest()
        cases.append("altered-result")
    for name in ([args.only] if args.only else cases):
        nonce = secrets.token_bytes(8)
        if name == "run-b" and runs:
            prior = input_number(bytes.fromhex(runs[0]["nonce"]))
            while input_number(nonce) == prior or fibonacci(input_number(nonce)) == fibonacci(prior):
                nonce = secrets.token_bytes(8)
        pin = report["challenge_child_sha256"] if name == "altered-result" else child_hash
        run = run_case(args, name, nonce, measurements, pin)
        runs.append(run)
        (args.output / "receipt.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        if name == "altered-result":
            verify_altered(run)
        else:
            verify(run)
        print(f"{name}: {len(run['guests'])} guest(s), observations verified", flush=True)
    if not args.only:
        defaults = next(r for r in runs if r["case"] == "build-feature-off")
        for run in runs:
            for role, receipt in run["guests"].items():
                baseline = defaults["guests"][role]["passed"]
                if len(baseline) < 72:
                    raise ValueError(f"{role}: incomplete baseline self-tests ({len(baseline)})")
                check(run["case"], role + " baseline self-tests", [s for s in receipt["passed"] if s != PASS[role]], baseline)


if __name__ == "__main__":
    main()
