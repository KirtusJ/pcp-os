"""The state machines of ACR and of the system, written as lists of accepted transitions.

This is the reference statement of the tables, kept independently of the Rust tables in `pcpos-core`
(`machine.rs`), which are written row by row. `export()` prints every (machine, state, event) with what
follows or '-' where the event is not accepted; the Rust test suite regenerates the same text from its own
tables and compares the two byte for byte. Anything not listed here is not accepted.
"""
from __future__ import annotations

MACHINES = {}


def _machine(name, states, events, initial, finals, accepted):
    table = {}
    for line in accepted:
        state, rest = line.split(":", 1)
        for part in rest.split(","):
            event, nxt = part.strip().split(">")
            key = (state.strip(), event.strip())
            assert key not in table, "listed twice: %s %s" % key
            assert key[0] in states and key[1] in events and nxt.strip() in states, line
            table[key] = nxt.strip()
    MACHINES[name] = {"states": states, "events": events, "initial": initial, "finals": finals, "table": table}


_machine("SYSTEM",
         ["START", "VERIFY", "OPERATIONAL", "DEGRADED", "MAINTENANCE", "SAFE", "STOPPING", "STOPPED"],
         ["BEGIN_VERIFY", "VERIFIED_OK", "VERIFY_FAILED", "SERVICE_LOST", "SERVICE_RESTORED", "MAINTENANCE_BEGIN",
          "MAINTENANCE_END", "INTEGRITY_FAULT", "SHUTDOWN", "STOPPED_OK"],
         "START", ["STOPPED"],
         ["START: BEGIN_VERIFY>VERIFY, INTEGRITY_FAULT>SAFE, SHUTDOWN>STOPPING",
          "VERIFY: VERIFIED_OK>OPERATIONAL, VERIFY_FAILED>SAFE, INTEGRITY_FAULT>SAFE, SHUTDOWN>STOPPING",
          "OPERATIONAL: SERVICE_LOST>DEGRADED, MAINTENANCE_BEGIN>MAINTENANCE, INTEGRITY_FAULT>SAFE, SHUTDOWN>STOPPING",
          "DEGRADED: SERVICE_LOST>DEGRADED, SERVICE_RESTORED>OPERATIONAL, MAINTENANCE_BEGIN>MAINTENANCE, INTEGRITY_FAULT>SAFE, SHUTDOWN>STOPPING",
          "MAINTENANCE: MAINTENANCE_END>VERIFY, INTEGRITY_FAULT>SAFE, SHUTDOWN>STOPPING",
          "SAFE: INTEGRITY_FAULT>SAFE, SHUTDOWN>STOPPING",
          "STOPPING: INTEGRITY_FAULT>STOPPED, STOPPED_OK>STOPPED"])

_machine("USE", ["NOT_IN_USE", "IN_USE"], ["ADOPT", "HALT"], "NOT_IN_USE", [],
         ["NOT_IN_USE: ADOPT>IN_USE, HALT>NOT_IN_USE", "IN_USE: ADOPT>IN_USE, HALT>NOT_IN_USE"])

_machine("REVIEW", ["ACTIVE", "PAUSED", "CLOSED_COMPLETE", "CLOSED_INCOMPLETE", "ABORTED"],
         ["PAUSE", "RESUME", "CLOSE_COMPLETE", "CLOSE_INCOMPLETE", "ABORT", "HALT", "OPEN_SESSION"], "ACTIVE", [],
         ["ACTIVE: PAUSE>PAUSED, CLOSE_COMPLETE>CLOSED_COMPLETE, CLOSE_INCOMPLETE>CLOSED_INCOMPLETE, ABORT>ABORTED, HALT>CLOSED_INCOMPLETE",
          "PAUSED: RESUME>ACTIVE, CLOSE_INCOMPLETE>CLOSED_INCOMPLETE, ABORT>ABORTED, HALT>CLOSED_INCOMPLETE",
          "CLOSED_COMPLETE: OPEN_SESSION>ACTIVE", "CLOSED_INCOMPLETE: OPEN_SESSION>ACTIVE", "ABORTED: OPEN_SESSION>ACTIVE"])

