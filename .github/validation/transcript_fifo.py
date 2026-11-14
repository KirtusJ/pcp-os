import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

binary = Path("PCP_OS_Rust/target/release/pcpos").resolve()
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    env = {**os.environ, "PCPOS_ROOT": tmp, "PCPOS_LOCK_WAIT": "0"}
    def hook(kind, data):
        return subprocess.run([binary, "hook", kind], cwd=root, env=env,
                              input=json.dumps({"cwd": tmp, **data}), text=True,
                              capture_output=True, timeout=3)
    hook("prompt", {"hook_event_name": "UserPromptExpansion", "expansion_type": "slash_command",
                    "command_name": "pcp-os:review", "command_args": "test", "prompt": "test"}).check_returncode()
    hook("subagent-start", {"agent_id": "a", "agent_type": "test"}).check_returncode()
    fifo = root / "transcript"
    os.mkfifo(fifo)
    stop = subprocess.Popen([binary, "hook", "subagent-stop"], cwd=root, env=env, stdin=subprocess.PIPE,
                            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        stop.stdin.write(json.dumps({"cwd": tmp, "agent_id": "a", "agent_transcript_path": str(fifo)}))
        stop.stdin.close()
        deadline = time.monotonic() + 6
        blocked = False
        while time.monotonic() < deadline and stop.poll() is None:
            if 'wait_for_partner' in Path(f"/proc/{stop.pid}/wchan").read_text():
                blocked = True
                break
            time.sleep(.02)
        print("STOP observed waiting on FIFO:", blocked, flush=True)
        assert blocked
        args = {"agent_id": "b", "agent_type": "new-live-task"}
        other = hook("subagent-start", args)
        raw = (root / ".pcp/register.jsonl").read_text()
        assert '"new-live-task"' not in raw
        print("Concurrent hook code and stderr:", other.returncode, other.stderr, flush=True)
        assert 'lock' in other.stderr.lower()
        with fifo.open("w") as writer:
            writer.write("synthetic completion")
        assert stop.wait(timeout=5) == 0
        hook("subagent-start", args).check_returncode()
        raw = (root / ".pcp/register.jsonl").read_text()
        assert '"new-live-task"' in raw
        print("Same hook recorded after FIFO writer released previous stop", flush=True)
    finally:
        if stop.poll() is None:
            stop.kill()
            stop.wait()
