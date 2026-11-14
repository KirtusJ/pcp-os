#!/usr/bin/env python3
"""Check the plugins as they are delivered: each archive extracted afresh, the way a licensee would.

    python3 check_packaged_plugins.py <release dir>

For each edition: the edition archive and the plugins archive are extracted with unzip; each one's own verifier
passes; the two hold the same PCP_1.0_Plugins folder, byte for byte and permission for permission; Claude Code
validates the marketplace and both plugins (--strict); the documented events are written by the packaged
program; and the launcher is tried with the program missing and with its permission to run removed. An
extraction that drops permissions (Python's zipfile) must be caught by the verifier. Needs unzip and claude.
"""
import json, os, shutil, subprocess, sys, tempfile, zipfile, filecmp
from pathlib import Path
REL = Path(sys.argv[1]).resolve(); HERE = Path(__file__).resolve().parent
run = lambda *a, **k: subprocess.run([str(x) for x in a], capture_output=True, text=True, **k)
env = {k: v for k, v in os.environ.items() if not k.startswith("PCPOS_")}
report = {}

def same_tree(a, b):
    fa = sorted(p.relative_to(a).as_posix() for p in a.rglob("*") if p.is_file()); fb = sorted(p.relative_to(b).as_posix() for p in b.rglob("*") if p.is_file())
    assert fa == fb, sorted(set(fa) ^ set(fb))[:5]
    for f in fa:
        assert (a / f).read_bytes() == (b / f).read_bytes() and os.access(a / f, os.X_OK) == os.access(b / f, os.X_OK), f
    return len(fa)

for name in ("Personal", "Commercial", "Enterprise"):
    r = report[name] = {}
    with tempfile.TemporaryDirectory(prefix="pcp-packaged-") as td:
        td = Path(td)
        roots = {}
        for kind, zname, top, verifier, ok in (("edition", "PCP_1.0_%s_Edition.zip" % name, "PCP_1.0_%s_Edition" % name, "verify_edition.py", "PASS_EDITION_CHECK"),
                                                ("plugins_archive", "PCP_1.0_Plugins_%s_Edition.zip" % name, "PCP_1.0_Plugins_%s_Edition" % name, "verify_plugins.py", "PASS_PLUGINS_CHECK")):
            assert run("unzip", "-q", REL / zname, "-d", td / kind).returncode == 0
            root = roots[kind] = td / kind / top
            v = run(sys.executable, "-I", "-B", root / verifier, cwd=root)
            assert v.returncode == 0 and v.stdout.startswith(ok), (zname, v.stdout, v.stderr)
            market = root / "PCP_1.0_Plugins"
            for t in (market, market / "plugins/pcp", market / "plugins/pcp-os"):
                c = run("claude", "plugin", "validate", "--strict", t)
                assert c.returncode == 0 and "Validation passed" in c.stdout and "warning" not in c.stdout.lower(), (t, c.stdout[-600:])
                assert "has no line that installs" not in c.stdout, (t, c.stdout[-600:])
            for th in sorted((market / "plugins").glob("*/themes/*.json")):
                j = json.loads(th.read_text()); assert j["base"] in ("dark", "light") and j["name"] and j["overrides"], th
            # an extraction that drops permissions is caught, with the remedy
            with zipfile.ZipFile(REL / zname) as z:
                z.extractall(td / (kind + "_noperm"))
            bad = run(sys.executable, "-I", "-B", td / (kind + "_noperm") / top / verifier, "--quick", cwd=td / (kind + "_noperm") / top)
            assert bad.returncode != 0 and "is not executable" in bad.stdout and "chmod +x" in bad.stdout, (zname, bad.stdout)
            r[kind] = {"verifier": v.stdout.splitlines()[0], "claude_validate_strict": "3 passed, no warning, no advice", "extracted_without_permissions": bad.stdout.splitlines()[0][:110]}
        r["same_marketplace_folder_files"] = same_tree(roots["edition"] / "PCP_1.0_Plugins", roots["plugins_archive"] / "PCP_1.0_Plugins")
        # the launcher
        os_plugin = roots["plugins_archive"] / "PCP_1.0_Plugins/plugins/pcp-os"
        launcher, program = os_plugin / "servers/pcpos", os_plugin / "servers/linux-x86_64/pcpos"
        proj = td / "project"; proj.mkdir()
        e = dict(env, CLAUDE_PLUGIN_ROOT=str(os_plugin), CLAUDE_PROJECT_DIR=str(proj), PCPOS_ROOT=str(proj))
        init = json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}}) + "\n" + json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}) + "\n"
        m = run(launcher, "mcp", input=init, env=e, cwd=proj)
        tools = [t["name"] for t in json.loads(m.stdout.splitlines()[1])["result"]["tools"]]
        assert m.returncode == 0 and len(tools) == 15 and json.loads(m.stdout.splitlines()[0])["result"]["serverInfo"]["version"].endswith(json.loads((os_plugin / ".claude-plugin/plugin.json").read_text())["version"]), m.stdout[:300]
        quiet = run(launcher, "hook", "prompt", input=json.dumps({"hook_event_name": "UserPromptSubmit", "prompt": "what is the weather", "cwd": str(proj)}), env=e, cwd=proj)
        assert quiet.returncode == 0 and quiet.stdout == "" and not (proj / ".pcp").exists(), quiet
        program.chmod(0o644)
        a = run(launcher, "mcp", input=init, env=e, cwd=proj); b = run(launcher, "hook", "prompt", input="{}", env=e, cwd=proj)
        assert a.returncode == 126 and "not executable" in a.stderr and "chmod +x" in a.stderr and b.returncode == 1 and "chmod +x" in b.stderr, (a, b)
        program.chmod(0o755)
        shutil.rmtree(program.parent)
        a = run(launcher, "mcp", input=init, env=e, cwd=proj); b = run(launcher, "hook", "prompt", input="{}", env=e, cwd=proj)
        assert a.returncode == 127 and "no program built" in a.stderr and b.returncode == 0 and b.stdout == "" and b.stderr == "", (a, b)
        r["launcher"] = {"tools": len(tools), "ordinary_prompt": "nothing written, nothing said", "program_not_executable": "tool server 126 and hook 1, each with the chmod command", "program_missing": "tool server 127, hook 0 and silent"}
print(json.dumps(report, indent=1))
print("PACKAGED_PLUGINS_OK 6 archives")
