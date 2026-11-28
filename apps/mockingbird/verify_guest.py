import json
import os
import socket
import sys
import threading
import time
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

    def cmd(name, args=None):
        wire.write(
            (json.dumps({"execute": name, "arguments": args or {}}) + "\n").encode()
        )
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

    def transfer(letter, value):
        before = len(f.writes)
        key("ctrl", letter)
        for i in range(1500):
            if len(f.writes) > before:
                assert f.writes[before] == value
                return
            time.sleep(0.1)
        raise AssertionError((len(value), [(len(v), v[:15]) for v in f.writes]))

    def screenshot(path=None, ready=None):
        path = path or Path(temp) / "screen.ppm"
        deadline = time.monotonic() + 8
        while True:
            cmd("screendump", {"filename": str(path)})
            with path.open("rb") as image:
                assert image.readline().strip() == b"P6"
                width, height = map(int, image.readline().split())
                assert image.readline().strip() == b"255"
                pixels = image.read()
            assert len(pixels) == width * height * 3
            if ready is None or ready(pixels):
                return width, height, pixels
            if time.monotonic() >= deadline:
                raise AssertionError(
                    f"Guest did not draw the expected frame for {path.name}"
                )
            time.sleep(0.05)

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
            key("ctrl", "a")
            transfer("c", expected)
            key("right")  # collapse selection at the end

        w, h, initial = screenshot()
        scale = max(1, min(2, w // 820, h // 475))
        cw, ch = 10 * scale, 20 * scale

        def color(pixels, x, y):
            start = (y * w + x) * 3
            return pixels[start : start + 3].hex()

        assert color(initial, 0, 3 * ch) == "dad6cb"
        assert color(initial, 0, 0) == "dce2e0"
        assert color(initial, 2 * cw, 3 * ch) == "1c1c1c"
        key("f6")
        _, _, dark = screenshot()
        assert color(dark, 0, 3 * ch) == "1c1c1c"
        assert color(dark, 0, 2 * ch) == "7f7e7e"
        assert color(dark, 2 * cw, 3 * ch) == "dad6cb"
        key("f6")
        assert screenshot()[2] == initial
        print(f"PASS: default light and reversible F6 at {w} x {h}", flush=True)

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
        key("ctrl", "a")
        f.value = b"a\tbcd"
        key("ctrl", "v")
        key("shift", "left")
        key("shift", "left")
        key("ctrl", "f")
        key("q")
        key("esc")
        key("shift", "right")
        transfer("c", b"d")

        key("ctrl", "home")
        key("right")
        key("shift", "right")
        transfer("c", b"\t")
        for sheet, selection in (("dad6cb", "1c1c1c"), ("1c1c1c", "b9b5b5")):
            pixels = screenshot()[2]
            for column in range(5):
                expected = selection if 1 <= column < 4 else sheet
                assert (
                    color(pixels, (2 + column) * cw + cw // 2, 3 * ch + 1) == expected
                )
            key("f6")
        transfer("c", b"\t")
        print(
            "PASS: reversed selection after a Find miss and full selected tab width",
            flush=True,
        )
        preview = Path(
            os.environ.get("MOCKINGBIRD_PREVIEW", str(Path(temp) / "find.ppm"))
        )
        note = b"Field notes\n\nMeet at the north entrance at 09:00.\nBring the field notebook and a spare pencil."
        key("ctrl", "a")
        f.value = note
        key("ctrl", "v")
        # Clipboard arrival can outlast a keypress. Wait for the actual caret
        # at the end of the pasted note before accepting a preview.
        last_line = note.splitlines()[-1]
        caret_x = (2 + len(last_line)) * cw
        caret_y = 6 * ch
        screenshot(
            preview.with_stem(preview.stem + "-sheet"),
            lambda px: (
                color(px, 0, caret_y) == "dad6cb"
                and color(px, caret_x, caret_y) == "1c1c1c"
            ),
        )
        key("f6")
        screenshot(
            preview.with_stem(preview.stem + "-dark-sheet"),
            lambda px: (
                color(px, 0, caret_y) == "1c1c1c"
                and color(px, caret_x, caret_y) == "dad6cb"
            ),
        )
        key("ctrl", "z")
        document(b"a\tbcd")
        key("ctrl", "y")
        document(note)
        key("ctrl", "f")
        for letter in "pen":
            key(letter)
        key("ctrl", "a")  # Both the query and its document match are selected.
        dark_find = screenshot(preview.with_stem(preview.stem + "-dark"))[2]
        query_x = 7 * cw + 4 * scale
        query_y = h - 3 * ch + 6 * scale
        assert color(dark_find, query_x, query_y) == "b9b5b5"
        assert color(dark_find, query_x + 3 * cw, query_y) == "dad6cb"
        key("f6")
        light_find = screenshot(preview)[2]
        assert color(light_find, query_x, query_y) == "1c1c1c"
        assert color(light_find, query_x + 3 * cw, query_y) == "1c1c1c"
        key("f6")
        assert screenshot()[2] == dark_find
        key("esc")
        transfer("c", b"pen")
        key("esc")
        dark_prompt = screenshot()[2]
        key("f6")
        key("f6")
        assert screenshot()[2] == dark_prompt
        key("esc")
        document(note)
        print(
            "PASS: theme switching preserves selection, Find, undo, redo and discard prompt",
            flush=True,
        )
        cols = (w - 4 * cw) // cw
        f.value = b"x" * cols + b"\ny"
        key("ctrl", "a")
        key("ctrl", "v")
        document(f.value)
        screenshot(ready=lambda px: color(px, 3 * cw, 4 * ch) == "dad6cb")
        f.value = b"abcde\nx\nABCDE"
        key("ctrl", "a")
        key("ctrl", "v")
        document(f.value)
        key("up")
        key("up")
        key("shift", "home")
        transfer("c", b"abcde")
        print(
            "PASS: exact-width newline and vertical navigation across a short line",
            flush=True,
        )
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
        transfer("x", b"hi world\nPCP")
        key("ctrl", "v")
        time.sleep(0.4)
        key("shift", "left")
        transfer("x", b"P")
        key("ctrl", "v")
        time.sleep(0.4)
        key("ctrl", "a")
        transfer("c", b"hi world\nPCP")
        u = "hello\n\u00e9\t\U0001f426".encode()
        f.value = u
        key("ctrl", "v")
        time.sleep(0.4)
        key("ctrl", "a")
        transfer("c", u)
        key("right")
        key("shift", "left")
        transfer("x", "\U0001f426".encode())
        full = b"0123456789abcdef" * 2048
        f.value = full
        key("ctrl", "a")
        key("ctrl", "v")
        time.sleep(4)
        key("a")
        key("ctrl", "a")
        transfer("c", full)
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
