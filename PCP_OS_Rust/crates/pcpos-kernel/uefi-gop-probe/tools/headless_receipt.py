#!/usr/bin/env python3
"""Bounded log oracle for the isolated Q35/OVMF *no-GOP* diagnostic.

This checks internal consistency of captured diagnostic text and the QEMU
process result. It does not authenticate the producer, attest firmware,
validate PCP-OS boot, or imply that a no-display configuration succeeded in
rendering a desktop. The legitimate EFI outcome here is EFI_NOT_FOUND.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

EFI_NOT_FOUND = "0x800000000000000e"
ENTRY = "PCPOS_EFI_ENTRY_V1"
NO_GOP = "PCPOS_GOP_PROBE_V1 NO_GOP status=" + EFI_NOT_FOUND
RETURN = "PCPOS_EFI_RETURN_V1 " + EFI_NOT_FOUND

_FORBIDDEN_PREFIXES = (
    "PCPOS_GOP_PROBE_V1 BEFORE ",
    "PCPOS_GOP_PROBE_V1 AFTER ",
    "PCPOS_GOP_PROBE_V1 RESTORED ",
    "PCPOS_GOP_PROBE_V1 QUERY ",
    "PCPOS_GOP_PROBE_V1 STANDALONE ",
    "PCPOS_GOP_PROBE_V1 STANDALONE_FIXTURE_SET_MODE ",
    "PCPOS_GOP_PROBE_V1 SET_MODE_STATUS ",
    "PCPOS_GOP_PROBE_V1 RESTORE_STATUS ",
    "PCPOS_GOP_PROBE_V1 END READ_ONLY",
)


class EvidenceMismatch(ValueError):
    """The diagnostic receipt contradicts the no-GOP acceptance criterion."""


def check_headless_receipt(report: str, qemu_exit: int) -> None:
    """Require exact ordered EFI_NOT_FOUND status and return, not substrings.

    The exit status 124 means the VM was timed out after the app returned.
    It is acceptable for the *test harness*, not an EFI success value.
    """
    if qemu_exit not in (0, 124):
        raise EvidenceMismatch(f"QEMU execution failed: exit {qemu_exit}")
    lines = report.splitlines()
    if not lines or any("\ufffd" in line for line in lines):
        raise EvidenceMismatch("Empty or invalid debugcon report")
    for forbidden in _FORBIDDEN_PREFIXES:
        if any(line.startswith(forbidden) for line in lines):
            raise EvidenceMismatch(f"Unexpected video or SetMode observation: {forbidden}")
    if lines.count(ENTRY) != 1:
        raise EvidenceMismatch("Exactly one EFI entry marker is required")
    if lines.count(NO_GOP) != 1:
        raise EvidenceMismatch("NO_GOP must report exactly EFI_NOT_FOUND once")
    if lines.count(RETURN) != 1:
        raise EvidenceMismatch("EFI return must report exactly EFI_NOT_FOUND once")
    # Explicitly reject extra, contradictory or ambiguous status/return lines.
    status_lines = [line for line in lines if line.startswith("PCPOS_GOP_PROBE_V1 NO_GOP status=")]
    return_lines = [line for line in lines if line.startswith("PCPOS_EFI_RETURN_V1 ")]
    if status_lines != [NO_GOP] or return_lines != [RETURN]:
        raise EvidenceMismatch("Conflicting or duplicated EFI status/return")
    entry_at, no_gop_at, return_at = (
        lines.index(ENTRY), lines.index(NO_GOP), lines.index(RETURN)
    )
    if not entry_at < no_gop_at < return_at:
        raise EvidenceMismatch("EFI entry, NO_GOP and return are out of order")
    # Warn about unrelated strings rather than interpreting them as receipts:
    # the critical status lines above must match as complete records.
    if any(line.startswith("PCPOS_EFI_ENTRY_V1") and line != ENTRY for line in lines):
        raise EvidenceMismatch("Ambiguous extra EFI entry marker")
    if any(re.search(r"PCPOS_GOP_PROBE_V1 (?:NO_GOP|BEFORE|QUERY|SET_MODE)", line)
           and line != NO_GOP for line in lines):
        raise EvidenceMismatch("Unrecognized GOP report alongside no-GOP")


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: headless_receipt.py <debugcon-log> <qemu-exit-code>", file=sys.stderr)
        return 2
    try:
        code = int(argv[2], 10)
        data = Path(argv[1]).read_bytes()
        text = data.decode("ascii", "strict")
        check_headless_receipt(text, code)
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"FAIL: no-GOP receipt rejected: {exc}", file=sys.stderr)
        return 1
    print("PASS: standalone UEFI no-GOP EFI_NOT_FOUND confirmed in bounded QEMU fixture")
    print(f"QEMU exit {code}; no PCP-OS kernel, physical display, or host calibration observed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
