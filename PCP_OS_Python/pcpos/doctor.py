"""What this machine can do, found by looking and not by assuming.

A missing means stops nothing by itself. Each line says what is absent and what it holds back, so that
a limit is written down instead of discovered half way through the work (Core 15.8).
"""
from __future__ import annotations
import ast, importlib.util, os, platform, shutil, subprocess, sys, tempfile, urllib.error, urllib.request
from pathlib import Path
from .base import PcposError, ENVIRONMENT, USAGE, name_issues

PIP_NAMES = {"pymupdf": "PyMuPDF", "fitz": "PyMuPDF", "yaml": "PyYAML", "PIL": "Pillow", "sklearn": "scikit-learn",
             "cv2": "opencv-python", "bs4": "beautifulsoup4", "dateutil": "python-dateutil", "Crypto": "pycryptodome"}
DEFAULT_HOSTS = ("https://pypi.org/simple/", "https://files.pythonhosted.org/", "https://github.com/", "https://arxiv.org/",
                 "https://doi.org/", "https://www.rfc-editor.org/")
PROGRAMS = ("git", "pdflatex", "latexmk", "lualatex", "bibtex", "make")


def _row(level, key, detail):
    return {"level": level, "key": key, "detail": detail}


def basics():
    rows = []
    v = sys.version_info
    rows.append(_row("ok" if v >= (3, 9) else "fail", "python", "%s at %s%s" % (platform.python_version(), sys.executable,
                     "" if v >= (3, 9) else "; PCP OS needs 3.9 or later")))
    rows.append(_row("ok", "system", "%s %s %s" % (platform.system(), platform.release(), platform.machine())))
    enc = (getattr(sys.stdout, "encoding", None) or "unknown")
    rows.append(_row("ok" if enc.lower().replace("-", "") == "utf8" else "note", "console encoding",
                     enc + ("" if enc.lower().replace("-", "") == "utf8" else "; non-ASCII output is written as UTF-8 bytes, which a legacy console may show wrongly (set PYTHONUTF8=1)")))
    try:
        with tempfile.NamedTemporaryFile(dir=os.getcwd()) as _f:
            pass
        rows.append(_row("ok", "current directory", "writable"))
    except OSError as e:
        rows.append(_row("warn", "current directory", "not writable (%s); create the workspace elsewhere" % e))
    try:
        free = shutil.disk_usage(os.getcwd()).free
        rows.append(_row("ok" if free > 200e6 else "warn", "disk", "%d MB free" % (free // 1e6)))
    except OSError:
        pass
    if os.name == "nt":
        which = shutil.which("python") or ""
        if "WindowsApps" in which:
            rows.append(_row("warn", "python on PATH", "%s is the Microsoft Store shortcut, not an interpreter; start programs with 'py' or the full path. Checks run through this program use %s"
                             % (which, sys.executable)))
        rows.append(_row("ok" if shutil.which("py") else "note", "py launcher", shutil.which("py") or "not found; use the interpreter's full path"))
        try:
            import winreg
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\FileSystem") as k:
                on = winreg.QueryValueEx(k, "LongPathsEnabled")[0] == 1
            rows.append(_row("ok" if on else "note", "long paths", "enabled" if on else
                             "not enabled: a full path over 259 characters fails; keep material near the drive root (see: pcpos doctor --tree)"))
        except (OSError, ImportError):
            rows.append(_row("note", "long paths", "could not be read; assume the 259-character limit"))
    for mod, why in (("venv", "pcpos env create"), ("ensurepip", "pcpos env create"), ("ssl", "https retrieval"), ("sqlite3", None), ("zlib", "bundles")):
        if why and importlib.util.find_spec(mod) is None:
            rows.append(_row("warn", "module " + mod, "absent from this Python; %s is held back%s" % (why, " (on Debian and Ubuntu: install python3-venv)" if mod in ("venv", "ensurepip") else "")))
    try:
        r = subprocess.run([sys.executable, "-m", "pip", "--version"], capture_output=True, text=True, timeout=30)
        rows.append(_row("ok" if r.returncode == 0 else "warn", "pip", r.stdout.strip().split(" from ")[0] if r.returncode == 0 else "not available to this Python; packages cannot be installed by it"))
    except (OSError, subprocess.TimeoutExpired):
        rows.append(_row("warn", "pip", "could not be run"))
    for prog in PROGRAMS:
        p = shutil.which(prog)
        rows.append(_row("ok" if p else "note", "program " + prog, p or "not found (needed only by work that uses it)"))
    proxies = sorted(k for k in os.environ if k.lower() in ("http_proxy", "https_proxy", "no_proxy", "all_proxy"))
    rows.append(_row("ok", "proxy variables", ", ".join(proxies) if proxies else "none set"))
    return rows


def network(hosts=DEFAULT_HOSTS, timeout=8):
    rows = []
    for url in hosts:
        try:
            req = urllib.request.Request(url, method="HEAD", headers={"User-Agent": "pcp-os-doctor"})
            with urllib.request.urlopen(req, timeout=timeout) as r:
                rows.append(_row("ok", "reach " + url, "HTTP %d" % r.status))
        except urllib.error.HTTPError as e:
            deny = e.headers.get("x-deny-reason") if e.headers else None
            e.close()
            if deny or e.code in (403, 407):
                rows.append(_row("warn", "reach " + url, "refused: HTTP %d%s" % (e.code, " (%s)" % deny if deny else "")))
            else:
                rows.append(_row("ok", "reach " + url, "reachable (HTTP %d to a HEAD request)" % e.code))
        except (urllib.error.URLError, OSError) as e:
            rows.append(_row("warn", "reach " + url, "unreachable: %s" % getattr(e, "reason", e)))
    return rows


def imports_of(directory):
    """Third-party modules imported by the Python files under a directory, with how many files import each."""
    std = set(getattr(sys, "stdlib_module_names", ()))
    if not std:                                     # Python 3.9 has no list of its own standard library: find it by location
        import sysconfig
        home = sysconfig.get_paths()["stdlib"]

        def is_std(name):
            if name in sys.builtin_module_names:
                return True
            try:
                spec = importlib.util.find_spec(name)
            except (ImportError, ValueError):
                return False
            origin = getattr(spec, "origin", None) or ""
            return origin in ("built-in", "frozen") or (origin.startswith(home) and "site-packages" not in origin)
    else:
        def is_std(name):
            return name in std
    directory = Path(directory)
    local = {p.stem for p in directory.rglob("*.py")} | {p.name for p in directory.rglob("*") if p.is_dir()}
    found, unreadable = {}, 0
    for p in directory.rglob("*.py"):
        try:
            tree = ast.parse(p.read_text(encoding="utf-8", errors="replace"))
        except (SyntaxError, ValueError, OSError):
            unreadable += 1; continue
        mods = set()
        for n in ast.walk(tree):
            if isinstance(n, ast.Import):
                mods |= {a.name.split(".")[0] for a in n.names}
            elif isinstance(n, ast.ImportFrom) and n.module and n.level == 0:
                mods.add(n.module.split(".")[0])
        for m in mods:
            if not is_std(m) and m not in local and not m.startswith("_"):
                found[m] = found.get(m, 0) + 1
    return found, unreadable


def requirements(directory, interpreter=None):
    interpreter = interpreter or sys.executable
    found, unreadable = imports_of(directory)
    rows, missing = [], []
    if found:
        probe = "import importlib.util,sys\nfor m in sys.argv[1:]:\n print(m, int(importlib.util.find_spec(m) is not None))"
        try:
            r = subprocess.run([interpreter, "-c", probe] + sorted(found), capture_output=True, text=True, timeout=120)
            have = {l.split()[0]: l.split()[1] == "1" for l in r.stdout.splitlines() if len(l.split()) == 2}
        except (OSError, subprocess.TimeoutExpired):
            have = {}
        for m in sorted(found):
            ok = have.get(m)
            rows.append(_row("ok" if ok else "warn", "module " + m, "%s (imported by %d file%s)" % ("available" if ok else "MISSING", found[m], "" if found[m] == 1 else "s")))
            if not ok:
                missing.append(PIP_NAMES.get(m, m))
    if unreadable:
        rows.append(_row("note", "unparsed files", "%d Python file(s) could not be parsed and were not scanned" % unreadable))
    if missing:
        pk = " ".join(sorted(set(missing)))
        rows.append(_row("warn", "to install", "pcpos env create --inherit --packages %s    (an environment inside the workspace; nothing global changes)   or: \"%s\" -m pip install %s"
                         % (pk, interpreter, pk)))
    elif found:
        rows.append(_row("ok", "requirements", "every third-party module imported under %s is available to %s" % (directory, interpreter)))
    else:
        rows.append(_row("ok", "requirements", "no third-party imports found under %s" % directory))
    return rows


def tree(directory):
    directory = Path(directory)
    if not directory.is_dir():
        raise PcposError(USAGE, "%s is not a directory" % directory)
    rels = sorted(p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file())
    longest = max((len(r) for r in rels), default=0)
    issues, seen = [], {}
    for r in rels:
        issues += ["%s: %s" % (r, w) for w in name_issues(r)]
        if r.lower() in seen:
            issues.append("%s: differs only by case from %s" % (r, seen[r.lower()]))
        seen.setdefault(r.lower(), r)
    here = len(str(directory.resolve()))
    rows = [_row("ok", "files", "%d under %s" % (len(rels), directory)),
            _row("ok" if longest + here + 1 <= 259 else "warn", "longest relative path", "%d characters; at this location the full path is %d. Without long-path support Windows fails above 259, so the folder holding this material may be at most %d characters long"
                 % (longest, longest + here + 1, max(0, 259 - longest - 1)))]
    rows.append(_row("ok" if not issues else "warn", "portable names", "no name fails on Windows, macOS or Linux" if not issues else
                     "%d issue(s), first: %s" % (len(issues), issues[0])))
    return rows


def render(rows):
    return "\n".join("%-5s %s: %s" % (r["level"], r["key"], r["detail"]) for r in rows)


def create_env(ops, packages=(), requirements_file=None, inherit=False, timeout=1800):
    """Create a Python environment inside the workspace and install into it. Nothing outside the workspace changes."""
    venv = ops.ws.local / "venv"
    if importlib.util.find_spec("venv") is None or importlib.util.find_spec("ensurepip") is None:
        raise PcposError(ENVIRONMENT, "this Python cannot create environments (the venv or ensurepip module is absent)",
                         "on Debian and Ubuntu install python3-venv; or install into the interpreter itself: \"%s\" -m pip install <packages>" % sys.executable)
    if not venv.is_dir():
        cmd = [sys.executable, "-m", "venv"] + (["--system-site-packages"] if inherit else []) + [str(venv)]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        if r.returncode != 0:
            shutil.rmtree(venv, ignore_errors=True)
            raise PcposError(ENVIRONMENT, "the environment could not be created: %s" % (r.stderr.strip().splitlines() or ["unknown error"])[-1])
    py = ops.interpreter()[0]
    args = list(packages) + (["-r", str(requirements_file)] if requirements_file else [])
    if args:
        r = subprocess.run([py, "-m", "pip", "install", "--disable-pip-version-check", "--no-input"] + args, capture_output=True, text=True, timeout=timeout)
        if r.returncode != 0:
            tail = (r.stderr.strip().splitlines() or r.stdout.strip().splitlines() or ["unknown error"])[-1]
            raise PcposError(ENVIRONMENT, "installation failed: %s" % tail,
                             "if the network is closed, packages cannot be downloaded: pcpos doctor --net shows what is reachable; --inherit reuses packages the host Python already has")
    r = subprocess.run([py, "-m", "pip", "freeze", "--all"], capture_output=True, text=True, timeout=300)
    freeze = r.stdout.encode("utf-8")
    digest = ops.ws.put_bytes(freeze)
    ver = subprocess.run([py, "-c", "import platform;print(platform.python_version())"], capture_output=True, text=True).stdout.strip()
    ev, _ = ops._write("ENVIRONMENT_RECORDED", {"python": ver, "packages": len(freeze.splitlines()), "freeze": digest, "inherit": bool(inherit),
                                                "requested": sorted(args)})
    return ev, len(freeze.splitlines())
