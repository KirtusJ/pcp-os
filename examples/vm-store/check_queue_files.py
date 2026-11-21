"""Check the saved queue's Files capture and single-change controls made from its pixels."""
from pathlib import Path
import runpy
import sys

from check_queue import saved_queue_visible


def check_capture(path):
    render = runpy.run_path(str(Path(__file__).resolve().parents[1] / "tester-cd/check_launcher_grants.py"))
    glyphs = render["SCREEN"]["kernel_glyphs"]()
    shapes = {char: shape for shape, char in glyphs.items()}
    read = render["SCREEN"]["read_screen"]
    original = path.read_bytes()
    rows = read(original, glyphs)
    if not rows or not saved_queue_visible(rows, original):
        raise ValueError(f"{path}: saved queue missing from the original Files screen")

    name_col = rows[4].index("parrots.queue")
    size_col = rows[4].index("3 KiB")
    controls = [
        ("wrong filename", 4, name_col, "parrots.queue", "parrots.queue-old"),
        ("wrong file size", 4, size_col, "3 KiB", "13 KiB"),
        ("extra heading text", 1, len(rows[1]), "", " pending"),
        ("extra sync text", 2, len(rows[2]), "", " pending"),
        ("second file", 5, name_col, "", "other.queue  1 KiB"),
        ("extra file above list", 3, name_col, "", "other.queue  1 KiB"),
        ("extra file beside footer", len(rows) - 2, len(rows[-2]), "", "  other.queue  1 KiB"),
        ("extra file below footer", len(rows) - 1, name_col, "", "other.queue  1 KiB"),
        ("leading space in filename", 4, name_col, "parrots.queue", " parrots.queue"),
        ("size one column left", 4, size_col - 1, " 3 KiB", "3 KiB "),
        ("extra space in size", 4, size_col, "3 KiB", "3  KiB"),
    ]
    for label, line, column, old, new in controls:
        changed = render["replace_text"](original, line, column, old, new, shapes)
        altered = read(changed, glyphs)
        expected = rows.copy()
        end = column + max(len(old), len(new))
        row = rows[line].ljust(end)
        expected[line] = (row[:column] + new.ljust(len(old)) + row[end:]).rstrip()
        if altered != expected or saved_queue_visible(altered, changed):
            raise ValueError(f"{path}: {label} did not produce the expected reading and rejection")
        print(f"rejected {label} at row {line}, column {column}: {new!r}")


if __name__ == "__main__":
    check_capture(Path(sys.argv[1]))
