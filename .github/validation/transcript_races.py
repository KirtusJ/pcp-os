"""Exercise public native hooks under controlled Linux path and lifecycle changes."""
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

bin_path, shim = map(lambda p: str(Path(p).resolve()), sys.argv[1:])
digest = lambda b: hashlib.sha256(b).hexdigest()

@contextmanager
def fixture():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        env = {**os.environ, "PCPOS_ROOT": tmp, "PCPOS_LOCK_WAIT": "0"}
        def hook(event, **data):
            p = subprocess.run([bin_path, "hook", event], env=env, cwd=root, input=json.dumps(data),
                               text=True, capture_output=True, timeout=5, check=True)
            assert "register is locked" not in p.stderr, p.stderr
            return p
        def bodies(kind):
            lines = (root / ".pcp/register.jsonl").read_text().splitlines()
            return [r["body"] for r in map(json.loads, lines) if r["kind"] == kind]
        hook("prompt", hook_event_name="UserPromptExpansion", expansion_type="slash_command",
             command_name="pcp-os:review", command_args="test", prompt="test")
        hook("subagent-start", agent_id="a", agent_type="worker")
        yield root, env, hook, bodies

@contextmanager
def delayed_stop(root, env, stage):
    target = root / "transcript"
    target.write_bytes(b"original")
    seen, release = root / "entered", root / "release"
    extra = dict(LD_PRELOAD=shim, PROBE_TARGET=str(target), PROBE_STAGE=stage,
                 PROBE_ENTERED=str(seen), PROBE_RELEASE=str(release))
    p = subprocess.Popen([bin_path, "hook", "subagent-stop"], cwd=root, env={**env, **extra},
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    p.stdin.write(json.dumps({"agent_id":"a", "agent_transcript_path":str(target)}).encode())
    p.stdin.close()
    try:
        deadline = time.monotonic() + 5
        while not seen.exists():
            assert p.poll() is None and time.monotonic() < deadline, (p.returncode, stage)
            time.sleep(.005)
        yield target
        release.touch()
        assert p.wait(timeout=5) == 0, p.stderr.read()
    finally:
        if p.poll() is None:
            p.kill()
            p.wait()

for stage, replacement in (("before", "fifo"), ("before", "other-inode"), ("before", "link"),
                           ("after", "other-inode"), ("after", "unlinked")):
    with fixture() as (root, env, hook, bodies):
        with delayed_stop(root, env, stage) as path:
            original = root / "original"
            path.rename(original)
            if replacement == "fifo": os.mkfifo(path)
            elif replacement == "other-inode":
                path.write_bytes(b"imposter")
                # Equal size and mtime. Only object identity can distinguish these.
                os.utime(path, ns=(original.stat().st_atime_ns, original.stat().st_mtime_ns))
            elif replacement == "link": path.symlink_to(original)
            hook("subagent-start", agent_id="b", agent_type="independent")
            assert [r["id"] for r in bodies("TASK_STARTED")] == ["T1", "T2"]
        assert [(r["id"], r["outcome"]) for r in bodies("TASK_ENDED")] == [("T1", "FAILED")]
        assert not [r for r in bodies("ARTIFACT_ADDED") if r.get("task") == "T1"]
    print("PASS", stage, replacement, "independent start committed while observation was held", flush=True)

with fixture() as (root, env, hook, bodies):
    with delayed_stop(root, env, "after"):
        hook("prompt", hook_event_name="UserPromptSubmit", prompt="stop")
        assert bodies("HALTED")
    assert [(r["id"], r["outcome"]) for r in bodies("TASK_ENDED")] == [("T1","STOPPED")]
    assert not [r for r in bodies("ARTIFACT_ADDED") if r.get("task") == "T1"]
    print("PASS halt during regular-file observation", flush=True)

with fixture() as (root, env, hook, bodies):
    with delayed_stop(root, env, "after"):
        hook("subagent-stop", agent_id="a", last_assistant_message="earlier exit")
        hook("subagent-start", agent_id="a", agent_type="resumed")
    assert [(r["id"], r["outcome"]) for r in bodies("TASK_ENDED")] == [("T1","DONE")]
    assert [r["id"] for r in bodies("TASK_STARTED")] == ["T1", "T2"]
    hook("subagent-stop", agent_id="a", last_assistant_message="its own exit")
    assert [(r["task"], r["sha256"]) for r in bodies("ARTIFACT_ADDED") if "task" in r] == [
        ("T1", digest(b"earlier exit")), ("T2", digest(b"its own exit"))]
    print("PASS competing completion followed by resume; original observer cannot credit resumed task", flush=True)
