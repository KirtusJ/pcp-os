"""Exercise transcript acceptance and result recording with synthetic inputs."""
import contextlib
import io
import json
from pathlib import Path
import selectors
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import check
from check import verify_log


class SerialAcceptance(unittest.TestCase):
    components = {"qwen-worker": "1" * 64, "tailorbird": "b5acb971d222184970b647d13e79cedf60bba681151cc40e0ae12cfb9503c465"}
    manifest = {"component_sha256": components, "factory_disk_guid": "00112233-4455-6677-8899-aabbccddeeff"}
    image = "3" * 64
    command = {
        "recovery": "image=" + image,
        "desktop": f"image={image} tailor_ui={components['tailorbird']} "
                   f"userdisk=33221100554477668899aabbccddeeff q={components['qwen-worker']}",
        "diagnostic": "auto=qwen.elf grant=files then=off image=" + image,
    }
    common = [
        "PCP OS EFI amd64 entry, native 64-bit handoff",
        "protections: long mode true",
    ]
    diagnostic = [
        f"starting qwen.elf (123 bytes, SHA-256 {components['qwen-worker']}) with: 3 17 488 220 17 284",
        "QWEN READY", "QWEN TOKEN 220", "QWEN TOKEN 19", "QWEN TOKEN 198", "QWEN COMPLETE",
        "qwen.elf ended DONE: exit 0", "ALL SELF-TESTS PASS",
    ]

    def check(self, mode, lines, *, command=None, grub="", kernel_measurement=None):
        arguments = self.command[mode] if command is None else command
        header = [grub, *self.common, "command line: " + arguments,
                  f"running image (code and read-only data), SHA-256: {self.image} "
                  "(measured; equal to the value the loader named)"]
        data = "\r\n".join(header + lines).encode()
        verify_log(data, mode, self.manifest, [220, 19, 198], kernel_measurement)

    def test_selected_entry_must_agree_with_measured_boot_arguments(self):
        lines = ["PCP OS launcher ready: 0 programs, 0 files", "ALL SELF-TESTS PASS"]
        good = "Booting `PCP OS recovery (no model)'"
        wrong = "Booting `Offline Qwen acquisition test'"
        self.check("recovery", lines, grub=good + good)
        self.check("recovery", lines, kernel_measurement=self.image)
        with self.assertRaisesRegex(ValueError, "manifest-matched ELF"):
            self.check("recovery", lines, kernel_measurement="4" * 64)
        for grub in (wrong, good + wrong):
            with self.subTest(grub=grub), self.assertRaisesRegex(ValueError, "GRUB selected"):
                self.check("recovery", lines, grub=grub)
        for command in ("", self.command["diagnostic"], "image=" + "4" * 64,
                        self.command["recovery"] + " image=" + self.image):
            with self.subTest(command=command), self.assertRaisesRegex(ValueError, "command line"):
                self.check("recovery", lines, command=command)
        for field in ("tailor_ui", "userdisk", "q"):
            wrong = self.command["desktop"].replace(field + "=", field + "=0")
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "command line"):
                self.check("desktop", [], command=wrong)

    def test_diagnostic_requires_expected_tokens_from_completed_worker(self):
        self.check("diagnostic", self.diagnostic)
        for index, replacement in (
            (0, self.diagnostic[0].replace("1" * 64, "0" * 64)),
            (0, self.diagnostic[0].replace("qwen.elf", "other.elf")),
            (0, self.diagnostic[0].replace("17 284", "17 285")),
            (3, "QWEN TOKEN 20"), (5, "QWEN READY"), (6, "qwen.elf ended FAILED"),
        ):
            with self.subTest(index=index, replacement=replacement), self.assertRaises(ValueError):
                altered = self.diagnostic.copy()
                altered[index] = replacement
                self.check("diagnostic", altered)
        with self.assertRaises(ValueError):
            self.check("diagnostic", self.diagnostic[:1] + self.diagnostic[2:5] +
                       ["QWEN READY"] + self.diagnostic[5:])

    def test_diagnostic_requires_distinct_lifecycle_records(self):
        for index, replacement in (
            (1, "QWEN READY_PENDING"), (1, "previous event: QWEN READY"),
            (5, "QWEN COMPLETELY UNFINISHED"), (6, "qwen.elf ended DONE: exit 01"),
            (7, "NOT ALL SELF-TESTS PASS"),
        ):
            with self.subTest(record=replacement), self.assertRaises(ValueError):
                lines = self.diagnostic.copy()
                lines[index] = replacement
                self.check("diagnostic", lines)

    def test_kernel_accounting_and_self_test_details_are_accepted(self):
        lines = self.diagnostic.copy()
        lines[-2] += " after 3 s; 1228 system calls; 151 frames shown (39.9 a second)"
        lines[-1] += ". Guest Ethernet support is compiled in."
        self.check("diagnostic", lines)

    def test_recovery_rejects_a_payload_even_if_self_tests_pass(self):
        lines = ["PCP OS launcher ready: 0 programs, 0 files", "ALL SELF-TESTS PASS"]
        self.check("recovery", lines)
        with self.assertRaises(ValueError):
            self.check("recovery", [lines[0], "starting worker.elf", lines[1]])

    def test_desktop_requires_manifest_matched_application_and_ready_worker(self):
        lines = [
            "Persistent application store mounted: 0 file(s)", "PCP OS launcher ready: 2 programs, 3 files",
            f"starting Tailorbird (512 bytes, SHA-256 {self.components['tailorbird']})",
            "Qwen worker admitted in separate address space: UI pid 1, worker pid 2",
            "TAILORBIRD MCP CONNECTED", "QWEN READY", "Tailorbird ended DONE: exit 0", "ALL SELF-TESTS PASS",
        ]
        self.check("desktop", lines)
        for index in (2, 3, 4, 5, 6):
            with self.subTest(index=index), self.assertRaises(ValueError):
                self.check("desktop", lines[:index] + lines[index + 1:])

        with self.assertRaises(ValueError):
            self.check("desktop", lines[:4] + ["TAILORBIRD MCP CONNECTED_PENDING"] + lines[5:])
        wrong_launch = lines[2].replace(self.components["tailorbird"], "0" * 64)
        for launches in ([lines[2], *lines[:2], wrong_launch, *lines[3:]],
                         [*lines[:3], wrong_launch, *lines[3:]]):
            with self.subTest(launches=launches), self.assertRaises(ValueError):
                self.check("desktop", launches)

    def test_installed_desktop_negotiated_mcp_and_clean_exit(self):
        # These eight lines come from guarded-child-child-desktop.serial (pcp-os#132,
        # comment 6919490251). The surrounding boot records here are synthetic.
        release = """starting Tailorbird (177232 bytes, SHA-256 b5acb971d222184970b647d13e79cedf60bba681151cc40e0ae12cfb9503c465)
Qwen worker admitted in separate address space: UI pid 1, worker pid 2, model file and message endpoint
TAILORBIRD MCP CONNECTED 2025-11-25
QWEN READY
TAILORBIRD RELEASE PROPOSAL STAGED
Qwen worker execution ended: Stopped
Qwen worker end Some(Stopped); 106299 frames released; W+X 0; supervisor reachable false
Tailorbird ended DONE: exit 0 after 4 s; 1872 system calls; 22 frames shown (4.9 a second); the processor idled 93% of the time; 0 ms of sound played, the device ran out of sound 0 time(s); 0 music register writes so far, the latest 0 ms late; 1818 pages at the end, 1013 of them the display's, 0 both writable and executable; 817 frames given back, 131072 free before and 131072 after; the store holds 0 file(s), 0 bytes; Tailorbird holds 0 app(s)""".splitlines()
        before = ["Persistent application store mounted: 0 file(s)",
                  "PCP OS launcher ready: 2 programs, 3 files"]
        self.check("desktop", before + release + ["ALL SELF-TESTS PASS"])
        for text in ("TAILORBIRD MCP CONNECTED 2025-11-25_PENDING",
                     "previous event: TAILORBIRD MCP CONNECTED 2025-11-25"):
            with self.subTest(record=text), self.assertRaises(ValueError):
                altered = release.copy()
                altered[2] = text
                self.check("desktop", before + altered + ["ALL SELF-TESTS PASS"])

    def test_repeated_diagnostic_launch_cannot_combine_two_workers(self):
        with self.assertRaises(ValueError):
            self.check("diagnostic", self.diagnostic[:2] + self.diagnostic)


