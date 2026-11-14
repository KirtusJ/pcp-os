"""Pure pass/fail checks for Doom soak and interactive-play diagnostics."""


def sound_runs_ok(underruns, expected_runs=2):
    """Missing sessions and any nonzero/non-integer underrun are failures."""
    return len(underruns) >= expected_runs and all(type(n) is int and n == 0 for n in underruns)


def unanswered_requests(log):
    """Check the entire log, not just the first diagnostic line."""
    return [line for line in log.splitlines() if "does not answer" in line]
