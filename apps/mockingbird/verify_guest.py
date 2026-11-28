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

        # Exercise local editing through PS/2 and observe text through a
        # synthetic clipboard. Each assertion requires a fresh guest copy.
        def document(expected):
            before = len(f.writes)
            key("ctrl", "a")
            key("ctrl", "c")
            for _ in range(100):
                if len(f.writes) > before:
                    break
                time.sleep(0.1)
            assert len(f.writes) > before and f.writes[-1] == expected
            key("right")  # collapse selection at the end

        key("a")
        key("b")
        f.value = b" fox one fox two"
        key("ctrl", "v")
        key("ctrl", "z")
        document(b"ab")
        key("ctrl", "shift", "z")
        document(b"ab fox one fox two")
        key("ctrl", "f")
        key("f")
        key("o")
        key("x")
        key("ret")
        key("esc")
        key("shift", "f3")
        key("x")
        document(b"ab x one fox two")
        key("ctrl", "z")
        key("f3")
        key("x")
        document(b"ab fox one x two")
        key("ctrl", "z")
        document(b"ab fox one fox two")
        key("ctrl", "f")
        key("f")
        key("o")
        key("x")
        key("shift", "ret")
        key("ret")
        key("esc")
        key("y")
        document(b"ab y one fox two")
        key("ctrl", "z")
        key("ctrl", "z")
        key("ctrl", "z")
        key("q")
        document(b"q")
        key("ctrl", "y")
        document(b"q")
        # The old query is now absent in this shorter document.
        key("f3")
        key("n")
        key("shift", "n")
        document(b"qnN")
        if os.environ.get("MOCKINGBIRD_PREVIEW"):
            key("ctrl", "a")
            f.value = b"Field notes\n\nMeet at the north entrance at 09:00.\nBring the field notebook and a spare pencil."
            key("ctrl", "v")
            time.sleep(0.5)
            key("ctrl", "f")
            key("p")
            key("e")
            key("n")
            cmd("screendump", {"filename": os.environ["MOCKINGBIRD_PREVIEW"]})
            key("esc")
        key("ctrl", "a")
        key("backspace")
        f.value = b"world\nPCP"
        print(
            "PASS: grouped undo, redo, redo invalidation, find next/previous and wrap",
            flush=True,
        )
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
