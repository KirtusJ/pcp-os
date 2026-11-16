#!/usr/bin/env python3
"""Test-only native PCP OS register evidence, NOT a production ledger export.

The diagnostic build prints an immutable view's original JSONL bytes (hex) before
drawing its register screen.  This runner reconstructs and checks the raw event
chain independently of the kernel's reported head, then compares the resulting
last-event digest to the actual QEMU P6 pixels using the pinned kernel glyphs.
The negative boot mutates a COPY in the guest and must render a refusal.

No filesystem, installed model, persistent supervisor or real user task state
is read. Only run on the controlled synthetic, disposable QEMU test image.
"""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import time

HERE = Path(__file__).resolve()
sys.path.insert(0, str(HERE.parents[1]))
from check_display import Monitor, wait_serial  # Native QEMU QMP transport only.

GROUND = bytes.fromhex("10283f")
TEAL = bytes.fromhex("41bad7")
BAD = bytes.fromhex("f08a72")
TEXT = bytes.fromhex("f2f7fc")
SOFT = bytes.fromhex("9fb4c8")
PANEL = bytes.fromhex("18364f")
FIELDS = {"actor", "auth", "body", "kind", "prev", "seq", "ts"}
HEAD_RX = re.compile(
    rb"D10_REGISTER_RAW_BEGIN bytes=([0-9]+)\r?\n"
    rb"([0-9a-f]+)\r?\n"
    rb"D10_REGISTER_RAW_END bytes=([0-9]+)(?:\r?\n)"
)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def no_duplicate_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def refuse_float(_):
    raise ValueError("a ledger value is not a signed integer")


def require_rust_value(value, depth=0):
    """Reject structures Python parses but Rust's canonical Value rejects."""
    if depth > 32:
        raise ValueError("event nesting exceeds Rust MAX_DEPTH")
    if value is None or type(value) is bool:
        return
    if type(value) is int:
        if not -(1 << 63) <= value < (1 << 63):
            raise ValueError("event integer exceeds Rust signed i64")
        return
    if type(value) is str:
        if any(0xd800 <= ord(char) <= 0xdfff for char in value):
            raise ValueError("unpaired UTF-16 surrogate accepted by Python JSON")
        return
    if type(value) is list:
        for item in value:
            require_rust_value(item, depth + 1)
        return
    if type(value) is dict:
        for key, item in value.items():
            require_rust_value(key, depth + 1)
            require_rust_value(item, depth + 1)
        return
    raise ValueError("unsupported Rust record value")


def read_register(raw):
    """Separate Python implementation of canonical event and chain verification."""
    if not raw or not raw.endswith(b"\n"):
        raise ValueError("empty/incomplete original JSONL")
    prev = "0" * 64
    count = 0
    for line in raw[:-1].split(b"\n"):
        if len(line) > 65536:
            raise ValueError("oversized JSONL event")
        obj = json.loads(
            line.decode("ascii"),
            object_pairs_hook=no_duplicate_pairs,
            parse_float=refuse_float,
            parse_constant=refuse_float,
        )
        require_rust_value(obj)
        canonical = json.dumps(
            obj, sort_keys=True, ensure_ascii=True, separators=(",", ":")
        ).encode("ascii")
        if line != canonical:
            raise ValueError("noncanonical JSONL event")
        if not isinstance(obj, dict) or set(obj) != FIELDS:
            raise ValueError("unexpected event fields")
        if not isinstance(obj["seq"], int) or isinstance(obj["seq"], bool):
            raise ValueError("noninteger sequence")
        if obj["seq"] != count or obj["prev"] != prev:
            raise ValueError("broken event predecessor/sequence")
        if not isinstance(obj["ts"], str) or not re.fullmatch(
            r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z",
            obj["ts"],
        ):
            raise ValueError("invalid timestamp shape")
        if obj["auth"] not in ("HOST", "DECLARED"):
            raise ValueError("unrecognized authority")
        if not isinstance(obj["actor"], str) or not isinstance(obj["kind"], str):
            raise ValueError("invalid actor or event kind")
        if not isinstance(obj["body"], dict):
            raise ValueError("nonobject event body")
        prev = sha(line)  # Head is the last CANONICAL EVENT LINE hash.
        count += 1
    return count, prev


