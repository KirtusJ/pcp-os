"""Build the session worker and the pinned upstream text-generation control."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import runpy

HERE = Path(__file__).resolve().parent
FIXTURE = HERE.parent / "fixture"
TEXT_INPUT = HERE.parents[1] / "method-readback" / "text-input"
fixture = runpy.run_path(str(FIXTURE / "build.py"))

parser = argparse.ArgumentParser(description=__doc__)
for name in ("model", "tokenizer", "reference", "out"):
    parser.add_argument("--" + name, required=True, type=Path)
args = parser.parse_args()
model, vocab, upstream, out = (p.resolve() for p in (args.model, args.tokenizer, args.reference, args.out))
verified = fixture["verified"]
verified(model, fixture["CHECKPOINT"])
verified(vocab, "037cb335abb25d1fa9e8ecae30ed2a3a8ace9302862ebcdc05d51a6bbb10c312")
reference = verified(upstream, fixture["REFERENCE"])
out.mkdir()
(out / "run.c").write_bytes(reference)
cc = os.environ.get("CC", "cc")
for name, source, flags in (
        ("text-reference", HERE / "oracle.c", ["-std=gnu11", "-O2", "-ffp-contract=off"]),
        ("indexed-reference", FIXTURE / "reference.c", ["-std=gnu11", "-O2", "-ffp-contract=off"])):
    subprocess.run([cc, *flags, "-I", out, source, "-lm", "-o", out / name], check=True)
subprocess.run([cc, "-std=c11", "-O2", "-ffreestanding", "-fno-builtin", "-fno-stack-protector",
                "-fno-pie", "-mno-red-zone", "-c", TEXT_INPUT / "memory.c", "-o", out / "memory.o"], check=True)
subprocess.run([os.environ.get("RUSTC", "rustc"), "--edition=2021", "--cfg", "written_trial",
                "-Copt-level=2", "-Cpanic=abort", "--target", "x86_64-unknown-linux-gnu",
                "-Clinker=ld", "-Clinker-flavor=ld", "-Clink-arg=-static", "-Cdefault-linker-libraries=no",
                "-Cno-redzone=yes", "-Clto=fat", "-Ccode-model=large", "-Crelocation-model=static",
                f"-Clink-arg=-T{FIXTURE / 'link.ld'}", "-Clink-arg=--build-id=none", "-Cstrip=symbols",
                f"-Clink-arg={out / 'memory.o'}", FIXTURE / "main.rs", "-o", out / "resident.elf"],
               check=True, env=dict(os.environ, PCPOS_WORKER_CHECKPOINT=str(model), PCPOS_TOKENIZER=str(vocab)))
(out / "oracle.json").write_bytes(subprocess.check_output([out / "indexed-reference", model]))
cases = []
for prompt in (HERE / "prompts.txt").read_text().splitlines():
    if not 0 < len(prompt) <= 92 or not all(32 <= ord(c) < 127 for c in prompt):
        raise ValueError("invalid printed prompt")
    inp, output, stop, decoded = subprocess.check_output(
        [out / "text-reference", model, vocab, prompt], text=True).splitlines()
    cases.append(dict(prompt=prompt, input_ids=list(map(int, inp.split()[1:])),
                      output_ids=list(map(int, output.split()[1:])),
                      stop=stop.removeprefix("STOP "), decoded_hex=decoded.removeprefix("DECODED_HEX ")))
(out / "tok512.bin").write_bytes(vocab.read_bytes())
(out / "written.json").write_text(json.dumps(cases, indent=2) + "\n")
