"""Exercise a source-selected prompt through the PCP OS guest against upstream llama2.c."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
FIXTURE = ROOT / "examples/resident-workers/fixture"
KERNEL = ROOT / "PCP_OS_Rust/crates/pcpos-kernel"
MANIFEST = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707"
CORE = "MODULES/01_Core/SOURCE/Specification.md"
TOKENIZER = "037cb335abb25d1fa9e8ecae30ed2a3a8ace9302862ebcdc05d51a6bbb10c312"
SENTENCES = (b"Any participant MAY halt its own participation.",
             b"A Review Session is a bounded assessment cycle.")

sys.path.insert(0, str(FIXTURE))
from build import CHECKPOINT, REFERENCE, verified


def sha(data):
    return hashlib.sha256(data).hexdigest()


def require(ok, message):
    if not ok:
        raise ValueError(message)


def build(out, model, tokenizer, reference):
    env = dict(os.environ, PCPOS_WORKER_CHECKPOINT=str(model), PCPOS_TOKENIZER=str(tokenizer))
    (out / "run.c").write_bytes(reference)
    cc = os.environ.get("CC", "cc")
    subprocess.run([cc, "-std=gnu11", "-O2", "-ffp-contract=off", "-I", out,
                    HERE / "oracle.c", "-lm", "-o", out / "oracle"], check=True)
    subprocess.run([cc, "-std=c11", "-O2", "-ffreestanding", "-fno-builtin", "-fno-stack-protector",
                    "-fno-pie", "-mno-red-zone", "-c", HERE / "memory.c", "-o", out / "memory.o"], check=True)
    subprocess.run([os.environ.get("RUSTC", "rustc"), "--edition=2021", "-Copt-level=2", "-Cpanic=abort",
                    "--target", "x86_64-unknown-linux-gnu", "-Clinker=ld", "-Clinker-flavor=ld",
                    "-Clink-arg=-static", "-Cdefault-linker-libraries=no", "-Cno-redzone=yes",
                    "-Clto=fat", "-Ccode-model=large", "-Crelocation-model=static",
                    f"-Clink-arg=-T{FIXTURE / 'link.ld'}", "-Clink-arg=--build-id=none", "-Cstrip=symbols",
                    f"-Clink-arg={out / 'memory.o'}", HERE / "guest.rs", "-o", out / "text.elf"],
                   check=True, env=env)


def boot(case, executable, kernel, measure, payload, offset, length, expected, status, qemu, firmware):
    case.mkdir()
    (case / "text.elf").hardlink_to(executable)
    modules = f"text.elf {offset} {length}"
    if payload is not None:
        (case / "core-spec.md").write_bytes(payload)
        modules += ",core-spec.md"
    cmd = [qemu]
    if firmware is not None:
        cmd.extend(["-L", firmware.resolve()])
    cmd.extend(["-machine", "microvm,isa-serial=on,pic=on,pit=on,rtc=on", "-device", "i8042",
                "-accel", "tcg,tb-size=4", "-icount", "shift=3,align=off,sleep=off", "-cpu", "max",
                "-m", "32M", "-nodefaults", "-nic", "none", "-display", "none", "-monitor", "none",
                "-serial", "stdio", "-no-reboot", "-kernel", kernel, "-initrd", modules,
                "-append", f"auto=text.elf then=off image={measure}",
                "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
    with (case / "boot.serial").open("wb") as serial, (case / "qemu.stderr").open("wb") as errors:
        result = subprocess.run(cmd, cwd=case, stdout=serial, stderr=errors, timeout=180)
    data = (case / "boot.serial").read_bytes()
    lines = data.decode("ascii").splitlines()
    entry = f"starting text.elf ({executable.stat().st_size} bytes, SHA-256 {sha(executable.read_bytes())}) with: {offset} {length}"
    require(result.returncode == 33 and lines.count(entry) == 1, f"{case.name}: guest startup")
    start = lines.index(entry)
    reports = [line for line in lines[:start] if line.startswith(("protections:", "running image ("))]
    require(reports == ["protections: paging true, supervisor write protection true, no-execute true, long mode true",
                        f"running image (code and read-only data), SHA-256: {measure} (measured; equal to the value the loader named)"],
            f"{case.name}: kernel measurement or protections")
    require(any(line.startswith("ALL SELF-TESTS PASS.") for line in lines)
            and not any(line.startswith("  FAIL") for line in lines), f"{case.name}: self-tests")
    after = lines[start + 1:]
    ends = [i for i, line in enumerate(after) if line.startswith("text.elf ended ")]
    require(len(ends) == 1, f"{case.name}: guest exit unavailable")
    finish = ends[0]
    kind = "DONE" if status == 0 else "FAILED"
    require(re.match(rf"^text\.elf ended {kind}: exit {status}\b", after[finish]), f"{case.name}: guest exit")
    require(after[:finish] == expected, f"{case.name}: guest text, tokens or prediction; see {case}/boot.serial")
    return {"exit": status, "serial_sha256": sha(data), "module_sha256": sha(payload) if payload is not None else None}


def run(args):
    manifest = verified(args.method / "MANIFEST.json", MANIFEST)
    row, = [r for r in json.loads(manifest)["files"] if r["path"] == CORE]
    source = verified(args.method / CORE, row["sha256"])
    model, tokenizer, kernel = (p.resolve(strict=True) for p in (args.model, args.tokenizer, args.kernel))
    verified(model, CHECKPOINT)
    verified(tokenizer, TOKENIZER)
    reference = verified(args.reference, REFERENCE)
    measure = subprocess.check_output([sys.executable, KERNEL / "tools/measure.py", kernel], text=True).strip()
    out = args.out.resolve()
    out.mkdir()
    build(out, model, tokenizer, reference)
    first, second = SENTENCES
    require(len(first) == len(second) and all(source.count(s) == 1 for s in SENTENCES), "unexpected source excerpts")
    offsets = [source.index(s) for s in SENTENCES]
    length = len(first)
    records = {}
    for name, payload, offset, excerpt, code in [
        ("original", source, offsets[0], first, 0),
        ("alternate", source, offsets[1], second, 0),
        ("same-offset-control", source.replace(first, second), offsets[0], second, 0),
        ("missing", None, offsets[0], None, 2),
        ("truncated", source[:offsets[0] + length - 1], offsets[0], None, 4),
    ]:
        expected = []
        record = {"offset": offset, "length": length}
        if excerpt is not None:
            ref = subprocess.check_output([out / "oracle", model, tokenizer, excerpt.decode("ascii")], text=True)
            inputs, predicted = ref.splitlines()
            require(re.fullmatch(r"INPUT 1(?: \d+)+", inputs) and re.fullmatch(r"OUTPUT(?: \d+){3}", predicted),
                    "unexpected oracle output")
            expected = ["EXCERPT " + excerpt.decode("ascii"), inputs, predicted]
            record.update(text=excerpt.decode("ascii"), excerpt_sha256=sha(excerpt),
                          tokens=list(map(int, inputs.split()[1:])), prediction=list(map(int, predicted.split()[1:])))
        record.update(boot(out / name, out / "text.elf", kernel, measure, payload, offset, length,
                           expected, code, args.qemu, args.firmware))
        records[name] = record
        print(name, json.dumps(record), flush=True)
    evidence = dict(manifest=MANIFEST, core=row["sha256"], model=CHECKPOINT, tokenizer=TOKENIZER,
                    upstream=REFERENCE, kernel_sha256=sha(kernel.read_bytes()), measured_kernel=measure,
                    executable_sha256=sha((out / "text.elf").read_bytes()), cases=records)
    (out / "results.json").write_text(json.dumps(evidence, indent=2) + "\n")
    require(records["original"]["prediction"] != records["alternate"]["prediction"], "outputs did not distinguish the sentences")
    require(records["alternate"]["prediction"] == records["same-offset-control"]["prediction"], "substitution control failed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for opt in ("model", "reference", "tokenizer", "method", "kernel", "out"):
        parser.add_argument("--" + opt, required=True, type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--firmware", type=Path)
    run(parser.parse_args())
