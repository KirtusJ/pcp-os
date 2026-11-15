"""Shared primitives: canonical JSON, digests, durable writes, time, portable names, errors."""
from __future__ import annotations
import hashlib, json, os, re, sys, tempfile, time
from pathlib import Path

# ---- exit codes (REFERENCE/Exit_Codes.csv) -----------------------------------
OK, NEGATIVE, USAGE, REFUSED, NOT_IN_USE, ENVIRONMENT, INTEGRITY = 0, 1, 2, 3, 4, 5, 6


class PcposError(Exception):
    """An error with a stable exit code, a plain message and, where one exists, the next step."""

    def __init__(self, code, message, hint=None, clause=None):
        super().__init__(message)
        self.code, self.message, self.hint, self.clause = code, message, hint, clause

    def render(self):
        out = "pcpos: " + self.message
        if self.clause:
            out += " [%s]" % self.clause
        if self.hint:
            out += "\n  next: " + self.hint
        return out


# ---- canonical JSON and strict parsing ----------------------------------------
def canon(obj) -> bytes:
    """One byte representation for one value: sorted keys, no spaces, ASCII only, no NaN."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")


def _no_dupes(pairs):
    out = {}
    for k, v in pairs:
        if k in out:
            raise ValueError("duplicate key %r" % k)
        out[k] = v
    return out


def _no_const(name):
    raise ValueError("non-finite number %s" % name)


def strict_loads(data):
    """Parse JSON, rejecting duplicate keys and non-finite numbers. The same parser validates and loads."""
    if isinstance(data, bytes):
        data = data.decode("utf-8")
    return json.loads(data, object_pairs_hook=_no_dupes, parse_constant=_no_const)


def no_floats(obj, where="value"):
    """Records carry integers and strings only; a float has no single byte form across platforms."""
    if isinstance(obj, float):
        raise PcposError(USAGE, "%s contains a non-integer number; use an integer or a string" % where)
    if isinstance(obj, dict):
        for k, v in obj.items():
            no_floats(v, where)
    elif isinstance(obj, (list, tuple)):
        for v in obj:
            no_floats(v, where)


# ---- digests -------------------------------------------------------------------
HEX64 = re.compile(r"^[0-9a-f]{64}\Z")          # \Z, not $: "$" also matches before a final newline
ZERO = "0" * 64


def sha_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha_file(path, chunk=1 << 20):
    h, n = hashlib.sha256(), 0
    with open(path, "rb") as f:
        while True:
            b = f.read(chunk)
            if not b:
                break
            h.update(b); n += len(b)
    return h.hexdigest(), n


# ---- time ----------------------------------------------------------------------
def now_utc() -> str:
    """UTC, to the second. PCPOS_FIXED_TIME (an ISO string) fixes it for reproducible runs and tests."""
    fixed = os.environ.get("PCPOS_FIXED_TIME")
    if fixed:
        return fixed
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


# ---- durable writes ------------------------------------------------------------
def _fsync_dir(path):
    if os.name != "posix":
        return                      # directory handles cannot be flushed this way on Windows
    try:
        fd = os.open(str(path), os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def atomic_write(path, data: bytes):
    """Write a whole file so that a reader sees the old bytes or the new bytes, never a mixture.

    Temporary file in the same directory, flush, fsync, replace, fsync the directory. A failed
    fsync is raised and never retried: after a failed flush the operating system may report the
    next one as successful without the data having been written.
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    # A predictable name opened with "wb" would follow a preplaced symlink.
    # mkstemp creates a private file exclusively in the destination directory.
    fd, filename = tempfile.mkstemp(prefix=path.name + ".tmp-", dir=path.parent)
    tmp = Path(filename)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
    _fsync_dir(path.parent)


def append_durable(path, line: bytes):
    """Append one complete line and flush it to the device before returning."""
    if not line.endswith(b"\n"):
        raise ValueError("append_durable needs a newline-terminated line")
    with open(path, "ab") as f:
        f.write(line); f.flush(); os.fsync(f.fileno())


# ---- portable names ------------------------------------------------------------
_RESERVED = {"CON", "PRN", "AUX", "NUL"} | {"COM%d" % i for i in range(1, 10)} | {"LPT%d" % i for i in range(1, 10)}
_BAD = re.compile(r'[<>:"|?*\\\x00-\x1f]')
LABEL = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")


def name_issues(rel: str):
    """Reasons a relative path (forward slashes) would fail or collide on Windows, macOS or Linux."""
    out = []
    for part in rel.split("/"):
        if part in ("", ".", ".."):
            out.append("empty, '.' or '..' component"); continue
        if _BAD.search(part):
            out.append("character not allowed on Windows in %r" % part)
        if part.endswith((" ", ".")):
            out.append("trailing space or period in %r" % part)
        if part.split(".")[0].upper() in _RESERVED:
            out.append("reserved device name %r" % part)
        if len(part.encode("utf-8")) > 255:
            out.append("component longer than 255 bytes")
        if not part.isascii():
            out.append("non-ASCII name %r (normalization differs between systems)" % part)
    return out


def check_label(value, what="label"):
    if not isinstance(value, str) or not LABEL.match(value) or value.split(".")[0].upper() in _RESERVED:
        raise PcposError(USAGE, "%s %r is not usable: letters, digits, '.', '_' and '-', at most 64, starting with a letter or digit" % (what, value))
    return value


def is_within(child, parent) -> bool:
    try:
        Path(child).resolve().relative_to(Path(parent).resolve())
        return True
    except ValueError:
        return False


def out_text(text):
    """Write to stdout in UTF-8 whatever the console code page is, without failing on a legacy console."""
    data = (text if text.endswith("\n") else text + "\n")
    try:
        sys.stdout.write(data)
    except UnicodeEncodeError:
        sys.stdout.buffer.write(data.encode("utf-8", "replace"))
    sys.stdout.flush()
