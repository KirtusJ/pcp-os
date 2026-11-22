#!/usr/bin/env python3
"""A worker that is started only beside another program is not in the list of programs, and is still in Details.

    python3 examples/companion/check_listed.py <kernel built with companion> <new output directory>

The kernel lists every file the loader hands over that begins as an ELF file, so the two "programs" here are
small made-up files that are never started: one stands for a program, the other for its worker. Each boot opens
the list of programs with Enter and reads it by the kernel's font, then opens Details (Ctrl+D) and reads that.
Predicates, fixed before the first run.

 L1 paired        the line carries worker_for=<the program's digest> worker_image=<the worker's digest>:
                  the list of programs names the program and does not name the worker; the serial line says
                  one worker is not listed; Details names both files and shows the worker's digest as it shows
                  any file's: the first sixteen digits on the wide list, all of them on the scrolling one.
                  (As first written this asked for the whole digest, and the first complete run failed it: at
                  1280 by 720 Details is the wide list, which has always shown sixteen digits of a file's
                  digest. The worker was there. The predicate now says what Details does.)
 L2 not paired    the same two files and no such options: both are in the list. The control.
 L3 nothing to    worker_for= names a digest no loaded file has: both are in the list, and the serial line
    be beside     does not say a worker was left out.
 L4 cut line      the pairing options and a line longer than the kernel takes whole: both are in the list.
 L6 not a program worker_for= names the digest of a loaded file that is not a program (a short text file):
                  the worker, the only program there is, is in the list, and the serial line does not say a
                  worker was left out. (Director 2: the first version looked among all the loader's files and
                  would have hidden it. Added after that finding, and fixed before its first run.)
 L5 every boot    ends with status 33 after F10 and Enter, with no line beginning "  FAIL".

These are emulated screens. Nothing here starts a worker; examples/companion/check.py does that.
"""
import hashlib
import runpy
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OPTICAL = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))
READ = runpy.run_path(str(ROOT / "examples/tester-cd/check_compact_pins.py"))["read"]
LEFT_OUT = "programs: 1 worker(s) started only beside another program are not listed; each is among the loader's files"


def program(seed):
    return b"\x7fELF" + hashlib.sha256(seed).digest() * 4


def boot(kernel, output, tag, files, extra, shown="front-program"):
    folder = output / tag
    (folder / "files").mkdir(parents=True)
    paths = []
    for name, data in files:
        (folder / "files" / name).write_bytes(data)
        paths.append(str(folder / "files" / name))
    measure = ROOT / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
    pin = subprocess.check_output([sys.executable, str(measure), str(kernel)], text=True).strip()
    command = f"image={pin} desktop=1280x720 ui_scale=1" + extra
    serial = folder / "serial.txt"
    invocation = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "256M", "-kernel", kernel.name,
                  "-initrd", ",".join(paths), "-append", command, "-vga", "std", "-display", "none", "-nic", "none",
                  "-serial", "file:" + str(serial), "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"]
    with subprocess.Popen(invocation, stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=kernel.parent) as vm:
        try:
            monitor = OPTICAL["Monitor"](vm)
            monitor.call("qmp_capabilities")
            screens = OPTICAL["DesktopScreens"](monitor, folder, 30)
            screens.read = READ
            screens.wait("home", lambda rows: any("Programs" in r for r in rows))
            monitor.key("ret")
            time.sleep(0.5)
            listed = screens.wait("programs", lambda rows: any(shown in r for r in rows) and any("F1 keys" in r for r in rows))
            (folder / "programs.txt").write_text("\n".join(listed) + "\n")
            monitor.key("ctrl", "d")
            time.sleep(0.5)
            pages = [screens.wait("details-0", lambda rows: any("PROGRAMS" in r for r in rows) and any("Esc back" in r or "P off" in r for r in rows))]
            if any("Up/Down scroll" in r for r in pages[0]):
                for step in range(1, 40):
                    monitor.key("down")
                    time.sleep(0.3)
                    rows = screens.wait("details-%d" % step, lambda rows: any("Esc back" in r for r in rows))
                    if rows == pages[-1]:
                        break
                    pages.append(rows)
            (folder / "details.txt").write_text("\n\n".join("\n".join(p) for p in pages) + "\n")
            monitor.key("esc")
            time.sleep(0.4)
            monitor.key("esc")
            time.sleep(0.4)
            monitor.key("f10")
            time.sleep(0.6)
            monitor.key("ret")
            try:
                status = vm.wait(timeout=30)
            except subprocess.TimeoutExpired:
                status = None
        finally:
            if vm.poll() is None:
                vm.kill()
    return listed, pages, serial.read_text(errors="replace").splitlines(), status


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    kernel, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    front, worker = program(b"a program that has a worker"), program(b"the worker")
    df, dw = hashlib.sha256(front).hexdigest(), hashlib.sha256(worker).hexdigest()
    files = [("front-program", front), ("its-worker", worker)]
    failed = []

    def predicate(name, holds, detail=""):
        print(("PASS " if holds else "FAIL ") + name + (" | " + detail if detail else ""))
        if not holds:
            failed.append(name)

    def names(rows):
        return [n for n in ("front-program", "its-worker") if any(n in r for r in rows)]

    statuses = []
    listed, pages, serial, status = boot(kernel, output, "paired", files, f" worker_for={df} worker_image={dw}")
    statuses.append((status, serial))
    details = " ".join(" ".join(" ".join(p).split()) for p in pages)
    joined = "".join("".join("".join(p).split()) for p in pages)
    predicate("L1 paired", names(listed) == ["front-program"] and LEFT_OUT in serial and "front-program" in details and "its-worker" in details and dw[:16] in joined,
              "listed %r, said %r, details names worker %r, shows its digest %r" % (names(listed), LEFT_OUT in serial, "its-worker" in details, dw[:16] in joined))
    for tag, extra, name in (("not-paired", "", "L2 not paired"),
                             ("nothing-to-be-beside", f" worker_for={'0' * 64} worker_image={dw}", "L3 nothing to be beside"),
                             ("cut-line", f" worker_for={df} worker_image={dw} padding=" + "x" * 1200, "L4 cut line")):
        listed, pages, serial, status = boot(kernel, output, tag, files, extra)
        statuses.append((status, serial))
        predicate(name, names(listed) == ["front-program", "its-worker"] and not any(line.startswith("programs: ") and "not listed" in line for line in serial),
                  "listed %r" % names(listed))
    document = b"a document the loader handed over, which is not a program\n"
    dd = hashlib.sha256(document).hexdigest()
    listed, pages, serial, status = boot(kernel, output, "for-a-document", [("front-program", document), ("its-worker", worker)],
                                         f" worker_for={dd} worker_image={dw}", shown="its-worker")
    statuses.append((status, serial))
    predicate("L6 not a program", names(listed) == ["its-worker"] and not any(line.startswith("programs: ") and "not listed" in line for line in serial),
              "listed %r" % names(listed))
    predicate("L5 every boot", all(status == 33 and not any(line.startswith("  FAIL") for line in serial) for status, serial in statuses),
              "statuses %r" % [s for s, _ in statuses])
    if failed:
        sys.exit("failed: " + ", ".join(failed))
    print("all six hold")


if __name__ == "__main__":
    main()
