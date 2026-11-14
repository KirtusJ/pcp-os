"""Public Windows stop hooks using temporary files, a real named-pipe server and a live writer."""
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = Path(sys.argv[1]).resolve()
kernel = ctypes.WinDLL("kernel32", use_last_error=True)
kernel.CreateNamedPipeW.argtypes = [wintypes.LPCWSTR] + [wintypes.DWORD]*6 + [wintypes.LPVOID]
kernel.CreateNamedPipeW.restype = wintypes.HANDLE
kernel.CloseHandle.argtypes = [wintypes.HANDLE]
invalid_handle = ctypes.c_void_p(-1).value
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    env = {**os.environ, "PCPOS_ROOT":tmp, "PCPOS_LOCK_WAIT":"0"}
    def hook(name, **data):
        return subprocess.run([binary,"hook",name],capture_output=True,text=True,input=json.dumps(data),
                              cwd=root,env=env,timeout=6,check=True)
    hook("prompt", hook_event_name="UserPromptExpansion", expansion_type="slash_command",
         command_name="pcp-os:review",command_args="x",prompt="x")
    transcript = root / "transcript"
    transcript.write_bytes(b"x\0y")
    def stop(label, path, message="", wanted=None):
        hook("subagent-start",agent_id=label,agent_type="fixture")
        hook("subagent-stop",agent_id=label,agent_transcript_path=str(path),last_assistant_message=message)
        row = json.loads((root/".pcp/register.jsonl").read_text(encoding="utf-8").splitlines()[-1])
        assert row["kind"] == "TASK_ENDED" and row["body"]["result"] == (
            hashlib.sha256(wanted).hexdigest() if wanted else None), row
        print("PASS Windows", label, row["body"]["outcome"], flush=True)
    stop("disk", transcript, wanted=b"x\0y")
    stop("absent", root / "absent")
    pipe = rf"\\.\pipe\pcpos-transcript-{os.getpid()}"
    server = kernel.CreateNamedPipeW(pipe, 2, 0, 1, 4096, 4096, 0, None)
    assert server != invalid_handle, ctypes.get_last_error()
    try:
        stop("real-idle-pipe", pipe)
        stop("message", pipe, "final", b"final")
    finally:
        kernel.CloseHandle(server)
    stop("UNC-pipe", rf"\\localhost\pipe\pcpos-transcript-{os.getpid()}")
    stop("device", "NUL")
    with transcript.open("r+b") as writer:
        stop("live-writer", transcript)
    stop("writer-closed", transcript, wanted=b"x\0y")
