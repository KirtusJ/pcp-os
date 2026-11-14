"""Exercise public native hooks under controlled Linux path and lifecycle changes."""
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
def fixture(review=False):
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
        if review:
            target = digest(b"target")
            records = [("ARTIFACT_ADDED", dict(origin="SUPPLIED", sha256=target, size=6)),
                       ("REVIEW_COMMISSIONED", dict(claims=[dict(label="A", origin="COMMISSIONER", text="target")],
                        director="principal", extent="TOTAL", label="R", procedure="FOCUSED", target={"sha256":target,"t":"artifact"}))]
            requests = [dict(id=i, jsonrpc="2.0", method="tools/call",
                             params=dict(name="acr_submit", arguments=dict(kind=kind,body=b)))
                        for i,(kind,b) in enumerate(records,1)]
            p = subprocess.run([bin_path,"mcp"], cwd=root, env=env, text=True, capture_output=True, timeout=5,
                               input="".join(json.dumps(req)+"\n" for req in requests), check=True)
            assert p.stdout.count("WRITTEN") == 2, p.stdout
        hook("subagent-start", agent_id="a", agent_type="worker")
        if review: assert bodies("TASK_STARTED")[-1]["review"] == "R"
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

with fixture(review=True) as (root, env, hook, bodies):
    with delayed_stop(root, env, "after"):
        hook("prompt", hook_event_name="UserPromptSubmit", prompt="stop")
        assert bodies("HALTED")
    assert [(r["id"], r["outcome"]) for r in bodies("TASK_ENDED")] == [("T1","STOPPED")], bodies("TASK_ENDED")
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

with fixture() as (root, env, hook, bodies):
    with delayed_stop(root, env, "after"):
        reg = root / ".pcp/register.jsonl"
        reg.rename(root / "archived-register")
        # A second independent synthetic review reuses both the hook key and T1.
        hook("prompt", hook_event_name="UserPromptExpansion", expansion_type="slash_command",
             command_name="pcp-os:review", command_args="new review", prompt="new review")
        hook("subagent-start", agent_id="a", agent_type="replacement")
        assert bodies("TASK_STARTED")[-1]["id"] == "T1"
        untouched = reg.read_bytes()
    assert reg.read_bytes() == untouched
    print("PASS new register with reused task and hook IDs is not credited", flush=True)

for mode in ("disabled", "unmatched", "ambiguous"):
    with fixture() as (root, env, hook, bodies):
        reg = root / ".pcp/register.jsonl"
        if mode == "disabled":
            reg.unlink()  # only this temporary fixture's synthetic register
        elif mode == "ambiguous":
            # Replay two historical observed tasks in the pre-correlation-key format.
            lines = reg.read_bytes().splitlines()
            start = json.loads(lines[-1])
            assert start["kind"] == "TASK_STARTED"
            del start["body"]["hook_attempt_key"]
            canonical = lambda v: json.dumps(v, separators=(",", ":"), sort_keys=True).encode()
            lines[-1] = canonical(start)
            start["prev"] = digest(lines[-1]); start["seq"] += 1; start["body"]["id"] = "T2"
            lines.append(canonical(start))
            reg.write_bytes(b"\n".join(lines) + b"\n")
            verify = subprocess.run([bin_path, "verify", str(reg)], cwd=root, env=env, capture_output=True, text=True)
            assert "guards replayed: 0 problem(s)" in verify.stdout, verify.stdout
        snapshot = reg.read_bytes() if reg.exists() else None
        target = root/"transcript"; target.write_bytes(b"regular")
        entered = root/"should-not-open"
        probe = {**env, "LD_PRELOAD":shim, "PROBE_TARGET":str(target), "PROBE_STAGE":"before",
                 "PROBE_ENTERED":str(entered), "PROBE_RELEASE":str(root/"never-released")}
        data = dict(agent_id="other" if mode == "unmatched" else "a", agent_transcript_path=str(target))
        subprocess.run([bin_path, "hook", "subagent-stop"], cwd=root, env=probe, input=json.dumps(data),
                       text=True, capture_output=True, timeout=5, check=True)
        assert not entered.exists() and (reg.read_bytes() if reg.exists() else None) == snapshot
    print("PASS", mode, "stop never opened transcript", flush=True)
