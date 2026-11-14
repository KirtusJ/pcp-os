"""Boot the kernel with distinct COM2 IDs and exercise the cbor3 receiver in QEMU."""
import argparse
from io import BytesIO
from itertools import product
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

import cbor2

from boot_report import boot, expected_admission, read_messages, validate, validate_host
from cross_boot_capture import BANNER_A, BANNER_B, measure, raw_records

# Distinct, deterministic fixture IDs. A supervisor supplies fresh IDs in use.
FAIL_ID = "00112233445566778899aabbccddeeff"
OK_ID = "ffeeddccbbaa99887766554433221100"
PIN_ID = "0123456789abcdef0123456789abcdef"
HELD_ID = "cafebabefeedface1122334455667788"
LIVE_ID = "9412a3bbccaabbcc0011223344556677"
LIMIT_ID = "a113d034442233665511779900eeccee"


def inner(record, boot_id):
    """Check each canonical envelope, then keep the original inner bytes."""
    prefix = b"\x83\x03\x50" + bytes.fromhex(boot_id)
    assert record.startswith(prefix)
    envelope = cbor2.loads(record)
    assert envelope[:2] == [3, bytes.fromhex(boot_id)] and len(envelope) == 3
    assert record == cbor2.dumps(envelope, canonical=True)
    return record[len(prefix):]


def check(host, wire, boot_id, code, expected, view="--health"):
    result = subprocess.run([host, "boot-report", view, "cbor3", boot_id],
                            input=wire, capture_output=True, timeout=10)
    assert (result.returncode, json.loads(result.stdout)) == (code, expected), result


def live(kernel, qemu, directory, host):
    """Require a verdict from an open guest stream before stopping the producer."""
    report = directory / "live.wire"
    cmd = [qemu, "-machine", "pc,accel=tcg", "-icount", "shift=2,align=off,sleep=off",
           "-cpu", "max", "-m", "128M", "-display", "none", "-nic", "none", "-monitor", "none",
           "-no-reboot", "-kernel", kernel, "-append", "report=cbor3 boot_id=" + LIVE_ID,
           "-serial", "file:" + str(directory / "live.log"), "-chardev",
           f"stdio,id=com2,signal=off,logfile={report}", "-serial", "chardev:com2"]
    with subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE) as guest:
        try:
            result = subprocess.run([host, "boot-report", "--admission", "cbor3", LIVE_ID],
                                    stdin=guest.stdout, capture_output=True, timeout=45)
            assert guest.poll() is None
            assert result.returncode == 0 and json.loads(result.stdout) == expected_admission(None, False), result
            raw = report.read_bytes()
            # Exactly two complete envelopes, with the guest still waiting for input.
            stream = BytesIO(raw)
            cbor2.load(stream)
            boundary = stream.tell()
            cbor2.load(stream)
            assert stream.tell() == len(raw)
            rows = read_messages("cbor2", inner(raw[:boundary], LIVE_ID) + inner(raw[boundary:], LIVE_ID))
            assert [r["event"] for r in rows] == ["boot", "session"] and rows[1]["admit"] is True
        finally:
            guest.kill()
            guest.wait(timeout=10)


