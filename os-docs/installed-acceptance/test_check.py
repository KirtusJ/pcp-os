"""Exercise the serial acceptance rules with deliberately short fixture transcripts."""
import unittest

from check import verify_log


class SerialAcceptance(unittest.TestCase):
    components = {"qwen-worker": "1" * 64, "tailorbird": "2" * 64}
    common = [
        "PCP OS EFI amd64 entry, native 64-bit handoff",
        "protections: long mode true",
        "abcd (measured; equal to the value the loader named)",
    ]
    diagnostic = [
        f"starting qwen.elf (123 bytes, SHA-256 {components['qwen-worker']}) with: 3 17 488 220 17 284",
        "QWEN READY", "QWEN TOKEN 220", "QWEN TOKEN 19", "QWEN TOKEN 198", "QWEN COMPLETE",
        "qwen.elf ended DONE: exit 0", "ALL SELF-TESTS PASS",
    ]

    def check(self, mode, lines):
        data = "\r\n".join(self.common + lines).encode()
        verify_log(data, mode, self.components, [220, 19, 198])

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


if __name__ == "__main__":
    unittest.main()
