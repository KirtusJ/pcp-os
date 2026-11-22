#!/usr/bin/env python3
"""The Details screen shows what the kernel says of processors and heat, in the words it writes to the serial line.

A laptop without a serial connector has no other way to show these. Predicates, fixed before the first run:

D1  On every kernel the Details screen carries the whole `processors:` sentence, and it is the sentence on the serial line.
D2  A kernel built with `processors`, on a machine of four, shows `other processors: 3 of 3 started and parked`;
    on a machine of one it shows `other processors: 0 of 0 started and parked`.
D3  A kernel built with `heat` shows a `heat:` sentence that is the serial line's sentence (the emulator offers
    no sensor, so both say so; a reading can differ between two samples and is then compared by its fixed words only).
D4  A kernel built without those features shows neither an `other processors:` nor a `heat:` line.

What this does not show: the narrow Details screen (under 96 columns) draws the same sentences through the
same function but is not opened here; and no reading of a real sensor, which no emulator here offers.

Usage: check_details.py <plain kernel> <kernel with processors,heat> <new output directory>
"""
import runpy
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OPTICAL = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))
READ = runpy.run_path(str(ROOT / "examples/tester-cd/check_compact_pins.py"))["read"]


def squeeze(text):
    return " ".join(text.split())


def details(kernel, cpus, output):
    """Boot on `cpus` processors straight to Details; return (everything read from the screen, the serial text)."""
    output.mkdir(parents=True)
    serial = output / "serial.txt"
    invocation = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-smp", str(cpus), "-m", "256M",
                  "-kernel", kernel.name, "-append", "first=details", "-vga", "std", "-display", "none", "-nic", "none",
                  "-serial", "file:" + str(serial), "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"]
    with subprocess.Popen(invocation, stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=kernel.parent) as vm:
        try:
            monitor = OPTICAL["Monitor"](vm)
            monitor.call("qmp_capabilities")
            screens = OPTICAL["DesktopScreens"](monitor, output, 60)
            screens.read = READ
            # `first=details` opens the wide Details list where the display has 96 columns or more.
            pages = [screens.wait("details-00", lambda rows: any("MACHINE" in row for row in rows) and any(row.strip().startswith("store:") for row in rows))]
            for index, rows in enumerate(pages):
                (output / ("details-%02d.txt" % index)).write_text("\n".join(rows) + "\n")
        finally:
            vm.kill()
    return pages, serial.read_text(errors="replace")


def sentence(pages, start):
    """The wrapped sentence beginning with `start`, joined, from the page that shows its beginning."""
    for rows in pages:
        body = [row.strip() for row in rows]
        for index, row in enumerate(body):
            if row.startswith(start):
                words = [row]
                for more in body[index + 1:]:
                    if not more or more.split(":")[0] in ("heat", "other processors", "processors", "display", "keyboard", "store") or "Esc back" in more:
                        break
                    words.append(more)
                return squeeze(" ".join(words))
    return None


def serial_sentence(text, start):
    found = [squeeze(line) for line in text.splitlines() if line.startswith(start)]
    return found[-1] if found else None


def main():
    plain, full, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), Path(sys.argv[3])
    output.mkdir(parents=True)
    failed = 0

    def check(name, passed, detail=""):
        nonlocal failed
        failed += not passed
        print(("PASS " if passed else "FAIL ") + name + ((" | " + detail) if detail else ""))

    for label, kernel, cpus in (("plain-1", plain, 1), ("plain-4", plain, 4), ("full-1", full, 1), ("full-4", full, 4)):
        pages, serial = details(kernel, cpus, output / label)
        shown = sentence(pages, "processors:")
        # The serial line also names where the root pointer is; the sentence compared is the one after it.
        said = serial_sentence("\n".join(l for l in serial.splitlines() if "root pointer" not in l), "processors: ")
        check("D1 %s: the processors sentence on Details is the serial line's" % label, shown is not None and shown == said, "%r / %r" % (shown, said))
        others, heat = sentence(pages, "other processors:"), sentence(pages, "heat:")
        if kernel is full:
            want = "other processors: %d of %d started and parked; none runs any work yet" % (cpus - 1, cpus - 1)
            check("D2 %s: other processors as started" % label, others == want, repr(others))
            said_heat = serial_sentence(serial, "heat: ")
            fixed = lambda s: s and " ".join(w for w in s.split() if not w.isdigit())
            check("D3 %s: the heat sentence on Details is the serial line's" % label, heat is not None and fixed(heat) == fixed(said_heat), "%r / %r" % (heat, said_heat))
        else:
            check("D4 %s: no other-processors or heat line without the features" % label, others is None and heat is None, "%r / %r" % (others, heat))
    print("%s: %d failed" % ("FAILED" if failed else "ALL PASS", failed))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
