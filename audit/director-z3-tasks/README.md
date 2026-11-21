# Director Z3's task record

Builds and checks that Director Z3 (a Claude session) runs for the OS baseline, kept so that the record
outlives the session's container. Started 21 November 2026 at the owner's request that these run as PCP
tasks and that the record last.

| Here | What it is |
| --- | --- |
| `.pcp/register.jsonl` | A PCP register. From the record `probe3` on, a task is admitted before it runs and released after, by the participant `DirectorZ3`, through the host's own tools. The three earlier records (`probe`, `probe2` and the build and check `hold-055845`) were run before the limits were set and have no admission or release: each one's `runner.txt` says so. Check the register with the host built from this repository, `pcpos` 1.0.22: `PCP_OS_Rust/target/release/pcpos verify audit/director-z3-tasks/.pcp/register.jsonl`. The host in the published Enterprise edition is older, does not know the event `TASK_LIMITS_SET`, and refuses the replay at event 5 (A3's observation). |
| `BASIS.md` | Why the task limits in that register carry the owner's label, and that the session wrote them. |
| `records/<label>-<time>/` | One run: `ASYNC_RECORD.json` (exit code, duration, timeout or not, and the SHA-256 of each file the task left) and the task's `T/STDOUT.txt` and `T/STDERR.txt`. |
| `records/<label>-<time>.plan/` | The command as it was run (`plan.json`), the runner's own line and the release (`runner.txt`), and the runner's exit code (`DONE`). |
| `tools/ptask.sh` | Runs one command this way: `tools/ptask.sh <label> <timeout seconds> <command...>`. |
| `tools/pmcp.py` | One call of a host tool over `pcpos mcp`, on this register. |
| `tools/async_run.py` | The runner, copied from the research repository; `tools/ORIGIN.txt` says from which commit. |

## What this does and does not show

- The register shows that a place was reserved and given back, in order, by whom. The host says so itself:
  admission "does not launch a process" and release "records a reservation change, not observed process
  termination".
- The runner's record shows that a command ran, how it ended and what it left. It is the runner's own
  statement, in Python, on one machine: the session's container.
- Neither authenticates anybody. The limits are `DECLARED`.
- Paths in the records are those of the container the task ran in. The first four records were made in the
  session's scratch folder before this folder existed and name it; the three named `probe` are trials of
  the route itself. Which records have an admission is said in the table above.
- The tools are a workspace helper. Under `RUST_PRIMACY.md` a command of the host that starts a program and
  records its end would be written in Rust; that is proposed, not done.
