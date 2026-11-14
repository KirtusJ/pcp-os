"""The ACR component: find it, check it against its own manifest and against the required pin, and read it.

PCP OS holds no copy of any ACR rule. Clause text, the kernel, the command parser and the reference
host are read from the verified ACR Implementation packet, so the method and the program cannot drift apart.
"""
from __future__ import annotations
import importlib.util, os, sys, zipfile
from pathlib import Path
from .base import (PcposError, INTEGRITY, ENVIRONMENT, USAGE, strict_loads, sha_bytes, sha_file, atomic_write, name_issues)

OS_ROOT = Path(__file__).resolve().parent.parent
MAX_MEMBER = 8 * 1024 * 1024


def required():
    return strict_loads((OS_ROOT / "RELEASE" / "Required_Component.json").read_bytes())


def candidates(ws_root=None):
    """Places a method folder or sealed archive is looked for, in order."""
    req = required()
    out = []
    if os.environ.get("PCPOS_METHOD"):
        out.append(Path(os.environ["PCPOS_METHOD"]))
    if ws_root is not None:
        out.append(Path(ws_root) / "method" / req["folder"])
    for base in (OS_ROOT.parent, OS_ROOT.parent.parent, OS_ROOT):
        out.append(base / req["folder"])
        out.append(base / req["archive"])
    return out


def locate(ws_root=None, explicit=None):
    for c in ([Path(explicit)] if explicit else candidates(ws_root)):
        if c.is_dir() and (c / "MANIFEST.json").is_file():
            return c
        if c.is_dir() and (c / required()["folder"] / "MANIFEST.json").is_file():
            return c / required()["folder"]
        if c.is_file() and c.suffix.lower() == ".zip":
            return c
    return None


def verify_dir(path):
    """Check every file of an extracted packet against the packet's own manifest. Returns its identity."""
    path = Path(path)
    try:
        raw = (path / "MANIFEST.json").read_bytes()
        man = strict_loads(raw)
        files = man["files"]
    except (OSError, ValueError, KeyError) as e:
        raise PcposError(INTEGRITY, "the method packet at %s has no readable manifest: %s" % (path, e))
    listed = set()
    for f in files:
        rel = f["path"]; listed.add(rel)
        p = path / rel
        if not p.is_file():
            raise PcposError(INTEGRITY, "method packet: %s is missing" % rel)
        digest, size = sha_file(p)
        if digest != f["sha256"] or size != f["bytes"]:
            raise PcposError(INTEGRITY, "method packet: %s does not match the packet's manifest" % rel,
                             "the file was altered after release (a checkout that converts line endings does this); extract the sealed archive again")
    extra = sorted(p.relative_to(path).as_posix() for p in path.rglob("*")
                   if p.is_file() and "__pycache__" not in p.parts and p.relative_to(path).as_posix() not in listed | {"MANIFEST.json"})
    if extra:
        raise PcposError(INTEGRITY, "method packet: %d file(s) not in its manifest, first: %s" % (len(extra), extra[0]))
    req = required()
    ident = {"release_fingerprint": sha_bytes(raw),
             "core_descriptor_sha256": sha_bytes((path / "RELEASE" / "Core_Descriptor.json").read_bytes()),
             "files": len(files)}
    ident["matches_required"] = (ident["release_fingerprint"] == req["release_fingerprint"]
                                 and ident["core_descriptor_sha256"] == req["core_descriptor_sha256"])
    return ident


