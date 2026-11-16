#!/usr/bin/env python3
"""Run and continuously recheck a tree of measured PCP OS numerical workers."""
import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import shutil
import signal
import subprocess
import sys
import time
import uuid

KERNEL_TOOLS = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/crates/pcpos-kernel/tools"
sys.path.insert(0, str(KERNEL_TOOLS))
from measure import measure

PARENTS = (0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12)
KEYS = tuple("1234567890") + ("minus", "equal", "backspace", "tab", "q", "w")
DEFAULT_STARTUP_TIMEOUT = 120
DEFAULT_RESPONSE_TIMEOUT = 60
SERIAL_LIMIT = 256 * 1024


def require(condition, detail):
    if not condition:
        raise RuntimeError(detail)


def stamp():
    return datetime.now(timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def snapshot(path, contents):
    temp = path.with_suffix(".tmp")
    temp.write_text(json.dumps(contents, indent=2) + "\n")
    temp.replace(path)


class Machine:
    """Own the child and its private stdio channel for the entire run."""

    def __init__(self, node, args, run, cleanup, startup_timeout, response_timeout):
        self.node = node
        self.response_timeout = response_timeout
        self.uuid = str(uuid.uuid4())
        self.serial = run / f"{node}.serial"
        self.position = 0
        self.qmp_data = b""
        self.sequence = 0
        error = cleanup.enter_context((run / f"{node}.stderr").open("xb"))
        self.child = subprocess.Popen(
            args + ["-uuid", self.uuid, "-serial", f"file:{self.serial}", "-qmp", "stdio"],
            cwd=run, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=error, bufsize=0,
        )
        cleanup.callback(self.close)
        self.selector = cleanup.enter_context(selectors.DefaultSelector())
        self.selector.register(self.child.stdout, selectors.EVENT_READ)
        require("QMP" in self.message(time.monotonic() + startup_timeout), f"guest {node}: QMP greeting")
        self.command("qmp_capabilities")

    def close(self):
        if self.child.poll() is None:
            self.child.terminate()
            try:
                self.child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.child.kill()
                self.child.wait()
        self.child.stdin.close()
        self.child.stdout.close()

    def message(self, deadline):
        while b"\n" not in self.qmp_data:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.selector.select(remaining):
                raise TimeoutError(f"guest {self.node}: QMP response")
            chunk = os.read(self.child.stdout.fileno(), 8192)
            require(chunk, f"guest {self.node}: QMP disconnected")
            self.qmp_data += chunk
            require(len(self.qmp_data) <= SERIAL_LIMIT, "oversized QMP message")
        line, self.qmp_data = self.qmp_data.split(b"\n", 1)
        return json.loads(line)

    def command(self, name, **arguments):
        self.sequence += 1
        packet = {"execute": name, "id": self.sequence, "arguments": arguments}
        wire = json.dumps(packet).encode() + b"\n"
        require(self.child.stdin.write(wire) == len(wire), f"guest {self.node}: incomplete QMP request")
        deadline = time.monotonic() + self.response_timeout
        while True:
            try:
                response = self.message(deadline)
            except TimeoutError as error:
                raise TimeoutError(f"guest {self.node}: {name} timed out") from error
            if "event" in response:
                continue
            require(response.get("id") == self.sequence and "return" in response,
                    f"guest {self.node}: unexpected QMP response {response}")
            return response["return"]

    def live(self):
        require(self.command("query-uuid")["UUID"] == self.uuid, f"guest {self.node}: UUID changed")
        require(self.command("query-status")["running"] is True, f"guest {self.node}: stopped VM")

    def through_ready(self, timeout):
        deadline = time.monotonic() + timeout
        end_marker = f"READY {self.node}\n".encode()
        while time.monotonic() < deadline:
            require(self.child.poll() is None, f"guest {self.node}: emulator exited")
            if self.serial.exists():
                with self.serial.open("rb") as stream:
                    require(stream.seek(0, 2) >= self.position, f"guest {self.node}: serial log truncated")
                    stream.seek(self.position)
                    raw = stream.read(SERIAL_LIMIT + 1)
                require(len(raw) <= SERIAL_LIMIT, f"guest {self.node}: serial output limit")
                data = raw.replace(b"\r\n", b"\n")
                require(b" ended FAILED" not in data and b" ended DONE" not in data,
                        f"guest {self.node}: worker exited")
                if data.endswith(end_marker):
                    self.position += len(raw)
                    return data.decode("ascii")
            time.sleep(0.04)
        raise TimeoutError(f"guest {self.node}: fresh READY missing")

    def challenge(self, prompt, oracle):
        self.live()
        self.command("send-key", keys=[{"type": "qcode", "data": KEYS[prompt]}], **{"hold-time": 25})
        reply = self.through_ready(self.response_timeout)
        tokens = " ".join(str(t) for t in oracle[prompt])
        require(reply == f"CHALLENGE {prompt}\nLM tokens {tokens}\nREADY {self.node}\n",
                f"guest {self.node}: response mismatch for prompt {prompt}")
        return {"prompt": prompt, "tokens": oracle[prompt], "serial_sha256": sha(self.serial.read_bytes())}


def verify_start(machine, serial, parent, incoming, salt, kernel_hash, elf_hash, oracle):
    node = machine.node
    ticket = (incoming * 31 + node * 13 + salt + 1) % 1000003
    children = [n for n in range(2, len(PARENTS)) if PARENTS[n] == node]
    require("long mode true" in serial and "  FAIL" not in serial and
            f"{kernel_hash} (measured; equal to the value the loader named)" in serial,
            f"guest {node}: kernel measurement or self-test")
    start = re.search(r"^starting resident\.elf \(\d+ bytes, SHA-256 ([a-f0-9]{64})\) with: (.+)$", serial, re.M)
    require(start and start.groups() == (elf_hash, f"{node} {incoming} {salt}"),
            f"guest {node}: admitted program or arguments")
    prompt = (node + salt + incoming) % len(oracle)
    wanted = [f"NODE {node} PARENT {parent} INPUT {incoming} TICKET {ticket}",
              "LM tokens " + " ".join(map(str, oracle[prompt])), "GATE 1"]
    wanted += [f"SPAWN {child} PARENT {node} TICKET {ticket}" for child in children]
    wanted += [f"READY {node}"]
    require(serial[start.end():].splitlines()[1:] == wanted, f"guest {node}: guest dispatch or model result")
    machine.live()
    return ticket, children


def supervise(opt, log):
    kernel = opt.kernel.resolve(strict=True)
    elf = opt.worker.resolve(strict=True)
    kernel_hash = measure(kernel.read_bytes())
    elf_hash = sha(elf.read_bytes())
    oracle_bytes = opt.oracle.read_bytes()
    oracle = json.loads(oracle_bytes)
    require(isinstance(oracle, list) and len(oracle) == 16 and all(
        isinstance(row, list) and len(row) == 3 and all(type(t) is int and 0 <= t < 512 for t in row)
        for row in oracle), "expected 16 reference outputs of three token IDs")
    run = opt.run.resolve()
    # The module name becomes argv[0] in the PCP OS launcher.
    shutil.copyfile(elf, run / "resident.elf")
    require(sha((run / "resident.elf").read_bytes()) == elf_hash, "worker changed while copying")
    salt = secrets.randbelow(16)
    initial = secrets.randbelow(900000) + 1
    # The existing guest ABI reserves zero as an invalid incoming ticket.
    # Pick a fresh root input if a non-leaf would issue that value.
    while True:
        tickets = {0: initial}
        for node, parent in enumerate(PARENTS[1:], 1):
            tickets[node] = (tickets[parent] * 31 + node * 13 + salt + 1) % 1000003
        if all(tickets[parent] for parent in PARENTS[1:]):
            break
        initial = secrets.randbelow(900000) + 1

    args = [opt.qemu]
    if opt.firmware:
        args += ["-L", str(opt.firmware.resolve())]
    args += ["-accel", "tcg,tb-size=4", "-cpu", "max", "-nodefaults", "-machine",
             "microvm,isa-serial=on,pic=on,pit=on,rtc=on", "-device", "i8042", "-m", "32M",
             "-display", "none", "-nic", "none", "-monitor", "none", "-no-reboot",
             "-kernel", str(kernel), "-append", "auto=resident.elf image=" + kernel_hash]
    log("inputs", kernel_sha256=sha(kernel.read_bytes()), measurement=kernel_hash, worker_sha256=elf_hash,
        oracle_sha256=sha(oracle_bytes), root_input=initial, salt=salt)
    with ExitStack() as cleanup:
        queue = [(1, 0, initial)]
        machines = {}
        identities = set()
        while queue:
            node, parent, incoming = queue.pop(0)
            if parent:
                machines[parent].live()
            require(node not in machines, f"duplicate node {node}")
            machine = Machine(node, args + ["-initrd", f"resident.elf {node} {incoming} {salt}"], run, cleanup,
                              opt.startup_timeout, opt.response_timeout)
            serial = machine.through_ready(opt.startup_timeout)
            ticket, children = verify_start(machine, serial, parent, incoming, salt, kernel_hash, elf_hash, oracle)
            require(machine.uuid not in identities, "duplicate VM UUID")
            identities.add(machine.uuid)
            machines[node] = machine
            log("admitted", node=node, parent=parent, input=incoming, ticket=ticket,
                uuid=machine.uuid, pid=machine.child.pid, children=children)
            queue.extend((child, node, ticket) for child in children)
        require(len(machines) == 24, "incomplete topology")
        cycle = 0
        previous = {}
        while True:
            rows = []
            for node, machine in machines.items():
                choices = [p for p in range(len(oracle)) if p != previous.get(node)]
                prompt = secrets.choice(choices)
                answer = machine.challenge(prompt, oracle)
                rows.append(dict(node=node, parent=PARENTS[node], uuid=machine.uuid, **answer))
                previous[node] = prompt
            for machine in machines.values():
                machine.live()
            cycle += 1
            snapshot(run / "readback.json", dict(time=stamp(), cycle=cycle, nodes=rows))
            log("readback", cycle=cycle, sha256=sha((run / "readback.json").read_bytes()))
            if opt.once:
                return
            time.sleep(opt.interval)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("kernel", "worker", "oracle", "run"):
        parser.add_argument("--" + key, required=True, type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--interval", type=int, default=60)
    parser.add_argument("--startup-timeout", type=int, default=DEFAULT_STARTUP_TIMEOUT)
    parser.add_argument("--response-timeout", type=int, default=DEFAULT_RESPONSE_TIMEOUT)
    parser.add_argument("--once", action="store_true", help="run one complete readback and shut down")
    opt = parser.parse_args()
    if min(opt.interval, opt.startup_timeout, opt.response_timeout) < 1:
        parser.error("interval and timeouts must be positive")
    opt.run.mkdir(mode=0o700, exist_ok=False)
    with (opt.run / "events.jsonl").open("x", buffering=1) as events:
        def log(event, **fields):
            events.write(json.dumps(dict(time=stamp(), event=event, **fields)) + "\n")
        def stop(signum, frame):
            raise KeyboardInterrupt
        signal.signal(signal.SIGTERM, stop)
        try:
            supervise(opt, log)
        except KeyboardInterrupt:
            log("stopped")
            return 130
        except (OSError, ValueError, RuntimeError, KeyError, EOFError) as error:
            log("failed", reason=str(error))
            print(error, file=sys.stderr)
            return 1
        log("finished")
    return 0


if __name__ == "__main__":
    sys.exit(main())
