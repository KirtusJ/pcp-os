"""Disposable register identity and repair-serialization negative controls."""
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch

from pcpos import base, store
from pcpos.store import Workspace


class RegisterIntegrity(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix="pcpos-register-integrity-")
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def test_normal_append_and_incomplete_tail_repair(self):
        ws = Workspace.create(self.root / "normal")
        ev, _ = ws.append("NOTE", {"text": "valid"}, "P")
        self.assertEqual(ev["seq"], 0)
        with ws.register_path.open("ab") as out:
            out.write(b'{"incomplete":true')
            out.flush(); os.fsync(out.fileno())
        self.assertEqual(ws.repair(), len(b'{"incomplete":true'))
        self.assertEqual(len(ws.events()[0]), 1)
        self.assertEqual(ws.repair(), 0)

    def test_register_symlink_and_hardlink_are_refused(self):
        for kind in ("symlink", "hardlink"):
            with self.subTest(kind=kind):
                folder = self.root / kind
                ws = Workspace.create(folder / "ws")
                exterior = folder / "outside.jsonl"
                exterior.write_bytes(ws.register_path.read_bytes())
                ws.register_path.unlink()
                if kind == "symlink":
                    try:
                        ws.register_path.symlink_to(exterior)
                    except OSError as error:
                        self.skipTest("symbolic links unavailable: %s" % error)
                else:
                    os.link(exterior, ws.register_path)
                old = exterior.read_bytes()
                for action in (ws.events, lambda: ws.append("NOTE", {"text": "external"}, "P"), ws.repair):
                    with self.assertRaises(base.PcposError) as caught:
                        action()
                    self.assertEqual(caught.exception.code, base.INTEGRITY)
                    self.assertEqual(exterior.read_bytes(), old)

    def test_repair_waits_for_writer_and_preserves_completed_event(self):
        ws = Workspace.create(self.root / "race")
        event = {"seq": 0, "prev": base.ZERO, "ts": base.now_utc(),
                 "actor": "P", "auth": "DECLARED", "kind": "NOTE", "body": {"text": "full"}}
        line = base.canon(event) + b"\n"
        split = len(line) // 2
        lock = ws._lock()
        entered_replace = threading.Event()
        release_replace = threading.Event()
        outcomes = {}
        prior = store.atomic_write

        def gate(path, data):
            entered_replace.set()
            if not release_replace.wait(3):
                raise RuntimeError("injected gate timed out")
            return prior(path, data)

        def runner():
            try:
                outcomes["removed"] = ws.repair()
            except Exception as error:
                outcomes["error"] = repr(error)

        try:
            with ws.register_path.open("ab") as handle:
                handle.write(line[:split]); handle.flush(); os.fsync(handle.fileno())
            with patch.object(store, "atomic_write", side_effect=gate):
                thread = threading.Thread(target=runner, daemon=True)
                thread.start()
                # On the old code repair reads and reaches atomic_write here,
                # despite the writer holding the cooperative lock.
                bypassed_lock = entered_replace.wait(0.4)
                with ws.register_path.open("ab") as handle:
                    handle.write(line[split:]); handle.flush(); os.fsync(handle.fileno())
                self.assertEqual(len(ws.events()[0]), 1)
                lock.unlink()
                release_replace.set()
                thread.join(3)
            self.assertFalse(thread.is_alive())
            self.assertNotIn("error", outcomes)
            self.assertFalse(bypassed_lock, "repair bypassed the held writer lock")
            self.assertEqual(outcomes.get("removed"), 0)
            self.assertEqual(len(ws.events()[0]), 1)
        finally:
            release_replace.set()
            lock.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
