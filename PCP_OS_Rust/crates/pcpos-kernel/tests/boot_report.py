"""Boot an existing kernel and check its two output channels. Requires QEMU and cbor2."""
import argparse
import io
import json
import os
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


def expected_admission(match, safe):
    fields = {"admit": not safe, "image": {True: "matched", False: "mismatch", None: "unchecked"}[match]}
    if safe:
        fields.update(fail=1, system="SAFE")
    return fields


def validate_host(host, mode, wire, match, safe=False, session_error=False):
    """Compare both summaries with the independently checked boot outcome."""
    failed = safe or session_error
    admission = expected_admission(match, safe)
    health = {"boot": "failed" if failed else "ok", "image": admission["image"]}
    if failed:
        health.update(admit=not safe, fail=1, replay=True, system="SAFE" if safe else "OPERATIONAL")
    for view, expected, code in (("--admission", admission, int(safe)), ("--health", health, int(failed))):
        result = subprocess.run([host, "boot-report", view, mode], input=wire, capture_output=True, timeout=10)
        assert result.returncode == code and json.loads(result.stdout) == expected, result


def live_admission(kernel, qemu, directory, host, mode, safe):
    """Connect the receiver directly to COM2, then ask the interactive kernel to stop."""
    case = mode + ("-held" if safe else "-admitted")
    wire = directory / (case + ".wire")
    console = directory / (case + ".log")
    monitor = directory / (case + "-monitor")
    monitor_input = Path(str(monitor) + ".in")
    os.mkfifo(monitor_input)
    os.mkfifo(str(monitor) + ".out")
    options = "checks=brief report=" + mode + (" image=" + "0" * 64 if safe else "")
    args = [qemu, "-machine", "pc,accel=tcg", "-icount", "shift=2,align=off,sleep=off",
            "-cpu", "max", "-m", "128M", "-display", "none", "-nic", "none", "-no-reboot",
            "-kernel", str(kernel), "-append", options, "-serial", "file:" + str(console),
            "-chardev", f"stdio,id=com2,signal=off,logfile={wire}", "-serial", "chardev:com2",
            "-monitor", f"pipe:{monitor}",
            "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
    with subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE) as guest:
        try:
            result = subprocess.run([host, "boot-report", "--admission", mode], stdin=guest.stdout,
                                    capture_output=True, timeout=45)
            want = expected_admission(False if safe else None, safe)
            assert result.returncode == int(safe) and json.loads(result.stdout) == want, result
            assert guest.poll() is None
            prefix = read_messages(mode, wire.read_bytes())
            assert [r["event"] for r in prefix] == ["boot", "session"]
            assert prefix[1]["admit"] is (not safe)
            # The admission record precedes menu initialization. A key sent during
            # drawing may be discarded, so retry until the normal shutdown finishes.
            with monitor_input.open("wb", buffering=0) as control:
                for _ in range(20):
                    control.write(b"sendkey p\n")
                    try:
                        guest.wait(timeout=0.5)
                        break
                    except subprocess.TimeoutExpired:
                        pass
            assert guest.poll() == (35 if safe else 33), (case, guest.poll())
            raw = wire.read_bytes()
            validate(mode, raw, console.read_bytes(), False if safe else None, safe=safe, brief=True)
            validate_host(host, mode, raw, False if safe else None, safe=safe)
        finally:
            if guest.poll() is None:
                guest.kill()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--host", type=Path, help="also verify each report with this pcpos executable")
    args = parser.parse_args()
    kernel = args.kernel.resolve()
    host = args.host.resolve() if args.host else None
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
            def check(wire, console, match, safe=False, session_error=False, brief=False):
                image = validate(mode, wire, console, match, safe, session_error, brief)
                if host:
                    validate_host(host, mode, wire, match, safe, session_error)
                return image

            console, wire = run("regular")
            image = check(wire, console, None)
            assert image == measured
            console, wire = run("match", "image=" + measured)
            check(wire, console, True)
            console, wire = run("mismatch", "image=" + "0" * 64, 35)
            check(wire, console, False, safe=True)
            console, wire = run("missing-program", "auto=missing", 35)
            check(wire, console, None, session_error=True)
            console, wire = run("brief", "checks=brief")
            check(wire, console, None, brief=True)
            console, wire = run("brief-mismatch", "checks=brief image=" + "0" * 64, 35)
            check(wire, console, False, safe=True, brief=True)
            assert b"  FAIL the running image measures as the value the loader named" in console
        console, _ = boot(kernel, args.qemu, directory, "no-port", "report=json", second=False)
        assert b"COM2 did not respond" in console and b"ALL SELF-TESTS PASS" in console
        if host:
            for mode in ("json", "cbor"):
                for safe in (False, True):
                    live_admission(kernel, args.qemu, directory, host, mode, safe)
    print("16 completed boot-report cases passed" + ("; 4 live admission cases passed" if host else ""))


if __name__ == "__main__":
    main()
