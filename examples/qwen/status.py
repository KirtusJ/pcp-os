"""Read the published exercise through the Rust MCP host and report it with local Qwen."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def ask(runner, model, question):
    command = [
        runner, "-m", model, "--conversation", "--single-turn", "--simple-io",
        "--no-display-prompt", "--no-warmup", "--temp", "0", "-t", "3",
        "-c", "2048", "-n", "60", "-p", question,
    ]
    response = subprocess.run(command, capture_output=True, text=True, encoding="utf-8",
                              errors="replace", check=True, timeout=180)
    return response.stdout.strip().removesuffix("[end of text]").strip()


def call_status(pcpos, workspace):
    env = {**os.environ, "PCPOS_ROOT": str(workspace), "PCPOS_REGISTER": "exercise.jsonl",
           "PCPOS_ACTOR": "qwen"}
    with subprocess.Popen([pcpos, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", env=env) as host:
        def send(message):
            host.stdin.write(json.dumps({"jsonrpc": "2.0", **message}) + "\n")
            host.stdin.flush()

        def receive(expected_id):
            result = json.loads(host.stdout.readline())
            if result.get("id") != expected_id or "error" in result:
                raise ValueError(f"Unexpected MCP reply: {result}")
            return result["result"]

        send({"id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "qwen-status-example", "version": "1"},
        }})
        if receive(1)["serverInfo"]["name"] != "pcpos":
            raise ValueError("Unexpected MCP host")
        send({"method": "notifications/initialized"})
        send({"id": 2, "method": "tools/call", "params": {"name": "acr_status", "arguments": {}}})
        reply = receive(2)
    if reply.get("isError"):
        raise ValueError(reply["content"])
    return "\n".join(part["text"] for part in reply["content"] if part["type"] == "text")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pcpos", required=True, type=Path)
    parser.add_argument("--runner", required=True, type=Path, help="llama-completion executable")
    parser.add_argument("--model", required=True, type=Path, help="Qwen instruction-tuned GGUF")
    args = parser.parse_args()
    runner = args.runner.resolve(strict=True)
    model = args.model.resolve(strict=True)
    choice = ask(runner, model, "The available read-only tool is acr_status. Respond only with "
                 "the exact tool name if we need to inspect the number of events in the PCP register.")
    if choice != "acr_status":
        raise ValueError(f"The model did not request acr_status. Its reply was {choice!r}")
    print("Model requested", choice)
    fixture = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors/register_fphpm_exercise.jsonl"
    with tempfile.TemporaryDirectory() as directory:
        workspace = Path(directory)
        register = workspace / "exercise.jsonl"
        original = fixture.read_bytes()
        register.write_bytes(original)
        status = call_status(args.pcpos.resolve(strict=True), workspace)
        if register.read_bytes() != original:
            raise RuntimeError("The copied register changed")
    print(status)
    answer = ask(runner, model, "Read the REGISTER line of this PCP tool result. How many "
                 "events are in the register? Answer with just the number.\n\n" + status)
    print("Model reply:", answer)


if __name__ == "__main__":
    main()