_machine("OBJECTION", ["OPEN", "RESOLVED", "WITHDRAWN", "OPEN_RECORDED"], ["RESOLVE", "WITHDRAW", "RECORD_OPEN"], "OPEN",
         ["RESOLVED", "WITHDRAWN", "OPEN_RECORDED"],
         ["OPEN: RESOLVE>RESOLVED, WITHDRAW>WITHDRAWN, RECORD_OPEN>OPEN_RECORDED"])

_machine("OBLIGATION", ["OPEN", "CLOSED"], ["DISCHARGE"], "OPEN", ["CLOSED"], ["OPEN: DISCHARGE>CLOSED"])

_machine("TASK", ["CREATED", "RUNNING", "WAITING", "STOPPING", "EXITED", "DONE", "FAILED", "STOPPED", "LOST"],
         ["START", "BLOCK", "WAKE", "HEARTBEAT", "EXIT_OK", "EXIT_FAIL", "RESULT_VERIFIED", "RESULT_MISSING", "STOP_REQUEST",
          "STOP_CONFIRMED", "DEADLINE"], "CREATED", ["DONE", "FAILED", "STOPPED", "LOST"],
         ["CREATED: START>RUNNING, STOP_REQUEST>STOPPED, DEADLINE>LOST",
          "RUNNING: BLOCK>WAITING, HEARTBEAT>RUNNING, EXIT_OK>EXITED, EXIT_FAIL>FAILED, STOP_REQUEST>STOPPING, DEADLINE>LOST",
          "WAITING: WAKE>RUNNING, HEARTBEAT>WAITING, EXIT_FAIL>FAILED, STOP_REQUEST>STOPPING, DEADLINE>LOST",
          "STOPPING: EXIT_OK>STOPPED, EXIT_FAIL>STOPPED, STOP_REQUEST>STOPPING, STOP_CONFIRMED>STOPPED, DEADLINE>LOST",
          "EXITED: RESULT_VERIFIED>DONE, RESULT_MISSING>FAILED, DEADLINE>FAILED"])

_machine("DELIVERY", ["NOT_DELIVERED", "DELIVERED", "STALE"], ["DELIVER", "CONTEXT_RESET", "RELEASE_CHANGED"], "NOT_DELIVERED", [],
         ["NOT_DELIVERED: DELIVER>DELIVERED, CONTEXT_RESET>NOT_DELIVERED, RELEASE_CHANGED>NOT_DELIVERED",
          "DELIVERED: DELIVER>DELIVERED, CONTEXT_RESET>STALE, RELEASE_CHANGED>STALE",
          "STALE: DELIVER>DELIVERED, CONTEXT_RESET>STALE, RELEASE_CHANGED>STALE"])

_machine("LINK", ["OFFLINE", "ONLINE"], ["UP", "DOWN"], "OFFLINE", [],
         ["OFFLINE: UP>ONLINE, DOWN>OFFLINE", "ONLINE: UP>ONLINE, DOWN>OFFLINE"])


def step(machine, state, event):
    """The state that follows, or None if the event is not accepted in that state."""
    return MACHINES[machine]["table"].get((state, event))


def export():
    lines = ["# machine\tstate\tevent\tnext    ('-' = not accepted)    initial and final states are marked in the 'state' rows"]
    for name, m in MACHINES.items():
        lines.append("%s\tINITIAL\t%s\tFINAL\t%s" % (name, m["initial"], ",".join(m["finals"]) or "-"))
        for s in m["states"]:
            for e in m["events"]:
                lines.append("%s\t%s\t%s\t%s" % (name, s, e, m["table"].get((s, e), "-")))
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    import sys
    sys.stdout.write(export())
