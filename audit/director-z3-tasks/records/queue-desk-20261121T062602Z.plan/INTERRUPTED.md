# T4 did not end; the machine was restarted under it

Admitted 21 November 2026 06:26 UTC. The session's container was restarted before the runner exited
(seen at 08:56 UTC: the machine had been up under a minute and no process of the task existed). So there
is no `DONE`, no `ASYNC_RECORD.json` and no captured output, and the end of this task was not observed.
It is recorded as that, not as a pass.

What the task had left on disk in the session's scratch folder, read afterwards by Director Z3 and not by
the runner, so with no recorded digests:
- the queue-desk check (`check.py --queue-desk`, the first of the task's two checks): `result.json` for all
  four sessions with `passed: true`, exit code 0, and a `summary.json`;
- the plain check run again (the second): `result.json` for two sessions with `passed: true`; the third
  session's folder had no result, and the fourth was never started.

The reservation was released by Director Z3 afterwards so that the register does not hold a task that no
longer exists. The release says nothing about how the task ended.
