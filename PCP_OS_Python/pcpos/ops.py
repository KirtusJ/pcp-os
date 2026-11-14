"""Operations on a workspace. Each one builds an event and appends it through the guards, under the lock.

An operation either writes one complete event or writes nothing and says why.
"""
from __future__ import annotations
import json, os, platform, subprocess, sys, time, urllib.error, urllib.request, uuid, zipfile
from pathlib import Path
from . import VERSION, RECORD_PROFILE
from .base import (PcposError, OK, NEGATIVE, USAGE, REFUSED, ENVIRONMENT, INTEGRITY, canon, strict_loads, sha_bytes, sha_file,
                   atomic_write, name_issues, check_label, is_within, now_utc)
from .store import Workspace
from . import model, method as method_mod

TREE_PROFILE = "ACR-OS-TREE-1"
EXCLUDED_DIRS = ("__pycache__",)
EXCLUDED_SUFFIXES = (".pyc",)
MAX_OUTPUT = 16 * 1024 * 1024
MAX_FETCH = 64 * 1024 * 1024
MAX_ZIP_TOTAL = 4 * 1024 * 1024 * 1024
MAX_ZIP_MEMBERS = 200000


class Ops:
    def __init__(self, ws: Workspace, actor=None):
        self.ws, self._actor = ws, actor

    # ---- plumbing -----------------------------------------------------------------
    def state(self):
        return model.fold(self.ws.events()[0])

    def config(self):
        try:
            return strict_loads((self.ws.local / "config.json").read_bytes())
        except (OSError, ValueError):
            return {}

    def set_config(self, **kw):
        cfg = self.config(); cfg.update(kw)
        atomic_write(self.ws.local / "config.json", canon(cfg) + b"\n")

    def actor(self, state=None):
        """Who is acting: --as, PCPOS_ACTOR, the workspace default, or the only candidate. Never guessed among several."""
        who = self._actor or os.environ.get("PCPOS_ACTOR") or self.config().get("actor")
        if who:
            return who
        state = state or self.state()
        others = [l for l, p in state.participants.items() if not p["principal"]]
        if len(others) == 1:
            return others[0]
        if not others and len(state.participants) == 1:
            return next(iter(state.participants))
        raise PcposError(USAGE, "more than one participant could be acting", "say who: --as <label>   (or set PCPOS_ACTOR, or: pcpos config actor <label>)")

    def _write(self, kind, body, actor=None, auth="DECLARED"):
        actor = actor or self.actor()

        def check(events):
            return model.guard(model.fold(events), kind, body, actor, self.ws)
        return self.ws.append(kind, body, actor, auth, check)

    def refs(self, tokens, state=None):
        state = state or self.state()
        return [model.parse_ref(state, t) for t in (tokens or [])]

    # ---- setting up ------------------------------------------------------------------
    @classmethod
    def init(cls, root, label="workspace", principal="principal", principal_name=None, method_path=None, require_method=False):
        ws = Workspace.create(root, label)
        ws.append("WORKSPACE_CREATED", {"profile": RECORD_PROFILE, "program_version": VERSION, "workspace": ws.meta["id"], "label": label}, "host")
        ops = cls(ws)
        ops._write("PARTICIPANT_ADDED", {"label": check_label(principal, "principal label"), "kind": "HUMAN", "principal": True,
                                         "name": principal_name}, actor="host")
        note = None
        try:
            ops.method_pin(method_path)
        except PcposError as e:
            if require_method or e.code == INTEGRITY:
                raise
            note = e
        return ops, note

    def participant_add(self, label, kind="AUTOMATED", name=None, model_name=None, provider=None, lineage=None, principal=False):
        return self._write("PARTICIPANT_ADDED", {"label": label, "kind": kind, "name": name, "model": model_name, "provider": provider,
                                                 "lineage": lineage, "principal": principal}, actor="host")

    def method(self, explicit=None):
        return method_mod.open_method(self.ws.root, explicit)

    def method_pin(self, explicit=None, accept_other=False):
        m = self.method(explicit)
        if not m.ident["matches_required"] and not accept_other:
            req = method_mod.required()
            raise PcposError(REFUSED, "the packet at %s is not the release PCP OS %s requires (fingerprint %s, required %s)"
                             % (m.path, VERSION, m.ident["release_fingerprint"][:12], req["release_fingerprint"][:12]),
                             "an equal name or a newer release does not satisfy an exact pin; to use it knowingly, repeat with --accept-other-release", "Core 14.1")
        return self._write("METHOD_PINNED", dict(m.record(), deviation=not m.ident["matches_required"]), actor="host")

    def method_declare(self, fingerprint, core_descriptor, note=None):
        """Record a method identity that this session could not verify (the packet is not at hand)."""
        return self._write("METHOD_PINNED", {"release_fingerprint": fingerprint, "core_descriptor_sha256": core_descriptor,
                                             "verified": "DECLARED_NOT_VERIFIED", "note": note,
                                             "matches_required": fingerprint == method_mod.required()["release_fingerprint"]}, actor="host")

    def adopt(self, by, basis):
        return self._write("ADOPTED", {"by": by, "basis": basis}, actor=by if by != "host" else None)

    def halt(self, by, words=None):
        return self._write("HALTED", {"by": by, "words": words}, actor=by if by != "host" else None)

    # ---- artifacts ---------------------------------------------------------------------
    def add_file(self, path, name=None, origin="SUPPLIED", extra=None):
        path = Path(path)
        if not path.is_file():
            raise PcposError(USAGE, "%s is not a file" % path)
        digest, size = self.ws.put_file(path)
        return self._artifact(digest, size, name, origin, extra, path.name)

    def add_bytes(self, data, name=None, origin="PRODUCED", extra=None, filename=None):
        digest = self.ws.put_bytes(data)
        return self._artifact(digest, len(data), name, origin, extra, filename)

    def _artifact(self, digest, size, name, origin, extra, filename):
        state = self.state()
        known = state.artifacts.get(digest)
        if known and (name is None or name in known["names"]) and known["stored"]:
            return None, digest                         # the same bytes are already recorded: nothing to write
        body = {"sha256": digest, "size": size, "name": name, "origin": origin if not known else known["origin"], "stored": True, "filename": filename}
        if known and known.get("source"):
            body["source"] = known["source"]
        if extra:
            body.update(extra)
        ev, _ = self._write("ARTIFACT_ADDED", body)
        return ev, digest

    def read(self, token, start=0, length=None, tree_dir=None):
        """Return bytes of an artifact or tree member, checked against the recorded digest, and record the delivery."""
        state = self.state()
        ref = model.parse_ref(state, token)["ref"]
        if ref["t"] == "artifact":
            data = self.ws.get(ref["sha256"])
        elif ref["t"] == "member":
            tree = state.trees.get(ref["tree"]) or model.refuse("no pinned tree %r" % ref["tree"])
            entries = {e[0]: e for e in strict_loads(self.ws.get(tree["manifest"]))["entries"]}
            if ref["path"] not in entries:
                model.refuse("%r is not a file of the pinned tree %s" % (ref["path"], ref["tree"]))
            want = entries[ref["path"]][2]
            if self.ws.has(want):
                data = self.ws.get(want)
            elif tree_dir:
                data = (Path(tree_dir) / ref["path"]).read_bytes()
                if sha_bytes(data) != want:
                    raise PcposError(INTEGRITY, "%s under %s is not the pinned file (its bytes differ)" % (ref["path"], tree_dir))
            else:
                raise PcposError(USAGE, "the tree %s was pinned by reference; its bytes are not in the store" % ref["tree"],
                                 "give the directory that holds it: --from <directory>")
        else:
            raise PcposError(USAGE, "only artifacts and tree members have bytes to read")
        end = len(data) if length is None else min(len(data), start + length)
        span = "all %d bytes" % len(data) if (start == 0 and end == len(data)) else "bytes %d-%d of %d" % (start, end, len(data))
        self._write("ARTIFACT_READ", {"ref": ref, "span": span})
        return data[start:end], span

    def inspect(self, token, extent):
        ref = model.parse_ref(self.state(), token)["ref"]
        return self._write("INSPECTION_DECLARED", {"ref": ref, "extent": extent})

    # ---- trees -----------------------------------------------------------------------
    def _scan_dir(self, root):
        root = Path(root)
        if is_within(self.ws.root, root):
            raise PcposError(REFUSED, "the workspace is inside %s" % root, hint="a workspace is kept outside the material it records, so that recording never changes the subject")
        entries, skipped, links = [], 0, 0
        for dp, dns, fns in os.walk(root):
            dns[:] = sorted(d for d in dns if d not in EXCLUDED_DIRS)
            for fn in sorted(fns):
                p = Path(dp) / fn
                if fn.endswith(EXCLUDED_SUFFIXES):
                    skipped += 1; continue
                if p.is_symlink():
                    links += 1; continue
                digest, size = sha_file(p)
                entries.append([p.relative_to(root).as_posix(), size, digest])
        entries.sort()
        return entries, skipped, links

    def _scan_zip(self, path):
        entries, total = [], 0
        try:
            z = zipfile.ZipFile(path)
        except (zipfile.BadZipFile, OSError) as e:
            raise PcposError(INTEGRITY, "%s is not a readable archive: %s" % (path, e))
        with z:
            infos = [i for i in z.infolist() if not i.is_dir()]
            if len(infos) > MAX_ZIP_MEMBERS:
                raise PcposError(REFUSED, "the archive has %d members; the limit is %d" % (len(infos), MAX_ZIP_MEMBERS))
            for info in infos:
                total += info.file_size
                if total > MAX_ZIP_TOTAL:
                    raise PcposError(REFUSED, "the archive expands beyond %d bytes" % MAX_ZIP_TOTAL)
                import hashlib
                h = hashlib.sha256()
                with z.open(info) as f:                 # read in place: nothing is extracted to disk
                    while True:
                        b = f.read(1 << 20)
                        if not b:
                            break
                        h.update(b)
                entries.append([info.filename, info.file_size, h.hexdigest()])
        entries.sort()
        return entries

    @staticmethod
    def portability(entries):
        issues, longest, seen, collisions = [], 0, {}, 0
        for rel, _size, _d in entries:
            longest = max(longest, len(rel))
            for why in name_issues(rel):
                issues.append("%s: %s" % (rel, why))
            low = rel.lower()
            if low in seen and seen[low] != rel:
                collisions += 1; issues.append("%s: differs only by case from %s" % (rel, seen[low]))
            seen.setdefault(low, rel)
        return {"longest_relative_path": longest, "issues": len(issues), "case_collisions": collisions,
                "windows_root_budget": max(0, 259 - longest - 1)}, issues

    def pin(self, path, name, origin="SUPPLIED", store=False):
        path = Path(path)
        check_label(name, "tree name")
        if path.is_dir():
            entries, skipped, links = self._scan_dir(path)
            container = {"container": "DIRECTORY", "excluded_byproducts": skipped, "symlinks_not_followed": links}
        elif path.is_file() and zipfile.is_zipfile(path):
            entries = self._scan_zip(path)
            container = {"container": "ZIP", "container_sha256": sha_file(path)[0], "container_name": path.name}
        else:
            raise PcposError(USAGE, "%s is neither a directory nor a zip archive" % path, "a single file is recorded with: pcpos add <file>")
        if not entries:
            raise PcposError(REFUSED, "%s holds no files" % path)
        port, issues = self.portability(entries)
        manifest = self.ws.put_bytes(canon({"profile": TREE_PROFILE, "entries": entries}))
        if store and path.is_dir():
            for rel, _s, digest in entries:
                if not self.ws.has(digest):
                    self.ws.put_file(path / rel)
        body = dict(container, name=name, manifest=manifest, files=len(entries), bytes=sum(e[1] for e in entries),
                    origin=origin, stored=bool(store and path.is_dir()), portability=port)
        ev, _ = self._write("TREE_PINNED", body)
        return ev, issues

    def tree_diff(self, name, path):
        """Compare a directory with a pinned tree. Returns (missing, changed, added)."""
        tree = self.state().trees.get(name)
        if tree is None:
            raise PcposError(USAGE, "no pinned tree %r" % name)
        pinned = {e[0]: e[2] for e in strict_loads(self.ws.get(tree["manifest"]))["entries"]}
        now = {e[0]: e[2] for e in self._scan_dir(path)[0]}
        return (sorted(set(pinned) - set(now)), sorted(k for k in pinned if k in now and pinned[k] != now[k]), sorted(set(now) - set(pinned)))

    # ---- checks ----------------------------------------------------------------------
    def interpreter(self):
        """The Python that runs checks: the workspace environment if one was created, else the one running this program."""
        venv = self.ws.local / "venv"
        cand = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        return (str(cand), "WORKSPACE_ENVIRONMENT") if cand.is_file() else (sys.executable, "HOST_INTERPRETER")

    def check(self, argv, name=None, cwd=None, timeout=None, expect_exit=0, tree=None, tree_dir=None, allow_drift=False):
        if not argv:
            raise PcposError(USAGE, "give the command to run after --")
        run = list(argv)
        interp, which = self.interpreter()
        if run[0] in ("python", "python3", "py"):
            run[0] = interp                              # 'python3' does not exist on Windows and 'python' may be a store stub
        before = after = None
        if tree:
            if not tree_dir:
                raise PcposError(USAGE, "--tree needs --at <directory> holding that tree")
            cwd = cwd or tree_dir
            miss, chg, add = self.tree_diff(tree, tree_dir)
            before = not (miss or chg or add)
            if not before and not allow_drift:
                raise PcposError(REFUSED, "%s is not the pinned tree %s (%d missing, %d changed, %d added); the check was not run"
                                 % (tree_dir, tree, len(miss), len(chg), len(add)),
                                 "a check supports a finding about the pinned subject only if it ran on it; restore the files, pin the new state under a new name, or pass --allow-drift",
                                 "Core 2.1, 8.1")
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", PYTHONIOENCODING="utf-8", PYTHONUTF8="1")
        self.ws.local.mkdir(exist_ok=True)
        tag = uuid.uuid4().hex[:8]
        o_path, e_path = self.ws.local / ("out.%s" % tag), self.ws.local / ("err.%s" % tag)
        started, timed_out, code = time.time(), False, None
        try:
            with open(o_path, "wb") as fo, open(e_path, "wb") as fe:
                try:
                    code = subprocess.run(run, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=fo, stderr=fe, timeout=timeout).returncode
                except subprocess.TimeoutExpired:
                    timed_out = True
                except (FileNotFoundError, PermissionError, OSError) as e:
                    raise PcposError(ENVIRONMENT, "the command could not be started: %s" % e,
                                     "check the program name and that it is installed; pcpos doctor lists what this machine has")
            ms = int((time.time() - started) * 1000)
            digests, truncated = [], False
            for p in (o_path, e_path):
                data = p.read_bytes()
                if len(data) > MAX_OUTPUT:
                    data, truncated = data[:MAX_OUTPUT], True
                digests.append(self.ws.put_bytes(data))
        finally:
            for p in (o_path, e_path):
                try:
                    p.unlink()
                except OSError:
                    pass
        if tree:
            miss, chg, add = self.tree_diff(tree, tree_dir)
            after = not (miss or chg or add)
        cwd_rec = None if cwd is None else (Path(cwd).name if Path(cwd).is_absolute() else Path(cwd).as_posix())
        body = {"name": name, "argv": list(argv), "cwd": cwd_rec, "exit": code, "expect_exit": expect_exit, "timeout": timed_out,
                "duration_ms": ms, "stdout": digests[0], "stderr": digests[1], "truncated": truncated,
                "python": platform.python_version(), "platform": "%s-%s" % (platform.system(), platform.machine()), "interpreter": which,
                "tree": None if not tree else {"name": tree, "matched_before": before, "matched_after": after},
                "observed": "RAN_BY_THIS_PROGRAM"}      # this program started the command itself and read its exit status and output
        ev, _ = self._write("CHECK_RAN", body)
        return ev

    # ---- retrieval -------------------------------------------------------------------
    def fetch(self, url, name=None, max_bytes=MAX_FETCH, timeout=30):
        if not url.lower().startswith(("http://", "https://")):
            raise PcposError(USAGE, "only http and https addresses are retrieved")
        req = urllib.request.Request(url, headers={"User-Agent": "pcp-os/%s (record keeping; +local)" % VERSION, "Accept": "*/*"})
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                data = r.read(max_bytes + 1)
                status, final = r.status, r.geturl()
                head = {k.lower(): v for k, v in r.headers.items() if k.lower() in ("content-type", "last-modified", "etag", "content-length")}
        except urllib.error.HTTPError as e:
            deny = e.headers.get("x-deny-reason") if e.headers else None
            e.close()
            raise PcposError(NEGATIVE, "the server answered %d %s for %s%s" % (e.code, e.reason, url, " (proxy: %s)" % deny if deny else ""),
                             "nothing was recorded; a failed retrieval is not a source" + ("; this network allows only listed hosts" if deny else ""))
        except (urllib.error.URLError, OSError, ValueError) as e:
            reason = getattr(e, "reason", e)
            raise PcposError(ENVIRONMENT, "could not reach %s: %s" % (url, reason),
                             "check the network, proxy settings (HTTPS_PROXY) and certificates; pcpos doctor --net tests reachability. Offline work continues on supplied material")
        if len(data) > max_bytes:
            raise PcposError(REFUSED, "the response exceeds %d bytes; nothing was recorded" % max_bytes, hint="raise the limit with --max-bytes if the source is expected to be that large")
        source = {"url": url, "final_url": final, "status": status, "retrieved_at": now_utc(), "headers": head}
        ev, digest = self.add_bytes(data, name, "RETRIEVED", {"source": source}, filename=final.rsplit("/", 1)[-1][:120] or None)
        return ev, digest, source, len(data)

    # ---- reviews ---------------------------------------------------------------------
    def commission(self, label, target, claims, procedure="FOCUSED", extent="TOTAL", director=None, title=None, modules=(),
                   gates=None, author=None, parent=None, arrangement="UNARY", expectation=None):
        state = self.state()
        if target.startswith("tree:"):
            tgt = {"t": "tree", "name": target[5:]}
        elif target in state.trees:
            tgt = {"t": "tree", "name": target}
        else:
            tgt = {"t": "artifact", "sha256": state.artifact(target)}
        mods = []
        if modules:
            m = self.method()
            seen = set()
            for name in modules:
                c = m.component(name)
                if c["key"] == "core":
                    raise PcposError(REFUSED, "Core applies to every review. Select only specialist modules.", clause="Core 14.1")
                if c["id"] in seen:
                    raise PcposError(REFUSED, "%s is listed twice" % c["id"], clause="Core 14.1")
                seen.add(c["id"])
                mods.append({"id": c["id"], "version": c["version"], "sha256": c["sha256"]})
        body = {"label": label, "title": title, "target": tgt, "extent": extent, "arrangement": arrangement, "procedure": procedure,
                "required_gates": gates, "director": director or (state.principals()[0]["label"] if state.principals() else None),
                "author": author, "parent": parent, "modules": mods, "claims": claims, "expectation": expectation}
        return self._write("REVIEW_COMMISSIONED", body)

    def claim_add(self, review, claim):
        return self._write("CLAIM_ADDED", {"review": review, "claim": claim})

    def gate(self, review, gate, basis=(), note=None):
        return self._write("GATE_COMMITTED", {"review": review, "gate": gate, "basis": self.refs(basis), "note": note})

    def objection(self, review, text, claim=None, basis=()):
        return self._write("OBJECTION_RAISED", {"review": review, "claim": claim, "text": text, "basis": self.refs(basis)})

    def objection_end(self, oid, resolution, how, basis=()):
        return self._write("OBJECTION_RESOLVED", {"id": oid, "resolution": resolution, "how": how, "basis": self.refs(basis)})

    def finding(self, review, claim, disposition, basis=(), contrary=(), conditions=(), estimate=None, left_open=None,
                grounding=None, noncritical=()):
        st = self.state()
        body = {"review": review, "claim": claim, "disposition": disposition, "basis": self.refs(basis, st),
                "contrary": self.refs(contrary, st), "conditions": list(conditions), "estimate": estimate, "left_open": left_open,
                "grounding_declared": grounding, "noncritical": list(noncritical)}
        return self._write("FINDING_RECORDED", body)

    def close(self, review, lifecycle, examined_extent="UNKNOWN", note=None):
        return self._write("REVIEW_CLOSED", {"review": review, "lifecycle": lifecycle, "examined_extent": examined_extent, "note": note})

    def session_open(self, review, reason):
        return self._write("SESSION_OPENED", {"review": review, "reason": reason})

    def condition(self, text, origin, review=None):
        return self._write("CONDITION_REGISTERED", {"text": text, "origin": origin, "review": review})

    def obligation(self, text, review=None):
        return self._write("OBLIGATION_OPENED", {"text": text, "review": review})

    def obligation_end(self, oid, how):
        return self._write("OBLIGATION_CLOSED", {"id": oid, "how": how})

    def note(self, text):
        return self._write("NOTE", {"text": model.text(text, "note", 4000)})

    # ---- integrity --------------------------------------------------------------------
    def verify(self, deep=True, anchor=None):
        """Check the chain, replay the guards, and check every stored object. Returns a report dict."""
        events, head = self.ws.events()
        state, problems = model.fold(events, check=True, store=self.ws if deep else None)
        bad_objects, missing = [], 0
        referenced = set(state.artifacts) | {t["manifest"] for t in state.trees.values()}
        for k in state.checks.values():
            referenced |= {k["stdout"], k["stderr"]}
        if deep:
            for d in sorted(referenced):
                p = self.ws.object_path(d)
                if not p.is_file():
                    missing += 1
                elif sha_file(p)[0] != d:
                    bad_objects.append(d)
        rep = {"events": len(events), "head": head, "guard_problems": problems, "altered_objects": bad_objects,
               "objects_by_reference_only": missing, "anchor": None}
        if anchor:
            rep["anchor"] = self.anchor_check(anchor, events)
        rep["ok"] = not problems and not bad_objects and rep["anchor"] in (None, "ANCESTOR", "EQUAL")
        return rep

    def anchor(self):
        events, head = self.ws.events()
        return "pcp-os-anchor:%s:%d:%s" % (self.ws.meta["id"].rsplit(":", 1)[-1][:8], len(events), head)

    @staticmethod
    def anchor_check(anchor, events):
        """Is the anchored state an ancestor of this register? Detects rewriting of history the holder had seen."""
        try:
            _tag, _ws, n, digest = anchor.strip().split(":")
            n = int(n)
        except ValueError:
            raise PcposError(USAGE, "that is not an anchor line as printed by: pcpos anchor")
        if n > len(events):
            return "REGISTER_IS_SHORTER"
        prev = sha_bytes(canon(events[n - 1])) if n else "0" * 64
        if prev != digest:
            return "HISTORY_DIFFERS"
        return "EQUAL" if n == len(events) else "ANCESTOR"
