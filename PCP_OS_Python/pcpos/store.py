"""The workspace: a content-addressed object store and an append-only, hash-chained register.

Nothing here interprets an event. The register is the record; everything else is derived from it.
"""
from __future__ import annotations
import json, os, re, time, uuid
from pathlib import Path
from . import WORKSPACE_PROFILE, RECORD_PROFILE, VERSION
from .base import (PcposError, INTEGRITY, USAGE, ENVIRONMENT, ZERO, HEX64, canon, strict_loads, no_floats,
                   sha_bytes, sha_file, now_utc, atomic_write, append_durable)

MARKER = "workspace.json"
MAX_EVENT_BYTES = 64 * 1024
TS = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")   # ASCII digits only, and no final newline
EVENT_KEYS = {"seq", "prev", "ts", "actor", "auth", "kind", "body"}
AUTH = ("DECLARED", "HOST")

# Keep the stored register's JSON grammar in step with the Rust core's
# Value and parser: no floats, signed i64 integers, Unicode scalar text,
# and no more than 32 levels of nested values.
MIN_I64, MAX_I64, MAX_WIRE_DEPTH = -(1 << 63), (1 << 63) - 1, 32


def _native_wire_value(value, depth=0):
    if depth > MAX_WIRE_DEPTH:
        raise ValueError("record nesting exceeds the native 32-level limit")
    if value is None or type(value) is bool:
        return
    if type(value) is int:
        if not MIN_I64 <= value <= MAX_I64:
            raise ValueError("record integer is outside the native signed 64-bit range")
        return
    if type(value) is str:
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise ValueError("record contains an unpaired Unicode surrogate")
        return
    if type(value) is list:
        for item in value:
            _native_wire_value(item, depth + 1)
        return
    if type(value) is dict:
        for key, item in value.items():
            if type(key) is not str:
                raise ValueError("record object keys must be strings")
            _native_wire_value(key, depth + 1)
            _native_wire_value(item, depth + 1)
        return
    raise ValueError("record contains an unsupported value type (including floats)")


def find_workspace(explicit=None, start=None):
    """Resolve the workspace: an explicit path, PCPOS_WS, the current directory or an ancestor, or ./pcp-os."""
    if explicit:
        cands = [Path(explicit)]
    elif os.environ.get("PCPOS_WS"):
        cands = [Path(os.environ["PCPOS_WS"])]
    else:
        here = Path(start or os.getcwd()).resolve()
        cands = [here] + list(here.parents) + [here / "pcp-os"]
    for c in cands:
        if (c / MARKER).is_file():
            return c
    raise PcposError(USAGE, "no workspace found", "create one with: pcpos init <directory>   (or pass --ws, or set PCPOS_WS)")


