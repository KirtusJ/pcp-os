"""Windows launcher and clipboard adapter for the PCP OS editor."""

from pathlib import Path
import ctypes as c
from ctypes import wintypes as w
import os, shutil, socket, struct, subprocess, tempfile, time

HERE = Path(__file__).resolve().parent
QEMU = Path(
    os.environ.get("PCPOS_QEMU")
    or shutil.which("qemu-system-x86_64")
    or Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "qemu/qemu-system-x86_64.exe"
)
CAP = 32768


class WindowsClipboard:
    def __init__(self):
        self.u = c.windll.user32
        self.k = c.windll.kernel32
        for f, args, ret in [
            (self.u.GetClipboardData, [w.UINT], w.HANDLE),
            (self.u.SetClipboardData, [w.UINT, w.HANDLE], w.HANDLE),
            (self.k.GlobalSize, [w.HANDLE], c.c_size_t),
            (self.k.GlobalAlloc, [w.UINT, c.c_size_t], w.HANDLE),
            (self.k.GlobalLock, [w.HANDLE], c.c_void_p),
            (self.k.GlobalUnlock, [w.HANDLE], w.BOOL),
            (self.k.GlobalFree, [w.HANDLE], w.HANDLE),
            (
                self.u.CreateWindowExW,
                [
                    w.DWORD,
                    w.LPCWSTR,
                    w.LPCWSTR,
                    w.DWORD,
                    c.c_int,
                    c.c_int,
                    c.c_int,
                    c.c_int,
                    w.HWND,
                    w.HMENU,
                    w.HINSTANCE,
                    c.c_void_p,
                ],
                w.HWND,
            ),
            (self.u.OpenClipboard, [w.HWND], w.BOOL),
        ]:
            f.argtypes = args
            f.restype = ret
        self.owner = self.u.CreateWindowExW(
            0, "STATIC", "", 0, 0, 0, 0, 0, None, None, None, None
        )
        if not self.owner:
            raise OSError("Could not create clipboard owner")

    def get(self):
        if not self.u.OpenClipboard(self.owner):
            raise OSError("Clipboard busy")
        try:
            if not self.u.IsClipboardFormatAvailable(13):
                return b""
            handle = self.u.GetClipboardData(13)
            ptr = self.k.GlobalLock(handle)
            if not ptr:
                raise OSError("Clipboard read failed")
            try:
                limit = min(self.k.GlobalSize(handle) // 2, 2 * CAP + 2)
                text, sep, _ = c.wstring_at(ptr, limit).partition("\0")
                if not sep:
                    raise ValueError("Clipboard exceeds guest capacity")
                value = text.replace("\r\n", "\n").replace("\r", "\n")
                data = value.encode("utf-8")
                if len(data) > CAP:
                    raise ValueError("Clipboard exceeds guest capacity")
                return data
            finally:
                self.k.GlobalUnlock(handle)
        finally:
            self.u.CloseClipboard()

    def put(self, data):
        raw = (data.decode("utf-8").replace("\n", "\r\n") + "\0").encode("utf-16-le")
        h = self.k.GlobalAlloc(2, len(raw))
        if not h:
            raise MemoryError()
        try:
            ptr = self.k.GlobalLock(h)
            if not ptr:
                raise OSError("Clipboard allocation failed")
            try:
                c.memmove(ptr, raw, len(raw))
            finally:
                self.k.GlobalUnlock(h)
            if not self.u.OpenClipboard(self.owner):
                raise OSError("Clipboard busy")
            try:
                if not self.u.EmptyClipboard() or not self.u.SetClipboardData(13, h):
                    raise OSError("Clipboard write failed")
                h = None
            finally:
                self.u.CloseClipboard()
        finally:
            if h:
                self.k.GlobalFree(h)


def receive(pipe, n):
    buf = bytearray()
    while len(buf) < n:
        chunk = pipe.read(n - len(buf))
        if not chunk:
            raise EOFError()
        buf.extend(chunk)
    return bytes(buf)


def serve(incoming, outgoing, clipboard):
    marker = b"PCP-EDITOR-CLIP/1\n"
    seen = bytearray()
    while not seen.endswith(marker):
        b = incoming.read(1)
        if not b:
            return
        seen = (seen + b)[-len(marker) :]
    while True:
        op = incoming.read(1)
        if not op:
            return
        if op == b"G":
            try:
                value = clipboard.get()
                packet = struct.pack("<I", len(value)) + value
            except (OSError, ValueError):
                packet = b"\xff" * 4
        elif op == marker[:1] and receive(incoming, len(marker) - 1) == marker[1:]:
            continue
        elif op == b"S":
            (length,) = struct.unpack("<I", receive(incoming, 4))
            if length > CAP:
                raise ValueError("Invalid clipboard frame")
            value = receive(incoming, length)
            try:
                clipboard.put(value)
                packet = b"\x01"
            except (OSError, ValueError, MemoryError):
                packet = b"\x00"
        else:
            raise ValueError("Unknown clipboard frame")
        outgoing.write(packet)
        outgoing.flush()


def peer_is_qemu(peer, port, pid):
    # Read the Windows TCP owner table before allowing clipboard exchange.
    get_table = c.windll.iphlpapi.GetExtendedTcpTable
    size = w.DWORD()
    get_table(None, c.byref(size), False, socket.AF_INET, 5, 0)
    for _ in range(4):
        table = c.create_string_buffer(size.value)
        code = get_table(table, c.byref(size), False, socket.AF_INET, 5, 0)
        if code == 122:
            continue
        if code:
            raise OSError(code, "Cannot verify local clipboard peer")
        (count,) = struct.unpack_from("<I", table)
        for state, addr, local, remote, other, owner in struct.iter_unpack(
            "<6I", table.raw[4 : 4 + 24 * count]
        ):
            if (
                owner == pid
                and addr == remote == 0x0100007F
                and socket.ntohs(local & 65535) == peer[1]
                and socket.ntohs(other & 65535) == port
            ):
                return True
        return False
    return False


def launch(clipboard, extra=(), visible=True):
    firmware = QEMU.parent / "share"
    with (
        tempfile.TemporaryDirectory(prefix="pcpos-editor-") as temp,
        socket.socket() as server,
    ):
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        server.settimeout(0.5)
        port = server.getsockname()[1]
        vars_file = Path(temp) / "vars.fd"
        shutil.copyfile(firmware / "edk2-i386-vars.fd", vars_file)
        args = [
            str(QEMU),
            "-name",
            "Mockingbird - PCP OS",
            "-machine",
            "q35",
            "-accel",
            "tcg",
            "-cpu",
            "max",
            "-m",
            "1024",
            "-drive",
            "if=pflash,unit=0,format=raw,readonly=on,file="
            + str(firmware / "edk2-x86_64-code.fd"),
            "-drive",
            "if=pflash,unit=1,format=raw,file=" + str(vars_file),
            "-drive",
            "if=none,id=media,format=raw,readonly=on,file=fat:" + str(HERE / "media"),
            "-device",
            "virtio-blk-pci,drive=media,bootindex=1",
            "-nic",
            "none",
            "-display",
            "gtk,zoom-to-fit=on" if visible else "none",
            "-serial",
            os.environ.get("PCPOS_EDITOR_DIAGNOSTICS", "null"),
            "-serial",
            f"tcp:127.0.0.1:{port}",
            "-monitor",
            "none",
            "-no-reboot",
            *extra,
        ]
        with subprocess.Popen(
            args,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW,
        ) as guest:
            try:
                deadline = time.monotonic() + 20
                while guest.poll() is None and time.monotonic() < deadline:
                    try:
                        conn, peer = server.accept()
                    except socket.timeout:
                        continue
                    with conn:
                        if not peer_is_qemu(peer, port, guest.pid):
                            continue
                        server.close()
                        with conn.makefile("rwb") as cable:
                            try:
                                serve(cable, cable, clipboard)
                            except (
                                ConnectionResetError,
                                ConnectionAbortedError,
                                BrokenPipeError,
                                EOFError,
                            ):
                                pass
                        break
            finally:
                if guest.poll() is None:
                    guest.terminate()
            return guest.wait()


if __name__ == "__main__":
    raise SystemExit(launch(WindowsClipboard()))
