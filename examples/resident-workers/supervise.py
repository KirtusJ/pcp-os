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

    def __init__(self, node, args, run, cleanup, startup_deadline, response_timeout, attempt):
        self.node = node
        self.response_timeout = response_timeout
        self.uuid = str(uuid.uuid4())
        self.serial = run / f"{node}-{attempt}.serial"
        self.position = 0
        self.qmp_data = b""
        self.sequence = 0
        self.last_command = "greeting"
        error = cleanup.enter_context((run / f"{node}-{attempt}.stderr").open("xb"))
        self.child = subprocess.Popen(
            args + ["-uuid", self.uuid, "-serial", f"file:{self.serial}", "-qmp", "stdio"],
            cwd=run, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=error, bufsize=0,
        )
        cleanup.callback(self.close)
        self.selector = cleanup.enter_context(selectors.DefaultSelector())
        self.selector.register(self.child.stdout, selectors.EVENT_READ)
        require("QMP" in self.message(startup_deadline), f"guest {node}: QMP greeting")
        self.command("qmp_capabilities", deadline=startup_deadline)

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
                raise TimeoutError(self.failure_details("QMP response timeout"))
            chunk = os.read(self.child.stdout.fileno(), 8192)
            if not chunk:
                raise ConnectionError(self.failure_details("QMP disconnected"))
            self.qmp_data += chunk
            require(len(self.qmp_data) <= SERIAL_LIMIT, "oversized QMP message")
        line, self.qmp_data = self.qmp_data.split(b"\n", 1)
        return json.loads(line)

    def failure_details(self, reason):
        status = self.child.poll()
        observed = "running at poll" if status is None else f"exit status {status}"
        return f"guest {self.node}: {self.last_command}: {reason}; emulator {observed}"

    def command(self, name, *, deadline=None, **arguments):
        self.last_command = name
        self.sequence += 1
        packet = {"execute": name, "id": self.sequence, "arguments": arguments}
        wire = json.dumps(packet).encode() + b"\n"
        try:
            written = self.child.stdin.write(wire)
        except BrokenPipeError as error:
            raise ConnectionError(self.failure_details("broken pipe")) from error
        require(written == len(wire), f"guest {self.node}: incomplete QMP request")
        response_deadline = time.monotonic() + self.response_timeout
        deadline = response_deadline if deadline is None else min(response_deadline, deadline)
        while time.monotonic() < deadline:
            try:
                response = self.message(deadline)
            except TimeoutError as error:
                raise TimeoutError(self.failure_details("response timeout")) from error
            if "event" in response:
                continue
            require(response.get("id") == self.sequence and "return" in response,
                    f"guest {self.node}: unexpected QMP response {response}")
            return response["return"]
        raise TimeoutError(self.failure_details("response timeout"))

    def live(self, deadline=None):
        require(self.command("query-uuid", deadline=deadline)["UUID"] == self.uuid, f"guest {self.node}: UUID changed")
        require(self.command("query-status", deadline=deadline)["running"] is True, f"guest {self.node}: stopped VM")

    def through_ready(self, deadline, idle_timeout=None):
        last_change = time.monotonic()
        length = self.position
        end_marker = f"READY {self.node}\n".encode()
        while time.monotonic() < deadline:
            if self.child.poll() is not None:
                raise ConnectionError(f"guest {self.node}: emulator exited with status {self.child.returncode}")
            if self.serial.exists():
                with self.serial.open("rb") as stream:
                    size = stream.seek(0, os.SEEK_END)
                    require(size >= length, f"guest {self.node}: serial log truncated")
                    require(size - self.position <= SERIAL_LIMIT, f"guest {self.node}: serial output limit")
                    if size > length:
                        stream.seek(self.position)
                        raw = stream.read(size - self.position)
                        require(len(raw) == size - self.position, f"guest {self.node}: serial log truncated")
                        length = size
                        last_change = time.monotonic()
                        data = raw.replace(b"\r\n", b"\n")
                        require(b" ended FAILED" not in data and b" ended DONE" not in data,
                                f"guest {self.node}: worker exited")
                        if data.endswith(end_marker):
                            self.position = size
                            return data.decode("ascii")
            if idle_timeout is not None and time.monotonic() - last_change >= idle_timeout:
                raise TimeoutError(f"guest {self.node}: serial idle at {length} bytes before READY")
            time.sleep(0.04)
        raise TimeoutError(f"guest {self.node}: overall deadline at {length} serial bytes before READY")

    def challenge(self, prompt, oracle):
        self.live()
        require(self.serial.stat().st_size == self.position,
                f"guest {self.node}: unsolicited serial output before challenge")
        self.command("send-key", keys=[{"type": "qcode", "data": KEYS[prompt]}], **{"hold-time": 25})
        reply = self.through_ready(time.monotonic() + self.response_timeout)
        tokens = " ".join(str(t) for t in oracle[prompt])
        require(reply == f"CHALLENGE {prompt}\nLM tokens {tokens}\nREADY {self.node}\n",
                f"guest {self.node}: response mismatch for prompt {prompt}")
        return {"prompt": prompt, "tokens": oracle[prompt], "serial_sha256": sha(self.serial.read_bytes())}


