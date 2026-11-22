#!/usr/bin/env python3
"""Fail-closed, no-network GGUF media integrity helper for PCP OS (host-side only).

This does not download, run, authenticate a publisher, or authorize a model.
The actual PCP OS runtime still needs a verified data-volume mount and Qwen backend.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile

SCHEMA = "pcpos-qwen-gguf-1"
MAX_SIZE = 64 * (1 << 30)
CHUNK_SIZE = 1 << 20
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
REV = re.compile(r"[0-9a-f]{40}\Z")
KEYS = frozenset(("schema", "model_id", "revision", "source", "file", "bytes", "sha256", "gguf_version"))


class Refused(Exception):
    """A requested file or declaration has not met the bounded contract."""


def require(ok, why):
    if not ok:
        raise Refused(why)


def plain_file_name(s):
    require(isinstance(s, str) and NAME.fullmatch(s) is not None,
            "a name must be one simple, portable path component")
    require(s not in (".", "..") and not s.endswith("."), "unsafe name")
    return s


def strict_path(path):
    """Refuse linked path components; this is NOT race-proof openat confinement."""
    path = Path(os.path.abspath(os.fspath(path)))
    for p in (*reversed(path.parents), path):
        try:
            st = p.lstat()
        except OSError as e:
            raise Refused("missing or inaccessible path component") from e
        require(not stat.S_ISLNK(st.st_mode), "linked path component refused")
    return path


def read_regular(path, *, digest=True, limit=MAX_SIZE):
    path = strict_path(path)
    require(hasattr(os, "O_NOFOLLOW"), "platform lacks O_NOFOLLOW; cannot safely inspect")
    flags = os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
    # A FIFO/device must not block the verifier before fstat checks its type.
    flags |= getattr(os, "O_NONBLOCK", 0)
    try:
        fd = os.open(path, flags)
    except OSError as e:
        raise Refused("source refused or inaccessible") from e
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode), "not a regular file")
        require(before.st_nlink == 1, "hard-linked file refused")
        require(0 < before.st_size <= limit, "file exceeds length limit or is empty")
        h = hashlib.sha256()
        size = 0
        prefix = b""
        data = bytearray() if not digest else None
        while True:
            buf = os.read(fd, min(CHUNK_SIZE, limit - size + 1))
            if not buf:
                break
            size += len(buf)
            require(size <= limit, "file changed size or exceeded limit")
            if len(prefix) < 16:
                prefix += buf[:16 - len(prefix)]
            h.update(buf)
            if data is not None:
                data.extend(buf)
        after = os.fstat(fd)
        require(size == before.st_size == after.st_size, "file size changed during verification")
        require((before.st_mtime_ns, before.st_ctime_ns) ==
                (after.st_mtime_ns, after.st_ctime_ns), "file changed during verification")
        return size, h.hexdigest(), prefix, bytes(data) if data is not None else None
    except OSError as e:
        raise Refused("file read failed") from e
    finally:
        os.close(fd)


def gguf_version(prefix):
    require(prefix[:4] == b"GGUF" and len(prefix) >= 16, "not a complete GGUF header")
    version = int.from_bytes(prefix[4:8], "little")
    require(version in (2, 3), "unsupported GGUF header version")
    return version


def strict_json_pairs(pairs):
    d = {}
    for k, v in pairs:
        require(k not in d, "duplicate JSON key")
        d[k] = v
    return d


def canonical_bytes(document):
    return (json.dumps(document, sort_keys=True, ensure_ascii=True,
                       separators=(",", ":"), allow_nan=False) + "\n").encode("utf-8")


def check_document(d):
    require(type(d) is dict and set(d) == KEYS, "unrecognized or missing manifest fields")
    require(d["schema"] == SCHEMA, "unrecognized manifest schema")
    plain_file_name(d["model_id"])
    require(type(d["revision"]) is str and REV.fullmatch(d["revision"]), "unpinned source revision")
    require(type(d["source"]) is str and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", d["source"]),
            "invalid source repository identifier")
    name = plain_file_name(d["file"])
    require(name.lower().endswith(".gguf"), "model must be a named GGUF file")
    require(type(d["bytes"]) is int and 16 <= d["bytes"] <= MAX_SIZE, "invalid length")
    require(type(d["sha256"]) is str and HEX64.fullmatch(d["sha256"]), "invalid digest")
    require(type(d["gguf_version"]) is int and d["gguf_version"] in (2, 3), "invalid GGUF version")
    return d


def build(folder, file_name, model_id, revision, source, expected_sha256):
    folder = strict_path(folder)
    require(folder.is_dir(), "model folder is not a directory")
    name = plain_file_name(file_name)
    require(name.lower().endswith(".gguf"), "expected .gguf")
    require(HEX64.fullmatch(expected_sha256) is not None, "independent SHA-256 pin is mandatory")
    size, got, prefix, _ = read_regular(folder / name)
    require(got == expected_sha256, "model bytes differ from independent expected digest")
    document = check_document({"schema": SCHEMA, "file": name,
                               "model_id": model_id, "revision": revision, "source": source,
                               "bytes": size, "sha256": got, "gguf_version": gguf_version(prefix)})
    target = folder / "MODEL_MANIFEST.json"
    require(not target.exists() and not target.is_symlink(), "refusing to replace an existing manifest")
    payload = canonical_bytes(document)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(prefix=".pcpos-qwen-", dir=folder, delete=False) as tmp:
            temporary = tmp.name
            os.chmod(tmp.name, 0o600)
            tmp.write(payload)
            tmp.flush()
            os.fsync(tmp.fileno())
        # Hard-link publication fails if a competing process created the target.
        os.link(temporary, target, follow_symlinks=False)
        if hasattr(os, "O_DIRECTORY"):
            directory_fd = os.open(folder, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    except OSError as e:
        raise Refused("atomic manifest publication failed") from e
    finally:
        if temporary:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass
    return hashlib.sha256(payload).hexdigest()


def verify(folder, pinned_manifest_sha256):
    folder = strict_path(folder)
    require(folder.is_dir(), "model folder is not a directory")
    require(HEX64.fullmatch(pinned_manifest_sha256) is not None,
            "independent expected manifest SHA-256 is required for activation")
    size, digest, _, payload = read_regular(folder / "MODEL_MANIFEST.json", digest=False, limit=8192)
    require(digest == pinned_manifest_sha256, "manifest is not the release-pinned identity")
    try:
        doc = json.loads(payload, object_pairs_hook=strict_json_pairs,
                         parse_constant=lambda x: (_ for _ in ()).throw(Refused("nonfinite JSON number")))
    except (UnicodeError, ValueError) as e:
        raise Refused("manifest JSON malformed") from e
    doc = check_document(doc)
    require(canonical_bytes(doc) == payload, "manifest encoding is not canonical")
    got_length, got_hash, prefix, _ = read_regular(folder / doc["file"])
    require(got_length == doc["bytes"] and got_hash == doc["sha256"],
            "model file does not match pinned manifest")
    require(gguf_version(prefix) == doc["gguf_version"], "model header changed")
    return {"status": "BYTES_MATCH_RELEASE_PIN", "model_id": doc["model_id"],
            "source": doc["source"], "revision": doc["revision"],
            "file": doc["file"], "bytes": got_length, "sha256": got_hash,
            "manifest_sha256": pinned_manifest_sha256,
            "limits": "No inference, publisher authentication, authorization, trusted volume mount or guest boot"}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    b = sub.add_parser("build", help="create a manifest only for externally pinned existing bytes")
    b.add_argument("folder")
    b.add_argument("--file", required=True)
    b.add_argument("--model-id", required=True)
    b.add_argument("--revision", required=True, help="exact upstream 40-hex commit")
    b.add_argument("--source", required=True, help="owner/repository")
    b.add_argument("--expected-sha256", required=True)
    v = sub.add_parser("verify", help="offline read-only check against trusted manifest pin")
    v.add_argument("folder")
    v.add_argument("--expected-manifest-sha256", required=True)
    a = parser.parse_args(argv)
    try:
        if a.command == "build":
            pin = build(a.folder, a.file, a.model_id, a.revision, a.source, a.expected_sha256)
            print(json.dumps({"status": "MANIFEST_CREATED_NOT_AUTHENTICATED", "manifest_sha256": pin}, sort_keys=True))
        else:
            print(json.dumps(verify(a.folder, a.expected_manifest_sha256), sort_keys=True))
    except Refused as e:
        print(json.dumps({"status": "REFUSED", "reason": str(e)}), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
