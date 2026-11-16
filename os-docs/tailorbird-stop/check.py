#!/usr/bin/env python3
"""Run the model-unavailable Tailorbird Stop regression on a supplied PCP OS release."""
import argparse
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time
from uuid import UUID, uuid4


SECTOR = 512
SLOT_SECTORS = 129
STORE_TYPE = UUID("659691f8-45ed-47f1-819c-21c44e90e08d").bytes_le
STAGED = b"TAILORBIRD MCP PROPOSAL STAGED"
SAVED = b"TAILORBIRD EDIT CONFIRMED"
RAN = b"TAILORBIRD APPLICATION READ"
ACCEPTED = b"TAILORBIRD STOP ACCEPTED"
STOPPED = b"TAILORBIRD STOP CONFIRMED"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def file_hash(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def read_at(stream, offset, size):
    stream.seek(offset)
    data = stream.read(size)
    require(len(data) == size, "short image read")
    return data


def prepare_disk(factory, target, guid):
    # Construct a nonbootable storage fixture. The supplied image remains read-only.
    with factory.open("rb") as source, target.open("xb") as disk:
        size = factory.stat().st_size
        header = read_at(source, SECTOR, SECTOR)
        require(header[:8] == b"EFI PART" and header[56:72] == guid.bytes_le, "factory GUID mismatch")
        table_lba, entries, width = struct.unpack_from("<QII", header, 72)
        require((table_lba, entries, width) == (2, 128, 128), "unsupported preview GPT layout")
        table = read_at(source, table_lba * SECTOR, entries * width)
        stores = [struct.unpack_from("<QQ", table, i + 32) for i in range(0, len(table), width)
                  if table[i:i + 16] == STORE_TYPE]
        require(len(stores) == 1, "expected one PCP OS application partition")
        first, last = stores[0]
        require(34 <= first <= last < size // SECTOR - 33, "application partition outside disk")
        require(1 + 2 * SLOT_SECTORS <= last - first + 1 <= 2048, "unsupported store size")
        disk.truncate(size)
        fixture_hash = hashlib.sha256(struct.pack("<Q", size))
        for offset, length in [(0, 34 * SECTOR), (size - 33 * SECTOR, 33 * SECTOR),
                               (first * SECTOR, (last - first + 1) * SECTOR)]:
            data = read_at(source, offset, length)
            fixture_hash.update(struct.pack("<QQ", offset, length))
            fixture_hash.update(data)
            disk.seek(offset)
            disk.write(data)
    return first * SECTOR, fixture_hash.hexdigest()


def checkpoint(disk, offset, expected_source=None):
    with disk.open("rb") as stream:
        region = read_at(stream, offset, (1 + 2 * SLOT_SECTORS) * SECTOR)
    require(region.startswith(b"PCPOS USER STORE 1\n"), "missing application store marker")
    slots = {}
    for sector in (1, 1 + SLOT_SECTORS):
        pos = sector * SECTOR
        head = region[pos:pos + SECTOR]
        if head == bytes(SECTOR):
            continue
        require(head[:8] == b"PCPSLOT1" and hashlib.sha256(head[:52]).digest() == head[52:84],
                "invalid store header")
        if any(head[84:]):
            require(head[84:88] == b"TBM1" and head[88] in (0, 1)
                    and hashlib.sha256(head[:89]).digest() == head[89:121], "invalid mode extension")
        generation, length = struct.unpack_from("<QI", head, 8)
        require(generation > 0 and length <= (SLOT_SECTORS - 1) * SECTOR, "invalid store slot")
        payload = region[pos + SECTOR:pos + SECTOR + length]
        require(hashlib.sha256(payload).digest() == head[20:52], "invalid payload checksum")
        require(generation not in slots, "ambiguous duplicate generation")
        slots[generation] = payload
    require(bool(slots), "no committed factory store")
    generation = max(slots)
    expected = b""
    if expected_source is not None:
        name = b"apps/lark.txt"
        expected = struct.pack("<HI", len(name), len(expected_source)) + name + expected_source
    require(slots[generation] == expected, f"unexpected committed bytes at generation {generation}")
    return {"generation": generation, "source": expected_source.decode() if expected_source else None,
            "store_sha256": hashlib.sha256(region).hexdigest()}


class Guest:
    def __init__(self, process, serial, timeout):
        self.process = process
        self.serial = serial
        self.timeout = timeout
        self.cursor = 0

    def key(self, name):
        self.process.stdin.write(f"sendkey {name} 35\n".encode("ascii"))
        self.process.stdin.flush()
        time.sleep(.10)

    def submit(self, command):
        for char in command:
            self.key("spc" if char == " " else char)
        self.key("ret")

    def wait(self, marker):
        if marker in (STAGED, SAVED, RAN, ACCEPTED, STOPPED):
            marker = b"\n" + marker + b"\r\n"
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            data = self.serial.read_bytes() if self.serial.exists() else b""
            at = data.find(marker, self.cursor)
            if at != -1:
                self.cursor = at + len(marker) - int(marker.endswith(b"\n"))
                return
            code = self.process.poll()
            if code is not None:
                raise RuntimeError(f"{self.serial.name}: QEMU exited {code} before {marker!r}")
            time.sleep(.1)
        raise TimeoutError(f"{self.serial.name}: deadline before {marker!r}")

    def enter_desktop(self):
        self.wait(b"long mode true")
        self.wait(b"Persistent application store mounted:")
        self.wait(b"TAILORBIRD MCP CONNECTED")
        # Worker exit and MCP initialization are independent startup events.
        cursor = self.cursor
        self.cursor = 0
        self.wait(b"Qwen worker execution ended: Exited(3)")
        self.cursor = max(self.cursor, cursor)


@contextmanager
def boot(args, output, name, pins, guid, results):
    serial = output / f"{name}.serial"
    stderr = output / f"{name}.stderr"
    identity = str(uuid4())
    options = [args.qemu]
    if args.firmware:
        options.extend(["-L", str(args.firmware.resolve())])
    options += ["-machine", "pc", "-accel", "tcg,tb-size=8", "-cpu", "max",
                "-m", "48M", "-nic", "none", "-display", "none", "-monitor", "stdio",
                "-no-reboot", "-vga", "std", "-uuid", identity,
                "-drive", "if=none,format=raw,id=os,file=store.img,cache=writeback",
                "-device", "virtio-blk-pci,drive=os,disable-modern=on", "-kernel", "kernel32.elf",
                "-initrd", "Tailorbird,qwen.elf", "-serial", f"file:{serial.name}", "-append",
                f"auto=Tailorbird tailor_ui={pins['tailorbird']} userdisk={guid.bytes_le.hex()} q={pins['qwen-worker']}"]
    record = {"name": name, "uuid": identity}
    results["boots"].append(record)
    with stderr.open("xb") as errors:
        process = subprocess.Popen(options, cwd=output, stdin=subprocess.PIPE,
                                   stdout=subprocess.DEVNULL, stderr=errors)
    try:
        guest = Guest(process, serial, args.timeout)
        guest.enter_desktop()
        yield guest
    finally:
        record["stopped_by_harness"] = process.poll() is None
        if record["stopped_by_harness"]:
            process.terminate()
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        process.stdin.close()
        record["exit_code"] = process.returncode
        if serial.exists():
            record["serial_sha256"] = file_hash(serial)


def check_trace(serial, expected):
    controls = {STAGED, SAVED, RAN, ACCEPTED, STOPPED}
    actual = [line for line in serial.read_bytes().splitlines() if line in controls]
    require(actual == expected, f"{serial.name}: unexpected control sequence {actual!r}")


def exercise(args, output, results):
    manifest = json.loads(args.manifest.read_bytes())
    guid = UUID(manifest["factory_disk_guid"])
    pins = manifest["component_sha256"]
    if args.fixture_sha256 is None:
        require(file_hash(args.disk) == manifest["output_sha256"]["PCP-OS.img"], "factory image digest mismatch")
    results["whole_image_verified"] = args.fixture_sha256 is None
    results.update(source_commit=manifest["git_commit"], manifest_sha256=file_hash(args.manifest),
                   component_sha256={k: pins[k] for k in ("kernel", "tailorbird", "qwen-worker")},
                   qemu_version=subprocess.check_output([args.qemu, "--version"], text=True).splitlines()[0],
                   manifest_factory_sha256=manifest["output_sha256"]["PCP-OS.img"])
    for path, name, key in [(args.kernel, "kernel.elf", "kernel"),
                            (args.desktop, "Tailorbird", "tailorbird"), (args.worker, "qwen.elf", "qwen-worker")]:
        dest = output / name
        shutil.copyfile(path, dest)
        require(file_hash(dest) == pins[key], f"{name} differs from component manifest")
    subprocess.run([args.objcopy, "-O", "elf32-i386", "kernel.elf", "kernel32.elf"], cwd=output, check=True)
    disk = output / "store.img"
    offset, fixture_hash = prepare_disk(args.disk, disk, guid)
    results["fixture_sha256"] = fixture_hash
    if args.fixture_sha256 is not None:
        require(fixture_hash == args.fixture_sha256, "fixture digest mismatch")
    observations = results["checkpoints"]
    observations["factory"] = checkpoint(disk, offset)
    with boot(args, output, "first", pins, guid, results) as guest:
        guest.submit("create lark print original")
        guest.wait(STAGED)
        guest.key("alt-a")
        guest.wait(SAVED)
        original = checkpoint(disk, offset, b"print original")
        require(original["generation"] > observations["factory"]["generation"], "save did not advance generation")
        observations["approved"] = original
        guest.submit("edit lark print changed then run lark")
        guest.wait(STAGED)
        guest.key("alt-s")
        guest.wait(ACCEPTED)
        guest.key("alt-a")
        guest.wait(STOPPED)
        guest.key("alt-a")
        # A new staged proposal proves the guest processed input after the stale decision.
        # Inspect the trace before sending any command that legitimately reads the app.
        guest.submit("edit lark print barrier")
        guest.wait(STAGED)
        first_events = [STAGED, SAVED, STAGED, ACCEPTED, STOPPED, STAGED]
        check_trace(guest.serial, first_events)
        guest.key("alt-s")
        guest.wait(ACCEPTED)
        guest.wait(STOPPED)
        check_trace(guest.serial, first_events + [ACCEPTED, STOPPED])
        guest.submit("run lark")
        guest.wait(RAN)
    check_trace(output / "first.serial", first_events + [ACCEPTED, STOPPED, RAN])
    observations["after_stop"] = checkpoint(disk, offset, b"print original")
    require(observations["after_stop"] == original, "Stop changed application store")
    with boot(args, output, "cold", pins, guid, results) as guest:
        guest.key("alt-a")
        guest.submit("run lark")
        guest.wait(RAN)
        observations["cold"] = checkpoint(disk, offset, b"print original")
        require(observations["cold"] == original, "cold boot or stale approval changed application store")
        guest.submit("edit lark print replacement then run lark")
        guest.wait(STAGED)
        guest.key("alt-a")
        guest.wait(SAVED)
        guest.wait(RAN)
    check_trace(output / "cold.serial", [RAN, STAGED, SAVED, RAN])
    observations["fresh_approval"] = checkpoint(disk, offset, b"print replacement")
    require(observations["fresh_approval"]["generation"] > original["generation"], "replacement was not committed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("manifest", "disk", "kernel", "desktop", "worker", "output"):
        parser.add_argument("--" + flag, type=Path, required=True)
    parser.add_argument("--fixture-sha256", help="verify only the copied regions against this independent SHA-256")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--objcopy", default="objcopy")
    parser.add_argument("--firmware", type=Path)
    parser.add_argument("--timeout", type=float, default=180, help="seconds per expected guest event")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    results = {"profile": "x86-64 direct boot, model unavailable", "passed": False, "boots": [], "checkpoints": {}}
    try:
        with tempfile.TemporaryDirectory(dir=output) as scratch:
            work = Path(scratch)
            try:
                exercise(args, work, results)
            finally:
                for pattern in ("*.serial", "*.stderr"):
                    for log in work.glob(pattern):
                        log.rename(output / log.name)
        results["passed"] = True
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.SubprocessError) as error:
        results["error"] = str(error)
        raise
    finally:
        (output / "result.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