class Workspace:
    def __init__(self, root):
        self.root = Path(root)
        try:
            meta = strict_loads((self.root / MARKER).read_bytes())
        except (OSError, ValueError) as e:
            raise PcposError(INTEGRITY, "cannot read %s: %s" % (self.root / MARKER, e))
        if meta.get("profile") != WORKSPACE_PROFILE:
            raise PcposError(INTEGRITY, "%s is not an %s workspace (profile %r)" % (self.root, WORKSPACE_PROFILE, meta.get("profile")),
                             "this program reads profile %s only; use the program version that wrote the workspace" % WORKSPACE_PROFILE)
        self.meta = meta
        self.register_path = self.root / "register.jsonl"
        self.objects = self.root / "objects"
        self.local = self.root / "local"

    # ---- creation ---------------------------------------------------------------
    @classmethod
    def create(cls, root, label="workspace"):
        root = Path(root)
        if (root / MARKER).exists():
            raise PcposError(USAGE, "%s already holds a workspace" % root)
        if root.exists() and any(root.iterdir()):
            raise PcposError(USAGE, "%s exists and is not empty" % root, "choose a new or empty directory; a workspace is never created inside existing material")
        (root / "objects").mkdir(parents=True, exist_ok=True)
        (root / "local").mkdir(exist_ok=True)
        meta = {"profile": WORKSPACE_PROFILE, "record_profile": RECORD_PROFILE, "id": "acr:packet:%s" % uuid.uuid4(),
                "label": label, "created": now_utc(), "program_version": VERSION}
        atomic_write(root / MARKER, canon(meta) + b"\n")
        atomic_write(root / "register.jsonl", b"")
        return cls(root)

    # ---- lock: one writer, held only across read-tail and append --------------------
    def _lock(self, wait=None):
        wait = float(os.environ.get("PCPOS_LOCK_WAIT", "15")) if wait is None else wait
        self.local.mkdir(exist_ok=True)
        path, deadline = self.local / "lock", time.time() + wait
        while True:
            try:
                fd = os.open(str(path), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
                os.write(fd, ("%d %s\n" % (os.getpid(), now_utc())).encode()); os.close(fd)
                return path
            except FileExistsError:
                if time.time() >= deadline:
                    try:
                        holder = path.read_text().strip()
                    except OSError:
                        holder = "unknown"
                    raise PcposError(ENVIRONMENT, "the workspace is locked by another writer (%s)" % holder,
                                     "wait for it to finish; if no pcpos process is running, remove the stale lock with: pcpos unlock")
                time.sleep(0.05)

    def unlock(self):
        try:
            (self.local / "lock").unlink(); return True
        except FileNotFoundError:
            return False

    # ---- objects ----------------------------------------------------------------
    def object_path(self, digest):
        return self.objects / digest[:2] / digest

    def has(self, digest):
        return self.object_path(digest).is_file()

    def put_bytes(self, data: bytes) -> str:
        digest = sha_bytes(data)
        p = self.object_path(digest)
        if not p.is_file():
            atomic_write(p, data)
        return digest

    def put_file(self, src) -> tuple:
        """Copy a file into the store while hashing it; returns (digest, size)."""
        import hashlib
        self.local.mkdir(exist_ok=True)
        tmp = self.local / ("in.%d.%s" % (os.getpid(), uuid.uuid4().hex[:8]))
        h, n = hashlib.sha256(), 0
        try:
            with open(src, "rb") as f, open(tmp, "wb") as g:
                while True:
                    b = f.read(1 << 20)
                    if not b:
                        break
                    h.update(b); g.write(b); n += len(b)
                g.flush(); os.fsync(g.fileno())
            digest = h.hexdigest()
            p = self.object_path(digest)
            if p.is_file():
                os.unlink(tmp)
            else:
                p.parent.mkdir(parents=True, exist_ok=True)
                os.replace(tmp, p)
        except BaseException:
            try:
                os.unlink(tmp)
            except OSError:
                pass
            raise
        return digest, n

    def get(self, digest) -> bytes:
        p = self.object_path(digest)
        if not p.is_file():
            raise PcposError(INTEGRITY, "object %s is not in this workspace's store" % digest[:12],
                             "it was recorded by reference only, or the store is incomplete; supply the bytes again with: pcpos add <file>")
        data = p.read_bytes()
        if sha_bytes(data) != digest:
            raise PcposError(INTEGRITY, "object %s does not match its digest (the stored bytes were altered)" % digest[:12])
        return data

    # ---- register ---------------------------------------------------------------
    def events(self):
        """Read and check the whole register: line form, sequence and hash chain. Returns (events, head)."""
        try:
            raw = self.register_path.read_bytes()
        except OSError as e:
            raise PcposError(INTEGRITY, "cannot read the register: %s" % e)
        if raw and not raw.endswith(b"\n"):
            raise PcposError(INTEGRITY, "the register ends in an incomplete line (an interrupted write)",
                             "inspect it, then remove only the incomplete final line with: pcpos repair")
        events, prev = [], ZERO
        for n, line in enumerate(raw.split(b"\n")[:-1] if raw else []):
            try:
                ev = strict_loads(line)
                _native_wire_value(ev)
            except ValueError as e:
                raise PcposError(INTEGRITY, "register line %d is not valid JSON: %s" % (n + 1, e))
            if not isinstance(ev, dict) or set(ev) != EVENT_KEYS:
                raise PcposError(INTEGRITY, "register line %d does not have exactly the fields %s" % (n + 1, sorted(EVENT_KEYS)))
            if canon(ev) != line:
                raise PcposError(INTEGRITY, "register line %d is not in canonical form" % (n + 1))
            if type(ev["seq"]) is not int or ev["seq"] != n or ev["prev"] != prev:
                raise PcposError(INTEGRITY, "register line %d breaks the chain (expected seq %d after %s)" % (n + 1, n, prev[:12]))
            if not (isinstance(ev["ts"], str) and TS.match(ev["ts"])) or ev["auth"] not in AUTH \
                    or not isinstance(ev["kind"], str) or not isinstance(ev["actor"], str) or not isinstance(ev["body"], dict):
                raise PcposError(INTEGRITY, "register line %d has a field of the wrong form" % (n + 1))
            prev = sha_bytes(line)
            events.append(ev)
        return events, prev

    def append(self, kind, body, actor, auth="DECLARED", check=None):
        """Append one event. `check(events)` runs under the lock against the events actually on disk and
        may return a replacement body; if it raises, nothing is written."""
        no_floats(body, "event body")
        lock = self._lock()
        try:
            events, head = self.events()
            if check is not None:
                replaced = check(events)
                if replaced is not None:
                    body = replaced
                    no_floats(body, "event body")
            ev = {"seq": len(events), "prev": head, "ts": now_utc(), "actor": actor, "auth": auth, "kind": kind, "body": body}
            # A successful append must never create a register that the
            # authoritative Rust reader would refuse on the next read.
            if not (isinstance(actor, str) and isinstance(kind, str) and auth in AUTH and isinstance(body, dict)):
                raise PcposError(USAGE, "event fields have the wrong types for a native register")
            try:
                _native_wire_value(ev)
            except ValueError as e:
                raise PcposError(USAGE, "event cannot be represented by the native register: %s" % e)
            line = canon(ev)
            if len(line) > MAX_EVENT_BYTES:
                raise PcposError(USAGE, "the event is %d bytes; the limit is %d" % (len(line), MAX_EVENT_BYTES),
                                 "store long text as an artifact (pcpos add) and refer to it")
            append_durable(self.register_path, line + b"\n")
            return ev, sha_bytes(line)
        finally:
            try:
                lock.unlink()
            except OSError:
                pass

    def repair(self):
        """Remove an incomplete final line under the same cooperative lock as append."""
        lock = self._lock()
        try:
            raw = self.register_path.read_bytes()
            if not raw or raw.endswith(b"\n"):
                return 0
            keep = raw[:raw.rfind(b"\n") + 1]
            atomic_write(self.register_path, keep)
            return len(raw) - len(keep)
        finally:
            try:
                lock.unlink()
            except OSError:
                pass
