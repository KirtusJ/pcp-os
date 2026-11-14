import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


scratch = Path(os.environ["RUNNER_TEMP"])
method = scratch / "method-packet" / "method"
hosts = {
    "base": scratch / "base/PCP_OS_Rust/target/release/pcpos",
    "head": Path.cwd() / "PCP_OS_Rust/target/release/pcpos",
}


def path_identity(binary):
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        reg = root / ".pcp/register.jsonl"
        reg.parent.mkdir()
        env = dict(os.environ, PCPOS_ROOT=str(root), PCPOS_METHOD=str(method))

        def run(*args):
            return subprocess.run([binary, *map(str, args)], cwd=root, env=env,
                                  capture_output=True, text=True, timeout=20)

        run("init", reg, "--principal", "p").check_returncode()
        src = root / "Specification.md"
        src.write_text("**Component:** `local.foo`, Version 1.0\n\nSynthetic.\n")
        add = run("module", "add", reg, src, "--by", "p")
        add.check_returncode()
        home = reg.parent / "modules"
        moved = home / "moved"
        (home / "foo").rename(moved)
        (home / "foo").mkdir()
        note = home / "foo/notes.txt"
        note.write_bytes(b"unrelated ordinary file\n")
        before = reg.read_bytes()
        result = run("module", "remove", reg, "foo", "--by", "p")
        observed = dict(exit=result.returncode, moved=(moved / "SOURCE/Specification.md").read_bytes() == src.read_bytes(),
                        note=note.exists() and note.read_bytes() == b"unrelated ordinary file\n",
                        register_unchanged=reg.read_bytes() == before, verifies=run("verify", reg).returncode == 0)
        print("path identity", json.dumps(observed, sort_keys=True), flush=True)
        return observed


def controls(name, binary):
    outcomes = {}
    for suite, check_key in [("native_host_contract", "pass_control"), ("native_nonmodule_contract", "pass")]:
        target = scratch / f"{name}-{suite}.json"
        result = subprocess.run([sys.executable, scratch / f"{suite}.py", "--pcpos", binary, "--method", method,
                                 "--expect-fixed", "--json", target], timeout=180)
        cases = json.loads(target.read_text())["cases"]
        if any(check_key not in case for case in cases.values()):
            raise RuntimeError(f"{name} {suite} has an inconclusive probe: {cases}")
        outcomes[suite] = {case: value[check_key] for case, value in cases.items()}
        print(name, suite, "conformance exit", result.returncode, json.dumps(outcomes[suite], sort_keys=True), flush=True)
        assert result.returncode == int(not all(outcomes[suite].values()))
    outcomes["path_identity"] = path_identity(binary)
    return outcomes


results = {name: controls(name, binary) for name, binary in hosts.items()}
print("BASE_AND_HEAD", json.dumps(results, sort_keys=True), flush=True)
for result in results.values():
    assert result["native_host_contract"]["normal"] and result["native_host_contract"]["artifact_read"]
    assert result["native_nonmodule_contract"]["normal"] and result["path_identity"]["moved"]
    assert result["path_identity"]["verifies"]
for suite in ["native_host_contract", "native_nonmodule_contract"]:
    for case, base_passed in results["base"][suite].items():
        if base_passed and not results["head"][suite][case]:
            raise RuntimeError(f"head failed a base-passing control: {suite}/{case}")
if results["base"]["path_identity"] != results["head"]["path_identity"]:
    raise RuntimeError("path identity behavior differs across revisions")
