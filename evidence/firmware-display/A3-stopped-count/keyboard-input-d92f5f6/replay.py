"""Recover the four keyboard runs from original frames and serial logs.

Usage: python3 -B replay.py PATH_TO_SOURCE_AT_D92F5F6 EXTRACTED_DIRECTORY
"""
import json
import re
import runpy
import sys
from pathlib import Path

source, root = (Path(name).resolve() for name in sys.argv[1:])
api = runpy.run_path(str(source / "examples/tester-cd/check_small_screens.py"))
glyphs = api["CHECK"]["kernel_glyphs"]()
skip = ("Not run at start: the tests of keys, sleep and stopping a program, "
        "because this keyboard controller does not replay keys for a test.")
results = {}
for receipt in sorted(root.glob("*-*/result.json")):
    folder = receipt.parent
    info = json.loads(receipt.read_text())
    demo = info["kind"] == "demo"
    width, height = (int(n) for n in info["size"].split("x"))

    def read(stem):
        data = (folder / (stem + ".ppm")).read_bytes()
        rows = api["PINS"]["read"](data, glyphs)
        assert rows == (folder / (stem + ".txt")).read_text().splitlines()
        return rows

    pages = sorted(folder.glob("verification-*.ppm"),
                   key=lambda p: int(p.stem.split("-")[-1]))
    text = []
    distinct = set()
    for page in pages:
        rows = read(page.stem)
        distinct.add(tuple(rows))
        tall = height // 20 >= 14
        first, count = (3, height // 20 - 6) if tall else (1, height // 20 - 2)
        segment = rows[first:first + count]
        overlap = max((k for k in range(1, min(len(text), len(segment)) + 1)
                       if text[-k:] == segment[:k]), default=0)
        if text:
            assert overlap
        text.extend(segment[overlap:])
    visible = " ".join(" ".join(text).split())
    assert visible == (folder / "verification-complete.txt").read_text().strip()
    assert (skip in visible) == demo
    start = int(re.search(r"Verified at start: all (\d+) self-tests passed", visible)[1])
    serial = (folder / "serial.txt").read_text()
    final = len(re.findall(r"^  pass ", serial, re.M))
    assert start + 1 == final == (59 if demo else 72)
    assert ("the asynchronous self-tests were not run" in serial) == demo
    assert "state=ACTIVE store=Session save" in serial
    assert "rdesk.elf ended DONE: exit 0" in serial and "ALL SELF-TESTS PASS." in serial
    assert "  FAIL" not in serial
    assert (folder / "home.ppm").read_bytes() == (folder / "home-returned.ppm").read_bytes()
    assert any("rdesk.elf" in row for row in read("programs"))
    assert not api["power_problems"](read("power"))
    api["CHECK"]["check_desk_label"]((folder / "saved-label.ppm").read_bytes(), "keys")
    assert info["exit_code"] == 0 and info["verification_pages"] == len(distinct)
    results[folder.name] = {"startup": start, "final": final, "F5_pages": len(distinct),
                            "skipped_reason_visible": demo, "saved_label": "keys"}
assert len(results) == 4
print(json.dumps(results, indent=2))