def install_from_zip(zip_path, dest_parent):
    """Extract a sealed packet without trusting its member names: only files named in its manifest are written,
    each is checked against the manifest as it is written, and nothing is written outside the destination."""
    req = required()
    zip_path, dest = Path(zip_path), Path(dest_parent) / req["folder"]
    archive_sha, _ = sha_file(zip_path)
    try:
        z = zipfile.ZipFile(zip_path)
    except (zipfile.BadZipFile, OSError) as e:
        raise PcposError(INTEGRITY, "%s is not a readable archive: %s" % (zip_path, e))
    with z:
        prefix = req["folder"] + "/"
        try:
            raw = z.read(prefix + "MANIFEST.json")
            man = strict_loads(raw)
        except (KeyError, ValueError) as e:
            raise PcposError(INTEGRITY, "the archive holds no %sMANIFEST.json: %s" % (prefix, e))
        for f in man["files"]:
            rel = f["path"]
            if name_issues(rel) or rel.startswith("/"):
                raise PcposError(INTEGRITY, "the packet manifest names an unsafe path: %r" % rel)
            info = z.getinfo(prefix + rel)
            if info.file_size != f["bytes"] or info.file_size > MAX_MEMBER:
                raise PcposError(INTEGRITY, "archive member %s has an unexpected size" % rel)
            data = z.read(info)
            if sha_bytes(data) != f["sha256"]:
                raise PcposError(INTEGRITY, "archive member %s does not match the packet's manifest" % rel)
            atomic_write(dest / rel, data)
        atomic_write(dest / "MANIFEST.json", raw)
    ident = verify_dir(dest)
    ident["archive_sha256"] = archive_sha
    ident["archive_matches_required"] = archive_sha == req["archive_sha256"]
    return dest, ident


class Method:
    """A verified ACR packet on disk, with its own reference programs loaded from it."""

    def __init__(self, path, ident):
        self.path, self.ident = Path(path), ident
        self._mods = {}

    def tool(self, name):
        """Load one of the packet's reference programs (acr, command_interface, acr_host) from the verified files."""
        if name not in self._mods:
            tools = str(self.path / "TOOLS")
            if tools not in sys.path:
                sys.path.insert(0, tools)        # the packet's programs import each other by name
            sys.dont_write_bytecode = True       # never add files to the verified packet
            spec = importlib.util.spec_from_file_location(name, self.path / "TOOLS" / (name + ".py"))
            mod = importlib.util.module_from_spec(spec)
            sys.modules[name] = mod
            spec.loader.exec_module(mod)
            self._mods[name] = mod
        return self._mods[name]

    @property
    def acr(self):
        return self.tool("acr")

    def _call(self, fn, *a, **kw):
        try:
            return fn(*a, root=self.path, **kw)
        except self.acr.AcrError as e:
            raise PcposError(USAGE, str(e))

    def clause(self, component, ref):
        return self._call(self.acr.clause, component, ref)

    def toc(self, component):
        return self._call(self.acr.toc, component)

    def find(self, terms, component=None, limit=12):
        return self._call(self.acr.find, terms, component, limit)

    def term(self, term):
        return self._call(self.acr.term_lookup, term)

    def kernel(self, card=False):
        return self._call(self.acr.kernel, card)

    def component(self, name):
        """(key, component id, version, digest of its Specification) for a module name or alias."""
        try:
            key = self.acr.component_key(name)
        except self.acr.AcrError as e:
            raise PcposError(USAGE, str(e))
        if key not in self.acr.COMPONENTS:
            raise PcposError(USAGE, "%r is a conditional contract, not a module" % name)
        cid, version, digest = self.acr.identity(key, self.path)
        return {"key": key, "id": cid, "version": version, "sha256": digest}

    def modules(self):
        return [self.component(k) for k in self.acr.COMPONENTS]

    def record(self):
        """The identity written to the register: what was verified, by which digests."""
        return {"release_fingerprint": self.ident["release_fingerprint"],
                "core_descriptor_sha256": self.ident["core_descriptor_sha256"],
                "matches_required": self.ident["matches_required"], "verified": "FILES_VERIFIED_AGAINST_OWN_MANIFEST",
                "components": {m["id"]: [m["version"], m["sha256"]] for m in self.modules()}}


def open_method(ws_root=None, explicit=None, allow_install=True):
    """Return a verified Method, extracting a sealed archive into the workspace when that is all there is."""
    found = locate(ws_root, explicit)
    if found is None:
        req = required()
        raise PcposError(ENVIRONMENT, "the ACR Implementation packet was not found",
                         "place %s or the folder %s beside the PCP OS packet, or pass --method <path>, or set PCPOS_METHOD" % (req["archive"], req["folder"]))
    if found.is_file():
        if ws_root is None or not allow_install:
            raise PcposError(ENVIRONMENT, "the ACR packet is a sealed archive and there is no workspace to extract it into",
                             "run: pcpos init <directory>   (it extracts and verifies the packet), or extract %s yourself" % found.name)
        path, ident = install_from_zip(found, Path(ws_root) / "method")
        return Method(path, ident)
    return Method(found, verify_dir(found))
