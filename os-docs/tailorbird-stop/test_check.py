"""Negative controls for the cancellation trace and application-store reader."""
import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile
import unittest

import check


def write_slot(region, sector, generation, payload):
    pos = sector * check.SECTOR
    header = b"PCPSLOT1" + struct.pack("<QI", generation, len(payload)) + hashlib.sha256(payload).digest()
    region[pos:pos + 84] = header + hashlib.sha256(header).digest()
    start = pos + check.SECTOR
    region[start:start + len(payload)] = payload


def entry(name, source):
    return struct.pack("<HI", len(name), len(source)) + name + source


class StopOracles(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / "fixture"

    def test_trace_requires_exact_order_and_whole_events(self):
        expected = [check.STAGED, check.SAVED, check.STAGED, check.ACCEPTED, check.STOPPED,
                    check.STAGED, check.ACCEPTED, check.STOPPED, check.RAN]
        self.path.write_bytes(b"\r\n".join(expected) + b"\r\n")
        check.check_trace(self.path, expected)
        self.path.write_bytes(b"\n".join(expected) + b"\n")
        check.check_trace(self.path, expected)
        for tail in (b"", b"\r"):
            with self.subTest(tail=tail), self.assertRaises(ValueError):
                self.path.write_bytes(b"\r\n".join(expected) + tail)
                check.check_trace(self.path, expected)
        cases = {
            "early read": expected[:4] + [check.RAN] + expected[4:],
            "extra save": expected[:5] + [check.SAVED] + expected[5:],
            "missing explicit read": expected[:-1],
            "look-alike marker": expected[:-1] + [check.RAN + b" FAILED"],
        }
        for name, altered in cases.items():
            with self.subTest(case=name), self.assertRaises(ValueError):
                self.path.write_bytes(b"\r\n".join(altered) + b"\r\n")
                check.check_trace(self.path, expected)

    def test_startup_records_belong_to_one_manifest_matched_launch(self):
        launch = b"starting Tailorbird (177232 bytes, SHA-256 " + b"a" * 64 + b")"
        mode = b"protections: paging true, supervisor write protection true, no-execute true, long mode true"
        mount = b"Persistent application store mounted: 1 file(s)"
        admission = (b"Qwen worker admitted in separate address space: UI pid 1, worker pid 2, "
                     b"model file and message endpoint")
        ended = b"Qwen worker execution ended: Exited(3)"
        connected = b"TAILORBIRD MCP CONNECTED"
        versioned = connected + b" 2025-11-25"
        events = [mode, mount, launch, admission, ended, versioned]
        desktop_exit = b"Tailorbird ended DONE: exit 0"

        def transcript(lines):
            return b"\r\n".join(lines) + b"\r\n"

        for tail in ([ended, versioned], [versioned, ended], [ended, connected]):
            with self.subTest(tail=tail):
                data = transcript(events[:4] + tail)
                self.assertEqual(check.desktop_startup(data + transcript([check.STAGED]), launch), len(data) - 1)
                self.assertIsNone(check.desktop_startup(data[:-1], launch))
        for missing in (ended, versioned):
            self.assertIsNone(check.desktop_startup(transcript([e for e in events if e != missing]), launch))
        ready = len(transcript(events)) - 1
        for tail in ([check.STAGED], [check.STAGED, desktop_exit, b"ALL SELF-TESTS PASS"]):
            self.assertEqual(check.desktop_startup(transcript(events + tail), launch), ready)

        cases = {
            "mode embedded in unrelated text": [b"protections: diagnostic, long mode true"] + events[1:],
            "wrong digest": events[:2] + [launch.replace(b"a" * 64, b"b" * 64)] + events[3:],
            "wrong size": events[:2] + [launch.replace(b"177232", b"177233")] + events[3:],
            "early worker exit": [ended] + events[:4] + [versioned],
            "connection before admission": events[:3] + [versioned, admission, ended],
            "second launch divides handshake": events[:5] + [launch, versioned],
            "relaunch after success": events + [launch],
            "reboot after success": events + [mode],
            "same pid": events[:3] + [admission.replace(b"pid 2", b"pid 1")] + events[4:],
            "pending connection": events[:5] + [connected + b"_PENDING"],
            "extra exit suffix": events[:4] + [ended + b" deferred", versioned],
            "model-ready": events + [b"QWEN READY"],
            "early desktop exit": events[:4] + [b"Tailorbird ended FAILED: exit 1"] + events[4:],
            "control before launch": [check.STAGED] + events,
            "control between worker exit and connection": events[:5] + [check.SAVED, versioned],
            "control between connection and worker exit": events[:4] + [versioned, check.RAN, ended],
            "control after exit": events + [desktop_exit, check.RAN],
            "duplicate desktop exit": events + [desktop_exit, desktop_exit],
        }
        for name, records in cases.items():
            with self.subTest(case=name), self.assertRaises(ValueError):
                check.desktop_startup(transcript(records), launch)

    @unittest.skipUnless(os.name == "posix", "POSIX signal exit codes")
    def test_requested_kill_is_distinct_from_a_prior_exit(self):
        with subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"]) as child:
            guest = check.Guest(child, self.path, 1)
            try:
                guest.interrupt()
                self.assertEqual(child.returncode, -signal.SIGKILL)
                self.assertTrue(guest.kill_requested)
            finally:
                if child.poll() is None:
                    child.kill()
        with subprocess.Popen([sys.executable, "-c", "pass"]) as exited:
            self.assertEqual(exited.wait(), 0)
            guest = check.Guest(exited, self.path, 1)
            with self.assertRaisesRegex(ValueError, "exited before interruption"):
                guest.interrupt()
            self.assertFalse(guest.kill_requested)

    def test_active_generation_requires_exact_path_and_bytes(self):
        region = bytearray(check.SECTOR * (1 + 2 * check.SLOT_SECTORS))
        label = b"PCPOS USER STORE 1\n"
        region[:len(label)] = label
        source = b"print original"
        approved = entry(b"apps/lark.txt", source)
        write_slot(region, 1, 2, approved)
        self.path.write_bytes(region)
        self.assertEqual(check.checkpoint(self.path, 0, source)["generation"], 2)
        cases = [
            ("draft only", 3, entry(b"tailor-drafts-v1/lark", source)),
            ("stale bytes", 3, entry(b"apps/lark.txt", b"print changed")),
            ("duplicate generation", 2, approved),
        ]
        for name, generation, payload in cases:
            with self.subTest(case=name), self.assertRaises(ValueError):
                altered = region.copy()
                write_slot(altered, 1 + check.SLOT_SECTORS, generation, payload)
                self.path.write_bytes(altered)
                check.checkpoint(self.path, 0, source)
        for offset in (check.SECTOR + 32, check.SECTOR + 85, check.SECTOR * 2):
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                altered = region.copy()
                altered[offset] ^= 1
                self.path.write_bytes(altered)
                check.checkpoint(self.path, 0, source)


if __name__ == "__main__":
    unittest.main()
