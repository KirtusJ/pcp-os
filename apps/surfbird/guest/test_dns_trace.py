import json
from pathlib import Path
import struct
import tempfile
import unittest

from dns_trace import verify_dns


class DnsReplayTest(unittest.TestCase):
    evidence = Path(__file__).parent / "evidence" / "dns"

    def check_case(self, arm, capture):
        case = json.loads((self.evidence / f"{arm}.json").read_text())
        return verify_dns(
            capture, case["resolver_port"], case["address"], arm, case["paths"], case["pages"]
        )

    def test_six_captures(self):
        reports = {}
        builds = set()
        for arm in ["allowed", "alternate", "retry", "nxdomain", "truncated", "mismatch"]:
            with self.subTest(arm=arm):
                report = self.check_case(arm, self.evidence / f"{arm}.pcap")
                case = json.loads((self.evidence / f"{arm}.json").read_text())
                self.assertEqual(report["pcap_sha256"], case["pcap_sha256"])
                builds.add((case["guest_sha256"], case["kernel_measurement"]))
                reports[arm] = report
        self.assertEqual(len(builds), 1)
        self.assertNotEqual(reports["allowed"]["destination"], reports["alternate"]["destination"])

    def test_syn_must_follow_dns_answer(self):
        raw = (self.evidence / "allowed.pcap").read_bytes()
        records = []
        pos = 24
        while pos < len(raw):
            size = struct.unpack_from("<I", raw, pos + 8)[0]
            records.append(raw[pos : pos + 16 + size])
            pos += 16 + size
        first_tcp = next(i for i, r in enumerate(records) if r[28:30] == b"\x08\x00" and r[39] == 6)
        records.insert(0, records.pop(first_tcp))
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / "syn-before-dns.pcap"
            capture.write_bytes(raw[:24] + b"".join(records))
            with self.assertRaisesRegex(ValueError, "preceded a matching DNS answer"):
                self.check_case("allowed", capture)
