"""Run the pre-SMP BIOS control against the queue-desk tester image and its CI receipt."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()

def require(test, message):
    if not test:
        raise RuntimeError(message)

def run(args):
    out = args.output
    out.mkdir(parents=True)
    receipt = json.loads(args.receipt.read_text())
    require(digest(args.iso) == receipt["iso_sha256"], "ISO does not match receipt")
    require(receipt["source_commit"] == "2f7d9d4315fdc485c6e0693e325643456993e1df", "different source commit")
    serial = out / "serial.txt"
    events = []
    cmd = [args.qemu, "-machine", "q35,accel=tcg", "-cpu", "qemu64", "-smp", args.topology,
           "-m", "64M", "-boot", "order=d", "-cdrom", str(args.iso.resolve()), "-vga", "std",
           "-nic", "none", "-audiodev", "none,id=snd0", "-device", "AC97,audiodev=snd0",
           "-display", "none", "-monitor", "none", "-qmp", "stdio",
           "-serial", f"file:{serial.resolve()}"]
    with (out / "stderr.txt").open("wb") as err, subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, text=True, encoding="utf-8") as guest:
        try:
            version = json.loads(guest.stdout.readline())["QMP"]["version"]
            def rpc(name, **arguments):
                guest.stdin.write(json.dumps({"execute":name, "arguments":arguments}) + "\n")
                guest.stdin.flush()
                while line := guest.stdout.readline():
                    reply = json.loads(line)
                    if "event" in reply:
                        events.append(reply)
                    else:
                        require("return" in reply, str(reply))
                        return reply["return"]
                raise RuntimeError(f"QMP closed during {name}")
            rpc("qmp_capabilities")
            seen = rpc("query-cpus-fast")
            cpus = [{"index": cpu["cpu-index"], "topology": cpu["props"],
                     "apic_id": rpc("qom-get", path=cpu["qom-path"], property="apic-id")}
                    for cpu in seen]
            ids = [cpu["apic_id"] for cpu in cpus]
            require(len(cpus) == args.count and len(set(ids)) == args.count, "QMP topology mismatch")
            media = [dev["inserted"] for dev in rpc("query-block") if "inserted" in dev]
            require(len(media) == 1 and media[0]["ro"], "expected one read-only CD")
            end = time.monotonic() + 90
            while True:
                boot = serial.read_text(errors="replace")
                require("it is in SAFE" not in boot and guest.poll() is None, "startup failed")
                if "leaves no memory behind" in boot: break
                require(time.monotonic() < end, "startup timed out")
                time.sleep(.15)
            require(receipt["kernel_measurement"] + " (measured; equal to the value the loader named)" in boot, "kernel measurement differs")
            require(sum(x.startswith("  pass ") for x in boot.splitlines()) == 71, "startup checks differ")
            time.sleep(.8)
            for key in ("f10", "ret"):
                rpc("send-key", keys=[{"type":"qcode","data":key}], **{"hold-time":80})
                time.sleep(.5)
            code = guest.wait(timeout=20)
            for line in guest.stdout:
                reply=json.loads(line)
                if "event" in reply: events.append(reply)
            shutdown = next((e["data"] for e in events if e["event"] == "SHUTDOWN"), {})
            require(code == 0 and shutdown.get("guest") is True, "guest shutdown unconfirmed")
        finally:
            if guest.poll() is None:
                guest.terminate()
                guest.wait(timeout=5)
    final = serial.read_text(errors="replace")
    passes = sum(x.startswith("  pass ") for x in final.splitlines())
    require(passes == 72 and not any(x.lower().startswith("  fail") for x in final.splitlines()), "one or more checks missing or failed")
    require("ALL SELF-TESTS PASS." in final and "a second processor." in final, "pre-SMP control missing")
    result = dict(topology=args.topology, machine="q35", clock="ordinary TCG", model="qemu64", qemu=version,
                  cpus=cpus, startup_passes=71, final_passes=passes, shutdown=shutdown, exit=code,
                  source=receipt["source_commit"], iso=receipt["iso_sha256"], kernel=receipt["kernel_measurement"],
                  serial_sha256=digest(serial))
    (out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
    print(args.topology, "APIC IDs", ids, "72 passes, guest shutdown", flush=True)

if __name__ == "__main__":
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--qemu",default="qemu-system-x86_64")
    p.add_argument("--iso",type=Path,required=True)
    p.add_argument("--receipt",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--topology",required=True)
    p.add_argument("--count",type=int,required=True)
    run(p.parse_args())
