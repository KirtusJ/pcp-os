"""Check that incomplete, stale and unauthenticated test observations cannot pass."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

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
                m.through_ready(.01)
            m.serial.write_bytes(b"READY 1\n")
            with self.assertRaisesRegex(RuntimeError, "truncated"):
                m.through_ready(.1)

    def test_reject_different_machine_or_stopped_emulator(self):
        m = object.__new__(sup.Machine)
        m.node, m.uuid, m.command = 1, "expected", Mock()
        for replies in ([{"UUID": "different"}], [{"UUID": "expected"}, {"running": False}]):
            m.command.side_effect = replies
            with self.assertRaises(RuntimeError):
                m.live()

    def test_guest_loss_during_collection_is_visible(self):
        m = object.__new__(sup.Machine)
        m.node, m.child = 7, Mock()
        m.child.poll.return_value = 9
        with self.assertRaisesRegex(ConnectionError, "emulator exited"):
            m.through_ready(.2)


if __name__ == "__main__":
    unittest.main()
