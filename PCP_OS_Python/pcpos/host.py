"""Hosted use: this program owns the exchange with a model, so some controls hold whatever the model does.

Two forms. `run` is one separate model call on stated material, whose exposure (exactly what was sent) is
recorded. `host` is an interactive session built on the ACR packet's reference host, with its events
written to the register. In both, the model is given no tool: permission is denied by default and this
version grants none. Provider calls reuse the ACR packet's own request code.
"""
from __future__ import annotations
import os, sys
from .base import PcposError, OK, ENVIRONMENT, USAGE, REFUSED, sha_bytes, out_text
from . import model as model_mod, view

ENFORCED = ["kernel in context only while ACR is in use", "halt applied by the host; an in-flight reply is discarded",
            "commands parsed on the principal's own message only; documents are passed as data",
            "tool permission denied by default; this version grants the model no tool",
            "every message sent and reply received is stored and digested in the register"]


def provider(method, a):
    pr = method.tool("probe_runner")
    if a.provider == "command":
        if not a.command:
            raise PcposError(USAGE, "--provider command needs --command \"<program>\"; it receives one JSON object on standard input and prints the reply")
        return (lambda system, messages: pr.call_command(a.command, system, messages)), "command"
    key = {"anthropic": "ANTHROPIC_API_KEY", "openai": "OPENAI_API_KEY"}[a.provider]
    if not os.environ.get(key):
        raise PcposError(ENVIRONMENT, "no credential: the environment variable %s is not set" % key,
                         "set it in this shell only (it is never written to the record), or use --provider command with a local program")
    if not a.model:
        raise PcposError(USAGE, "--model <model identifier> is required for this provider")
    if a.provider == "anthropic":
        return (lambda system, messages: pr.call_anthropic(a.model, system, messages, a.max_tokens)), "anthropic"
    base = a.base_url or "https://api.openai.com/v1"
    return (lambda system, messages: pr.call_openai(base, a.model, system, messages, a.max_tokens)), base


def _call(fn, system, messages):
    try:
        out = fn(system, messages)
    except KeyboardInterrupt:
        raise
    except Exception as e:                               # the packet's request code raises several kinds
        code = getattr(e, "code", None)
        raise PcposError(ENVIRONMENT, "the model request failed: %s%s" % (type(e).__name__, " (HTTP %s)" % code if code else ": %s" % str(e)[:200]),
                         "check the credential, the model identifier and the network; nothing was recorded for this request")
    if not isinstance(out, dict) or not isinstance(out.get("text"), str):
        raise PcposError(ENVIRONMENT, "the provider returned no text")
    return out


