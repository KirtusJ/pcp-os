"""Historical round selection, evidence boundaries and branch replacement."""
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from uuid import UUID
import zipfile

from archive import freeze

from supervise import PARENTS, sha
from verify import verify


class HistoricalReadback(unittest.TestCase):
    def setUp(self):
        tmp = TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.run = Path(tmp.name)
        self.oracle = self.run / "oracle.json"
        self.oracle.write_text(json.dumps([[p, 2, 3] for p in range(16)]))
        self.events = [{"event": "inputs", "measurement": "a" * 64, "worker_sha256": "b" * 64,
                        "kernel_sha256": "c" * 64, "oracle_sha256": sha(self.oracle.read_bytes()),
                        "root_input": 1, "salt": 0}]
        self.active = {}
        for node in range(1, 25):
            self.admit(node, 1)
        self.round(1, repairs=0)
        self.events.append({"event": "interrupted", "node": 3})
        for node in (19, 20, 8, 21, 9, 3):
            self.events.append({"event": "retired", "node": node, "uuid": self.active[node]["uuid"]})
        for node in (3, 8, 9, 19, 20, 21):
            self.admit(node, 2)
        self.round(2, repairs=1)

    def admit(self, node, attempt):
        parent = PARENTS[node]
        incoming = self.active[parent]["ticket"] if parent else 1
        ticket = (incoming * 31 + node * 13 + 1) % 1000003
        children = [c for c in range(2, 25) if PARENTS[c] == node]
        admission = dict(node=node, parent=parent, input=incoming, ticket=ticket,
                         uuid=str(UUID(int=100 * attempt + node)), attempt=attempt, children=children)
        self.events.append(dict(event="admitted", **admission))
        self.active[node] = admission
        prompt = (node + incoming) % 16
        lines = ["protections: paging true, supervisor write protection true, no-execute true, long mode true",
                 f"running image (code and read-only data), SHA-256: {'a' * 64} (measured; equal to the value the loader named)",
                 f"starting resident.elf (10 bytes, SHA-256 {'b' * 64}) with: {node} {incoming} 0",
                 f"NODE {node} PARENT {parent} INPUT {incoming} TICKET {ticket}", f"LM tokens {prompt} 2 3", "GATE 1"]
        lines.extend(f"SPAWN {c} PARENT {node} TICKET {ticket}" for c in children)
        lines.append(f"READY {node}")
        (self.run / f"{node}-{attempt}.serial").write_bytes(("\r\n".join(lines) + "\r\n").encode())

    def round(self, cycle, repairs):
        rows = []
        for node, entry in self.active.items():
            serial = self.run / f"{node}-{entry['attempt']}.serial"
            with serial.open("ab") as stream:
                stream.write(f"CHALLENGE {cycle}\r\nLM tokens {cycle} 2 3\r\nREADY {node}\r\n".encode())
            rows.append(dict(node=node, parent=PARENTS[node], uuid=entry["uuid"], prompt=cycle,
                             tokens=[cycle, 2, 3], serial_file=serial.name, serial_bytes=serial.stat().st_size,
                             serial_sha256=sha(serial.read_bytes())))
        record = dict(cycle=cycle, repairs=repairs, time=f"2026-11-16T00:00:0{cycle}+00:00", nodes=rows)
        self.events.append(dict(event="readback", cycle=cycle, record=record, sha256=self.digest(record)))

    @staticmethod
    def digest(record):
        return sha((json.dumps(record, indent=2) + "\n").encode())

    def check(self, cycle):
        (self.run / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in self.events))
        return verify(self.run, self.oracle, cycle)

    def test_old_and_replacement_rounds_after_new_output(self):
        with (self.run / "3-2.serial").open("ab") as stream:
            stream.write(b"partial subsequent output")
        self.assertEqual(self.check(1)["guests"], 24)
        self.assertEqual(self.check(2)["repairs"], 1)
        with self.assertRaisesRegex(ValueError, "round 3 unavailable"):
            self.check(3)

    def test_substitution_even_with_updated_round_digest(self):
        e = self.events[-1]
        row = e["record"]["nodes"][2]
        for key, bad, reason in (("uuid", self.events[3]["uuid"], "identity"),
                                 ("serial_file", "3-1.serial", "per-attempt"),
                                 ("serial_file", "../other", "per-attempt"),
                                 ("serial_bytes", row["serial_bytes"] + 1, "truncated"),
                                 ("tokens", [2, 2, 4], "oracle")):
            with self.subTest(key=key):
                original = row[key]
                row[key] = bad
                e["sha256"] = self.digest(e["record"])
                with self.assertRaisesRegex(RuntimeError, reason):
                    self.check(2)
                row[key] = original

    def test_repeated_prefix_and_altered_transcript(self):
        old = next(e for e in self.events if e["event"] == "readback")
        event = self.events[-1]
        event["record"]["nodes"][0] = old["record"]["nodes"][0].copy()
        event["sha256"] = self.digest(event["record"])
        with self.assertRaisesRegex(RuntimeError, "reused serial prefix"):
            self.check(2)
        path = self.run / "2-1.serial"
        path.write_bytes(path.read_bytes().replace(b"GATE 1", b"GATE 0"))
        with self.assertRaisesRegex(RuntimeError, "serial digest mismatch"):
            self.check(1)

    def test_unexpected_serial_output_between_complete_challenges(self):
        path = self.run / "1-1.serial"
        raw = path.read_bytes().replace(b"CHALLENGE 2", b"unsolicited text\r\nCHALLENGE 2")
        path.write_bytes(raw)
        event = self.events[-1]
        row = event["record"]["nodes"][0]
        row.update(serial_bytes=len(raw), serial_sha256=sha(raw))
        event["sha256"] = self.digest(event["record"])
        with self.assertRaisesRegex(RuntimeError, "unexpected challenge record"):
            self.check(2)

    def test_offline_round_requires_the_kernel_measurement_line(self):
        path = self.run / "1-1.serial"
        raw = path.read_bytes().replace(b"running image (", b"command line: running image (", 1)
        path.write_bytes(raw)
        event = self.events[-1]
        event["record"]["nodes"][0].update(serial_bytes=len(raw), serial_sha256=sha(raw))
        event["sha256"] = self.digest(event["record"])
        with self.assertRaisesRegex(RuntimeError, "kernel protection or measurement report"):
            self.check(2)

    def test_archive_retains_each_round_through_the_cutoff(self):
        self.check(2)
        with (self.run / "events.jsonl").open("ab") as log:
            log.write(b'{"event":"later')
        with (self.run / "1-1.serial").open("ab") as serial:
            serial.write(b"later serial output")
        records = [e["record"] for e in self.events if e["event"] == "readback"]
        for cycle in (1, 2):
            with self.subTest(cycle=cycle):
                output = self.run / f"through-{cycle}.zip"
                self.assertEqual(freeze(self.run, self.oracle, cycle, output)["cycle"], cycle)
                with zipfile.ZipFile(output) as archive:
                    root = zipfile.Path(archive)
                    self.assertEqual(json.loads(archive.read("cohort/readback.json")), records[cycle - 1])
                    for n in range(1, cycle + 1):
                        result = verify(root / "cohort", root / "fixture" / "oracle.json", n)
                        self.assertEqual(result["sha256"], self.digest(records[n - 1]))
                    ends = {row["serial_file"]: row["serial_bytes"] for record in records[:cycle]
                            for row in record["nodes"]}
                    serials = {name.removeprefix("cohort/"): archive.getinfo(name).file_size
                               for name in archive.namelist() if name.endswith(".serial")}
                    self.assertEqual(serials, ends)
                    ledger = archive.read("cohort/events.jsonl")
                    self.assertEqual(json.loads(ledger.splitlines()[-1])["cycle"], cycle)
                    self.assertEqual(json.loads(ledger.splitlines()[-1])["event"], "readback")

    def test_archive_checks_retired_evidence_and_preserves_existing_output(self):
        self.check(2)
        output = self.run / "rounds.zip"
        output.write_bytes(b"existing archive")
        with self.assertRaises(FileExistsError):
            freeze(self.run, self.oracle, 2, output)
        self.assertEqual(output.read_bytes(), b"existing archive")
        output.unlink()
        with self.assertRaisesRegex(ValueError, "round 3 unavailable"):
            freeze(self.run, self.oracle, 3, output)
        self.assertFalse(output.exists())
        retired = self.run / "3-1.serial"
        retired.write_bytes(retired.read_bytes().replace(b"GATE 1", b"GATE 0"))
        with self.assertRaisesRegex(RuntimeError, "serial digest mismatch"):
            freeze(self.run, self.oracle, 2, output)
        self.assertFalse(output.exists())

    def test_legacy_digest_alone_cannot_verify_a_round(self):
        del next(e for e in self.events if e["event"] == "readback")["record"]
        with self.assertRaisesRegex(RuntimeError, "historical round record unavailable"):
            self.check(1)
