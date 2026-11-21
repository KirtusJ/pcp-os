"""Exercise the firmware-owned optical handoff in disposable Q35 guests."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from image import digest, make


def wait_for(test, label, guest, seconds=120):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if test():
            return
        if guest.poll() is not None:
            raise RuntimeError(f"Guest exited before {label}")
        time.sleep(.06)
    raise TimeoutError(label)


def verify_loaded_memory(image, elf, work, rpc):
    """Read back each PT_LOAD, including its zero-filled tail, and each module."""
    from image import MEMBERS
    raw = elf.read_bytes()
    results = {}
    capture = work / "loaded-memory.tmp"

    def verify(label, start, size, expected_digest):
        rpc("pmemsave", val=start, size=size, filename=str(capture))
        actual = digest(capture)
        capture.unlink()
        if actual != expected_digest:
            raise ValueError(f"Physical memory content differs for {label}")
        results[label] = {"start": start, "bytes": size, "sha256": actual}

    for index, (start, mem_size, offset, file_size, _) in enumerate(image["segments"]):
        expected = hashlib.sha256(raw[offset:offset + file_size])
        expected.update(bytes(mem_size - file_size))
        verify(f"PT_LOAD_{index}", start, mem_size, expected.hexdigest())
    for name, _, expected in MEMBERS:
        start, end = image["modules"][name]
        verify(name, start, end - start, expected)
    return results


def observe(work, out, ram, mode, image, kernel):
    out.mkdir()
    trace = out / "firmware.txt"
    serial = out / "serial.txt"
    code = Path("/usr/share/OVMF/OVMF_CODE_4M.fd")
    variables = work / f"{ram}-{mode}.fd"
    shutil.copyfile("/usr/share/OVMF/OVMF_VARS_4M.fd", variables)
    qmp, second, gate = (work / f"{ram}-{mode}-{s}" for s in ("q", "m", "g"))
    cmd = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", str(ram), "-smp", "1",
           "-nic", "none", "-vga", "std", "-audiodev", "none,id=snd", "-device", "AC97,audiodev=snd", "-display", "none",
           "-monitor", "none", "-qmp", f"unix:{qmp},server=on,wait=off",
           "-chardev", f"socket,id=observations,path={second},server=on,wait=off", "-mon", "chardev=observations,mode=control",
           "-serial", "file:" + str(serial), "-serial", f"unix:{gate},server=on,wait=off",
           "-debugcon", "file:" + str(trace), "-global", "isa-debugcon.iobase=0xe9",
           "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={code}",
           "-drive", f"if=pflash,unit=1,format=raw,file={variables}", "-boot", "order=d,strict=on",
           "-cdrom", str(work / "owned.iso"), "-icount", "shift=3,align=off,sleep=off", "-no-reboot"]
    (out / "qemu-command.json").write_text(json.dumps(cmd, indent=2) + "\n")
    report = {"ram_mib": ram, "mode": mode, "ovmf_sha256": digest(code), "iso_sha256": image["iso_sha256"]}
    def log(path):
        return path.read_text(errors="replace") if path.exists() else ""
    with (out / "qemu.stderr").open("wb") as stderr, subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stderr=stderr) as guest:
        try:
            wait_for(lambda: "await-host\n" in log(trace), "initial firmware map", guest)
            ranges = [(int(a, 16), int(b, 16)) for a, b in re.findall(r"^nvs=(\w+),(\w+)$", log(trace), re.M)]
            if not ranges or any(hi <= lo or hi - lo > 32 << 20 for lo, hi in ranges):
                raise ValueError("Unexpected NVS inventory")
            with socket.socket(socket.AF_UNIX) as control, socket.socket(socket.AF_UNIX) as keys:
                control.settimeout(15)
                control.connect(str(second))
                keys.settimeout(10)
                keys.connect(str(gate))
                with control.makefile("rwb", buffering=0) as io:
                    if "QMP" not in json.loads(io.readline()):
                        raise ValueError("Missing QMP greeting")

                    def rpc(command, **args):
                        io.write((json.dumps(dict(execute=command, arguments=args)) + "\n").encode())
                        while True:
                            msg = json.loads(io.readline())
                            if "error" in msg:
                                raise RuntimeError(msg["error"])
                            if "return" in msg:
                                return msg["return"]
                    rpc("qmp_capabilities")

                    def nvs():
                        hashes = {}
                        for lo, hi in ranges:
                            file = work / f"nvs-{ram}-{mode}.tmp"
                            rpc("pmemsave", val=lo, size=hi - lo, filename=str(file))
                            hashes[f"{lo:x}-{hi:x}"] = digest(file)
                            file.unlink()
                        return hashes
                    report["nvs_before_allocation"] = nvs()
                    keys.sendall(mode.encode())
                    if mode == "X":
                        if guest.wait(timeout=45) != 0:
                            raise RuntimeError("Refused VM exit")
                        records = log(trace)
                        if ("control_page=0000000000000000" not in records or "allocation=800000000000000e" not in records
                                or "handoff-refused" not in records or "owned-before-write" in records or "owned-handoff" in records
                                or "PCP OS kernel " in log(serial)):
                            raise ValueError("The occupied-range control did not refuse before loading")
                        report["refused_before_write"] = True
                    else:
                        wait_for(lambda: "await-content-check" in log(trace), "EFI copy", guest)
                        report["loaded_memory"] = verify_loaded_memory(
                            image, kernel / "target/pcpos-kernel.elf32", work, rpc)
                        report["nvs_after_copy"] = nvs()
                        if report["nvs_after_copy"] != report["nvs_before_allocation"]:
                            raise ValueError("NVS changed before ExitBootServices")
                        keys.sendall(b"C")
                        wait_for(lambda: "leaves no memory behind" in log(serial) or "it is in SAFE" in log(serial),
                                 "kernel startup", guest)
                        report["nvs_at_launcher"] = nvs()

                        def at_stop():
                            wait_for(lambda: "surf.elf ended STOPPED" in log(serial), "keyboard Stop", guest)
                            return nvs()
                        with ThreadPoolExecutor(max_workers=1) as pool:
                            later = pool.submit(at_stop)
                            try:
                                exercise(
                                    guest,
                                    qmp,
                                    serial,
                                    out,
                                    False,
                                    image["measurement"],
                                    False,
                                    False,
                                    True)
                            finally:
                                if not later.done() and "surf.elf ended STOPPED" not in log(serial):
                                    if guest.poll() is None:
                                        guest.terminate()
                            report["nvs_after_stop"] = later.result()
                        check_register(kernel / "src/font.rs", out)
                        records = log(trace)
                        actual = [int(s, 16) for s in re.findall(r"^exit_status=(\w+)$", records, re.M)]
                        expected = [(1 << 63) | 2, 0] if mode == "R" else [0]
                        if (actual != expected or records.count("owned-before-write\n") != 1
                                or records.count("owned-handoff\n") != 1 or "allocation=0000000000000000" not in records):
                            raise ValueError(f"Missing firmware ownership or exit sequence: {actual}")
                        report["exit_statuses"] = actual
                        guest_log = log(serial)
                        members = {name: [int(addr, 16), int(addr, 16) + int(size)] for name, size, addr in re.findall(
                            r"file from the loader: ([^ ]+) \((\d+) bytes at (0x[\da-f]+)\)", guest_log)}
                        if members != image["modules"]:
                            raise ValueError("Guest module placement differs")
                        p = re.search(
                            r"memory for programs: (\d+) frames of 4 KiB, (0x[\da-f]+)\.\.(0x[\da-f]+)", guest_log)
                        if not p:
                            raise ValueError("Guest omitted its program frame pool")
                        lo, hi = int(p[2], 16), int(p[3], 16)
                        # Only use the final map when retrying ExitBootServices.
                        section = records.rsplit("map_descriptors=", 1)[1]
                        descriptors = [tuple(int(x, 16) for x in row)
                                       for row in re.findall(r"^range=(\w+),(\w+),(\w+)$", section, re.M)]
                        if len(descriptors) != int(section.splitlines()[0], 16):
                            raise ValueError("Incomplete final map")
                        if mode == "R":
                            extra = re.search(r"^stale_control_page=(\w+)$", records, re.M)
                            if not extra or "stale_control_status=0000000000000000" not in records:
                                raise ValueError("The stale-key control did not allocate its page")
                            page = int(extra[1], 16)
                            if not any(t == 2 and a <= page and page + 4096 <=
                                       a + n for a, n, t in descriptors):
                                raise ValueError("Refreshed map did not retain the stale-key control page")
                            report["stale_control_page"] = page
                        covered = sum(max(0, min(hi, a + n) - max(lo, a))
                                      for a, n, t in descriptors if t == 1)
                        if (hi <= lo or lo < image["arena"][1] or hi - lo != int(p[1]) * 4096 or covered != hi - lo
                                or any(t != 1 and a < hi and a + n > lo for a, n, t in descriptors)):
                            raise ValueError(
                                "Frame pool outside conventional memory or overlapping the firmware allocation")
                        if report["nvs_before_allocation"] != report["nvs_at_launcher"] or report["nvs_after_stop"] != report["nvs_at_launcher"]:
                            raise ValueError("NVS digest changed")
                        report.update(pool=[lo, hi], modules=members, exit_code=guest.returncode)
        finally:
            if guest.poll() is None:
                guest.kill()
            guest.wait()
            (out / "result.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("distribution", "kernel", "output"):
        p.add_argument(name, type=Path)
    args = p.parse_args()
    distro = args.distribution.resolve()
    kernel = args.kernel.resolve()
    out = args.output.resolve()
    out.mkdir()
    sys.path.insert(0, str(distro / "audit/public-live-cd"))
    from check import exercise
    from check_register import check_register
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        desc = make(distro / "PCP-OS.iso", kernel / "target/pcpos-kernel.elf32",
                    kernel / "tools/measure.py", work, Path(__file__).resolve().parent)
        (out / "image.json").write_text(json.dumps(desc, indent=2) + "\n")
        for ram, gate in ((128, "X"), (128, "G"), (256, "R")):
            observe(work, out / f"{ram}-{gate}", ram, gate, desc, kernel)
