"""Read a pinned Core in a public resident worker before it reaches its normal entry point."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

from check import CORE, HERE, digest, reference_object

BUILD22 = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707"
FIXTURE = "f1d1ec07ccc30dec4a8c54a6b454efec30cc0f6a"
CLOCK = ["-icount", "shift=3,align=off,sleep=off"]
PARENTS = (0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12)


def check(condition, message):
    if not condition:
        raise RuntimeError(message)


def build(fixture, model, upstream, out, source):
    build_dir = fixture / "examples/resident-workers/fixture"
    subprocess.run([sys.executable, build_dir / "build.py", "--model", model,
                    "--reference", upstream, "--out", out / "control"], check=True)
    reference_object(out, source)
    subprocess.run(["cc", "-O2", "-Wall", "-Wextra", "-Werror", "-ffreestanding",
                    "-fno-stack-protector", "-fno-pie", "-mno-red-zone", "-mcmodel=large",
                    "-DREADBACK_EMBEDDED", "-c", HERE / "readback.c", "-o", out / "reader.o"], check=True)
    subprocess.run(["cc", "-c", HERE / "worker-entry.S", "-o", out / "entry.o"], check=True)
    # Compile the same Rust source with the same guest ABI as the pinned builder.
    subprocess.run([os.environ.get("RUSTC", "rustc"), "--edition=2021", "-Copt-level=2",
                    "-Cpanic=abort", "--target", "x86_64-unknown-linux-gnu", "-Clinker=ld",
                    "-Clinker-flavor=ld", "-Clink-arg=-static", "-Cdefault-linker-libraries=no",
                    "-Cno-redzone=yes", "-Clto=fat", "-Ccode-model=large", "-Crelocation-model=static",
                    f"-Clink-arg=-T{build_dir / 'link.ld'}", "-Clink-arg=--build-id=none",
                    "-Cstrip=symbols", "-Clink-arg=--entry=core_entry", "-Clink-arg=entry.o",
                    "-Clink-arg=reader.o", "-Clink-arg=reference.o", build_dir / "main.rs",
                    "-o", "resident.elf"], cwd=out, env=dict(os.environ, PCPOS_WORKER_CHECKPOINT=str(model)),
                   check=True)


def negative_controls(out, kernel, measure, source, qemu, worker_hash):
    for name, payload, status in (("changed", source[:-1] + bytes([source[-1] ^ 1]), 1),
                                  ("absent", None, 2)):
        case = out / name
        case.mkdir()
        shutil.copyfile(out / "resident.elf", case / "resident.elf")
        modules = "resident.elf 1 7 0"
        if payload is not None:
            (case / "core-spec.md").write_bytes(payload)
            modules += ",core-spec.md"
        with (case / "boot.serial").open("w") as log:
            boot = subprocess.run([qemu, *CLOCK, "-machine", "microvm,isa-serial=on,pic=on,pit=on,rtc=on",
                                   "-device", "i8042", "-accel", "tcg,tb-size=4", "-cpu", "max", "-m", "32M",
                                   "-nodefaults", "-nic", "none", "-display", "none", "-monitor", "none",
                                   "-serial", "stdio", "-no-reboot", "-kernel", kernel, "-initrd", modules,
                                   "-append", f"auto=resident.elf then=off image={measure}",
                                   "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                                  cwd=case, stdout=log, stderr=subprocess.STDOUT, timeout=180)
        text = (case / "boot.serial").read_text()
        start = re.findall(r"^starting resident\.elf \(\d+ bytes, SHA-256 ([0-9a-f]{64})\) with: (.+)$", text, re.M)
        exits = re.findall(r"^resident\.elf ended FAILED: exit (\d+)\b", text, re.M)
        check(boot.returncode == 33 and measure + " (measured; equal to the value the loader named)" in text
              and start == [(worker_hash, "1 7 0")] and exits == [str(status)]
              and not re.search(r"^(NODE |LM tokens |READY )", text, re.M), f"{name}: see {case}/boot.serial")
        print(f"{name}: guest exit {status} before model startup", flush=True)


def cohort(fixture, worker, oracle, out, kernel, qemu, source_file=None):
    # QEMU takes extra Multiboot modules in the existing, comma-separated -initrd argument.
    env = dict(os.environ, PCPOS_REAL_QEMU=qemu, PCPOS_CORE_MODULE=str(source_file or ""))
    subprocess.run([sys.executable, fixture / "examples/resident-workers/supervise.py",
                    "--kernel", kernel, "--worker", worker, "--oracle", oracle, "--run", out,
                    "--qemu", HERE / "qemu-worker", "--once", "--startup-timeout", "240",
                    "--startup-idle-timeout", "120", "--response-timeout", "150"], check=True, env=env)
    ledger = json.loads((out / "readback.json").read_bytes())
    events = [json.loads(line) for line in (out / "events.jsonl").read_text().splitlines()]
    admissions = {e["node"]: e for e in events if e["event"] == "admitted"}
    rows = ledger["nodes"]
    check(ledger["cycle"] == 1 and ledger["repairs"] == 0 and len(rows) == 24 and len(admissions) == 24
          and {r["node"] for r in rows} == set(range(1, 25)) and len({r["uuid"] for r in rows}) == 24,
          "incomplete cohort")
    inputs = json.loads(oracle.read_bytes())
    count = 0
    for row in rows:
        node = row["node"]
        a = admissions[node]
        serial = (out / f"{node}-{a['attempt']}.serial").read_bytes()
        text = serial.decode("ascii").splitlines()
        check(row["uuid"] == a["uuid"] and row["parent"] == a["parent"] == PARENTS[node]
              and sorted(a["children"]) == [n for n in range(2, 25) if PARENTS[n] == node]
              and (node == 1 or admissions[a["parent"]]["ticket"] == a["input"])
              and row["tokens"] == inputs[row["prompt"]] and digest(serial) == row["serial_sha256"],
              f"receipt mismatch for node {node}")
        deliveries = [line for line in text if line.startswith("file from the loader: core-spec.md ")]
        if source_file:
            check(len(deliveries) == 1 and deliveries[0].startswith(
                f"file from the loader: core-spec.md ({source_file.stat().st_size} bytes at "), f"node {node}: missing Core")
        else:
            check(not deliveries, f"unmodified control unexpectedly received Core, node {node}")
        count += len(a["children"])
    print(f"{out.name}: {len(rows)} guest replies, {count} parent-issued edges, zero repairs", flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("fixture", "kernel", "method", "model", "reference", "out"):
        p.add_argument("--" + name, required=True, type=Path)
    p.add_argument("--qemu", default="qemu-system-x86_64")
    a = p.parse_args()
    fixture = a.fixture.resolve(strict=True)
    head = subprocess.check_output(["git", "-C", fixture, "rev-parse", "HEAD"], text=True).strip()
    check(head == FIXTURE, "checkout does not identify the expected public fixture")
    manifest = (a.method / "MANIFEST.json").read_bytes()
    check(digest(manifest) == BUILD22, "unexpected build 22 method manifest")
    source_path = a.method / CORE
    source = source_path.read_bytes()
    rows = [r for r in json.loads(manifest)["files"] if r["path"] == CORE]
    check(len(rows) == 1 and rows[0]["sha256"] == digest(source), "Core digest mismatch")
    qemu = shutil.which(a.qemu)
    check(qemu is not None, "QEMU executable missing")
    out = a.out.resolve()
    out.mkdir()
    kernel = a.kernel.resolve(strict=True)
    measure = subprocess.check_output([sys.executable, fixture / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py",
                                       kernel], text=True).strip()
    build(fixture, a.model.resolve(strict=True), a.reference.resolve(strict=True), out, source)
    module = out / "core-spec.md"
    module.write_bytes(source)
    worker = out / "resident.elf"
    oracle = out / "control/oracle.json"
    hashes = {name: digest(path.read_bytes()) for name, path in
              (("kernel", kernel), ("worker", worker), ("control", out / "control/resident.elf"),
               ("oracle", oracle), ("Core", module))}
    print(json.dumps(dict(fixture=head, measurement=measure, manifest=BUILD22, hashes=hashes)), flush=True)
    cohort(fixture, out / "control/resident.elf", oracle, out / "unaltered", kernel, qemu)
    negative_controls(out, kernel, measure, source, qemu, hashes["worker"])
    cohort(fixture, worker, oracle, out / "with-Core", kernel, qemu, module)


if __name__ == "__main__":
    main()
