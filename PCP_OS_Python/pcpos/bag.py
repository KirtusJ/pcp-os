"""Bundles: a workspace exported as one archive laid out as a BagIt bag (RFC 8493), and its checking on receipt.

Receiving a bundle runs nothing from it. Member names are never trusted, nothing is extracted to disk,
and the bundle's own register is held as data: its findings are another party's statements.
"""
from __future__ import annotations
import io, zipfile
from pathlib import Path
from . import VERSION, BUNDLE_PROFILE, WORKSPACE_PROFILE
from .base import PcposError, INTEGRITY, REFUSED, USAGE, ZERO, canon, strict_loads, sha_bytes, sha_file, name_issues, atomic_write
from . import model
from .store import EVENT_KEYS, TS, AUTH

MAX_TOTAL = 4 * 1024 * 1024 * 1024
MAX_MEMBERS = 500000
STAMP = (1980, 1, 1, 0, 0, 0)


def parse_register(raw: bytes):
    """The same checks the workspace applies to its own register, for bytes received from elsewhere."""
    if raw and not raw.endswith(b"\n"):
        raise PcposError(INTEGRITY, "the register ends in an incomplete line")
    events, prev = [], ZERO
    for n, line in enumerate(raw.split(b"\n")[:-1] if raw else []):
        try:
            ev = strict_loads(line)
        except ValueError as e:
            raise PcposError(INTEGRITY, "register line %d is not valid JSON: %s" % (n + 1, e))
        if not isinstance(ev, dict) or set(ev) != EVENT_KEYS or canon(ev) != line or ev["seq"] != n or ev["prev"] != prev \
                or not (isinstance(ev["ts"], str) and TS.match(ev["ts"])) or ev["auth"] not in AUTH \
                or not isinstance(ev["kind"], str) or not isinstance(ev["actor"], str) or not isinstance(ev["body"], dict):
            raise PcposError(INTEGRITY, "register line %d is malformed or breaks the chain" % (n + 1))
        prev = sha_bytes(line)
        events.append(ev)
    return events, prev


def export(ops, out=None):
    ws = ops.ws
    events, head = ws.events()
    state = model.fold(events)
    top = "%s-seq%d-%s" % (ws.meta.get("label") or "workspace", len(events), head[:8])
    payload = {"data/workspace.json": (ws.root / "workspace.json").read_bytes(), "data/register.jsonl": ws.register_path.read_bytes()}
    for p in sorted(ws.objects.rglob("*")):
        if p.is_file() and len(p.name) == 64:
            payload["data/objects/%s/%s" % (p.name[:2], p.name)] = None      # read when written
    def body(name):
        return payload[name] if payload[name] is not None else (ws.objects / name.split("/", 2)[2]).read_bytes()
    manifest, total = [], 0
    for name in sorted(payload):
        data = body(name)
        manifest.append("%s  %s\n" % (sha_bytes(data), name)); total += len(data)
    m = state.method or {}
    info = ("ACR-OS-Bundle-Profile: %s\nACR-OS-Register-Events: %d\nACR-OS-Register-Head: %s\nACR-Release-Fingerprint: %s\n"
            "Bag-Software-Agent: PCP OS %s\nBagging-Date: %s\nExternal-Identifier: %s\nPayload-Oxum: %d.%d\n") % (
        BUNDLE_PROFILE, len(events), head, m.get("release_fingerprint", "NOT_PINNED"), VERSION,
        (events[-1]["ts"][:10] if events else "1980-01-01"), ws.meta["id"], total, len(payload))
    tags = {"bagit.txt": b"BagIt-Version: 1.0\nTag-File-Character-Encoding: UTF-8\n", "bag-info.txt": info.encode("utf-8"),
            "manifest-sha256.txt": "".join(manifest).encode("utf-8")}
    tags["tagmanifest-sha256.txt"] = "".join("%s  %s\n" % (sha_bytes(tags[k]), k) for k in sorted(tags)).encode("utf-8")
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for name in sorted(tags) + sorted(payload):
            zi = zipfile.ZipInfo("%s/%s" % (top, name), STAMP)
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.external_attr = 0o644 << 16
            z.writestr(zi, tags[name] if name in tags else body(name))
    out = Path(out) if out else Path.cwd() / (top + ".zip")
    if out.is_dir():
        out = out / (top + ".zip")
    atomic_write(out, buf.getvalue())
    return out, sha_bytes(buf.getvalue()), len(events), head


