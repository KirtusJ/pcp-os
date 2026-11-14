"""Run without QEMU or any real game assets; synthetic log controls only."""
import unittest
from doom_oracles import sound_runs_ok, unanswered_requests


class DoomOracleTests(unittest.TestCase):
    def test_two_healthy_runs_pass(self):
        self.assertTrue(sound_runs_ok([0, 0]))
        self.assertTrue(sound_runs_ok([0, 0, 0]))

    def test_any_underrun_fails(self):
        for values in ([0, 1], [1, 0], [0, -1], [0, 0, 3]):
            with self.subTest(values=values):
                self.assertFalse(sound_runs_ok(values))

    def test_missing_or_malformed_sound_counts_fail(self):
        for values in ([], [0], [0, True], [0, "0"], [0, 0.0]):
            with self.subTest(values=values):
                self.assertFalse(sound_runs_ok(values))

    def test_unanswered_calls_are_not_discarded(self):
        log = "started\ncall a does not answer after timeout\nplaying\ncall b does not answer after timeout\n"
        self.assertEqual(len(unanswered_requests(log)), 2)
        self.assertIn("call b", unanswered_requests(log)[1])

    def test_normal_soak_log_has_no_unanswered_calls(self):
        self.assertEqual(unanswered_requests("started\nframes changed\nended cleanly\n"), [])


if __name__ == "__main__":
    unittest.main()
