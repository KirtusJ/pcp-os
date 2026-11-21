#!/usr/bin/env python3
"""Boot a tester CD made by build.py, with only that CD attached, and check one whole session.

    python3 examples/tester-cd/check.py <output directory of build.py> <new evidence directory>

For each of QEMU's `pc` and `q35` machines, on the ordinary clock and on the instruction-counted one: the
kernel must measure as the value in the receipt, pass every startup self-test, start the Surfbird demo from
the keyboard and see it end DONE, start it again and stop it with the kernel's Stop key, show the register,
and power off when asked. The guest has no disk and no network device. Screens are kept as evidence and are
not compared: every assertion is on the serial log, QEMU's own events and its exit status.

This is one emulator on one host. It is not a test of a physical machine, of UEFI firmware or of any other
hypervisor.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time


def session(qemu, iso, out, machine, counted, measurement, timeout):
    out.mkdir()
    serial, sock = out / "serial.txt", out / "qmp.sock"
    cmd = [qemu, "-machine", machine, "-accel", "tcg", "-cpu", "qemu64", "-m", "64M", "-boot", "order=d", "-cdrom", str(iso),
           "-vga", "std", "-nic", "none", "-audiodev", "none,id=snd0", "-device", "AC97,audiodev=snd0", "-display", "none",
           "-monitor", "none", "-qmp", "unix:%s,server=on,wait=off" % sock, "-serial", "file:%s" % serial]
    if counted:
        cmd += ["-icount", "shift=3,align=off,sleep=off"]
    result = {"machine": machine, "clock": "instruction-counted" if counted else "ordinary", "argv": cmd}
    with (out / "qemu-stderr.txt").open("wb") as err:
        guest = subprocess.Popen(cmd, stderr=err)
    try:
        end = time.monotonic() + 20
        while not sock.exists():
            if time.monotonic() > end or guest.poll() is not None:
                raise RuntimeError("QEMU did not offer its monitor")
            time.sleep(0.1)
        channel = socket.socket(socket.AF_UNIX)
        channel.settimeout(20)
        channel.connect(str(sock))
        stream = channel.makefile("rw")
        events = []

        def rpc(command, **arguments):
            stream.write(json.dumps({"execute": command, "arguments": arguments}) + "\n")
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if "event" in reply:
                    events.append(reply)
                elif "error" in reply:
                    raise RuntimeError("%s: %s" % (command, reply["error"]))
                elif "return" in reply:
                    return reply["return"]

        def log():
            return serial.read_text(errors="replace") if serial.exists() else ""

        def wait(text, count=1):
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                seen = log()
                if "it is in SAFE" in seen:
                    raise RuntimeError("the guest entered SAFE; see serial.txt for the failed self-tests")
                if seen.count(text) >= count:
                    return
                if guest.poll() is not None:
                    raise RuntimeError("QEMU exited with status %s while waiting for: %s" % (guest.returncode, text))
                time.sleep(0.25)
            raise TimeoutError("waited %s s for: %s" % (timeout, text))

        def key(*names):
            rpc("send-key", keys=[{"type": "qcode", "data": n} for n in names], **{"hold-time": 80})

        def screen(name):
            time.sleep(1.5)
            rpc("screendump", filename=str(out / (name + ".ppm")))

        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("no QMP greeting")
        rpc("qmp_capabilities")
        media = [d for d in rpc("query-block") if "inserted" in d]
        result["inserted_media"] = len(media)
        if len(media) != 1 or not media[0]["inserted"]["ro"]:
            raise RuntimeError("expected exactly one inserted medium, read-only")

        wait(measurement + " (measured; equal to the value the loader named)")
        wait("leaves no memory behind")
        seen = log()
        result["startup_pass_lines"] = sum(line.startswith("  pass ") for line in seen.splitlines())
        result["startup_fail_lines"] = sum(line.lower().startswith("  fail") for line in seen.splitlines())
        screen("launcher")
        key("ret")
        wait("SURFBIRD media ready")
        screen("surfbird")
        key("spc")
        wait("surf.elf ended DONE: exit 0")
        key("ret")
        wait("SURFBIRD media ready", 2)
        key("ctrl", "alt", "delete")
        wait("surf.elf ended STOPPED")
        key("r")
        screen("register")
        key("esc")
        time.sleep(1)
        key("p")
        wait("ALL SELF-TESTS PASS.")
        try:
            result["exit_code"] = guest.wait(timeout=40)
        except subprocess.TimeoutExpired:
            raise RuntimeError("the guest did not power off after P")
        while not any(e.get("event") == "SHUTDOWN" for e in events):
            line = stream.readline()
            if not line:
                break
            reply = json.loads(line)
            if "event" in reply:
                events.append(reply)
        result["shutdown"] = next((e["data"] for e in events if e.get("event") == "SHUTDOWN"), None)
        final = log()
        result["final_pass_lines"] = sum(line.startswith("  pass ") for line in final.splitlines())
        # The kernel says which optional parts it was built with on its last line, at power off.
        result["guest_ethernet_compiled_in"] = "Guest Ethernet support is compiled in" in final
        problems = []
        if result["startup_fail_lines"]:
            problems.append("a startup self-test failed")
        if result["startup_pass_lines"] < 1:
            problems.append("no startup self-test was reported")
        if result["guest_ethernet_compiled_in"]:
            problems.append("guest Ethernet is compiled into a CD that is meant to have none")
        if result["exit_code"] != 0 or not result["shutdown"] or result["shutdown"].get("guest") is not True:
            problems.append("the guest did not request its own shutdown with QEMU exit 0")
        result["problems"] = problems
    except (OSError, RuntimeError, TimeoutError, ValueError) as error:
        result["problems"] = [str(error)]
    finally:
        if guest.poll() is None:
            guest.kill()
            guest.wait()
        if sock.exists():
            sock.unlink()
    result["passed"] = not result["problems"]
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print("%-4s %-19s %s%s" % (machine, result["clock"], "passed" if result["passed"] else "FAILED: ", "; ".join(result["problems"])), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("built", type=Path, help="directory holding pcp-os-tester.iso and receipt.json")
    parser.add_argument("evidence", type=Path, help="new directory for serial logs, screens and results")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--timeout", type=float, default=90, help="seconds to wait for each expected line")
    args = parser.parse_args()
    receipt = json.loads((args.built / "receipt.json").read_text())
    iso = (args.built / "pcp-os-tester.iso").resolve(strict=True)
    if hashlib.sha256(iso.read_bytes()).hexdigest() != receipt["iso_sha256"]:
        sys.exit("the ISO is not the one its receipt describes")
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    version = subprocess.check_output([args.qemu, "--version"], text=True).splitlines()[0]
    results = [session(args.qemu, iso, evidence / ("%s-%s" % (machine, "counted" if counted else "ordinary")), machine, counted,
                       receipt["kernel_measurement"], args.timeout)
               for machine in ("pc", "q35") for counted in (False, True)]
    summary = {"iso_sha256": receipt["iso_sha256"], "source_commit": receipt["source_commit"], "kernel_measurement": receipt["kernel_measurement"],
               "qemu": version, "host": " ".join(os.uname()[i] for i in (0, 2, 4)), "passed": all(r["passed"] for r in results),
               "sessions": [{k: r.get(k) for k in ("machine", "clock", "passed", "startup_pass_lines", "final_pass_lines", "exit_code", "problems")} for r in results]}
    (evidence / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