def _ints(obj):
    """Usage figures as the provider returned them, with anything that is not an integer or text kept as text."""
    if isinstance(obj, dict):
        return {str(k): _ints(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [_ints(v) for v in obj]
    if isinstance(obj, bool) or obj is None or isinstance(obj, (int, str)):
        return obj
    return str(obj)


def data_block(method, origin, text):
    h = method.tool("acr_host")
    return h.DATA_OPEN % origin + text + h.DATA_CLOSE


def cmd_run(a, ops):
    from .cli import T
    st = ops.state()
    if a.participant not in st.participants:
        raise PcposError(USAGE, "participant %r is not registered" % a.participant, "pcpos participant add %s --lineage <model family> --model <identifier>" % a.participant)
    m = ops.method()
    fn, endpoint = provider(m, a)
    system = None
    if a.kernel != "none" and st.adopted:
        system = m.kernel(card=(a.kernel == "card"))
    task = T(a.task)
    parts, exposure = [task], [{"what": "task", "sha256": sha_bytes(task.encode("utf-8"))}]
    if a.review:
        events, head = ops.ws.events()
        brief = view.brief(st, len(events), head, a.participant, a.review)
        parts.append(data_block(m, "the record (State Brief)", brief))
        exposure.append({"what": "state brief of %s at event %d" % (a.review, len(events) - 1), "sha256": sha_bytes(brief.encode("utf-8"))})
    ops2 = type(ops)(ops.ws, a.participant)
    for token in a.include or ():
        data, _span = ops2.read(token, tree_dir=a.source)
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            raise PcposError(REFUSED, "%s is not text; only text is sent to a model by this program" % token)
        parts.append(data_block(m, "item " + token, text))
        exposure.append({"what": token, "sha256": sha_bytes(data)})
    if system:
        exposure.append({"what": "kernel" if a.kernel == "kernel" else "kernel card", "sha256": sha_bytes(system.encode("utf-8"))})
    out = _call(fn, system, [{"role": "user", "content": "\n\n".join(parts)}])
    _ev, o_digest = ops2.add_bytes(out["text"].encode("utf-8"), a.name, "PRODUCED", {"by_run": st.next_id("R")}, filename="run-output.txt")
    t_digest = None
    if out.get("reasoning"):
        _e, t_digest = ops2.add_bytes(str(out["reasoning"]).encode("utf-8"), None, "PRODUCED", {"by_run": st.next_id("R"), "is_trace": True}, filename="run-trace.txt")
    ev, head = ops2._write("RUN_COMPLETED", {
        "provider": a.provider, "endpoint": endpoint if a.provider != "command" else "local command", "model": out.get("model") or a.model,
        "participant": a.participant, "acr_in_use": st.adopted, "kernel_sent": bool(system), "exposure": exposure, "output": o_digest,
        "trace": "UNKNOWN" if t_digest else "NONE", "trace_sha256": t_digest, "usage": _ints(out.get("usage")),
        "note": "a returned trace is an output of the participant; whether it is complete or a summary is not known to this program (Core 15.8)"})
    out_text("run %s by %s: %d characters returned, stored as %s%s | exposure: %d item(s), each by digest | event %d head %s\n  read it with: pcpos show %s"
             % (ev["body"]["id"], a.participant, len(out["text"]), o_digest[:12], "; a trace was returned and stored as %s" % t_digest[:12] if t_digest else "",
                len(exposure), ev["seq"], head[:12], a.name or o_digest[:12]))
    return OK


def cmd_host(a, ops):
    st = ops.state()
    principals = st.principals()
    principal = a.principal or (principals[0]["label"] if len(principals) == 1 else None)
    if principal is None or not st.participants.get(principal, {}).get("principal"):
        raise PcposError(USAGE, "name the person directing this session: --principal <label>")
    if a.participant not in st.participants:
        raise PcposError(USAGE, "participant %r is not registered" % a.participant)
    m = ops.method()
    fn, endpoint = provider(m, a)
    H = m.tool("acr_host")
    writer = type(ops)(ops.ws, a.participant)
    as_principal = type(ops)(ops.ws, principal)

    class Monitor(H.Host):
        def _event(self, kind, **kw):
            super()._event(kind, **kw)
            if kind == "ADOPTED":
                if not as_principal.state().adopted:
                    as_principal._write("ADOPTED", {"by": principal, "basis": "an -acr request in a hosted session"}, auth="HOST")
            elif kind == "HALTED":
                as_principal._write("HALTED", {"by": principal, "words": "halt in a hosted session", "host": _ints(kw)}, auth="HOST")
            else:
                writer._write("HOST_EVENT", dict(_ints(kw), event=kind), auth="HOST")

    turn = {"n": 0}

    def model_fn(system, messages):
        try:
            out = _call(fn, system, messages)
        except KeyboardInterrupt:
            host.halt("principal")
            raise RuntimeError("interrupted by the principal")
        turn["n"] += 1
        sent = writer.ws.put_bytes(repr((system, messages)).encode("utf-8"))
        got = writer.ws.put_bytes(out["text"].encode("utf-8"))
        writer._write("HOST_EVENT", {"event": "TURN", "n": turn["n"], "sent": sent, "received": got, "kernel_sent": bool(system),
                                     "usage": _ints(out.get("usage")), "model": out.get("model") or a.model}, auth="HOST")
        return out["text"]

    host = Monitor(model_fn, adopted=st.adopted)
    writer._write("HOST_EVENT", {"event": "SESSION_STARTED", "provider": a.provider, "endpoint": endpoint, "model": a.model,
                                 "principal": principal, "enforced": ENFORCED}, auth="HOST")
    out_text("hosted session | principal %s | participant %s | ACR in use: %s\n"
             "type a message; -acr commands are read from your messages only; /doc <ref> gives the model an item as data; /brief [review]; /status; /quit. Ctrl-C halts ACR use."
             % (principal, a.participant, "YES" if st.adopted else "NO"))
    while True:
        try:
            sys.stdout.write("> "); sys.stdout.flush()
            line = sys.stdin.readline()
        except KeyboardInterrupt:
            out_text("\n" + host.halt("principal")["text"]); continue
        if not line:
            break
        line = line.rstrip("\r\n")
        if not line.strip():
            continue
        try:
            if line == "/quit":
                break
            if line == "/status":
                out_text(str(host.status())); continue
            if line.startswith("/doc "):
                token = line[5:].strip()
                data, span = writer.read(token)
                host.add_document(token, data.decode("utf-8"))
                out_text("[given to the model as data: %s, %s]" % (token, span)); continue
            if line.startswith("/brief"):
                events, head = writer.ws.events()
                host.add_document("state brief", view.brief(model_mod.fold(events), len(events), head, a.participant, line[6:].strip() or None))
                out_text("[state brief given to the model as data]"); continue
            reply = host.user(line)
            out_text(("[host] " if reply.get("from") == "host" else "") + reply.get("text", ""))
        except PcposError as e:
            sys.stderr.write(e.render() + "\n")
        except UnicodeDecodeError:
            sys.stderr.write("pcpos: that item is not text\n")
    fid = host.fidelity()
    writer._write("HOST_EVENT", {"event": "SESSION_ENDED", "turns": turn["n"], "fidelity": _ints(fid)}, auth="HOST")
    out_text("session ended; %d model turn(s) recorded" % turn["n"])
    return OK