def expect_failure(raw):
    try:
        read_register(raw)
    except (ValueError, UnicodeError, json.JSONDecodeError):
        return True
    return False


def parse_snapshot(serial):
    matches = list(HEAD_RX.finditer(serial))
    if len(matches) != 1:
        raise ValueError(f"expected exactly one complete raw-register frame, found {len(matches)}")
    length_a, hexbytes, length_b = matches[0].groups()
    if length_a != length_b:
        raise ValueError("serial begin/end register byte counts differ")
    raw = bytes.fromhex(hexbytes.decode("ascii"))
    if len(raw) != int(length_a):
        raise ValueError("serial declared register length differs")
    return raw


def font_from_rust(path):
    source = path.read_text()
    table = re.search(r"pub static GLYPHS:[^=]+=\s*(\[.*?\]);", source, re.DOTALL)
    if not table:
        raise ValueError("pinned kernel font table missing")
    cells = ast.literal_eval(table[1])
    if len(cells) != 95 or any(len(cell) != 200 for cell in cells):
        raise ValueError("pinned kernel glyph extents differ")
    return cells


def read_p6(path):
    with path.open("rb") as stream:
        if stream.readline() != b"P6\n":
            raise ValueError("not an unmodified binary P6 QMP frame")
        size = stream.readline().split()
        if len(size) != 2:
            raise ValueError("malformed P6 extent")
        width, height = map(int, size)
        if stream.readline() != b"255\n":
            raise ValueError("unexpected P6 colour maximum")
        pixels = stream.read()
    if width <= 0 or height <= 0 or len(pixels) != 3 * width * height:
        raise ValueError("short or oversized native QMP scanout")
    return (width, height), pixels


def exact_glyphs(frame, pos, phrase, cells, foreground, ground=GROUND):
    (width, height), pixels = frame
    x, y = pos
    if x < 0 or y < 0 or x + len(phrase) * 10 > width or y + 20 > height:
        return False
    for row in range(20):
        expected = bytearray()
        for character in phrase:
            idx = ord(character) - 32
            if idx < 0 or idx >= 95:
                raise ValueError("glyph is outside the real kernel font")
            for coverage in cells[idx][row * 10:row * 10 + 10]:
                expected.extend(
                    (bg * (255 - coverage) + fg * coverage + 127) // 255
                    for bg, fg in zip(ground, foreground)
                )
        observed = pixels[3 * ((y + row) * width + x):
                          3 * ((y + row) * width + x + 10 * len(phrase))]
        if observed != expected:
            return False
    return True


