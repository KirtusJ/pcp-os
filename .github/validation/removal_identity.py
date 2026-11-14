from pathlib import Path
import hashlib
import json
import os
import subprocess
import tempfile


binary = Path.cwd() / "PCP_OS_Rust/target/release/pcpos"
print("RELEASE_SHA256", hashlib.sha256(binary.read_bytes()).hexdigest(), flush=True)
foo = b"**Component:** `local.foo`, Version 1.0\n\nSynthetic registered source.\n"
other = b"**Component:** `local.other`, Version 1.0\n\nUnrelated module.\n"


def probe(case):
    with tempfile.TemporaryDirectory(prefix="module-removal-") as folder:
        root = Path(folder)
        method = root / "method"
        core = method / "MODULES/01_Core/SOURCE/Specification.md"
        core.parent.mkdir(parents=True)
        core.write_text("**Component:** `acr.core`, Version 1.0\n\nSynthetic Core.\n")
        reg = root / ".pcp/register.jsonl"
        env = dict(os.environ, PCPOS_ROOT=str(root), PCPOS_METHOD=str(method))

        def run(*args):
            return subprocess.run([binary, *map(str, args)], cwd=root, env=env,
                                  capture_output=True, text=True, timeout=15)

        run("init", reg, "--principal", "p").check_returncode()
        src = root / "Specification.md"
        src.write_bytes(foo)
        run("module", "add", reg, src, "--by", "p").check_returncode()
        old = reg.parent / "modules/foo"
        moved = old.with_name("moved")
        if case.startswith("moved"):
            old.rename(moved)
            old.mkdir()
        elif case.startswith("historical"):
            run("module", "remove", reg, "foo", "--by", "p").check_returncode()
            assert not old.exists()
            old.mkdir()
        if case == "moved_other":
            (old / "SOURCE").mkdir()
            (old / "SOURCE/Specification.md").write_bytes(other)
        if case == "historical_valid":
            (old / "SOURCE").mkdir()
            (old / "SOURCE/Specification.md").write_bytes(foo)
        note = old / "notes.txt"
        if case != "ordinary":
            note.write_bytes(b"unrelated\n")
        before = reg.read_bytes()
        result = run("module", "remove", reg, "foo", "--by", "p")
        after = reg.read_bytes()
        events = [json.loads(line)["body"] for line in after[len(before):].splitlines()]
        obs = dict(exit=result.returncode, directory=old.exists(), note=note.exists() and note.read_bytes() == b"unrelated\n",
                   moved=(moved / "SOURCE/Specification.md").is_file() and (moved / "SOURCE/Specification.md").read_bytes() == foo,
                   other=(old / "SOURCE/Specification.md").is_file() and (old / "SOURCE/Specification.md").read_bytes() == other,
                   stable=(before == after), added=[b.get("event") for b in events],
                   verifies=run("verify", reg).returncode == 0)
        print("OBSERVED", case, json.dumps(obs, sort_keys=True), flush=True)
        return obs


cases = {case: probe(case) for case in ("ordinary", "moved_plain", "moved_other", "historical_plain", "historical_valid")}
role = os.environ["ROLE"]
for case, obs in cases.items():
    assert obs["verifies"] and obs["moved"] == case.startswith("moved")
    if case == "ordinary":
        preserved = False
    elif case.startswith("moved"):
        preserved = role == "corrected"
    elif case == "historical_plain":
        preserved = role != "master"
    else:
        # Record whether a valid specification put back by hand is treated as installed.
        # This control measures a different property from the directory/identity mismatch.
        continue
    assert (obs["exit"] != 0) == preserved
    assert obs["stable"] == preserved and obs["directory"] == preserved
    assert obs["added"] == ([] if preserved else ["MODULE_REMOVED"])
    if case != "ordinary":
        assert obs["note"] == preserved
    if case == "moved_other":
        assert obs["other"] == preserved
