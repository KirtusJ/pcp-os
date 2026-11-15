"""Recover one complete engine source tree from the original common archive and game patch."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tarfile


GAMES = Path(__file__).resolve().parents[2]
VARIANTS = {
    "ultra-omega-barrel": "omega",
    "barrel-bowl": "bowl",
    "midnight-barrel-express": "rail",
    "forklift-certified": "fork",
}


def stage(game, destination):
    destination.mkdir(parents=True, exist_ok=False)
    archive = GAMES / "ultra-omega-barrel/source/omega-source.tar.gz"
    with tarfile.open(archive, "r:gz") as source:
        source.extractall(destination, filter="data")
    root = destination / "omega-source"
    if game != "ultra-omega-barrel":
        variant = GAMES / game / "source"
        patch = variant / (VARIANTS[game] + ".patch")
        # Use a fresh work tree even when the caller puts the output inside a source checkout.
        subprocess.run(["git", "init", "--quiet", str(root / "engine")], check=True)
        subprocess.run(["git", "-c", "safe.directory=" + (root / "engine").resolve().as_posix(),
                        "-c", "core.autocrlf=false", "apply", "--whitespace=nowarn", str(patch)],
                       cwd=root / "engine", check=True)
        (root / "BUILD.txt").rename(root / "OMEGA-BUILD.txt")
        for name in ("BUILD.txt", "build_pcpos.py", patch.name):
            shutil.copyfile(variant / name, root / name)
    return root


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game", choices=VARIANTS)
    parser.add_argument("new_directory", type=Path)
    args = parser.parse_args()
    print(stage(args.game, args.new_directory))