def launcher_ready(frame, cells, width, height):
    return frame[0] == (width, height) and (
        exact_glyphs(frame, (190, 40), "PCP OS", cells, TEXT)
        and exact_glyphs(frame, (20, (height // 20 - 2) * 20), "Up/Down choose", cells, TEXT, PANEL)
        and exact_glyphs(frame, (20, (height // 20 - 1) * 20), "Ctrl+Alt+Delete", cells, SOFT, PANEL)
    )


def screenshot(monitor, root, name):
    path = root / (name + ".ppm")
    monitor.execute("screendump", filename=str(path))
    return path, read_p6(path)


def wait_image(monitor, root, name, predicate, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        path, frame = screenshot(monitor, root, name)
        if predicate(frame):
            return path, frame
        if monitor.guest.poll() is not None:
            raise RuntimeError("native guest exited before required pixels")
        time.sleep(.25)
    raise TimeoutError(f"{name}: required native QMP glyphs not observed")


def one_boot(kernel, measurement, font, root, width, height, vram, mode, timeout):
    root.mkdir(parents=True, exist_ok=False)
    serial = root / "serial.log"
    report = {
        "mode": mode,
        "resolution": [width, height],
        "vram_mib": vram,
        "kernel_sha256": sha(kernel.read_bytes()),
        "kernel_measurement": measurement,
        "frames": {},
        "source": "QEMU QMP P6 plus feature-gated native-guest serial bytes",
        "same_view_raw_ledger": False,
        "passed": False,
    }
    command = [
        "qemu-system-x86_64", "-machine", "q35", "-accel", "tcg",
        "-icount", "shift=3", "-cpu", "max", "-m", "128M",
        "-nic", "none", "-display", "none", "-vga", "none",
        "-device", f"VGA,vgamem_mb={vram}",
        "-monitor", "none", "-qmp", "stdio",
        "-serial", "file:" + str(serial), "-no-reboot",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-kernel", str(kernel),
        "-append", f"image={measurement} test={mode}",
    ]
    report["qemu_args"] = command
    guest = None
    monitor = None
    try:
        with (root / "qemu.stderr").open("wb") as stderr:
            guest = subprocess.Popen(command, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=stderr)
            monitor = Monitor(guest, timeout)
            if "QMP" not in monitor.receive(time.monotonic() + timeout):
                raise RuntimeError("native QMP greeting missing")
            monitor.execute("qmp_capabilities")
            wait_serial(guest, serial, f"display: Bochs, {width}x{height}".encode(), timeout)
            launcher, original = wait_image(
                monitor, root, "launcher",
                lambda frame: launcher_ready(frame, font, width, height), timeout)
            report["frames"]["launcher"] = sha(launcher.read_bytes())
            monitor.key("r")
            wait_serial(guest, serial, b"D10_REGISTER_RAW_END bytes=", timeout)
            marker = {
                "register-corrupt": b"D10_REGISTER_INJECT offset=0",
                "register-chain-break": b"D10_REGISTER_INJECT_CHAIN offset=",
                "register-tail-change": b"D10_REGISTER_INJECT_TAIL offset=",
                "register-snapshot": b"D10_REGISTER_UNMODIFIED",
            }[mode]
            wait_serial(guest, serial, marker, timeout)
            live_serial = serial.read_bytes()
            raw = parse_snapshot(live_serial)
            count, head = read_register(raw)
            if count < 1 or len(head) != 64:
                raise ValueError("impossible native ledger count or head")
            report["original_raw_sha256"] = sha(raw)
            report["original_raw_length"] = len(raw)
            report["verified_event_count"] = count
            report["independent_last_event_sha256"] = head
            report["raw_ledger_filename"] = "view-original.jsonl"
            (root / "view-original.jsonl").write_bytes(raw)
            # A cleanly framed raw snapshot alone is NOT enough.  Tamper controls
            # must fail the separate parser before any pixel acceptance.
            damaged = bytearray(raw)
            damaged[0] ^= 1
            if not expect_failure(bytes(damaged)) or not expect_failure(raw[:-1]):
                raise AssertionError("independent parser accepted a broken negative control")
            # These are independent grammar discriminators, not broken chains.
            # Python json.loads accepts all three; Rust Value::parse refuses.
            fake = {
                "actor": "diagnostic", "auth": "HOST", "body": {},
                "kind": "NOTE", "prev": "0" * 64, "seq": 0,
                "ts": "2026-11-12T00:00:00Z",
            }
            def encode_one(obj):
                return json.dumps(
                    obj, sort_keys=True, ensure_ascii=True,
                    separators=(",", ":")
                ).encode("ascii") + b"\n"
            fake["actor"] = "\ud800"
            if not expect_failure(encode_one(fake)):
                raise AssertionError("Python accepted Rust-refused unpaired surrogate")
            fake["actor"] = "diagnostic"
            fake["body"] = {"big": 1 << 63}
            if not expect_failure(encode_one(fake)):
                raise AssertionError("Python accepted Rust-refused integer overflow")
            deeply = {}
            for _ in range(33):
                deeply = [deeply]
            fake["body"] = {"deep": deeply}
            if not expect_failure(encode_one(fake)):
                raise AssertionError("Python accepted Rust-refused excessive nesting")
            report["tampered_first_byte_refused_by_python"] = True
            report["missing_final_newline_refused_by_python"] = True
            report["unpaired_surrogate_refused_by_python"] = True
            report["i64_overflow_refused_by_python"] = True
            report["excessive_nesting_refused_by_python"] = True
            # The following mutations are constructed by this independent
            # host-side parser, using only original serial-framed JSONL bytes.
            # They do not trust the guest's computed head as an oracle.
            modified = None
            if mode == "register-corrupt":
                modified = bytes(damaged)
                inject = re.findall(
                    rb"D10_REGISTER_INJECT offset=0 before=([0-9a-f]{2}) after=([0-9a-f]{2})",
                    live_serial,
                )
                if len(inject) != 1 or inject[0] != (
                    f"{raw[0]:02x}".encode(), f"{damaged[0]:02x}".encode()
                ):
                    raise ValueError("actual in-guest byte mutation not correlated")
            elif mode == "register-chain-break":
                first_end = raw.index(b"\n")
                label = b'"label":"kernel"'
                if raw[:first_end].count(label) != 1:
                    raise ValueError("synthetic first event has no unique kernel label")
                at = raw.index(label, 0, first_end) + len(b'"label":"k')
                if raw[at] != 0x65:
                    raise ValueError("kernel-label byte is not expected ASCII e")
                altered = bytearray(raw)
                altered[at] = 0x45
                modified = bytes(altered)
                marker_check = f"D10_REGISTER_INJECT_CHAIN offset={at} before=65 after=45".encode()
                if live_serial.count(marker_check) != 1:
                    raise ValueError("guest chain-change offset/bytes not correlated")
                # First line MUST still parse and be canonical: the refusal
                # is for the successor's unchanged predecessor, not syntax.
                first = modified[:first_end]
                event = json.loads(first.decode("ascii"), object_pairs_hook=no_duplicate_pairs)
                canonical = json.dumps(event, sort_keys=True, ensure_ascii=True,
                                       separators=(",", ":")).encode("ascii")
                if canonical != first or event.get("body", {}).get("label") != "kErnel":
                    raise ValueError("chain-break case lost canonical first event")
                if not expect_failure(modified):
                    raise ValueError("independent chain parser accepted broken predecessor")
                report["canonical_early_change_refused_by_python"] = True
            elif mode == "register-tail-change":
                last_start = raw[:-1].rfind(b"\n") + 1
                tail = raw[last_start:-1]
                needle = b'"outcome":"LOST"'
                if tail.count(needle) != 1:
                    raise ValueError("synthetic final event does not have one LOST outcome")
                at = last_start + tail.index(needle) + len(b'"outcome":"')
                if raw[at:at + 4] != b"LOST":
                    raise ValueError("terminal outcome is not expected LOST")
                altered = bytearray(raw)
                altered[at:at + 4] = b"DONE"
                modified = bytes(altered)
                marker_check = f"D10_REGISTER_INJECT_TAIL offset={at} before=LOST after=DONE".encode()
                if live_serial.count(marker_check) != 1:
                    raise ValueError("guest terminal change offset/bytes not correlated")
                mutant_count, mutant_head = read_register(modified)
                if mutant_count != count or mutant_head == head:
                    raise ValueError("terminal mutation did not produce new valid anchored head")
                report["valid_terminal_mutation_new_head"] = mutant_head
                report["prior_original_head_is_not_independent_external_commitment"] = True

            if modified is not None:
                report["modified_copy_length"] = len(modified)
                report["modified_copy_sha256"] = sha(modified)
                report["only_detached_copy_changed"] = True
                (root / "view-modified-copy.jsonl").write_bytes(modified)

            if mode in ("register-corrupt", "register-chain-break"):
                if mode == "register-corrupt" and not expect_failure(modified):
                    raise AssertionError("independent parser accepted malformed record")
                refusal = "THE KERNEL'S REGISTER: verification failed"
                observed, frame = wait_image(
                    monitor, root, "register",
                    lambda f: f[0] == (width, height) and exact_glyphs(
                        f, (20, 20), refusal, font, BAD
                    ), timeout,
                )
                report["expected_guest_error_heading"] = refusal
                report["refusal_observed_from_guest_pixels"] = True
                if mode == "register-chain-break":
                    error = "BrokenChain(2)"
                    if not exact_glyphs(frame, (20, 40), error, font, TEXT):
                        raise ValueError("native guest error is not BrokenChain(2)")
                    report["expected_guest_verifier_error"] = error
                else:
                    error = "Parse(1, UnexpectedByte(0))"
                    if not exact_glyphs(frame, (20, 40), error, font, TEXT):
                        raise ValueError("native guest error is not first-byte parse refusal")
                    report["expected_guest_verifier_error"] = error
                if exact_glyphs(
                    frame, (20, 20),
                    f"THE KERNEL'S REGISTER: {count} events, chain intact",
                    font, TEAL,
                ):
                    raise ValueError("invalid-ledger native screen falsely claims success")
            else:
                displayed_head = (
                    mutant_head if mode == "register-tail-change" else head
                )
                expected = "HEAD " + displayed_head
                observed, frame = wait_image(
                    monitor, root, "register",
                    lambda f: f[0] == (width, height) and exact_glyphs(
                        f, (20, 40), expected, font, TEAL
                    ), timeout,
                )
                if len(expected) != 69:
                    raise AssertionError("not a full SHA-256 display")
                report["same_view_raw_ledger"] = mode == "register-snapshot"
                report["full_head_pixels_match_independent_ledger"] = True
                report["displayed_head_from_independent_bytes"] = displayed_head
                wrong = displayed_head[:-1] + ("0" if displayed_head[-1] != "0" else "1")
                if exact_glyphs(frame, (20, 40), "HEAD " + wrong, font, TEAL):
                    raise AssertionError("wrong final hex digit falsely accepted")
                report["wrong_last_digit_rejected_by_raster"] = True
            report["frames"]["register"] = sha(observed.read_bytes())
            monitor.key("esc")
            returned, restored = wait_image(
                monitor, root, "returned",
                lambda f: f == original, timeout,
            )
            report["frames"]["returned"] = sha(returned.read_bytes())
            monitor.key("p")
            report["qemu_exit"] = guest.wait(timeout=timeout)
            finished = serial.read_bytes()
            report["serial_sha256"] = sha(finished)
            if (report["qemu_exit"] != 33
                    or b"ALL SELF-TESTS PASS" not in finished
                    or b"  FAIL" in finished
                    or (measurement + " (measured; equal").encode() not in finished):
                raise RuntimeError("native guest/measurement/shutdown acceptance failed")
            report["passed"] = True
    except Exception as exc:
        report["failure"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        (root / "result.json").write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n"
        )
        if monitor is not None:
            monitor.selector.close()
        if guest is not None:
            if guest.poll() is None:
                guest.kill()
            guest.wait()
            guest.stdin.close()
            guest.stdout.close()
    return report



def one_default_no_export_boot(kernel, measurement, font, root, timeout):
    """Challenge the ordinary no-feature guest with the diagnostic command line.

    This is an *actual QEMU negative control* on the default-feature binary,
    not proof of global noninterference or binary identity with the #233 image.
    """
    root.mkdir(parents=True, exist_ok=False)
    width, height, vram = 800, 600, 4
    serial = root / "serial.log"
    markers = (
        b"D10_REGISTER_RAW_BEGIN", b"D10_REGISTER_RAW_END",
        b"D10_REGISTER_UNMODIFIED", b"D10_REGISTER_INJECT",
    )
    report = {
        "kind": "no-feature guest with attempted diagnostic command",
        "kernel_sha256": sha(kernel.read_bytes()),
        "kernel_measurement": measurement,
        "resolution": [width, height],
        "command_line_challenge": "test=register-snapshot",
        "frames": {},
        "passed": False,
    }
    command = [
        "qemu-system-x86_64", "-machine", "q35", "-accel", "tcg",
        "-icount", "shift=3", "-cpu", "max", "-m", "128M",
        "-nic", "none", "-display", "none", "-vga", "none",
        "-device", "VGA,vgamem_mb=4", "-monitor", "none",
        "-qmp", "stdio", "-serial", "file:" + str(serial),
        "-no-reboot", "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-kernel", str(kernel),
        "-append", f"image={measurement} test=register-snapshot",
    ]
    report["qemu_args"] = command
    guest = None
    monitor = None
    try:
        with (root / "qemu.stderr").open("wb") as stderr:
            guest = subprocess.Popen(
                command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=stderr,
            )
            monitor = Monitor(guest, timeout)
            if "QMP" not in monitor.receive(time.monotonic() + timeout):
                raise RuntimeError("native QMP greeting absent")
            monitor.execute("qmp_capabilities")
            wait_serial(guest, serial, b"display: Bochs, 800x600", timeout)
            launcher, initial = wait_image(
                monitor, root, "launcher",
                lambda frame: launcher_ready(frame, font, width, height),
                timeout,
            )
            report["frames"]["launcher"] = sha(launcher.read_bytes())
            monitor.key("r")
            register, frame = wait_image(
                monitor, root, "register",
                lambda f: f[0] == (width, height)
                and exact_glyphs(f, (20, 40), "HEAD ", font, TEAL),
                timeout,
            )
            report["frames"]["register"] = sha(register.read_bytes())
            raw = serial.read_bytes()
            if b"test=register-snapshot" not in raw:
                raise AssertionError("native guest did not receive diagnostic challenge")
            if any(marker in raw for marker in markers):
                raise AssertionError("default guest emitted the forbidden diagnostic marker")
            report["no_serial_export_before_exit"] = True
            monitor.key("esc")
            returned, restored = wait_image(
                monitor, root, "returned", lambda f: f == initial, timeout,
            )
            report["frames"]["returned"] = sha(returned.read_bytes())
            monitor.key("p")
            report["qemu_exit"] = guest.wait(timeout=timeout)
            raw = serial.read_bytes()
            report["serial_sha256"] = sha(raw)
            if (report["qemu_exit"] != 33
                    or b"ALL SELF-TESTS PASS" not in raw or b"  FAIL" in raw
                    or (measurement + " (measured; equal").encode() not in raw):
                raise RuntimeError("ordinary guest shutdown / self-tests failed")
            if any(marker in raw for marker in markers):
                raise AssertionError("default guest emitted diagnostic material after exit")
            report["no_serial_export_after_exit"] = True
            report["passed"] = True
    except Exception as exc:
        report["failure"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        (root / "result.json").write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n"
        )
        if monitor is not None:
            monitor.selector.close()
        if guest is not None:
            if guest.poll() is None:
                guest.kill()
            guest.wait()
            guest.stdin.close()
            guest.stdout.close()
    return report


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--kernel", type=Path, required=True)
    p.add_argument("--measurement", required=True)
    p.add_argument("--font", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--timeout", type=int, default=210)
    p.add_argument("--default-no-export", action="store_true", help="test uninstrumented QEMU boot with diagnostic challenge")
    args = p.parse_args()
    kernel = args.kernel.resolve()
    font = font_from_rust(args.font)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    if args.default_no_export:
        result = one_default_no_export_boot(
            kernel, args.measurement, font, output / "ordinary-no-export", args.timeout,
        )
        print(json.dumps(result, sort_keys=True), flush=True)
        return 0
    observations = [
        (800, 600, 4, "register-snapshot"),
        (1024, 768, 16, "register-snapshot"),
        (800, 600, 4, "register-corrupt"),
        (800, 600, 4, "register-chain-break"),
        (1024, 768, 16, "register-tail-change"),
    ]
    failures = []
    for width, height, vram, mode in observations:
        path = output / f"{width}x{height}-{mode}"
        try:
            result = one_boot(
                kernel, args.measurement, font, path,
                width, height, vram, mode, args.timeout,
            )
            print(json.dumps(result, sort_keys=True), flush=True)
        except Exception as exc:
            failures.append(f"{width}x{height}-{mode}: {exc}")
            print(f"FAIL {failures[-1]}", flush=True)
    (output / "summary.json").write_text(json.dumps({
        "observations": len(observations), "failures": failures,
        "all_passed": not failures,
        "production_register_untouched": True,
        "independent_executable": False,
    }, indent=2, sort_keys=True) + "\n")
    return int(bool(failures))


if __name__ == "__main__":
    sys.exit(main())
