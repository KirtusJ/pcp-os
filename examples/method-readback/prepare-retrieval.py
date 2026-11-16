"""Prepare the released Build 22 retrieval tool in a new assessor directory."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

MANIFEST_SHA256 = "7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707"


def prepare(method, lookup, output):
    manifest = (method / "MANIFEST.json").read_bytes()
    if hashlib.sha256(manifest).hexdigest() != MANIFEST_SHA256:
        raise ValueError("expected the Build 22 method manifest")
    files = {row["path"]: row for row in json.loads(manifest)["files"]}
    tools = {
        "TOOLS/acr.py": lookup,
        "TOOLS/calibrate.py": Path(__file__).resolve().parents[2] / "build/method/calibrate_tool.py",
    }
    paths = [path for path in files
             if path.startswith("MODULES/") and path.endswith("/SOURCE/Specification.md")]
    paths.extend(["REFERENCE/Defined_Terms.csv", *tools])
    payload = {"MANIFEST.json": manifest}
    for path in paths:
        data = tools.get(path, method / path).read_bytes()
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
    args = parser.parse_args()
    try:
        prepare(args.method, args.lookup, args.out)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    print(f"Build 22 retrieval inputs ready in {args.out}")


if __name__ == "__main__":
    main()
