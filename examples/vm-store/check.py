"""Cold-boot regression for the opt-in PCP OS VM store. All disks are new regular files."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time

from create import create

HERE = Path(__file__).resolve().parent
MEASURE = HERE.parents[1] / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"


def digest(data):
    return hashlib.sha256(data).digest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def make_programs(out, clang, objcopy):
    for action in range(1, 9):
        obj, raw = out / "guest.o", out / "guest.bin"
        subprocess.run([clang, "--target=x86_64-unknown-linux-gnu", f"-DACTION={action}",
                        "-c", str(HERE / "probe.S"), "-o", str(obj)], check=True)
        subprocess.run([objcopy, "-j", ".text", "-O", "binary", str(obj), str(raw)], check=True)
        code = raw.read_bytes()
        size = 120 + len(code)
        elf = struct.pack("<16sHHIQQQIHHHHHH", b"\x7fELF\x02\x01\x01", 3, 62, 1, 120,
                          64, 0, 0, 64, 56, 1, 0, 0, 0)
        segment = struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, size, size, 4096)
        (out / f"action{action}.elf").write_bytes(elf + segment + code)
    raw.unlink()
    obj.unlink()


def check_slots(image, expected):
    """Decode the two on-disk records independently for the known single-file test cases."""
    found = {}
    for sector in (1, 130):
        header = image[sector * 512:(sector + 1) * 512]
        if header == bytes(512):
            continue
        require(header[:8] == b"PCPSLOT1" and digest(header[:52]) == header[52:84],
                "invalid commit header")
        generation, length = struct.unpack_from("<QI", header, 8)
        start = (sector + 1) * 512
        payload = image[start:start + length]
        require(length <= 65536 and digest(payload) == header[20:52], "invalid slot payload")
        require(generation not in found, "duplicate generation")
        if payload:
            n, v = struct.unpack_from("<HI", payload)
            require(6 + n + v == len(payload) and payload[6:6+n] == b"guest-file",
                    "unexpected stored name or length")
            payload = payload[6+n:]
        found[generation] = payload
    require(found == expected, f"unexpected generations or contents: {found!r}")


def test(args):
    out = args.output.resolve()
    out.mkdir()
    identity = create(out / "volume")
    disk = out / "volume/data.raw"
    shutil.copyfile(args.kernel, out / "kernel.elf")
    measured = subprocess.check_output([sys.executable, MEASURE, out / "kernel.elf"], text=True).strip()
    make_programs(out, args.clang, args.objcopy)
    qemu = [args.qemu]
    if args.bios_dir:
        qemu += ["-L", str(args.bios_dir.resolve())]
    receipts = []
    mount = f"store=ide0 store_id={identity}"

    def boot(label, action, *, options=mount, role=True, interrupt=False, refuse=False):
        guest = f"action{action}.elf"
        serial = out / (label + ".serial")
        before = disk.read_bytes()
        drive = "ide-hd,drive=store,bus=ide.0,unit=0" + (",serial=PCP-VM-STORE" if role else "")
        cmd = qemu + ["-machine", "pc", "-accel", "tcg", "-cpu", "max", "-m", "256M",
                      "-nic", "none", "-display", "none", "-vga", "std" if interrupt else "none",
                      "-monitor", "none", "-no-reboot", "-icount", "shift=4,align=off,sleep=off",
                      "-kernel", "kernel.elf", "-initrd", guest, "-append",
                      f"auto={guest} then={'menu' if interrupt else 'off'} {options} image={measured}",
                      "-drive", "if=none,id=store,format=raw,file=volume/data.raw", "-device", drive,
                      "-serial", "file:" + serial.name, "-device", "isa-debug-exit,iobase=0xf4,iosize=4"]
        with (out / (label + ".stderr")).open("wb") as err:
            vm = subprocess.Popen(cmd, cwd=out, stdout=subprocess.DEVNULL, stderr=err)
            try:
                if interrupt:
                    deadline = time.monotonic() + args.timeout
                    while time.monotonic() < deadline:
                        log = serial.read_text(errors="replace") if serial.exists() else ""
                        if guest + " ended DONE" in log:
                            break
                        require(vm.poll() is None, label + " exited before the planned stop")
                        time.sleep(0.02)
                    else:
                        raise TimeoutError(label)
                    require(vm.poll() is None, label + " already powered off")
                    vm.kill()
                status = vm.wait(timeout=args.timeout)
            finally:
                if vm.poll() is None:
                    vm.kill()
                    vm.wait()
        log = serial.read_text(errors="replace")
        require(measured in log, label + " missing kernel measurement")
        if refuse:
            require(status == 35 and "VM checkpoint store could not be mounted" in log
                    and "starting " + guest not in log, label + " failed to refuse the mount")
        else:
            require((status != 33 if interrupt else status == 33) and "STORE PROBE OK" in log
                    and guest + " ended DONE" in log, label + " failed")
        after = disk.read_bytes()
        receipts.append(dict(case=label, exit=status, before=digest(before).hex(),
                             after=digest(after).hex()))
        print(label, flush=True)
        return after

    initial = disk.read_bytes()
    check_slots(initial, {1: b""})
    old = boot("save-old", 1)
    check_slots(old, {1: b"", 2: b"oldbytes"})
    require(boot("cold-old", 2) == old, "read modified disk")
    require(boot("unsynced-replace-power-cut", 5, interrupt=True) == old, "unsynced write reached disk")
    require(boot("cold-after-power-cut", 2) == old, "recovery modified disk")
    new = boot("save-replacement", 3)
    check_slots(new, {2: b"oldbytes", 3: b"newbytes"})
    require(boot("cold-replacement", 4) == new, "read modified disk")
    require(boot("capacity", 7) == new, "full checkpoint changed disk")
    require(boot("no-store-grant", 6, options=mount + " grant=none") == new, "denied save changed disk")
    require(boot("volatile", 8, options="") == new, "volatile guest changed disk")
    require(boot("missing-role", 4, role=False, refuse=True) == new, "missing-role disk changed")
    require(boot("missing-id", 4, options="store=ide0", refuse=True) == new, "incomplete request wrote")
    disk.write_bytes(new + bytes(512))
    require(boot("wrong-size", 4, refuse=True) == new + bytes(512), "wrong-size disk changed")

    # Model a stop after the replacement data reached disk but before its commit
    # sector, then a partial commit-sector write. An older valid slot survives both.
    partial = bytearray(old)
    partial[1024:1536] = new[1024:1536]
    disk.write_bytes(partial)
    require(boot("payload-only", 2) == partial, "incomplete commit changed on recovery")
    partial[512:544] = new[512:544]
    disk.write_bytes(partial)
    require(boot("partial-header", 2) == partial, "partial header changed on recovery")
    disk.write_bytes(new)
    require(boot("final-cold", 4) == new, "final stored bytes changed")
    (out / "results.json").write_text(json.dumps(dict(kernel_measurement=measured, boots=receipts),
                                                indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("output", type=Path, help="new directory for disk, programs and serial receipts")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--bios-dir", type=Path)
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--objcopy", default="objcopy")
    parser.add_argument("--timeout", type=int, default=60, help="timeout per VM, in seconds")
    test(parser.parse_args())
