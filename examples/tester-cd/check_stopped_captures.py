#!/usr/bin/env python3
"""Check the stopped-screen reader against wrong pairings and edits of captured guest pixels.

Run after check_stopped_screen.py with both keyboard demonstration kernels. All edits are to
in-memory copies. The JSON report identifies the original P6 and serial files and the edited P6
digests. The same font reader and stopped-screen predicate judge original and adverse inputs.
"""
import argparse
import hashlib
from itertools import permutations
import json
from pathlib import Path
import runpy

STOP = runpy.run_path(str(Path(__file__).with_name("check_stopped_screen.py")))
# Within each group every boot reports 66 passes, but each has a different keyboard outcome.
GROUPS = (("nokbd-320", "busy-320", "silent-320"), ("noreplay-800", "busy-800", "silent-800"))
OTHER = ("nokbd-1080", "kbd-800")
# What each boot's fixture says of the tests that press keys (H8), by the start of its folder's name.
SKIPPED = {"nokbd": STOP["NO_CONTROLLER"], "busy": STOP["SETUP_REFUSED"], "silent": STOP["NO_ACK"],
           "noreplay": STOP["NO_REPLAY"], "kbd": None}
CUTS = ("Not", "Not ru", "Not r", "the six tests that press", "there is no keyboard")


class CapturedBoot(STOP["Guest"]):
    """Use the live checker's serial accessors on one finished boot without starting QEMU."""

    def __init__(self, folder, glyphs):
        self.out, self.serial = folder, folder / "serial.txt"
        self.frame = (folder / "stopped.ppm").read_bytes()
        self.rows = STOP["PINS"]["read"](self.frame, glyphs)
        self.keyboard_outcome = self.outcome()
        self.reason = STOP["REASONS"][self.keyboard_outcome]
        self.passes = self.passed()

    def problems(self, rows):
        return STOP["stopped_problems"](rows, self.keyboard_outcome != "no controller",
                                        self.passes, self.keyboard_outcome)

    def skipped(self, rows):
        """H8 and H9 alone, with this boot's own serial record."""
        name = self.out.name
        return STOP["skip_problems"](self.serial.read_text(), rows, SKIPPED[name.split("-")[0]], compact=name.endswith("-320"))


