"""Prepare a verified retrieval scorer and Build 22 sources in a new assessor directory."""
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

MANIFEST_SHA256 = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707"
REACH_CHECKED_SHA256 = "3138c84bc5db7daeb09d9ebfca4f9dcbb75c8b6083473fd74bfbe9c164048c49"
HERE = Path(__file__).resolve().parent
SCORER = HERE.parents[1] / "build/method/calibrate_tool.py"


def scorer_bytes(variant, released_digest):
    released = SCORER.read_bytes()
    if hashlib.sha256(released).hexdigest() != released_digest:
        raise ValueError("Build 22 digest mismatch for TOOLS/calibrate.py")
    if variant == "build22":
        return released

    with tempfile.TemporaryDirectory() as scratch:
        root = Path(scratch)
        tool = root / "TOOLS/calibrate.py"
        tool.parent.mkdir()
        tool.write_bytes(released)
        applied = subprocess.run(["git", "apply", "--include=TOOLS/calibrate.py",
                                  HERE / "retrieval-reach.patch"], cwd=root, capture_output=True, text=True)
        if applied.returncode:
            raise ValueError(f"could not apply retrieval scoring correction: {applied.stderr.strip()}")
        corrected = tool.read_bytes()
    if hashlib.sha256(corrected).hexdigest() != REACH_CHECKED_SHA256:
        raise ValueError("digest mismatch for reach-checked scorer")
    return corrected


def prepare(method, lookup, output, variant):
    manifest = (method / "MANIFEST.json").read_bytes()
    if hashlib.sha256(manifest).hexdigest() != MANIFEST_SHA256:
        raise ValueError("expected the Build 22 method manifest")
    files = {row["path"]: row for row in json.loads(manifest)["files"]}
    generator = "TOOLS/calibrate.py"
    released_digest = files[generator]["sha256"]
    scorer = scorer_bytes(variant, released_digest)
    paths = [path for path in files
             if path.startswith("MODULES/") and path.endswith("/SOURCE/Specification.md")]
    paths.extend(["REFERENCE/Defined_Terms.csv", "TOOLS/acr.py"])
    payload = {"MANIFEST.json": manifest, generator: scorer}
    for path in paths:
        data = (lookup if path == "TOOLS/acr.py" else method / path).read_bytes()
        if hashlib.sha256(data).hexdigest() != files[path]["sha256"]:
            raise ValueError(f"Build 22 digest mismatch for {path}")
        payload[path] = data

    # Copy only the verified input bytes. A module in the caller's checkout
    # cannot join the tool's source discovery or become an executable import.
    output.mkdir(mode=0o700)
    try:
        for path, data in payload.items():
            destination = output / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
    except OSError:
        shutil.rmtree(output)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--method", type=Path, required=True, help="installed Build 22 method directory")
    parser.add_argument("--lookup", type=Path, required=True, help="Build 22 text adapter's method/TOOLS/acr.py")
    parser.add_argument("--out", type=Path, required=True, help="new assessor directory")
    parser.add_argument("--scorer", choices=("build22", "reach-checked"), default="build22")
    args = parser.parse_args()
    try:
        prepare(args.method, args.lookup, args.out, args.scorer)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    print(f"Build 22 retrieval inputs ready in {args.out} (scorer: {args.scorer})")


if __name__ == "__main__":
    main()
