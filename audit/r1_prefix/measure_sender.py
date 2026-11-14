"""Measure the Linux R1 sender at its first verified status line."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from measure import make_register


def sample(binary, source, anchor, wire):
    # Fill stdout before startup. The first write is the verified source head.
    # At that point all prefix processing has finished and the child is still alive.
    read_fd, write_fd = os.pipe()
    with os.fdopen(read_fd, "rb", buffering=0) as reader, os.fdopen(write_fd, "wb", buffering=0) as writer:
        capacity = fcntl.fcntl(writer, fcntl.F_SETPIPE_SZ, 4096)
        padding = b"x" * capacity
        writer.write(padding)
        with subprocess.Popen([binary, source, "--r1", wire, "--after", anchor], stdout=writer) as child:
            writer.close()
            deadline = time.monotonic() + 90
            try:
                while child.poll() is None:
                    if Path(f"/proc/{child.pid}/wchan").read_text().strip() in ("pipe_write", "anon_pipe_write"):
                        break
                    if time.monotonic() > deadline:
                        raise TimeoutError("could not observe the sender's first write")
                    time.sleep(0.002)
                else:
                    raise ValueError("sender exited before reporting a verified source")
                status = Path(f"/proc/{child.pid}/status").read_text()
                peak = int(next(s for s in status.splitlines() if s.startswith("VmHWM:")).split()[1])
                if reader.read(capacity) != padding:
                    raise ValueError("unexpected status pipe contents")
                report = reader.read()
                if child.wait() or b"chain intact" not in report:
                    raise ValueError(report)
            finally:
                if child.poll() is None:
                    child.kill()
    return peak, wire.read_bytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    binaries = [args.baseline.resolve(), args.candidate.resolve()]
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for count in (1000, 10000, 50000):
            source, base, anchor, target = make_register(root, count)
            wire = root / "update.r1"
            expected = None
            samples = [[], []]
            for order in [(0, 1), (1, 0), (0, 1)]:
                for i in order:
                    peak, frames = sample(binaries[i], source, anchor, wire)
                    if expected is None:
                        expected = frames
                    if frames != expected:
                        raise ValueError("wire mismatch")
                    samples[i].append(peak)
            restored = subprocess.check_output([binaries[1], "--from-r1", wire, target, "--base", base])
            if restored != source.read_bytes():
                raise ValueError("receiver did not reconstruct the source")
            print(json.dumps(dict(prefix_events=count, source_bytes=source.stat().st_size,
                                  wire_bytes=len(expected), baseline_KiB=samples[0],
                                  candidate_KiB=samples[1])), flush=True)


if __name__ == "__main__":
    main()
