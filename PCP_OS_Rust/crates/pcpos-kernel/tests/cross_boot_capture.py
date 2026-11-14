"""Reproduce the COM2 capture-origin limitation in issue #64 using two real guest images.

Run with a built kernel, --host PATH_TO_PCPOS and --out NEW_DIRECTORY. Each
QEMU process writes its own COM1 log and COM2 capture. Reassembly occurs only
after the guest exits. This experiment describes the receiver at the tested
revision; it is not a compatibility requirement for future receivers.
"""
import argparse
import io
import json
from pathlib import Path
import subprocess
import sys

import cbor2
from boot_report import REPORT_MODES, boot, validate

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from measure import measure


BANNER_A = b"new kernel, written in Rust | early"
BANNER_B = b"new kernel, written in Rust | Early"


def raw_records(mode, wire):
    """Find each record boundary without changing the bytes on either side."""
    if mode == "json":
        records = wire.splitlines(keepends=True)
    else:
        records = []
        stream = io.BytesIO(wire)
        while stream.tell() < len(wire):
            start = stream.tell()
            cbor2.load(stream)
            records.append(wire[start:stream.tell()])
    assert len(records) == 3 and b"".join(records) == wire
    return records


def check_receiver(host, directory, mode, case, wire, code, expected, view="--health"):
    (directory / (case + ".wire")).write_bytes(wire)
    result = subprocess.run([host, "boot-report", view, mode], input=wire,
                            capture_output=True, timeout=10)
    want = (json.dumps(expected, sort_keys=True, separators=(",", ":")) + "\n").encode()
    assert (result.returncode, result.stdout) == (code, want), (mode, case, result)
    return result.stdout


def exercise(mode, directory, kernel_a, kernel_b, digest_a, digest_b, host, qemu):
    directory.mkdir()
    pin = "image=" + digest_a

    def capture(name, kernel, options="", match=None, held=False, late_failure=False):
        console, wire = boot(kernel, qemu, directory, name, f"report={mode} {options}".strip(),
                             expected=35 if held or late_failure else 33)
        digest = validate(mode, wire, console, match, safe=held, session_error=late_failure)
        assert digest == (digest_a if kernel == kernel_a else digest_b)
        assert (BANNER_A if kernel == kernel_a else BANNER_B) in console.splitlines()[0]
        if late_failure:
            assert b"auto=missing: the loader handed over no program of that name" in console
        return wire

    failed = capture("matched-failed", kernel_a, pin + " auto=missing", match=True, late_failure=True)
    healthy = capture("unchecked-healthy", kernel_b)
    held = capture("mismatch-held", kernel_b, pin, match=False, held=True)
    positive = capture("matched-healthy", kernel_a, pin, match=True)

    def check(name, data, code, **expected):
        return check_receiver(host, directory, mode, name, data, code, expected)

    failure = dict(boot="failed", replay=True, fail=1)
    check("original-failed", failed, 1, image="matched", admit=True, system="OPERATIONAL", **failure)
    check("original-unchecked", healthy, 0, boot="ok", image="unchecked")
    check("original-held", held, 1, image="mismatch", admit=False, system="SAFE", **failure)
    good = check("original-matched", positive, 0, boot="ok", image="matched")

    fail_boot, fail_session, fail_stop = raw_records(mode, failed)
    ok_boot, ok_session, ok_stop = raw_records(mode, healthy)
    held_boot, _, _ = raw_records(mode, held)
    assert fail_boot != ok_boot and fail_stop != ok_stop
    splice = fail_boot + ok_session + ok_stop
    assert splice not in (failed, healthy, positive)
    assert check("splice", splice, 0, boot="ok", image="matched") == good
    check("reverse", ok_boot + fail_session + fail_stop, 1, image="unchecked",
          admit=True, system="OPERATIONAL", **failure)

    for name, wire in (("mismatch-admitted", held_boot + ok_session + ok_stop),
                       ("concatenated", failed + healthy), ("truncated", splice[:-1])):
        check(name, wire, 6, boot="unknown")
    check_receiver(host, directory, mode, "splice-admission", fail_boot + ok_session,
                   0, {"admit": True, "image": "matched"}, "--admission")
    print(f"{mode}: four boots; genuine and spliced health both {good.decode().strip()}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True, help="new directory for images and captures")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    if not __debug__:
        parser.error("run without Python optimization so all assertions are checked")
    directory = args.out.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    original = args.kernel.resolve()
    source = original.read_bytes()
    if source.count(BANNER_A) != 1:
        parser.error("expected one copy of the startup banner in the supplied kernel")
    variant = directory / "variant.elf32"
    image = source.replace(BANNER_A, BANNER_B, 1)
    variant.write_bytes(image)
    digest_a, digest_b = measure(source), measure(image)
    assert digest_a != digest_b
    print(f"Measured original {digest_a}\nMeasured variant  {digest_b}")
    for mode in REPORT_MODES:
        exercise(mode, directory / mode, original, variant, digest_a, digest_b,
                 args.host.resolve(), args.qemu)


if __name__ == "__main__":
    main()
