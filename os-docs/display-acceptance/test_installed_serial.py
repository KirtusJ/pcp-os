"""Drive the installed display checker through its Enter and Escape sequence."""
import hashlib
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
                                   app_sha256=pin, check_title_1080p=False, check_launcher_1080p=False,
                                   check_launcher_identity_1080p=False)
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

            def capture(_monitor, _out, name, *_, **kwargs):
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


class LauncherCaptureTests(unittest.TestCase):
    def test_visible_identity_matches_installed_frame_and_changes_with_last_nibble(self):
        app = "bb2eea2eb758a7351d2949d01535f0da869d00dd182404b03521643166a95e7d"
        rect, digest = display.launcher_identity_field(app)
        self.assertEqual(rect, (460, 980, 1280, 1000))
        self.assertEqual(digest, "b9ecc6d0739f45d2fe568aa5c0a7b845c2e480e9e27be8c090b95869d668eeb6")
        self.assertNotEqual(display.launcher_identity_field(app[:-1] + "c")[1], digest)

    def test_fields_must_coincide_in_one_native_frame(self):
        colors = [bytes([80+i, 120, 164]) for i in range(3)]
        references = {str(i): ((i, 0, i+1, 1), hashlib.sha256(c).hexdigest())
                      for i, c in enumerate(colors)}
        captured = []
        def paint(_action, *, filename):
            mask = captured.pop(0)
            pixels = bytearray(1920 * 1080 * 3)
            for i, color in enumerate(colors):
                if mask & (1 << i):
                    pixels[3*i:3*i+3] = color
            Path(filename).write_bytes(b"P6\n1920 1080\n255\n" + pixels)

        with TemporaryDirectory() as directory, patch.object(display, "LAUNCHER_FIELDS", references), \
             patch.object(display.time, "sleep"):
            monitor = MagicMock(timeout=5)
            monitor.execute.side_effect = paint
            captured.extend([5, 2, 7])
            with patch.object(display.time, "monotonic", side_effect=[0, 1, 2]):
                result = display.capture(monitor, Path(directory), "launcher", check_launcher=True)
            self.assertEqual(monitor.execute.call_count, 3)
            self.assertEqual(result["launcher_field_sha256"], {k: v[1] for k,v in references.items()})
            captured.append(5)
            monitor.timeout = 0
            with self.assertRaisesRegex(TimeoutError, "fields missing: 1"):
                display.capture(monitor, Path(directory), "returned", check_launcher=True)
            self.assertTrue((Path(directory)/"returned.ppm").exists())

            # All three original fields pass. A stale displayed executable still fails.
            captured.append(7)
            with self.assertRaisesRegex(TimeoutError, "fields missing: selected_sha256"):
                display.capture(monitor, Path(directory), "returned", check_launcher=True,
                                identity_field=((3, 0, 4, 1), hashlib.sha256(b"abc").hexdigest()))


if __name__ == "__main__":
    unittest.main()
