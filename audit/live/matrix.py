"""Live matrix: the method's 26 behavioural probes, each in six arms of delivery.
   none    no ACR text
   card    KERNEL_CARD.md as system text (absorbed)
   kernel  KERNEL.md as system text (absorbed)
   paste   KERNEL_CARD.md pasted as the first user message (absorbed, as a person would in a chat)
   claudemd  KERNEL_CARD.md as the project's stored instructions (CLAUDE.md), which the host loads itself
   pcp     the text-only plugin, from a release archive extracted afresh (integrated)
   pcp-os  the plugin with the recorded register, likewise (integrated)
 Each conversation runs in its own empty directory. Screens are the packet's own (TOOLS/probe_runner.py)."""
import json, os, subprocess, sys, tempfile, pathlib, concurrent.futures as cf, hashlib, shutil, time
METHOD, PLUG, OUT, MODEL = pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve(), pathlib.Path(sys.argv[3]).resolve(), sys.argv[4]
ARMS = sys.argv[5].split(","); ONLY = sys.argv[6].split(",") if len(sys.argv) > 6 and sys.argv[6] != "all" else None
REPEAT = int(sys.argv[7]) if len(sys.argv) > 7 else 1
sys.path.insert(0, str(METHOD / "TOOLS")); import probe_runner as pr
probes = [p for p in pr.load_probes() if not ONLY or p["id"] in ONLY]
ALLOW = "mcp__plugin_pcp-os_pcp,Read,Grep,Glob,Skill"
# What an arm can reach is what the arm is, and nothing else. The host's file tools read any file on the machine
# without asking, and the machine these runs are made on holds the packet, the probes' criteria and earlier readings.
# Build 21 kept those away by confining each session's file tools (--restricted), which also kept the two plugin
# arms from the plugin's own files: most of their reviews could not be commissioned, and the counts described an
# arrangement nobody uses (Audit 13). Since build 22 a session keeps its tools as they are and runs in a private
# mount namespace in which the folders and files named in PCP_HIDE are empty (audit/calibration/hidden.sh); the
# caller checks, by searching from inside the same namespace, that no probe text is to be found. An arm that is
# text only still gets no tool and is told so; a plugin arm gets the file tools and the Skill tool, and pcp-os its
# own tool server. No arm gets a shell or a write tool: that is the one thing withheld, and the records say so.
# The runs of builds 16 to 19 had neither protection, and whether any of those conversations read such a file was
# not looked at.
NO_TOOLS = "This session has no tools: it cannot read files, search or run commands. Anything written in the form of a tool call is plain text and does nothing. Answer from what is in this conversation."
import re
LIMIT = re.compile(r"hit your .{0,40}limit|usage limit reached", re.I)
HIDDEN = pathlib.Path(__file__).resolve().parent.parent / "calibration" / "hidden.sh"
HIDE = [h for h in os.environ.get("PCP_HIDE", "").split(":") if h]
WORK = pathlib.Path(os.environ.get("PCP_WORK", "/opt/pcpcal/work")).resolve()
assert HIDE, "PCP_HIDE names what a measured session must not be able to read; without it this harness does not run"
assert not any(str(x).startswith(h.rstrip("/") + "/") or str(x) == h for h in HIDE for x in (PLUG, WORK)), "the plugins and the working folders must be outside what is hidden"
def confine(arm):
    if arm in ("pcp", "pcp-os"): return ["--tools", "Read,Grep,Glob,Skill"]
    return ["--tools", ""]