class Bundle:
    """A received bundle, read in place. `get` serves its objects by digest, checked."""

    def __init__(self, path):
        self.path = Path(path)
        try:
            self.z = zipfile.ZipFile(self.path)
        except (zipfile.BadZipFile, OSError) as e:
            raise PcposError(INTEGRITY, "%s is not a readable archive: %s" % (path, e))
        infos = [i for i in self.z.infolist() if not i.is_dir()]
        if len(infos) > MAX_MEMBERS or sum(i.file_size for i in infos) > MAX_TOTAL:
            raise PcposError(REFUSED, "the bundle is larger than this program reads (%d members)" % len(infos))
        tops = {i.filename.split("/", 1)[0] for i in infos}
        if len(tops) != 1 or any("/" not in i.filename for i in infos):
            raise PcposError(INTEGRITY, "a bundle has exactly one top-level folder")
        self.top = tops.pop()
        self.members = {}
        for i in infos:
            rel = i.filename.split("/", 1)[1]
            if name_issues(rel) or i.filename.startswith("/") or rel in self.members:
                raise PcposError(INTEGRITY, "the bundle holds an unsafe or duplicate member name: %r" % i.filename)
            self.members[rel] = i

    def read(self, rel):
        if rel not in self.members:
            raise PcposError(INTEGRITY, "the bundle lacks %s" % rel)
        return self.z.read(self.members[rel])

    def get(self, digest):
        data = self.read("data/objects/%s/%s" % (digest[:2], digest))
        if sha_bytes(data) != digest:
            raise PcposError(INTEGRITY, "bundle object %s does not match its digest" % digest[:12])
        return data

    def has(self, digest):
        return "data/objects/%s/%s" % (digest[:2], digest) in self.members

    def verify(self):
        if self.read("bagit.txt") != b"BagIt-Version: 1.0\nTag-File-Character-Encoding: UTF-8\n":
            raise PcposError(INTEGRITY, "bagit.txt does not declare BagIt 1.0 in UTF-8")
        listed = {}
        for tagfile, prefix in (("tagmanifest-sha256.txt", ""), ("manifest-sha256.txt", "data/")):
            for line in self.read(tagfile).decode("utf-8").splitlines():
                digest, _, name = line.partition("  ")
                if not name or not name.startswith(prefix) or name in listed:
                    raise PcposError(INTEGRITY, "%s holds a malformed or repeated line" % tagfile)
                listed[name] = digest
        for name, digest in listed.items():
            if sha_bytes(self.read(name)) != digest:
                raise PcposError(INTEGRITY, "bundle file %s does not match its manifest" % name)
        extra = sorted(set(self.members) - set(listed) - {"tagmanifest-sha256.txt"})
        if extra:
            raise PcposError(INTEGRITY, "the bundle holds %d file(s) outside its manifests, first: %s" % (len(extra), extra[0]),
                             "a bundle is complete and closed; an unlisted file is not part of it")
        meta = strict_loads(self.read("data/workspace.json"))
        if meta.get("profile") != WORKSPACE_PROFILE:
            raise PcposError(INTEGRITY, "the bundle was written under profile %r; this program reads %s" % (meta.get("profile"), WORKSPACE_PROFILE))
        events, head = parse_register(self.read("data/register.jsonl"))
        state, problems = model.fold(events, check=True, store=None)
        return {"workspace": meta["id"], "label": meta.get("label"), "events": len(events), "head": head, "state": state,
                "guard_problems": problems, "method": state.method, "bundle_sha256": sha_file(self.path)[0],
                "objects": sum(1 for m in self.members if m.startswith("data/objects/"))}


def admit(ops, path, label, assessed_note=None):
    """Record another party's bundle as a submission and take its stored artifacts into the store as supplied material."""
    b = Bundle(path)
    rep = b.verify()
    if rep["guard_problems"]:
        raise PcposError(REFUSED, "the bundle's own record does not pass the guards (%d event(s), first: event %d %s)"
                         % (len(rep["guard_problems"]), rep["guard_problems"][0][0], rep["guard_problems"][0][2]),
                         hint="it was written or altered outside this program; treat its content as unverified material: pcpos pin <bundle.zip> --name <label>")
    mine = ops.state().method
    theirs = rep["method"]
    if theirs is None:
        compat = "METHOD_NOT_STATED"
    elif mine and theirs["release_fingerprint"] == mine["release_fingerprint"]:
        compat = "SAME_METHOD"
    else:
        compat = "DIFFERENT_METHOD_ASSESSED" if assessed_note else "DIFFERENT_METHOD_UNKNOWN"
    register = ops.ws.put_bytes(b.read("data/register.jsonl"))
    ev, _ = ops._write("SUBMISSION_ADMITTED", {
        "label": label, "bundle_sha256": rep["bundle_sha256"], "workspace": rep["workspace"], "head": rep["head"], "events": rep["events"],
        "method": None if theirs is None else {k: theirs.get(k) for k in ("release_fingerprint", "core_descriptor_sha256", "verified")},
        "compatibility": compat, "note": assessed_note, "register": register, "objects": rep["objects"]})
    taken = 0
    for digest, a in sorted(rep["state"].artifacts.items()):
        if b.has(digest):
            ops.ws.put_bytes(b.get(digest))
            got, _ = ops._artifact(digest, a["size"], None, "SUPPLIED", {"via_submission": label, "origin_there": a["origin"]}, a.get("filename"))
            taken += 1 if got else 0
    for t in rep["state"].trees.values():
        if b.has(t["manifest"]):
            ops.ws.put_bytes(b.get(t["manifest"]))
    return ev, rep, compat, taken
