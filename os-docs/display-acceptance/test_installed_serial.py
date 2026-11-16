"""Drive the installed display checker through its Enter and Escape sequence."""
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

import check_display as display


APP = "a" * 64
START = f"starting Tailorbird (177232 bytes, SHA-256 {APP})"
CONNECTED = "TAILORBIRD MCP CONNECTED 2025-11-25"
READY = "QWEN READY"
EXIT = "Tailorbird ended DONE: exit 0 after 1 s; 934 system calls"


class InstalledSerialTests(unittest.TestCase):
    def exercise(self, launch, *, earlier=(), ending=EXIT, required=True, pin=None):
        with TemporaryDirectory() as directory:
            out = Path(directory)
            firmware = out / "firmware"
            firmware.touch()
            serial = out / "serial.log"

            def emit(*lines):
                with serial.open("ab") as stream:
                    for line in lines:
                        stream.write(line.encode() + b"\r\n")

            emit("Offline Qwen acquisition test", *earlier)
            args = SimpleNamespace(output=out, code=firmware, vars=firmware, image=firmware,
                                   qemu="unused", firmware_dir=None, mode=(1920, 1080),
                                   ram=1536, vram=32, timeout=0.04, require_qwen_ready=required,
                                   app_sha256=pin, check_title_1080p=False)
            guest, monitor = MagicMock(), MagicMock()
            guest.poll.return_value = None
            monitor.receive.return_value = {"QMP": {}}
            properties = {"edid": True, "xres": 1920, "yres": 1080, "vgamem_mb": 32}
            monitor.execute.side_effect = lambda action, **args: properties.get(args.get("property"))
            enters = 0

            def key(name):
                nonlocal enters
                if name == "ret":
                    enters += 1
                    if enters == 1:
                        emit("PCP OS launcher ready: 2 programs, 3 files")
                    else:
                        emit(*launch)
                elif name == "esc":
                    emit(ending)

            def capture(_monitor, _out, name, *_):
                return {"width": 1920, "height": 1080, "ppm_sha256": name}

            monitor.key.side_effect = key
            report = {"uuid": "test", "scanout": {}}
            with patch.object(display.subprocess, "Popen", return_value=guest), \
                 patch.object(display, "Monitor", return_value=monitor), \
                 patch.object(display, "capture", side_effect=capture), \
                 patch.object(display.time, "sleep"):
                display.run(args, report)
            return report

    def test_ready_in_same_launch_in_either_service_order(self):
        for service in [(CONNECTED, READY), (READY, CONNECTED)]:
            with self.subTest(service=service):
                report = self.exercise([START, *service], pin=APP)
                self.assertTrue(report["qwen_reported_ready"])
                self.assertEqual(report["app_sha256"], APP)

    def test_ready_must_be_a_whole_record(self):
        for text in ["QWEN READY_PENDING", "diagnostic: QWEN READY", "QWEN READY "]:
            with self.subTest(text=text), self.assertRaises((TimeoutError, RuntimeError)):
                self.exercise([START, CONNECTED, text])

    def test_earlier_launch_cannot_supply_readiness(self):
        with self.assertRaises((TimeoutError, RuntimeError)):
            self.exercise([START, CONNECTED], earlier=[START, CONNECTED, READY, EXIT])

    def test_second_launch_cannot_supply_missing_evidence(self):
        with self.assertRaises(RuntimeError):
            self.exercise([START, CONNECTED, START, READY])

    def test_clean_exit_before_escape_fails(self):
        with self.assertRaises(RuntimeError):
            self.exercise([START, CONNECTED, READY, EXIT])

    def test_unexpected_binary_fails(self):
        with self.assertRaises(RuntimeError):
            self.exercise([START, CONNECTED, READY], pin="b" * 64)

    def test_misleading_exit_status_fails(self):
        for ending in ["Tailorbird ended DONE: exit 01", "Tailorbird ended FAILED: fault"]:
            with self.subTest(ending=ending), self.assertRaises((TimeoutError, RuntimeError)):
                self.exercise([START, CONNECTED, READY], ending=ending)

    def test_optional_readiness_stays_scoped(self):
        report = self.exercise([START, CONNECTED, "QWEN READY_PENDING"],
                               earlier=[READY], required=False)
        self.assertFalse(report["qwen_reported_ready"])

    def test_fragment_at_launch_boundary_and_incomplete_readiness(self):
        with TemporaryDirectory() as directory:
            serial = Path(directory) / "serial.log"
            serial.write_bytes(START[:10].encode())
            launch = display.TailorbirdLaunch(serial, APP)
            with serial.open("ab") as stream:
                stream.write((START[10:] + "\r\n" + READY + "\r\n").encode())
            launch.read()
            self.assertIsNone(launch.digest)
            self.assertFalse(launch.ready)
            with serial.open("ab") as stream:
                stream.write((START + "\n" + CONNECTED + "\nQWEN READY").encode())
            launch.read()
            self.assertTrue(launch.connected)
            self.assertFalse(launch.ready)
            with serial.open("ab") as stream:
                stream.write(b"_PENDING\nQWEN READY\r\n")
            launch.read()
            self.assertTrue(launch.ready)


if __name__ == "__main__":
    unittest.main()
