"""Replay Surfbird against a loopback HTTP fixture over guest Ethernet or a serial cable."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
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
from urllib.parse import urlsplit
from packet_trace import verify as verify_trace


def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def hosted_check(binary, port, targets, tag):
    """Drive the published reader's actual JSON interface over the same fixture."""
    origin = f"http://127.0.0.1:{port}"
    with subprocess.Popen([binary, "--listen", "127.0.0.1:0"], stdin=subprocess.PIPE,
                          stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True) as browser:
        try:
            prefix = "Machine session 127.0.0.1:"
            announcement = browser.stderr.readline().strip()
            require(announcement.startswith(prefix), "Hosted listener missing")
            with socket.create_connection(("127.0.0.1", int(announcement[len(prefix):])), timeout=8) as s, s.makefile("rwb") as stream:
                def act(op, **fields):
                    stream.write((json.dumps(dict(op=op, **fields)) + "\n").encode())
                    stream.flush()
                    reply = json.loads(stream.readline(65536))
                    require(reply["ok"], reply)
                    return reply["state"]
                home = act("open", url=origin + "/harbour")
                p = home["page"]
                require((p["title"], p["lines"][0]) == ("Harbour", f"Tide {tag}"), p)
                require([(l["label"], urlsplit(l["url"]).path) for l in p["links"]] ==
                        list(zip(["Bar", "Jetty"], targets)), p)
                for index, title in [(2, "Jetty"), (1, "Bar")]:
                    dest = act("follow", link=index, revision=home["revision"])["page"]
                    require((urlsplit(dest["url"]).path, dest["title"], dest["lines"]) ==
                            (targets[index-1], title, [f"Report {tag} {title}"]), dest)
                    home = act("back")
                    require(home["page"] == p, home)
        finally:
            browser.stdin.close()
            try:
                browser.wait(timeout=5)
            except subprocess.TimeoutExpired:
                browser.kill()


