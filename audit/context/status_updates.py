"""Measure conditional full status and opt-in updates on every prefix of a published register."""
import argparse
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from measure import canonical, counters


class Client:
    def __init__(self, child, measures):
        self.child = child
        self.measures = measures
        self.wire = 0
        self.model = {name: 0 for name in measures}
        self.calls = 0

    def request(self, method, params):
        self.calls += 1
        message = {"id": self.calls, "jsonrpc": "2.0", "method": method, "params": params}
        request = (canonical(message) + "\n").encode()
        self.child.stdin.write(request)
        self.child.stdin.flush()
        response = self.child.stdout.readline()
        parsed = json.loads(response)
        if parsed["id"] != self.calls:
            raise ValueError("reply ID mismatch")
        self.wire += len(request) + len(response)
        result = parsed["result"]
        if method == "tools/list":
            fields = [canonical(result["tools"])]
        else:
            if result["isError"]:
                raise ValueError(result)
            fields = [params["name"], canonical(params["arguments"]), result["content"][0]["text"]]
        for name, measure in self.measures.items():
            self.model[name] += sum(measure(field) for field in fields)
        return fields[-1]


def run(root, binaries, lines, measures, fixture, start=1):
    register = root / ".pcp/register.jsonl"
    register.parent.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith(("PCPOS_", "CLAUDE_"))}
    env["PCPOS_ROOT"] = str(root)
    with ExitStack() as stack:
        clients = []
        for binary in binaries:
            child = stack.enter_context(subprocess.Popen([binary, "mcp"], cwd=root, env=env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL))
            stack.callback(child.stdin.close)
            client = Client(child, measures)
            client.request("tools/list", {})
            clients.append(client)
        head = None
        body = None
        headers = 0
        for count, line in enumerate(lines, 1):
            with register.open("ab") as writer:
                writer.write(line)
            if count < start:
                continue
            options = {"if_head": head} if head is not None else {}
            replies = []
            for i, client in enumerate(clients):
                args = dict(options, view="update") if i else options
                replies.append(client.request("tools/call", {"name": "acr_status", "arguments": args}))
            head = hashlib.sha256(line[:-1]).hexdigest()
            header = f"REGISTER  .pcp/register.jsonl  |  {count} events  |  head {head}\n"
            full, update = replies
            if not full.startswith(header):
                raise ValueError(f"{fixture}, event {count}: unexpected full header")
            if update.startswith("["):
                if body is None or json.loads(update) != [count, head]:
                    raise ValueError(f"{fixture}, event {count}: unexpected update")
                update = header + body
                headers += 1
            if update != full:
                raise ValueError(f"{fixture}, event {count}: reconstruction differs")
            body = full[len(header):]
        for label, client in zip(("full", "update"), clients):
            print(fixture, label, len(lines) - start + 1, headers if label == "update" else 0, client.wire,
                  *client.model.values(), sep="\t")
            client.child.stdin.close()
            if client.child.wait():
                raise ValueError("host exited abnormally")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--tokens", action="store_true")
    args = parser.parse_args()
    measures = counters(args.tokens)
    binaries = [p.resolve() for p in (args.baseline, args.candidate)]
    vectors = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/vectors"
    print("fixture", "view", "reads", "header_updates", "RPC_bytes", *measures, sep="\t")
    for fixture in ("register_scenario", "register_fphpm_exercise", "register_reference"):
        lines = (vectors / (fixture + ".jsonl")).read_bytes().splitlines(keepends=True)
        with tempfile.TemporaryDirectory() as temp:
            run(Path(temp), binaries, lines, measures, fixture)
        if fixture == "register_scenario":
            with tempfile.TemporaryDirectory() as temp:
                run(Path(temp), binaries, lines, measures, "scenario_77_to_82", start=77)


if __name__ == "__main__":
    main()
