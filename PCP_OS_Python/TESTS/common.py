"""Shared fixtures for the PCP OS tests. Standard library only."""
import atexit
import io, os, shutil, sys, tempfile, unittest, contextlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
sys.dont_write_bytecode = True
os.environ.setdefault("PCPOS_LOCK_WAIT", "5")

from pcpos import method as method_mod                      # noqa: E402
from pcpos.base import PcposError                           # noqa: E402
from pcpos.ops import Ops                                   # noqa: E402
from pcpos import cli                                       # noqa: E402

_METHOD = {}


def method_dir():
    """An extracted, verified ACR packet for the tests, or None when the packet is not beside this one."""
    if "path" not in _METHOD:
        found = method_mod.locate(None, os.environ.get("PCPOS_TEST_METHOD"))
        if found is None:
            _METHOD["path"] = None
        elif found.is_file():
            tmp = tempfile.mkdtemp(prefix="pcpos-method-")
            # Removed when the tests end. Until build 22 it was left behind, and every run of the tests left a copy of the
            # packet, its probes and their criteria in the system's temporary folder, where a session being measured could read it.
            atexit.register(shutil.rmtree, tmp, True)
            _METHOD["path"], _ = method_mod.install_from_zip(found, tmp)
            _METHOD["zip"] = found
        else:
            _METHOD["path"] = found
    return _METHOD["path"]


def method_zip():
    method_dir()
    return _METHOD.get("zip") or method_mod.locate(None, None)


class Case(unittest.TestCase):
    """Each test gets an empty directory and, where the ACR packet is available, a workspace with two participants."""
    need_method = True

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pcpos-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self._env = dict(os.environ)
        self.addCleanup(self._restore)
        for k in ("PCPOS_WS", "PCPOS_ACTOR", "PCPOS_LABEL_FILE"):
            os.environ.pop(k, None)
        md = method_dir()
        if self.need_method and md is None:
            self.skipTest("the ACR Implementation packet is not beside this packet")
        if md is not None:
            os.environ["PCPOS_METHOD"] = str(md)
        self.ops, _ = Ops.init(self.tmp / "ws", "t", "P", "Principal Person")
        self.ops.participant_add("Z", lineage="family-one", model_name="model-z")
        self.ops.participant_add("G", lineage="family-two", model_name="model-g")
        self.z = Ops(self.ops.ws, "Z")
        self.g = Ops(self.ops.ws, "G")
        self.ws = self.ops.ws

    def _restore(self):
        os.environ.clear(); os.environ.update(self._env)

    # ---- helpers ---------------------------------------------------------------
    def refused(self, fn, *a, code=3, clause=None, **kw):
        with self.assertRaises(PcposError) as cm:
            fn(*a, **kw)
        self.assertEqual(cm.exception.code, code, cm.exception.message)
        if clause:
            self.assertIn(clause, cm.exception.clause or "", cm.exception.message)
        return cm.exception

    def adopt(self):
        self.ops.adopt("P", "Please use ACR for this.")

    def tree(self, name="t1", files=None):
        d = self.tmp / ("src-" + name)
        for rel, data in (files or {"a.txt": b"alpha\n", "sub/b.txt": b"beta\n", "ok.py": b"import sys\nsys.exit(0)\n",
                                    "bad.py": b"import sys\nsys.exit(3)\n"}).items():
            (d / rel).parent.mkdir(parents=True, exist_ok=True)
            (d / rel).write_bytes(data)
        self.z.pin(d, name)
        return d

    def review(self, label="R", kind="UNSPECIFIED", criterion="the stated criterion", author=None, procedure="FOCUSED", **kw):
        d = self.tree("t-" + label)
        self.adopt() if not self.z.state().adopted else None
        self.z.commission(label, "t-" + label, [{"label": "A", "text": "The claim as posed.", "origin": "COMMISSIONER",
                                                "criterion": criterion, "kind": kind}], procedure, author=author, **kw)
        return d

    def cli(self, *argv, ws=True):
        """Run the command line in-process; returns (exit code, stdout, stderr)."""
        out, err = io.StringIO(), io.StringIO()
        args = (["--ws", str(self.ws.root)] if ws else [])
        argv = list(argv)
        full = argv[:1] + args + argv[1:] if ws else argv
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = cli.main(full)
        return code, out.getvalue(), err.getvalue()
