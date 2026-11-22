#!/usr/bin/env python3
"""The Details screen shows what the kernel says of processors and heat, in the words it writes to the serial line.

A laptop without a serial connector has no other way to show these. Predicates, fixed before the first run:

D1  On every kernel the Details screen carries the whole `processors:` sentence, and it is the sentence on the serial line.
D2  A kernel built with `processors`, on a machine of four, shows `other processors: 3 of 3 started; each waits,
    parked, and runs no program`; on a machine of one, `0 of 0`.
D3  A kernel built with `heat` shows a `heat:` sentence that is the serial line's sentence (the emulator offers
    no sensor, so both say so; a reading can differ between two samples and is then compared by its fixed words only).
D4  A kernel built without those features shows neither an `other processors:` nor a `heat:` line.
D5  Where the plain list cannot hold everything above its keys (B3: 1024 by 600 with files from the loader; any
    display under 96 columns), Details is the screen of text that scrolls, and each of the three sentences can
    be read whole by pressing Down. Run on four processors at 1024x600 with nine files from the loader, and at 640x480.

D6  On every kernel the Details screen carries the two sentences on how the screen is written, `display memory:`
    (how many of its pages are mapped write-combining) and `display: a program that asks ...`, and each is the
    serial line's sentence.

D7  On every kernel the Details screen carries the `elapsed clock:` sentence and it is the serial line's: the
    HPET on the machines above, and on a machine built without one (q35,hpet=off) the fallback's sentence.

What this does not show: a reading of a real sensor, which no emulator here offers.

Usage: check_details.py <plain kernel> <kernel with processors,heat> <new output directory>
"""
import runpy
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OPTICAL = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))
READ = runpy.run_path(str(ROOT / "examples/tester-cd/check_compact_pins.py"))["read"]


def squeeze(text):
    return " ".join(text.split())


def details(kernel, cpus, output, size=None, documents=0, machine="q35"):
    """Boot on `cpus` processors straight to Details; return (every page read from the screen, the serial text).
    With `size` the desktop is asked for that mode; `documents` small files are handed over by the loader."""
    output.mkdir(parents=True)
    serial = output / "serial.txt"
    names = []
    for index in range(documents):
        names.append("note%d.txt" % index)
        (output / names[-1]).write_text("a file from the loader, number %d\n" % index)
    command = "first=details" + ((" desktop=" + size) if size else "")
    invocation = ["qemu-system-x86_64", "-machine", machine, "-accel", "tcg", "-cpu", "max", "-smp", str(cpus), "-m", "256M",
                  "-kernel", str(kernel)] + (["-initrd", ",".join(names)] if names else []) + [
                  "-append", command, "-vga", "std", "-display", "none", "-nic", "none",
                  "-serial", "file:" + str(serial.resolve()), "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"]
    with subprocess.Popen(invocation, stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=output) as vm:
        try:
            monitor = OPTICAL["Monitor"](vm)
            monitor.call("qmp_capabilities")
            screens = OPTICAL["DesktopScreens"](monitor, output.resolve(), 60)
            screens.read = READ
            # A frame is read only once it is whole: the keys' line is the last thing either Details view draws
            # (B3 saw a frame accepted at its PROGRAMS heading, before the MACHINE lines were on it).
            pages = [screens.wait("details-00", lambda rows: any("PROGRAMS" in row for row in rows)
                                  and any("Esc back" in row or "P off" in row for row in rows))]
            # The screen of text scrolls with Down, half a page at a time; the plain list does not scroll.
            if any("Up/Down scroll" in row for row in pages[0]):
                for step in range(1, 40):
                    monitor.key("down")
                    time.sleep(0.4)
                    rows = screens.wait("details-%02d" % step, lambda rows: any("Esc back" in row for row in rows))
                    if rows == pages[-1]:
                        break
                    pages.append(rows)
            for index, rows in enumerate(pages):
                (output / ("details-%02d.txt" % index)).write_text("\n".join(rows) + "\n")
        finally:
            vm.kill()
    return pages, serial.read_text(errors="replace")


def sentence(pages, start):
    """The wrapped sentence beginning with `start`, joined: the longest reading of it on any page (a page can cut it)."""
    best = None
    for rows in pages:
        body = [row.strip() for row in rows]
        for index, row in enumerate(body):
            if row.startswith(start):
                words = [row]
                for more in body[index + 1:]:
                    if not more or more.split(":")[0] in ("heat", "other processors", "processors", "display", "display memory", "elapsed clock", "keyboard", "store") or "Esc back" in more or "Up/Down" in more:
                        break
                    words.append(more)
                found = squeeze(" ".join(words))
                if best is None or len(found) > len(best):
                    best = found
    return best


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
        shown, told = sentence(pages, "elapsed clock: "), serial_sentence(serial, "elapsed clock: ")
        check("D7 %s: the elapsed clock on Details is the serial line's, the HPET" % label, shown is not None and shown == told and "HPET" in shown, "%r / %r" % (shown, told))
        for start in ("display memory: ", "display: a program that asks "):
            shown, told = sentence(pages, start), serial_sentence(serial, start)
            check("D6 %s: %r on Details is the serial line's" % (label, start.strip()), shown is not None and told is not None and shown == told, "%r / %r" % (shown, told))
        others, heat = sentence(pages, "other processors:"), sentence(pages, "heat:")
        if kernel is full:
            want = "other processors: %d of %d started; each waits, parked, and runs no program" % (cpus - 1, cpus - 1)
            check("D2 %s: other processors as started" % label, others == want, repr(others))
            said_heat = serial_sentence(serial, "heat: ")
            fixed = lambda s: s and " ".join(w for w in s.split() if not w.isdigit())
            check("D3 %s: the heat sentence on Details is the serial line's" % label, heat is not None and fixed(heat) == fixed(said_heat), "%r / %r" % (heat, said_heat))
        else:
            check("D4 %s: no other-processors or heat line without the features" % label, others is None and heat is None, "%r / %r" % (others, heat))
    for label, size, documents in (("short-1024x600", "1024x600", 9), ("narrow-640x480", "640x480", 0)):
        pages, serial = details(full, 4, output / label, size, documents)
        scrolls = any("Up/Down scroll" in row for row in pages[0])
        said = serial_sentence("\n".join(l for l in serial.splitlines() if "root pointer" not in l), "processors: ")
        fixed = lambda s: s and " ".join(w for w in s.split() if not w.isdigit())
        whole = (sentence(pages, "processors:") == said
                 and sentence(pages, "other processors:") == "other processors: 3 of 3 started; each waits, parked, and runs no program"
                 and fixed(sentence(pages, "heat:")) == fixed(serial_sentence(serial, "heat: ")))
        check("D5 %s: Details scrolls, and all three sentences are read whole" % label, scrolls and whole,
              "scrolls=%r pages=%d %r / %r / %r" % (scrolls, len(pages), sentence(pages, "processors:"), sentence(pages, "other processors:"), sentence(pages, "heat:")))
    pages, serial = details(plain, 1, output / "no-hpet", machine="q35,hpet=off")
    shown, told = sentence(pages, "elapsed clock: "), serial_sentence(serial, "elapsed clock: ")
    check("D7 no-hpet: without the timer, Details says the fallback, as the serial line does", shown is not None and shown == told and "delivered PIT interrupts" in shown, "%r / %r" % (shown, told))
    print("%s: %d failed" % ("FAILED" if failed else "ALL PASS", failed))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