def turn(cwd, prompt, extra, sid, raw):
    cmd = [str(HIDDEN), str(cwd.parent / "store"), "claude", "-p", "--model", MODEL, "--output-format", "stream-json", "--verbose", "--allowedTools", ALLOW] + extra + (["--resume", sid] if sid else [])
    for attempt in range(8):
        r = subprocess.run(cmd, cwd=cwd, input=prompt.encode(), capture_output=True, timeout=900)   # the prompt on standard input: one that begins with "-" is then not read as an option
        lines = [json.loads(l) for l in r.stdout.decode(errors="replace").splitlines() if l.startswith("{")]
        res = next((l for l in lines if l.get("type") == "result"), None)
        # The provider's notice that a usage limit was reached arrives as an ordinary result. It is not a reply of
        # the model: in the Calibration of build 21, 91 conversations were first recorded with it as the reply.
        limited = bool(res) and LIMIT.search(res.get("result") or "") is not None
        if res and not res.get("is_error") and not limited: break
        if limited: res = None
        time.sleep(60 if limited else 5 * (attempt + 1))
    with open(raw, "ab") as f: f.write(r.stdout)
    if not res: return None, sid, [], {}
    tools = [c["name"] + (":" + str(c["input"].get("skill", "")) if c["name"] == "Skill" else "") for l in lines if l.get("type") == "assistant" for c in l["message"]["content"] if c.get("type") == "tool_use"]
    return res.get("result", ""), res.get("session_id", sid), tools, {"cost": res.get("total_cost_usd"), "turns": res.get("num_turns"), "out_tokens": (res.get("usage") or {}).get("output_tokens")}
def conversation(job):
    probe, arm, rep = job
    name = f"{probe['id']}_{arm}_{rep}"; done = OUT / f"{name}.json"
    if done.exists(): return json.loads(done.read_text())
    (WORK / f"{MODEL}-{name}").mkdir(parents=True, exist_ok=True); cwd = pathlib.Path(tempfile.mkdtemp(prefix="cwd-", dir=WORK / f"{MODEL}-{name}")).resolve(); extra = confine(arm); turns = list(probe["turns"]); skip = 0
    if arm in ("card", "kernel"): extra = extra + ["--append-system-prompt", NO_TOOLS + "\n\n" + pr.arm_text(arm)]
    elif arm not in ("pcp", "pcp-os"): extra = extra + ["--append-system-prompt", NO_TOOLS]
    elif arm == "paste": turns = [(METHOD / "KERNEL_CARD.md").read_text()] + turns; skip = 1
    elif arm in ("pcp", "pcp-os"): extra = extra + ["--plugin-dir", str(PLUG / arm)]
    elif arm == "claudemd": (cwd / "CLAUDE.md").write_text((METHOD / "KERNEL_CARD.md").read_text())   # the card as a project's stored instructions
    sid, replies, used, meta = None, [], [], []
    for t in turns:
        reply, sid, tools, m = turn(cwd, t, extra, sid, OUT / f"{name}.jsonl")
        replies.append(reply); used.append(tools); meta.append(m)
        if reply is None: break
    reg = cwd / ".pcp/register.jsonl"
    kinds = [json.loads(l)["kind"] for l in reg.read_text().splitlines()] if reg.exists() else None
    verify = None
    if kinds is not None:
        v = subprocess.run([str(PLUG / "pcp-os/servers/pcpos"), "verify", str(reg)], capture_output=True, cwd=cwd).stdout.decode()
        verify = v.strip().splitlines()[-3:]
    rec = dict(probe=probe["id"], arm=arm, rep=rep, model=MODEL, replies=replies[skip:], pasted_reply=replies[0] if skip else None, tools=used, meta=meta,
               flags=pr.screen_probe(probe, [r for r in replies[skip:]]), register_kinds=kinds, verify=verify)
    done.write_text(json.dumps(rec, indent=1)); shutil.rmtree(cwd.parent, ignore_errors=True)
    return rec
OUT.mkdir(parents=True, exist_ok=True)
jobs = [(p, a, r) for r in range(REPEAT) for p in probes for a in ARMS]
with cf.ThreadPoolExecutor(int(os.environ.get("PCP_POOL", "8"))) as ex:
    for rec in ex.map(conversation, jobs):
        print(rec["probe"], rec["arm"], rec["rep"], "CLEAR" if not rec["flags"] else "FLAG %d" % len(rec["flags"]), flush=True)
