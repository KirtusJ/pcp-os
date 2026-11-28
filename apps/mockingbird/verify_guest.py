import json, os, socket, sys, threading, time
from pathlib import Path
from tempfile import TemporaryDirectory

if len(sys.argv) != 2:
    raise SystemExit("Usage: python verify_guest.py BUILT_PACKAGE")
sys.path.insert(0, str(Path(sys.argv[1]).resolve()))
import launch


class Fixture:
    def __init__(self):
        self.value = b"world\nPCP"
        self.writes = []

    def get(self):
        print("Sending", len(self.value), "bytes", flush=True)
        return self.value

    def put(self, v):
        self.value = v
        self.writes.append(v)
        print("Received", len(v), "bytes", flush=True)


with TemporaryDirectory() as temp:
    f = Fixture()
    log = Path(temp) / "guest.log"
    os.environ["PCPOS_EDITOR_DIAGNOSTICS"] = "file:" + str(log)
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    t = threading.Thread(
        target=launch.launch,
        args=(f, ["-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off"], False),
        daemon=True,
    )
    t.start()
    for i in range(200):
        try:
            s = socket.create_connection(("127.0.0.1", port), timeout=2)
            break
        except OSError:
            if not t.is_alive():
                raise RuntimeError("VM exited during start")
            time.sleep(0.1)
    s.settimeout(8)
    wire = s.makefile("rwb", buffering=0)

    def cmd(name, args={}):
        wire.write((json.dumps({"execute": name, "arguments": args}) + "\n").encode())
        while True:
            r = json.loads(wire.readline())
            if "return" in r:
                return r["return"]
            if "error" in r:
                raise RuntimeError(r)

    print(wire.readline()[:24], flush=True)
    cmd("qmp_capabilities")

    def key(*keys):
        cmd(
            "send-key",
            {"keys": [{"type": "qcode", "data": k} for k in keys], "hold-time": 60},
        )
        time.sleep(0.19)

    def check(value):
        for i in range(1500):
            if f.writes and f.writes[-1] == value:
                return
            time.sleep(0.1)
        raise AssertionError((len(value), [(len(v), v[:15]) for v in f.writes]))

    try:
        for i in range(300):
            lines = log.read_text(errors="replace") if log.exists() else ""
            if "Mockingbird ready" in lines:
                break
            if not t.is_alive() or "in SAFE" in lines:
                raise RuntimeError(lines[-1800:])
            time.sleep(0.2)
        else:
            raise RuntimeError(lines[-2000:])
        print("Editor running, sending controls", flush=True)
        key("h")
        key("i")
        key("spc")
        key("ctrl", "v")
        time.sleep(0.6)
        key("ctrl", "a")
        key("ctrl", "x")
        check(b"hi world\nPCP")
        key("ctrl", "v")
        time.sleep(0.4)
        key("shift", "left")
        key("ctrl", "x")
        check(b"P")
        key("ctrl", "v")
        time.sleep(0.4)
        key("ctrl", "a")
        key("ctrl", "c")
        check(b"hi world\nPCP")
        u = "hello\n\u00e9\t\U0001f426".encode()
        f.value = u
        key("ctrl", "v")
        time.sleep(0.4)
        key("ctrl", "a")
        key("ctrl", "c")
        check(u)
        key("right")
        key("shift", "left")
        key("ctrl", "x")
        check("\U0001f426".encode())
        full = b"0123456789abcdef" * 2048
        f.value = full
        key("ctrl", "a")
        key("ctrl", "v")
        time.sleep(4)
        key("a")
        key("ctrl", "a")
        key("ctrl", "c")
        check(full)
        print(
            "PASS: Unicode, whole-codepoint selection, 32 KiB boundary and refused overflow",
            flush=True,
        )
        print(
            "PASS: typed, pasted multiline, cut all, cut selection, pasted, copied exact result",
            flush=True,
        )
    finally:
        try:
            cmd("quit")
        except (ValueError, OSError):
            pass
        t.join(timeout=8)