class ResultRecording(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.disk = self.root / "disk"
        self.disk.write_bytes(b"synthetic installed disk")
        self.results = self.root / "results"
        manifest = self.root / "release.json"
        manifest.write_text(json.dumps({
            "git_commit": "synthetic", "component_sha256": {},
            "tailorbird_request": {"factory_profile": "recovery 5"},
            "output_sha256": {"PCP-OS.img": check.sha256(self.disk)},
        }))
        self.args = ["--disk", str(self.disk), "--manifest", str(manifest),
                     "--ovmf-code", str(self.disk), "--ovmf-vars", str(self.disk),
                     "--results", str(self.results), "--qemu", sys.executable,
                     "--ram-directory", str(self.root), "--expected-tokens", "220", "19", "198",
                     "--modes", "recovery", "desktop"]

    def result(self):
        return json.loads((self.results / "result.json").read_bytes())

    def run_with_boot(self, boot):
        with mock.patch.object(sys, "argv", ["check.py", *self.args]), \
                mock.patch.object(check, "boot", side_effect=boot), \
                contextlib.redirect_stdout(io.StringIO()):
            check.main()

    def test_complete_after_both_boots_and_disk_verification(self):
        self.run_with_boot(lambda *args: {"qemu_exit": 33})
        result = self.result()
        self.assertEqual(result["status"], "complete")
        self.assertIsNone(result["active_mode"])
        self.assertEqual(list(result["boots"]), ["recovery", "desktop"])
        self.assertEqual(result["disk_sha256_after"], result["disk_sha256_before"])

    def test_boot_failure_and_changed_disk_are_both_recorded(self):
        def failed_boot(*args):
            self.disk.write_bytes(b"changed")
            raise ValueError("fixture boot failed")

        with self.assertRaisesRegex(RuntimeError, "disk changed"):
            self.run_with_boot(failed_boot)
        result = self.result()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["failure"], {"mode": "recovery", "reason": "fixture boot failed"})
        self.assertIn("disk changed", result["disk_verification_error"])
        self.assertNotEqual(result["disk_sha256_after"], result["disk_sha256_before"])

    def test_disk_rehash_failure_after_all_boots(self):
        def boot_then_remove(args, mode, components, tokens, temp):
            if mode == "desktop":
                self.disk.unlink()
            return {"qemu_exit": 33}

        with self.assertRaises(FileNotFoundError):
            self.run_with_boot(boot_then_remove)
        result = self.result()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(list(result["boots"]), ["recovery", "desktop"])
        self.assertIn(str(self.disk), result["disk_verification_error"])
        self.assertNotIn("disk_sha256_after", result)

    def test_killed_runner_preserves_completed_boot(self):
        script = """
import check
import sys

def fixture_boot(args, mode, components, tokens, temp):
    if mode == "desktop":
        print("desktop started", flush=True)
        sys.stdin.readline()
    return {"qemu_exit": 33}

check.boot = fixture_boot
check.main()
"""
        with subprocess.Popen([sys.executable, "-c", script, *self.args], cwd=Path(check.__file__).parent,
                              stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as child:
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(child.stdout, selectors.EVENT_READ)
                    self.assertTrue(selector.select(timeout=60), "fixture did not reach desktop")
                self.assertEqual(child.stdout.readline().strip(), "desktop started")
            finally:
                child.kill()
                child.wait()
        result = self.result()
        self.assertEqual(result["status"], "in_progress")
        self.assertEqual(result["active_mode"], "desktop")
        self.assertEqual(result["boots"], {"recovery": {"qemu_exit": 33}})
        self.assertNotIn("disk_sha256_after", result)


if __name__ == "__main__":
    unittest.main()
