"""Host filesystem regression controls for the portable Python implementation.

All files are created under short-lived temporary directories. The tests
check only two named conditions: exclusive temporary files, and cooperative
writer/repair exclusion. They do not certify general path confinement.
"""
import os
from pathlib import Path
import tempfile
import threading
import unittest
from concurrent.futures import ThreadPoolExecutor, TimeoutError as FutureTimeout

from pcpos.base import ZERO, atomic_write, canon, now_utc
from pcpos.store import Workspace


def future_done(future, timeout):
    try:
        future.result(timeout=timeout)
        return True
    except FutureTimeout:
        return False


class AtomicWriteRegression(unittest.TestCase):
    def test_write_and_replace_regular_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "subdir" / "result"
            atomic_write(dest, b"FIRST")
            self.assertTrue(dest.is_file())
            self.assertFalse(dest.is_symlink())
            self.assertEqual(dest.read_bytes(), b"FIRST")
            atomic_write(dest, b"SECOND")
            self.assertEqual(dest.read_bytes(), b"SECOND")

    def test_legacy_predictable_temp_symlink_is_not_followed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            dest = root / "out" / "result"
            dest.parent.mkdir()
            sentinel = root / "external-sentinel"
            sentinel.write_bytes(b"UNCHANGED")
            link = dest.with_name(dest.name + ".tmp%d" % os.getpid())
            try:
                link.symlink_to(sentinel)
            except (OSError, NotImplementedError) as error:
                self.skipTest("creating file symlinks is not available: %s" % error)
            atomic_write(dest, b"NEW DATA")
            self.assertEqual(sentinel.read_bytes(), b"UNCHANGED")
            self.assertFalse(dest.is_symlink())
            self.assertEqual(dest.read_bytes(), b"NEW DATA")


class RepairLockRegression(unittest.TestCase):
    def test_repair_waits_for_writer_that_completes_its_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            ws = Workspace.create(Path(tmp) / "workspace")
            event = dict(seq=0, prev=ZERO, ts=now_utc(), actor="P",
                         auth="DECLARED", kind="NOTE", body={"message": "synthetic"})
            line = canon(event) + b"\n"
            split = len(line) // 2
            held = ws._lock()
            future = None
            try:
                with ws.register_path.open("ab") as writer, ThreadPoolExecutor(max_workers=1) as pool:
                    writer.write(line[:split]); writer.flush(); os.fsync(writer.fileno())
                    ready = threading.Event()
                    def repair():
                        ready.set()
                        return ws.repair()
                    future = pool.submit(repair)
                    self.assertTrue(ready.wait(2), "repair thread did not start")
                    # The writer owns the cooperative lock; repair must not
                    # replace the partial file even after a scheduling window.
                    self.assertFalse(future_done(future, 0.15),
                                     "repair bypassed the writer lock")
                    writer.write(line[split:]); writer.flush(); os.fsync(writer.fileno())
                    self.assertEqual(ws.register_path.read_bytes(), line)
                    held.unlink()
                    self.assertEqual(future.result(timeout=3), 0)
            finally:
                held.unlink(missing_ok=True)
            self.assertEqual(ws.register_path.read_bytes(), line)
            events, head = ws.events()
            self.assertEqual(len(events), 1)
            self.assertEqual(events[0], event)
            self.assertNotEqual(head, ZERO)


if __name__ == "__main__":
    unittest.main()
