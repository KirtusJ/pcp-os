#!/usr/bin/env python3
"""Observe one opt-in PCP OS virtio Ethernet disconnect/reconnect cycle in QEMU.

This test uses ONLY a private QEMU user-mode NIC and local HTTP fixture.
QMP set_link changes virtual carrier; it is not a native OS network control.
A QMP acknowledgement is a control receipt, not proof that guest software
perceived the change. The observations are bounded to this test boot.
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hashlib
import json
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time


class QMP:
    def __init__(self, path, journal):
        self.journal = journal
        stop = time.monotonic() + 12
        while time.monotonic() < stop:
            try:
                self.sock = socket.socket(socket.AF_UNIX)
                self.sock.settimeout(0.5)
                self.sock.connect(str(path))
                break
            except (ConnectionError, FileNotFoundError, TimeoutError, OSError):
                try:
                    self.sock.close()
                except Exception:
                    pass
                time.sleep(0.05)
        else:
            raise RuntimeError("QMP server not ready")
        self.sock.settimeout(7)
        self.file = self.sock.makefile("rwb")
        greeting = self.next()
        if "QMP" not in greeting:
            raise RuntimeError("unrecognized QMP greeting")
        self.journal.append({"kind": "greeting", "data": greeting})
        self.call("qmp_capabilities")

    def next(self):
        return json.loads(self.file.readline().decode("utf-8"))

    def call(self, command, arguments=None):
        request = {"execute": command, "id": len(self.journal)}
        if arguments is not None:
            request["arguments"] = arguments
        self.file.write((json.dumps(request) + "\n").encode("utf-8"))
        self.file.flush()
        while True:
            result = self.next()
            if result.get("id") == request["id"]:
                self.journal.append({"kind": "call", "request": request, "reply": result})
                if "error" in result:
                    raise RuntimeError(f"QMP {command} refused: {result['error']}")
                return result
            self.journal.append({"kind": "asynchronous", "reply": result})

    def hmp(self, line):
        return self.call("human-monitor-command", {"command-line": line})

    def link(self, active):
        return self.call("set_link", {"name": "nic0", "up": bool(active)})

    def close(self):
        try:
            self.file.close()
        finally:
            self.sock.close()


def wait_serial(path, marker, count=1, seconds=35.0, child=None):
    stop = time.monotonic() + seconds
    while time.monotonic() < stop:
        text = path.read_text(errors="replace") if path.exists() else ""
        if text.count(marker) >= count:
            return text
        if child is not None and child.poll() is not None:
            raise RuntimeError(f"QEMU exited before {marker!r}: {text[-2000:]}")
        time.sleep(0.05)
    raise RuntimeError(f"serial timeout for {marker!r}: {text[-2000:]}")


def wait_any_serial(path, markers, seconds=40.0, child=None):
    stop = time.monotonic() + seconds
    while time.monotonic() < stop:
        transcript = path.read_text(errors="replace") if path.exists() else ""
        for marker in markers:
            if marker in transcript:
                return marker
        if child is not None and child.poll() is not None:
            raise RuntimeError("QEMU ended before observed guest outcome")
        time.sleep(0.05)
    raise RuntimeError("no bounded guest outcome observed: " + " | ".join(markers))


def run(args, directory):
    directory.mkdir(parents=True)
    report = {
        "scope": "single disposable QEMU x86_64 PCP kernel and virtual virtio NIC",
        "transport": "QEMU user-mode network to host-loopback fixture only",
        "wireless_tested": False,
        "real_ethernet_hardware_tested": False,
        "kernel_link_manager_tested": False,
        "in_session_network_reconnect_established": False,
        "network_firewall_established": False,
        "result": "INCOMPLETE",
    }
    qmp_journal = []
    urls = []
    qemu = None
    qmp = None

    class Fixture(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *args):
            pass

        def do_GET(self):
            urls.append(self.path)
            if self.path not in ("/harbour", "/bar"):
                self.send_error(404)
                return
            html = (
                "<title>Harbour</title><p>Tide checked</p>"
                '<a href="/bar">Bar</a>'
                if self.path == "/harbour" else
                "<title>Bar</title><p>Result checked</p>"
            ).encode("ascii")
            self.send_response_only(200)
            self.send_header("Content-Length", str(len(html)))
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(html)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.daemon_threads = True
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        shutil.copyfile(args.kernel, directory / "kernel.elf32")
        shutil.copyfile(args.guest, directory / "surf.elf")
        digest = hashlib.sha256(args.guest.read_bytes()).hexdigest()
        measure = subprocess.check_output(
            [sys.executable, args.measure, args.kernel], text=True).strip()
        (directory / "net.cfg").write_bytes(
            server.server_port.to_bytes(2, "big") + secrets.token_bytes(10)
        )
        # QEMU only exposes a userspace virtual NIC; do not bridge to actual
        # hardware. The fixture is bound exclusively to host loopback.
        cmd = [
            args.qemu, "-cpu", "max", "-accel", "tcg",
            "-m", "256M", "-vga", "std", "-display", "none", "-no-reboot",
            "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-audiodev", "none,id=out", "-device", "AC97,audiodev=out",
            "-serial", "file:serial.log", "-qmp", "unix:qmp.sock,server=on,wait=off",
            "-kernel", "kernel.elf32", "-initrd", "surf.elf,net.cfg",
            "-append", f"image={measure} net_image={digest}",
            "-netdev", "user,id=fixture,ipv6=off",
            "-device", "virtio-net-pci,id=nic0,netdev=fixture,disable-modern=on,vectors=0,mac=02:00:00:00:00:01",
            "-object", "filter-dump,id=trace,netdev=fixture,file=frames.pcap",
        ]
        report["kernel_measurement_expected"] = measure
        report["guest_digest"] = digest
        report["network_target_port"] = server.server_port
        stderr = (directory / "qemu.stderr").open("wb")
        try:
            qemu = subprocess.Popen(
                cmd, cwd=directory, stdout=subprocess.DEVNULL,
                stderr=stderr, stdin=subprocess.DEVNULL
            )
            qmp = QMP(directory / "qmp.sock", qmp_journal)
            serial = directory / "serial.log"
            initial_up = args.arm == "up-baseline"
            qmp.link(initial_up)
            report["first_virtual_link_command"] = "on" if initial_up else "off"
            report["arm"] = args.arm
            wait_serial(serial, "a program that cannot be admitted", child=qemu)
            time.sleep(0.40)
            qmp.hmp("sendkey ret 70")
            if initial_up:
                first = wait_any_serial(serial, ("SURFBIRD ready", "surf.elf ended FAILED"),
                                        seconds=55, child=qemu)
                report["first_guest_observation"] = first
                report["http_before_any_link_transition"] = list(urls)
                if first != "SURFBIRD ready" or urls != ["/harbour"]:
                    raise AssertionError(
                        "positive never-disconnected Ethernet control did not open HTTP"
                    )
                report["http_on_normal_link"] = list(urls)
                report["normal_guest_connected"] = True
            else:
                first = wait_any_serial(serial, ("SURFBIRD error ", "surf.elf ended FAILED"),
                                        seconds=55, child=qemu)
                wait_serial(serial, "surf.elf ended FAILED", child=qemu, seconds=15)
                report["first_guest_observation"] = first
                transcript = serial.read_text(errors="replace")
                report["first_guest_error"] = next(
                    (line.strip() for line in transcript.splitlines()
                     if "SURFBIRD error " in line), "NO_APPLICATION_ERROR"
                )
                report["first_guest_exit"] = next(
                    (line.strip() for line in transcript.splitlines()
                     if "surf.elf ended FAILED" in line), "NO_KERNEL_END_EVENT"
                )
                report["first_guest_refused_instruction"] = (
                    "processor refused an act of the program" in report["first_guest_exit"]
                )
                report["http_before_link_restored"] = list(urls)
                # The boot might have an actual kernel-reported user fault.
                # Preserve and investigate it, never relabel it a TCP timeout.
                if urls:
                    raise AssertionError("HTTP reached fixture despite QMP carrier-down")
                qmp.link(True)
                report["second_virtual_link_command"] = "on"
                time.sleep(0.35)
                # This is a NEW app process, not a connected socket resuming.
                qmp.hmp("sendkey ret 70")
                # Wait for second program start and for a completed page/error.
                wait_serial(serial, "starting surf.elf", count=2, child=qemu)
                second = wait_any_serial(serial, ("SURFBIRD ready",),
                                         seconds=55, child=qemu)
                report["second_guest_observation"] = second
                if urls != ["/harbour"]:
                    raise AssertionError(f"wrong HTTP after virtual carrier restored: {urls}")
                report["fresh_guest_after_link_up"] = True
                report["http_after_link_restored"] = list(urls)
            qmp.hmp("sendkey esc 70")
            wait_serial(serial, "surf.elf ended DONE", child=qemu, seconds=20)
            time.sleep(0.35)
            qmp.hmp("sendkey p 70")
            report["qemu_exit_code"] = qemu.wait(timeout=20)
            if report["qemu_exit_code"] != 33:
                raise AssertionError("guest did not shut down with expected self-test code")
            transcript = serial.read_text(errors="replace")
            if "kernel's register of its own start, verification and stop verifies" not in transcript:
                raise AssertionError("native kernel's final register verification missing")
            report["verified_register_at_shutdown"] = True
            if initial_up:
                report["result"] = "POSITIVE_BASELINE_VIRTUAL_ETHERNET"
            elif report["first_guest_refused_instruction"]:
                report["result"] = "ADVERSE_FIRST_GUEST_FAULT_WITH_FRESH_PROCESS_RECOVERY"
                raise AssertionError(
                    "down-link guest terminated by processor fault; this is not a clean disconnect"
                )
            else:
                report["result"] = "FRESH_PROCESS_AFTER_LINK_UP_NOT_IN_SESSION_RECONNECT"
        finally:
            if qmp is not None:
                qmp.close()
            if qemu is not None and qemu.poll() is None:
                qemu.kill()
                qemu.wait(timeout=6)
            stderr.close()
    except Exception as exc:
        report["error"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=4)
        report["qmp_commands"] = [
            e["request"] for e in qmp_journal if e.get("kind") == "call"
        ]
        report["http_fixture_paths"] = urls
        report["qmp_replies_error_free"] = all(
            "error" not in e["reply"] for e in qmp_journal if e.get("kind") == "call"
        )
        (directory / "qmp-journal.json").write_text(
            json.dumps(qmp_journal, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        (directory / "result.json").write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    print(json.dumps(report, sort_keys=True))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--kernel", type=Path, required=True)
    p.add_argument("--guest", type=Path, required=True)
    p.add_argument("--measure", type=Path, required=True)
    p.add_argument("--qemu", default="qemu-system-x86_64")
    p.add_argument("--arm", choices=("up-baseline", "down-up"), required=True,
                   help="separate positive control from carrier-change trial")
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    run(a, a.output.resolve())


if __name__ == "__main__":
    main()
