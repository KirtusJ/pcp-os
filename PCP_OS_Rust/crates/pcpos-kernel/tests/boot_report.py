"""Boot an existing kernel and check its two output channels. Requires QEMU and cbor2."""
import argparse
import io
import json
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory
import sys

import cbor2


def read_messages(mode, wire):
    if mode == "json":
        assert wire.endswith(b"\n") and b"\r" not in wire
        return [json.loads(line) for line in wire.splitlines()]
    stream = io.BytesIO(wire)
    messages = []
    while stream.tell() < len(wire):
        raw = cbor2.load(stream)
        messages.append({
            "event" if key == 70 else key: value.hex() if isinstance(value, bytes) else value
            for key, value in raw.items()
        })
        for key in ("head", "image"):
            if key in raw:
                assert isinstance(raw[key], bytes) and len(raw[key]) == 32
    return messages


def boot(kernel, qemu, directory, case, options="", second=True, expected=33, initrd=None):
    console = directory / (case + ".log")
    report = directory / (case + ".wire")
    args = [qemu, "-machine", "pc,accel=tcg", "-icount", "shift=2,align=off,sleep=off", "-cpu", "max", "-m", "128M",
            "-display", "none", "-nic", "none", "-monitor", "none", "-no-reboot",
            "-serial", "file:" + str(console), "-kernel", str(kernel),
            "-append", ("then=off " + options).rstrip(),
            "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
    if second:
        args += ["-serial", "file:" + str(report)]
    if initrd:
        args += ["-initrd", str(initrd)]
    result = subprocess.run(args, capture_output=True, timeout=45)
    if result.returncode != expected:
        failures = [line for line in console.read_bytes().splitlines() if line.startswith(b"  FAIL")]
        raise AssertionError((case, result.returncode, result.stderr, failures))
    return console.read_bytes(), report.read_bytes() if second else b""


def validate(mode, wire, console, match, safe=False, session_error=False, brief=False):
    booted, session, stop = read_messages(mode, wire)
    assert [v["event"] for v in (booted, session, stop)] == ["boot", "session", "stop"]
    assert all(type(v["v"]) is int and v["v"] == 1 for v in (booted, session, stop))
    image = re.search(rb"running image .*?SHA-256: ([0-9a-f]{64})", console)[1].decode()
    footer = re.search(rb"kernel register: (\d+) events, head ([0-9a-f]{64})", console)
    passes = sum(line.startswith(b"  pass ") for line in console.splitlines())
    if brief:
        assert passes == 0
        totals = re.search(rb"self-test totals: (\d+) passed, (\d+) failures", console)
        assert totals and int(totals[2]) == int(safe or session_error)
        passes = int(totals[1])
    assert booted["image"] == image and booted["match"] is match
    assert session["admit"] is (not safe) and session["system"] == ("SAFE" if safe else "OPERATIONAL")
    assert session["fail"] == int(safe) and session["pass"] == passes - 1
    assert stop["fail"] == int(safe or session_error) and stop["pass"] == passes
    assert stop["head"] == footer[2].decode() and stop["events"] == int(footer[1])
    assert stop["replay"] is True
    return image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    kernel = args.kernel.resolve()
    with TemporaryDirectory() as tmp:
        directory = Path(tmp)
        for case, option in (("default", ""), ("unknown", "report=other"), ("substring", "xreport=json")):
            console, wire = boot(kernel, args.qemu, directory, case, option)
            assert wire == b"" and b"ALL SELF-TESTS PASS" in console
        measured = subprocess.check_output(
            [sys.executable, str(Path(__file__).resolve().parents[1] / "tools/measure.py"), str(kernel)],
            text=True).strip()
        for mode in ("json", "cbor"):
            def run(case, opts="", expected=33):
                return boot(kernel, args.qemu, directory, mode + case,
                            "report=" + mode + " " + opts, expected=expected)
            console, wire = run("regular")
            image = validate(mode, wire, console, None)
            assert image == measured
            console, wire = run("match", "image=" + measured)
            validate(mode, wire, console, True)
            console, wire = run("mismatch", "image=" + "0" * 64, 35)
            validate(mode, wire, console, False, safe=True)
            console, wire = run("missing-program", "auto=missing", 35)
            validate(mode, wire, console, None, session_error=True)
            console, wire = run("brief", "checks=brief")
            validate(mode, wire, console, None, brief=True)
            console, wire = run("brief-mismatch", "checks=brief image=" + "0" * 64, 35)
            validate(mode, wire, console, False, safe=True, brief=True)
            assert b"  FAIL the running image measures as the value the loader named" in console
        console, _ = boot(kernel, args.qemu, directory, "no-port", "report=json", second=False)
        assert b"COM2 did not respond" in console and b"ALL SELF-TESTS PASS" in console
    print("16 boot-report cases passed")


if __name__ == "__main__":
    main()
