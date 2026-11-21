# Coordination for the OS baseline

For a director session starting work on the PCP OS baseline. Read `RUST_PRIMACY.md` and `AGENTS.md` first;
this file adds only what the baseline work has agreed. It is a record of agreements, and each one names
where it was made. If this file and a director's own statement on the check-in thread differ, the thread
is right and this file needs correcting.

## The goal

A clean baseline that is relatively safe to boot across platforms and usable for daily work on desktops
and laptops, with the PCP repositories (`pcp-os`, `pcp`, `pcp-fidelity-lab`, `pcp-os-claude`) tidy.
Assume everything is faulty, old or untidy until it has been audited. Security and common standard
practice decide where there is a choice (owner, 20 November 2026).

## How the OS is approached

As though it were inside a commercial airliner (owner, 21 November 2026). For the desktop this was agreed
as five rules (A3 and B3, check-in thread, 21 November 2026):

1. No consequential action on a single unguarded press. What stops, removes or changes takes two
   deliberate steps: choose, then confirm.
2. Everything is reachable from what is visible. A shortcut is a faster route, never the only one.
3. State is always shown, and there are no hidden modes.
4. A key means one thing everywhere on the desktop.
5. Typing keys do nothing by themselves.

The same care applies below the desktop: the kernel writes to a hardware port only where it has reason to
know what is there. `PCP_OS_Rust/crates/pcpos-kernel/tests/port_writes.py` lists every port write and
fails when one is not on the reviewed list beside it.

## Lanes

| Director | Lane |
|---|---|
| A3 | Published media and the first-boot guide (in `pcp`); the distribution's checks |
| B3 | Boot loader, firmware, disk and persistence; their checks |
| Z3 | Source baseline, host platforms, tidiness, the desktop; the tester CD recipe and check |

A lane is who leads, not who may look. Each director reads the others' work and says what they find.

## Working together

- Check in on the directors' check-in thread: https://github.com/KirtusJ/pcp-os/pull/276 at the time of
  writing. Say who you are in each post, since all sessions post under the owner's account.
- Never state that something was coordinated unless the other directors' answers are on the thread.
- A change to `master`: post the exact commit, what it changes, and the checks that passed on it; wait
  for the other directors to read that commit back; merge with the expected head given.
- A result names the commit, the command, the output, and whose machine ran it. "Passed in my container"
  is not "passed in CI", and neither is "runs on hardware".
- A change that moves another director's checker is proposed first and made after they answer.
- Keep failed checks and controls in the record. Correct your own mistakes on the thread.

## What is not established

Nothing has been run on a physical machine. The open hardware gaps are in issue 309. Until they close, the
first-boot instruction stays: a virtual machine, the CD only, no disk attached.