def rows_changed(boot, start, end, glyphs, words=None):
    """Clear the text rows from `start` to `end`; then, if given, draw `words` on the first of them in the
    kernel's own font, in the colours of the frame's title."""
    magic, dimensions, depth, pixels = boot.frame.split(b"\n", 3)
    width = int(dimensions.split()[0])
    stride, changed = width * 3, bytearray(pixels)
    for offset in range(start * 20 * stride, end * 20 * stride, stride):
        changed[offset:offset + stride] = pixels[offset:offset + 3] * width
    if words:
        title = next(i for i, row in enumerate(boot.rows) if row.strip())
        cell = [pixels[at:at + 3] for y in range(20) for x in range(10) for at in (((title * 20 + y) * width + 20 + x) * 3,)]
        ground = max(set(cell), key=cell.count)
        ink = next(pixel for pixel in cell if pixel != ground)
        shapes = {character: shape for shape, character in glyphs.items()}
        for column, character in enumerate(words, 2):
            for at, lit in enumerate(shapes[character]):
                if lit:
                    offset = ((start * 20 + at // 10) * width + column * 10 + at % 10) * 3
                    changed[offset:offset + 3] = ink
    return b"\n".join((magic, dimensions, depth, changed))


def reason_rows(rows, reason):
    """Find the whole sentence in one or more consecutive occupied rows."""
    for start, row in enumerate(rows):
        if not row.strip():
            continue
        for end in range(start + 1, len(rows) + 1):
            words = STOP["text_of"](rows[start:end])
            if words == reason:
                return start, end
            if not reason.startswith(words + " "):
                break
    raise ValueError("the reason has no whole row span: " + reason)


def change_reason(target, donor=None):
    """Erase the target's reason, or append the donor's reason below the target's existing text."""
    magic, dimensions, depth, pixels = target.frame.split(b"\n", 3)
    width, height = map(int, dimensions.split())
    if magic != b"P6" or depth != b"255" or len(pixels) != width * height * 3:
        raise ValueError("malformed frame: " + target.out.name)
    stride = width * 3
    changed = bytearray(pixels)
    if donor is None:
        start, end = reason_rows(target.rows, target.reason)
        # Each row's first pixel is in the left margin. Use its own ground color.
        for offset in range(start * 20 * stride, end * 20 * stride, stride):
            changed[offset:offset + stride] = pixels[offset:offset + 3] * width
    else:
        start, end = reason_rows(donor.rows, donor.reason)
        dest = 1 + max(i for i, row in enumerate(target.rows) if row.strip())
        header, size, maximum, source = donor.frame.split(b"\n", 3)
        if (magic, dimensions, depth) != (header, size, maximum) or dest + end - start > len(target.rows):
            raise ValueError("no matching row band available for " + donor.out.name)
        band = source[start * 20 * stride:end * 20 * stride]
        offset = dest * 20 * stride
        changed[offset:offset + len(band)] = band
    return b"\n".join((magic, dimensions, depth, changed))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="completed stopped-screen evidence directory")
    args = parser.parse_args()
    glyphs = STOP["CHECK"]["kernel_glyphs"]()
    names = (*GROUPS[0], *GROUPS[1], *OTHER)
    boots = {name: CapturedBoot(args.evidence / name, glyphs) for name in names}
    checks = []
    failures = []

    def check(case, serial_from, frame_from, frame, rows, adverse):
        found = boots[serial_from].problems(rows)
        checks.append({"case": case, "serial_from": serial_from, "frame_from": frame_from,
                       "frame_sha256": hashlib.sha256(frame).hexdigest(), "problems": found})
        if bool(found) != adverse:
            failures.append(f"{case}: {frame_from} pixels with {serial_from} serial: {found}")

    for name, boot in boots.items():
        check("original", name, name, boot.frame, boot.rows, False)

    def check_paragraph(case, name, frame, wanted, adverse):
        """`wanted` is what the edited frame must read as, so that the edit is known to be the one meant."""
        boot = boots[name]
        rows = STOP["PINS"]["read"](frame, glyphs)
        if STOP["text_of"](rows) != wanted:
            raise ValueError(f"{name}: {case} did not make the frame meant: {STOP['text_of'](rows)!r}")
        found = boot.skipped(rows)
        checks.append({"case": case, "serial_from": name, "frame_from": name,
                       "frame_sha256": hashlib.sha256(frame).hexdigest(), "problems": found})
        if bool(found) != adverse:
            failures.append(f"{case}: {name}: {found}")

    for name, boot in boots.items():
        check_paragraph("original, skipped tests", name, boot.frame, STOP["text_of"](boot.rows), False)
    # A small screen that shows part of the skipped-tests paragraph where its keyboard reason was.
    for name in GROUPS[0]:
        boot = boots[name]
        start, end = reason_rows(boot.rows, boot.reason)
        before, after = (STOP["text_of"](part) for part in (boot.rows[:start], boot.rows[end:]))
        for cut in CUTS:
            check_paragraph("part of the paragraph: " + cut, name, rows_changed(boot, start, end, glyphs, cut),
                            " ".join((before, cut, after)), True)
    # The roomy screen that shows the paragraph on two rows, with either row or both cleared.
    boot = boots["noreplay-800"]
    start, end = reason_rows(boot.rows, "Not run: " + STOP["WHY"])
    if end - start != 2:
        raise ValueError("the paragraph is not on two rows at 800 by 600")
    for case, first, last in (("first row", start, start + 1), ("second row", start + 1, end), ("both rows", start, end)):
        wanted = STOP["text_of"](boot.rows[:first] + boot.rows[last:])
        check_paragraph("paragraph cleared: " + case, "noreplay-800", rows_changed(boot, first, last, glyphs), wanted, True)

    for group in GROUPS:
        peers = [boots[name] for name in group]
        if (len({boot.passes for boot in peers}) != 1 or len({boot.keyboard_outcome for boot in peers}) != len(group)
                or len({boot.frame.split(b"\n", 2)[1] for boot in peers}) != 1):
            raise ValueError(f"expected equal counts and geometry, distinct keyboard outcomes in {group}")
        for target, source in permutations(group, 2):
            other = boots[source]
            check("stale", target, source, other.frame, other.rows, True)

    def check_edit(case, target, donor=None):
        boot = boots[target]
        reason = boots[donor].reason if donor else boot.reason
        frame = change_reason(boot, boots[donor] if donor else None)
        rows = STOP["PINS"]["read"](frame, glyphs)
        before, after = STOP["text_of"](boot.rows), STOP["text_of"](rows)
        # No other words may disappear or change, including the power instructions and pass count.
        if (after.count(reason) != before.count(reason) + (1 if donor else -1)
                or " ".join(after.replace(reason, "").split()) != " ".join(before.replace(reason, "").split())):
            raise ValueError(f"{target}: {case} changed something besides the reason")
        check(case, target, target, frame, rows, True)
        if donor:
            checks[-1]["reason_pixels_from"] = donor

    for name, boot in boots.items():
        if boot.reason:
            check_edit("erased", name)
            # A 320x200 capture can have no spare row. Test duplication on the larger layouts.
            if not name.endswith("-320"):
                check_edit("duplicated", name, name)
    for donor in ("busy-800", "silent-800"):
        for target in ("kbd-800", *GROUPS[1]):
            if target != donor:
                check_edit("foreign reason added", target, donor)

    serial = {name: hashlib.sha256(boot.serial.read_bytes()).hexdigest() for name, boot in boots.items()}
    report = {"serial_sha256": serial, "checks": checks, "failures": failures}
    (args.evidence / "capture-controls.json").write_text(json.dumps(report, indent=2) + "\n")
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"stopped captures: {2 * len(boots)} readings of {len(boots)} originals accepted; {len(checks) - 2 * len(boots)} adverse pairs or edits rejected")


if __name__ == "__main__":
    main()