def run_arm(args, work, scenario):
    work.mkdir()
    for source, name in [(args.kernel, "kernel.elf32"), (args.guest, "surf.elf")]:
        shutil.copyfile(source, work / name)
    tag = None
    targets = []
    pages = {}
    requests = []

    class Fixture(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *args):
            pass

        def handle(self):
            try:
                super().handle()
            except ConnectionResetError:
                # QEMU may reset its virtual cable when the VM powers off.
                pass

        def do_GET(self):
            nonlocal tag
            if tag is None and self.path == "/harbour":
                tag = secrets.token_hex(6)
                targets.extend([f"/bar-{tag}", f"/jetty-{tag}"])
                pages["/harbour"] = f'<title>Harbour</title><p>Tide {tag}</p><a href="{targets[0]}">Bar</a><a href="{targets[1]}">Jetty</a>'
                for title, target in zip(["Bar", "Jetty"], targets):
                    pages[target] = f"<title>{title}</title><p>Report {tag} {title}</p>"
            requests.append((self.path, self.headers.get("Host")))
            if self.path not in pages or self.headers.get("Host") not in (
                    "fixture.test", f"127.0.0.1:{self.server.server_port}"):
                self.send_error(404)
                return
            body = pages[self.path].encode("ascii")
            self.send_response_only(200)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            if scenario == "ambiguous":
                self.send_header("Transfer-Encoding", "chunked")
            if scenario == "space-before-colon":
                self.flush_headers()
                self.wfile.write(b"Transfer-Encoding : chunked\r\n")
            if scenario == "folded":
                self.flush_headers()
                self.wfile.write(b"X-Fixture: one\r\n\tTransfer-Encoding: chunked\r\n")
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.daemon_threads = True
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    tools = Path(__file__).resolve().parents[3] / "PCP_OS_Rust/crates/pcpos-kernel/tools"
    measure = subprocess.check_output([sys.executable, str(tools / "measure.py"), str(args.kernel)], text=True).strip()
    digest = hashlib.sha256(args.guest.read_bytes()).hexdigest()
    pin = "0" * 64 if scenario == "denied" else digest
    port = server.server_port
    refused = None
    if scenario == "refused":
        # Reserve a port without a listener. NAT may return RST or drop a SYN.
        refused = socket.socket()
        refused.bind(("127.0.0.1", 0))
        port = refused.getsockname()[1]
    cmd = [args.qemu]
    if args.qemu_data:
        cmd.extend(["-L", str(args.qemu_data)])
    cmd.extend(["-cpu", "max", "-accel", "tcg", "-m", "256M", "-vga", "std", "-display", "none",
                "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
                "-audiodev", "none,id=out", "-device", "AC97,audiodev=out",
                "-serial", "file:serial.log", "-monitor", "stdio", "-kernel", "kernel.elf32"])
    if args.tcp:
        # Distinct boot entropy for smoltcp and ephemeral-port selection.
        (work / "net.cfg").write_bytes(port.to_bytes(2, "big") + secrets.token_bytes(10))
        cmd.extend(["-netdev", "user,id=fixture,ipv6=off",
                    "-device", "virtio-net-pci,netdev=fixture,disable-modern=on,vectors=0,mac=02:00:00:00:00:01",
                    "-object", "filter-dump,id=trace,netdev=fixture,file=guest.pcap",
                    "-initrd", "surf.elf,net.cfg", "-append", f"image={measure} net_image={pin}"])
    else:
        cmd.extend(["-nic", "none", "-serial", f"tcp:127.0.0.1:{server.server_port}",
                    "-initrd", "surf.elf", "-append", f"image={measure} link_image={pin}"])
    log = work / "serial.log"

    with (work / "stderr.log").open("wb") as errors:
        try:
            with subprocess.Popen(cmd, cwd=work, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                  stderr=errors) as vm:
                def wait(marker, count=1):
                    until = time.monotonic() + 20
                    while time.monotonic() < until:
                        data = log.read_text(errors="replace") if log.exists() else ""
                        if data.count(marker) >= count:
                            return
                        if vm.poll() is not None or "it is in SAFE" in data or (
                                "SURFBIRD error " in data and "SURFBIRD error " not in marker
                                and scenario == "allowed"):
                            break
                        time.sleep(.04)
                    raise RuntimeError(f"Missing {marker!r} in {work}\n{data[-1300:]}")

                def key(name):
                    vm.stdin.write(f"sendkey {name} 70\n".encode())
                    vm.stdin.flush()
                    time.sleep(.15)

                try:
                    wait("a program that cannot be admitted")
                    time.sleep(.55)
                    key("ret")
                    if scenario == "allowed":
                        wait("SURFBIRD ready")
                        wait("SURFBIRD title Harbour")
                        wait(f"SURFBIRD text Tide {tag}")
                        wait(f"SURFBIRD link1 {targets[0]}")
                        wait(f"SURFBIRD link2 {targets[1]}")
                        key("2")
                        wait("SURFBIRD title Jetty")
                        wait(f"SURFBIRD text Report {tag} Jetty")
                        wait("SURFBIRD ready", 2)
                        key("b")
                        wait(f"SURFBIRD text Tide {tag}", 2)
                        wait("SURFBIRD ready", 3)
                        key("1")
                        wait("SURFBIRD title Bar")
                        wait(f"SURFBIRD text Report {tag} Bar")
                        wait("SURFBIRD ready", 4)
                        for n in range(args.extra_visits):
                            key("b")
                            wait("SURFBIRD ready", 5 + n*2)
                            key("1")
                            wait("SURFBIRD ready", 6 + n*2)
                        key("esc")
                        wait("surf.elf ended DONE")
                        expected = ["/harbour", targets[1], targets[0]] + [targets[0]]*args.extra_visits
                    elif scenario == "denied":
                        wait("SURFBIRD error Ethernet grant unavailable" if args.tcp else "SURFBIRD error cable grant unavailable")
                        expected = []
                    elif scenario == "refused":
                        wait("SURFBIRD error TCP connect")
                        require("SURFBIRD error TCP connection refused" in log.read_text() or
                                "SURFBIRD error TCP connect timeout" in log.read_text(), "Unexpected connection failure")
                        expected = []
                    else:
                        reason = "unsupported encoding" if scenario == "ambiguous" else "HTTP header syntax"
                        wait("SURFBIRD error " + reason)
                        expected = ["/harbour"]
                    time.sleep(.35)
                    key("p")
                    require(vm.wait(timeout=10) == 33, "Kernel did not power off normally")
                    require(requests == [(p, "fixture.test") for p in expected], requests)
                    frames = 4 + 2*args.extra_visits if scenario == "allowed" else 0
                    require(f"; {frames} frames shown (" in log.read_text(), "Unexpected rendered frame count")
                    require("kernel's register of its own start, verification and stop verifies" in log.read_text(), "Register verification missing")
                    if args.hosted and scenario == "allowed":
                        hosted_check(args.hosted, server.server_port, targets, tag)
                        require(requests[len(expected):] == [(p,f"127.0.0.1:{server.server_port}") for p in expected[:3]], requests)
                    if scenario != "allowed":
                        require("SURFBIRD ready" not in log.read_text(), "Unexpected page publication")
                finally:
                    if vm.poll() is None:
                        vm.kill()
        finally:
            if refused is not None:
                refused.close()
            server.shutdown()
            server.server_close()
            worker.join()
    ethernet = verify_trace(work/"guest.pcap", port, expected, scenario, pages) if args.tcp else None
    if scenario == "refused":
        reason = "TCP connection refused" if ethernet["reset"] else "TCP connect timeout"
        require("SURFBIRD error " + reason in log.read_text(), "guest verdict differs from capture")
    print(json.dumps({"arm": scenario, "kernel_measurement": measure, "guest_sha256": digest,
                      "fixture_nonce": tag, "http_requests": requests, "serial": str(log),
                      "ethernet":ethernet}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=lambda s: Path(s).resolve())
    parser.add_argument("guest", type=lambda s: Path(s).resolve())
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--qemu-data", type=Path)
    parser.add_argument("--tcp", action="store_true", help="use a guest-ethernet kernel, TCP-enabled guest and a virtual NIC instead of the COM2 cable")
    parser.add_argument("--arm", choices=["allowed","denied","ambiguous","space-before-colon","folded","refused"],
                        help="run one arm and preserve its new logs separately")
    parser.add_argument("--extra-visits", type=int, default=0,
                        help="repeat Back and link 1 after the ordinary navigation for ring-reuse testing")
    parser.add_argument("--hosted", type=lambda s: Path(s).resolve(), help="cross-check the same pages with the hosted Surfbird executable")
    parser.add_argument("--output", type=Path, help="parent directory for unique run logs")
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix="surfbird-", dir=args.output))
    if args.arm == "refused" and not args.tcp:
        parser.error("the closed-port arm requires --tcp")
    if args.extra_visits < 0:
        parser.error("--extra-visits must be nonnegative")
    for scenario in ([args.arm] if args.arm else ["allowed", "denied", "ambiguous", "space-before-colon", "folded"] + (["refused"] if args.tcp else [])):
        run_arm(args, root / scenario, scenario)


if __name__ == "__main__":
    main()
