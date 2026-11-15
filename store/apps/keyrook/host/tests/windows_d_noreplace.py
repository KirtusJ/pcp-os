"""Independent Win32 D: no-replace primitive check for Keyrook exports.

Run on a Windows host with a writable PCP D: workspace:
    set KEYROOK_TEST_ROOT=D:\work\some-writable-folder
    python store\apps\keyrook\host\tests\windows_d_noreplace.py

This checks MoveFileExW flags 0x8 and writable-handle file syncing on
the actual drive. It is NOT a Keyrook Rust binary or power-loss test.
The Keyrook CLI integration replay uses the same KEYROOK_TEST_ROOT plus
KEYROOK_REQUIRE_D=1 and is separately required.
"""
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import tempfile

MOVEFILE_WRITE_THROUGH = 0x8  # no COPY_ALLOWED, no REPLACE_EXISTING


def main():
    if os.name != "nt":
        raise RuntimeError("Windows D: test requires Windows, not an emulation")
    root_text = os.environ.get("KEYROOK_TEST_ROOT")
    if not root_text:
        raise RuntimeError("set KEYROOK_TEST_ROOT to an existing writable D: directory")
    root = Path(root_text).resolve(strict=True)
    if root.drive.upper() != "D:" or not root.is_dir():
        raise RuntimeError("KEYROOK_TEST_ROOT is not an actual D: directory")
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    move = kernel32.MoveFileExW
    move.argtypes = (wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD)
    move.restype = wintypes.BOOL
    with tempfile.TemporaryDirectory(prefix="keyrook-d-noreplace-", dir=root) as work:
        folder = Path(work)
        stage = folder / ".stage.sqlite"
        target = folder / "out.sqlite"
        stage.write_bytes(b"complete staged output")
        target.write_bytes(b"DO NOT REPLACE")
        with stage.open("r+b") as handle:
            os.fsync(handle.fileno())  # Windows writable-handle flush
        ctypes.set_last_error(0)
        refused = move(str(stage), str(target), MOVEFILE_WRITE_THROUGH)
        existing_error = ctypes.get_last_error()
        assert not refused and existing_error != 0, (refused, existing_error)
        assert target.read_bytes() == b"DO NOT REPLACE"
        assert stage.read_bytes() == b"complete staged output"
        target.unlink()
        ctypes.set_last_error(0)
        moved = move(str(stage), str(target), MOVEFILE_WRITE_THROUGH)
        assert moved, ctypes.get_last_error()
        assert not stage.exists()
        assert target.read_bytes() == b"complete staged output"
        print(json.dumps({
            "platform": "Windows", "target_drive": root.drive,
            "flags": MOVEFILE_WRITE_THROUGH,
            "existing_destination_refused": True,
            "existing_destination_error": existing_error,
            "source_preserved_on_refusal": True,
            "same_volume_move_succeeded": True,
            "source_removed_after_success": True,
            "writable_handle_sync_succeeded": True,
            "result": "PASS",
            "scope": "Win32 API on tested D: filesystem; not compiled Keyrook",
        }, sort_keys=True))


if __name__ == "__main__":
    main()
