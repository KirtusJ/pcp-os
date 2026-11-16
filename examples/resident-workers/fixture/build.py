"""Build the native QA worker and cross-check its 16 prompts with upstream llama2.c."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
CHECKPOINT = "b0a507e7ad0f626624f17112325e66691f9076d622e1d3274d103d00299f2696"
REFERENCE = "9c4f2d5c6ae01b71726d1cc37530d71e60bff0ec7cc012565f16a43c1ca658bd"
LINKER_SCRIPT = HERE / "link.ld"


def verified(path, digest):
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"{path}: SHA-256 mismatch")
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--reference", required=True, type=Path, help="pinned upstream run.c")
    parser.add_argument("--out", required=True, type=Path, help="new output directory")
    args = parser.parse_args()
    model = verified(args.model, CHECKPOINT)
    if (len(model) != 1_056_540 or
            struct.unpack_from("<7i", model) != (64, 172, 5, 8, 4, 512, 512)):
        raise ValueError("unexpected checkpoint layout")
    reference = verified(args.reference, REFERENCE)
    output = args.out.resolve()
    if output.exists():
        raise FileExistsError(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        work = Path(temp)
        checkpoint = work / "checkpoint.bin"
        checkpoint.write_bytes(model)
        (work / "run.c").write_bytes(reference)
        env = dict(os.environ, PCPOS_WORKER_CHECKPOINT=str(checkpoint))
        rustc = os.environ.get("RUSTC", "rustc")
        subprocess.run([os.environ.get("CC", "cc"), "-std=gnu11", "-O2", "-ffp-contract=off",
                        "-I", work, HERE / "reference.c", "-lm", "-o", work / "oracle"], check=True)
        oracle = subprocess.check_output([work / "oracle", checkpoint])
        flags = [rustc, "--edition=2021", "-Copt-level=2", "-Cpanic=abort"]
        subprocess.run([*flags, HERE / "check.rs", "-o", work / "check"], env=env, check=True)
        actual = json.loads(subprocess.check_output([work / "check"]))
        if actual != json.loads(oracle):
            raise RuntimeError("native inference disagrees with the upstream reference")
        ready = work / "ready"
        ready.mkdir()
        # No OS libraries, interpreter, or runtime startup objects enter this ELF.
        # PCP userspace supports the normal x86-64 SysV floating-point ABI.
        # The kernel's own soft-float target is inappropriate for this ELF.
        subprocess.run([*flags, "--target", "x86_64-unknown-linux-gnu", "-Clinker=ld",
                        "-Clinker-flavor=ld", "-Clink-arg=-static", "-Cdefault-linker-libraries=no",
                        "-Cno-redzone=yes", "-Clto=fat",
                        "-Ccode-model=large", "-Crelocation-model=static",
                        f"-Clink-arg=-T{LINKER_SCRIPT}", "-Clink-arg=--build-id=none", "-Cstrip=symbols",
                        HERE / "main.rs", "-o", ready / "resident.elf"], env=env, check=True)
        (ready / "oracle.json").write_bytes(oracle)
        ready.rename(output)


if __name__ == "__main__":
    main()
