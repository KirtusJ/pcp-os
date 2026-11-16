"""Freeze a completed round from the session runner while the guests continue."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

cycle = int(sys.argv[1])
run = Path("receipts/session")
snapshot = run / "readback.json"
pid = int((Path(os.environ["RUNNER_TEMP"]) / "d9-supervisor.pid").read_text())
deadline = time.monotonic() + (600 if cycle == 1 else 1800)
while True:
    if snapshot.exists() and json.loads(snapshot.read_bytes())["cycle"] >= cycle:
        break
    os.kill(pid, 0)
    if time.monotonic() >= deadline:
        sys.exit(f"Timed out before round {cycle}")
    time.sleep(2)

destination = Path("receipts/checkpoints") / str(cycle)
destination.mkdir(parents=True)
archive = destination / "readback.zip"
result = subprocess.check_output([
    sys.executable, "-O", str(Path(__file__).with_name("archive.py")),
    "--run", str(run), "--oracle", "receipts/control/oracle.json",
    "--cycle", str(cycle), "--output", str(archive),
])
(destination / "verified.json").write_bytes(result)
with archive.open("rb") as stream:
    digest = hashlib.file_digest(stream, "sha256").hexdigest()
(destination / "readback.sha256").write_text(f"{digest}  readback.zip\n")
print(result.decode().rstrip(), flush=True)