def verify_start(machine, serial, parent, incoming, salt, kernel_hash, elf_hash, oracle, deadline=None):
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
    machine.live(deadline=deadline)
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
        machines = {}
        attempts = {}
        used_uuids = set()
        previous = {}
        repairs = 0
        cycle = 0

        def remove_branch(node):
            for child in list(machines):
                if PARENTS[child] == node:
                    remove_branch(child)
            entry = machines.pop(node, None)
            if entry:
                machine, resources, _ = entry
                resources.close()
                log("retired", node=node, uuid=machine.uuid)
                previous.pop(node, None)

        def failed(node, error):
            nonlocal repairs
            log("interrupted", node=node, reason=str(error))
            remove_branch(node)
            if repairs >= opt.repairs:
                raise RuntimeError(f"guest {node}: {error}; repair allowance exhausted") from error
            repairs += 1

        while True:
            # Numbering visits parents first. A missing ancestor is restored before
            # any descendant can consume its new ticket. Siblings keep their UUIDs.
            for node in range(1, len(PARENTS)):
                if node in machines:
                    continue
                parent = PARENTS[node]
                if parent:
                    parent_machine, _, admitted = machines[parent]
                    try:
                        parent_machine.live()
                    except OSError as error:
                        failed(parent, error)
                        break
                    require(node in admitted[1], f"parent {parent} did not issue child {node}")
                    incoming = admitted[0]
                else:
                    incoming = initial
                attempt = attempts[node] = attempts.get(node, 0) + 1
                resources = ExitStack()
                startup_deadline = time.monotonic() + opt.startup_timeout
                try:
                    machine = Machine(node, args + ["-initrd", f"resident.elf {node} {incoming} {salt}"],
                                      run, resources, startup_deadline, opt.response_timeout, attempt)
                    serial = machine.through_ready(startup_deadline, opt.startup_idle_timeout)
                    admitted = verify_start(machine, serial, parent, incoming, salt, kernel_hash, elf_hash, oracle, startup_deadline)
                    require(machine.uuid not in used_uuids, "duplicate VM UUID")
                except (OSError, RuntimeError, ValueError) as error:
                    resources.close()
                    # An incorrect program, token or edge is an evidence failure.
                    # Retry only transport or liveness faults, never a contrary result.
                    if not isinstance(error, OSError):
                        raise
                    failed(node, error)
                    break
                except BaseException:
                    resources.close()
                    raise
                cleanup.callback(resources.close)
                machines[node] = machine, resources, admitted
                used_uuids.add(machine.uuid)
                log("admitted", node=node, parent=parent, input=incoming, ticket=admitted[0],
                    uuid=machine.uuid, pid=machine.child.pid, children=admitted[1], attempt=attempt)
            else:
                # Every node exists. Collect a full fresh cycle after any loss.
                rows = []
                for node in range(1, len(PARENTS)):
                    machine = machines[node][0]
                    choices = [p for p in range(len(oracle)) if p != previous.get(node)]
                    prompt = secrets.choice(choices)
                    try:
                        answer = machine.challenge(prompt, oracle)
                    except OSError as error:
                        failed(node, error)
                        break
                    rows.append(dict(node=node, parent=PARENTS[node], uuid=machine.uuid, **answer))
                    previous[node] = prompt
                else:
                    for node, (machine, _, _) in machines.items():
                        try:
                            machine.live()
                        except OSError as error:
                            failed(node, error)
                            break
                    else:
                        cycle += 1
                        snapshot(run / "readback.json", dict(time=stamp(), cycle=cycle, repairs=repairs, nodes=rows))
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
    parser.add_argument("--repairs", type=int, default=0, help="maximum branch restart attempts per run")
    parser.add_argument("--startup-timeout", type=int, default=DEFAULT_STARTUP_TIMEOUT)
    parser.add_argument("--startup-idle-timeout", type=int, help="seconds without new serial bytes while waiting for startup READY")
    parser.add_argument("--response-timeout", type=int, default=DEFAULT_RESPONSE_TIMEOUT)
    parser.add_argument("--once", action="store_true", help="run one complete readback and shut down")
    opt = parser.parse_args()
    if min(opt.interval, opt.startup_timeout, opt.response_timeout) < 1:
        parser.error("interval and timeouts must be positive")
    if opt.startup_idle_timeout is not None and opt.startup_idle_timeout < 1:
        parser.error("startup idle timeout must be positive")
    if opt.repairs < 0:
        parser.error("repairs cannot be negative")
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
