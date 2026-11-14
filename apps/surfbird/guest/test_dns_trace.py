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

    def test_published_captures(self):
        reports = {}
        builds = set()
        for arm in [
            "allowed",
            "alternate",
            "retry",
            "nxdomain",
            "truncated",
            "mismatch",
            "malformed",
        ]:
            with self.subTest(arm=arm):
                report = self.check_case(arm, self.evidence / f"{arm}.pcap")
                case = json.loads((self.evidence / f"{arm}.json").read_text())
                self.assertEqual(report["pcap_sha256"], case["pcap_sha256"])
                builds.add((case["guest_sha256"], case["kernel_measurement"]))
                reports[arm] = report
        self.assertEqual(len(builds), 1)
        self.assertNotEqual(reports["allowed"]["destination"], reports["alternate"]["destination"])

    def records(self, arm):
        raw = (self.evidence / f"{arm}.pcap").read_bytes()
        records = []
        pos = 24
        while pos < len(raw):
            size = struct.unpack_from("<I", raw, pos + 8)[0]
            records.append(raw[pos : pos + 16 + size])
            pos += 16 + size
        return raw[:24], records

    def test_syn_must_follow_dns_answer(self):
        header, records = self.records("allowed")
        first_tcp = next(i for i, r in enumerate(records) if r[28:30] == b"\x08\x00" and r[39] == 6)
        records.insert(0, records.pop(first_tcp))
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / "syn-before-dns.pcap"
            capture.write_bytes(header + b"".join(records))
            with self.assertRaisesRegex(ValueError, "preceded a matching DNS answer"):
                self.check_case("allowed", capture)

    def test_retransmission_must_precede_answer(self):
        header, records = self.records("retry")
        udp = [i for i, r in enumerate(records) if r[28:30] == b"\x08\x00" and r[39] == 17]
        self.assertEqual(len(udp), 3)
        _, retry, answer = udp
        records[retry], records[answer] = records[answer], records[retry]
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / "retry-after-answer.pcap"
            capture.write_bytes(header + b"".join(records))
            with self.assertRaisesRegex(ValueError, "before the retry"):
                self.check_case("retry", capture)

    def test_single_lookup_rejects_post_answer_query(self):
        header, records = self.records("allowed")
        query = next(r for r in records if r[28:30] == b"\x08\x00" and r[39] == 17)
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / "extra-query.pcap"
            capture.write_bytes(header + b"".join(records) + query)
            with self.assertRaisesRegex(ValueError, "query after"):
                self.check_case("allowed", capture)