def exercise(kernel, qemu, directory, host):
    original = kernel.read_bytes()
    assert original.count(BANNER_A) == 1
    modified = original.replace(BANNER_A, BANNER_B, 1)
    digest_a, digest_b = measure(original), measure(modified)
    assert digest_a != digest_b
    variant = directory / "variant.elf32"
    variant.write_bytes(modified)

    # Keep the actual QEMU kernel filenames short. Multiboot includes the filename
    # in its command line and some tests deliberately check the 255-byte boundary.
    with TemporaryDirectory(dir="/tmp", prefix="pcpb-") as tmp:
        a, b = (Path(tmp) / name for name in ("a", "b"))
        a.symlink_to(kernel)
        b.symlink_to(variant)
        pin = "image=" + digest_a

        def capture(name, image, boot_id, options="", match=None, held=False, late=False):
            console, wire = boot(image, qemu, directory, name,
                                 f"report=cbor3 boot_id={boot_id} {options}".rstrip(),
                                 expected=35 if held or late else 33)
            records = raw_records("cbor3", wire)
            positional = b"".join(inner(record, boot_id) for record in records)
            observed = validate("cbor2", positional, console, match, safe=held, session_error=late)
            assert observed == (digest_a if image == a else digest_b)
            assert (BANNER_A if image == a else BANNER_B) in console.splitlines()[0]
            if late:
                assert b"auto=missing: the loader handed over no program of that name" in console
            admission = expected_admission(match, held)
            health = {"boot": "failed" if held or late else "ok", "image": admission["image"]}
            if held or late:
                health.update(admit=not held, replay=True, fail=1, system="SAFE" if held else "OPERATIONAL")
            check(host, b"".join(records[:2]), boot_id, int(held), admission, "--admission")
            check(host, wire, boot_id, int(held or late), health)
            return records

        failed = capture("matched-failed", a, FAIL_ID, pin + " auto=missing", match=True, late=True)
        healthy = capture("unchecked-healthy", b, OK_ID)
        capture("mismatch-held", b, HELD_ID, pin, match=False, held=True)
        genuine = capture("matched-healthy", a, PIN_ID, pin, match=True)

        for ids in product((0, 1), repeat=3):
            if len(set(ids)) == 1:
                continue
            mixed = b"".join((failed, healthy)[item][stage] for stage, item in enumerate(ids))
            (directory / ("mixed-" + "".join(map(str, ids)) + ".wire")).write_bytes(mixed)
            for expected_id in (FAIL_ID, OK_ID):
                check(host, mixed, expected_id, 6, {"boot": "unknown"})
        check(host, failed[0] + healthy[1], FAIL_ID, 6, {"admit": None}, "--admission")
        check(host, b"".join(genuine), OK_ID, 6, {"boot": "unknown"})

        console, old = boot(a, qemu, directory, "legacy", "report=cbor2 " + pin)
        assert validate("cbor2", old, console, True) == digest_a
        validate_host(host, "cbor2", old, True)
        check(host, old, PIN_ID, 6, {"boot": "unknown"})
        assert sum(map(len, genuine)) == len(old) + 57
        print(f"Healthy matched capture: {len(old)} bytes in cbor2, {sum(map(len, genuine))} in cbor3")

        for case, options in (("missing", ""), ("invalid", "boot_id=" + FAIL_ID.upper()),
                              ("duplicate", f"boot_id={FAIL_ID} boot_id={FAIL_ID}")):
            console, raw = boot(a, qemu, directory, case, "report=cbor3 console=quiet " + options)
            assert raw == b"" and b"cbor3 requires exactly one boot_id" in console
            assert b"ALL SELF-TESTS PASS" in console

        # Compare what this QEMU loader actually supplied, including its pathname.
        legacy_prefix = next(line for line in console_path(directory, "legacy")
                             if line.startswith(b"command line: "))
        loader = legacy_prefix.removeprefix(b"command line: ").removesuffix(("report=cbor2 " + pin).encode())
        assert loader.endswith(b"then=off ")
        start = "report=cbor3 boot_id=" + LIMIT_ID + " pad="
        pad = 255 - len(loader) - len(start)
        assert pad > 0
        capture("exact-limit", a, LIMIT_ID, "pad=" + "x" * pad)
        visible = next(line for line in console_path(directory, "exact-limit")
                       if line.startswith(b"command line: ")).removeprefix(b"command line: ")
        assert visible == loader + start.encode() + b"x" * pad and len(visible) == 255
        console, raw = boot(a, qemu, directory, "hidden-duplicate",
                            start + "x" * pad + " boot_id=" + FAIL_ID)
        assert raw == b"" and b"incomplete cbor3 command line" in console
        assert b"ALL SELF-TESTS PASS" in console
        live(a, qemu, directory, host)
    print("10 completed boots and 1 live admission passed, including six cross-boot mixtures")


def console_path(directory, name):
    return (directory / (name + ".log")).read_bytes().splitlines()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--out", type=Path, help="new directory for original logs, wire captures and mixtures")
    args = parser.parse_args()
    if not __debug__:
        parser.error("run without Python optimization so assertions are checked")
    if args.out is None:
        with TemporaryDirectory(prefix="pcpos-bound-report-") as tmp:
            exercise(args.kernel.resolve(), args.qemu, Path(tmp), args.host.resolve())
    else:
        directory = args.out.resolve()
        directory.mkdir(parents=True, exist_ok=False)
        exercise(args.kernel.resolve(), args.qemu, directory, args.host.resolve())


if __name__ == "__main__":
    main()
