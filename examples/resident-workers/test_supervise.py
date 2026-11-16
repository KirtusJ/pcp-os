"""Check that incomplete, stale and unauthenticated test observations cannot pass."""
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

import supervise as sup


class WorkerReadback(unittest.TestCase):
    def setUp(self):
        self.guest = Mock(node=1)
        self.oracle = [[10, 11, 12] for _ in range(16)]
        self.elf = "a" * 64
        self.trace = (
            "long mode true\n"
            "kernel (measured; equal to the value the loader named)\n"
            f"starting resident.elf (100 bytes, SHA-256 {self.elf}) with: 1 1 0\n"
            "NODE 1 PARENT 0 INPUT 1 TICKET 45\nLM tokens 10 11 12\nGATE 1\n"
            "SPAWN 2 PARENT 1 TICKET 45\nSPAWN 3 PARENT 1 TICKET 45\n"
            "SPAWN 4 PARENT 1 TICKET 45\nREADY 1\n"
        )

    def verify(self, trace):
        return sup.verify_start(self.guest, trace, 0, 1, 0, "kernel", self.elf, self.oracle)

    def test_only_the_requested_edges_are_admitted(self):
        self.assertEqual(self.verify(self.trace), (45, [2, 3, 4]))
        for original, replacement in (("SPAWN 2", "SPAWN 3"), ("PARENT 1 TICKET 45", "PARENT 2 TICKET 45"),
                                      ("GATE 1", "GATE 0"), ("10 11 12", "10 11 13"), (self.elf, "d" * 64)):
            with self.subTest(original=original), self.assertRaises(RuntimeError):
                self.verify(self.trace.replace(original, replacement))

    def test_old_answer_cannot_satisfy_a_new_challenge(self):
        with tempfile.TemporaryDirectory() as d:
            m = object.__new__(sup.Machine)
            m.node = 1
            m.serial = Path(d) / "1.serial"
            m.serial.write_bytes(b"CHALLENGE 2\nLM tokens 10 11 12\nREADY 1\n")
            m.position = m.serial.stat().st_size
            m.child = Mock()
            m.child.poll.return_value = None
            with self.assertRaises(TimeoutError):
                m.through_ready(time.monotonic() + .01)
            m.serial.write_bytes(b"READY 1\n")
            with self.assertRaisesRegex(RuntimeError, "truncated"):
                m.through_ready(time.monotonic() + .1)

    def test_queued_matching_output_is_rejected_before_keypress(self):
        with tempfile.TemporaryDirectory() as d:
            m = object.__new__(sup.Machine)
            m.node, m.live, m.command = 1, Mock(), Mock()
            m.serial = Path(d) / "serial"
            m.serial.write_bytes(b"READY 1\n")
            m.position = m.serial.stat().st_size
            with m.serial.open("ab") as stream:
                stream.write(b"CHALLENGE 2\nLM tokens 10 11 12\nREADY 1\n")
            with self.assertRaisesRegex(RuntimeError, "unsolicited"):
                m.challenge(2, self.oracle)
            m.command.assert_not_called()

    def test_serial_progress_idle_and_overall_deadlines(self):
        cases = [
            ([b"start\n", b"model\n", b"READY 1\n"], 3, 6, None),
            ([b"start\n", b"model\n"], 2, 10, "serial idle"),
            ([b"x"] * 8, 3, 5, "overall deadline"),
        ]
        for updates, idle, limit, error in cases:
            with self.subTest(error=error), tempfile.TemporaryDirectory() as d:
                m = object.__new__(sup.Machine)
                m.node, m.position, m.child = 1, 0, Mock()
                m.child.poll.return_value = None
                m.serial = Path(d) / "serial"
                clock = 0

                def advance(_):
                    nonlocal clock
                    if clock < len(updates):
                        with m.serial.open("ab") as serial:
                            serial.write(updates[clock])
                    clock += 1

                with patch.object(sup.time, "monotonic", side_effect=lambda: clock), patch.object(sup.time, "sleep", advance):
                    if error:
                        with self.assertRaisesRegex(TimeoutError, error):
                            m.through_ready(limit, idle)
                    else:
                        self.assertEqual(m.through_ready(limit, idle), b"".join(updates).decode())
                        self.assertEqual(m.position, m.serial.stat().st_size)

    def test_reject_different_machine_or_stopped_emulator(self):
        m = object.__new__(sup.Machine)
        m.node, m.uuid, m.command = 1, "expected", Mock()
        for replies in ([{"UUID": "different"}], [{"UUID": "expected"}, {"running": False}]):
            m.command.side_effect = replies
            with self.assertRaises(RuntimeError):
                m.live()

    def test_disconnect_identifies_request_and_observed_exit(self):
        m = object.__new__(sup.Machine)
        m.node, m.sequence, m.child = 12, 0, Mock()
        m.child.poll.return_value = -9
        m.child.stdin.write.side_effect = BrokenPipeError()
        with self.assertRaisesRegex(ConnectionError, "guest 12: query-uuid: broken pipe; emulator exit status -9"):
            m.command("query-uuid")

    def test_failed_kernel_boot_is_evidence_even_without_ready(self):
        with tempfile.TemporaryDirectory() as d:
            m = object.__new__(sup.Machine)
            m.node, m.position, m.child = 2, 0, Mock()
            m.child.poll.return_value = None
            m.serial = Path(d) / "serial"
            for record in (b"  FAIL keyboard idle bound", b"1 SELF-TEST(S) FAILED"):
                with self.subTest(record=record):
                    m.serial.write_bytes(b"long mode true\r\n" + record + b"\r\n")
                    with self.assertRaisesRegex(RuntimeError, "guest 2: kernel self-test") as caught:
                        m.through_ready(time.monotonic() + 2)
                    self.assertIn(record.decode().strip(), str(caught.exception))

    def test_guest_loss_during_collection_is_visible(self):
        m = object.__new__(sup.Machine)
        m.node, m.position, m.child = 7, 0, Mock()
        m.child.poll.return_value = 9
        with self.assertRaisesRegex(ConnectionError, "emulator exited"):
            m.through_ready(time.monotonic() + .2)


if __name__ == "__main__":
    unittest.main()
