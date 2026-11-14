"""The record: canonical form, durable writes, the hash chain, the lock, bundles and anchors."""
import json, multiprocessing, os, shutil, subprocess, sys, unittest, zipfile
from pathlib import Path
from common import Case, ROOT, PcposError, Ops
from pcpos import base, bag, model
from pcpos.store import Workspace


def _appender(args):
    root, tag, n = args
    sys.path.insert(0, str(ROOT))
    from pcpos.store import Workspace as W
    w = W(root)
    for i in range(n):
        w.append("NOTE", {"text": "%s-%d" % (tag, i)}, "P")
    return n


class Canonical(Case):
    need_method = False

    def test_one_value_one_byte_form(self):
        self.assertEqual(base.canon({"b": 1, "a": [2, "é"]}), b'{"a":[2,"\\u00e9"],"b":1}')
        self.assertEqual(base.canon({"a": 1, "b": 2}), base.canon(json.loads('{"b":2,"a":1}')))

    def test_strict_parser_rejects_what_a_lenient_one_accepts(self):
        for bad in ('{"a":1,"a":2}', '{"a":NaN}', '{"a":Infinity}'):
            with self.assertRaises(ValueError):
                base.strict_loads(bad)

    def test_floats_are_not_recorded(self):
        n = len(self.ws.events()[0])
        with self.assertRaises(PcposError):
            self.ws.append("NOTE", {"x": 0.5}, "P")
        self.assertEqual(len(self.ws.events()[0]), n)

    def test_names_that_fail_somewhere(self):
        for rel in ("a/CON.txt", "nul", "a:b", "x/trailing.", "x/space ", "a/../b", "q?", "COM1.tar.gz", "caf\u00e9.txt"):
            self.assertTrue(base.name_issues(rel), rel)
        for rel in ("a/b.txt", "CONTENTS.md", ".hidden", "x-y_z.1"):
            self.assertFalse(base.name_issues(rel), rel)

    def test_atomic_write_leaves_no_partial_file(self):
        p = self.tmp / "f.bin"
        base.atomic_write(p, b"one")
        base.atomic_write(p, b"two")
        self.assertEqual(p.read_bytes(), b"two")
        self.assertEqual([x.name for x in self.tmp.iterdir() if x.name.startswith("f.bin")], ["f.bin"])


class Chain(Case):
    need_method = False

    def test_any_edit_of_history_is_detected(self):
        self.z.note("one"); self.z.note("two"); self.z.note("three")
        raw = self.ws.register_path.read_bytes()
        lines = raw.split(b"\n")[:-1]
        cases = {"changed byte": raw.replace(b'"one"', b'"0ne"'), "removed line": b"\n".join(lines[:5] + lines[6:]) + b"\n",
                 "swapped lines": b"\n".join(lines[:5] + [lines[6], lines[5]]) + b"\n", "duplicated line": b"\n".join(lines + [lines[-1]]) + b"\n",
                 "pretty printed": raw.replace(b'{"actor"', b'{ "actor"', 1)}
        for name, altered in cases.items():
            self.ws.register_path.write_bytes(altered)
            with self.assertRaises(PcposError, msg=name) as cm:
                self.ws.events()
            self.assertEqual(cm.exception.code, base.INTEGRITY, name)
        self.ws.register_path.write_bytes(raw)
        self.assertEqual(len(self.ws.events()[0]), len(lines))

    def test_interrupted_write_is_reported_then_repaired_without_touching_complete_lines(self):
        self.z.note("kept")
        raw = self.ws.register_path.read_bytes()
        with open(self.ws.register_path, "ab") as f:
            f.write(b'{"actor":"Z","auth":"DECL')
        e = self.refused(self.ws.events, code=base.INTEGRITY)
        self.assertIn("repair", e.hint)
        self.refused(self.z.note, "blocked", code=base.INTEGRITY)
        self.assertEqual(self.ws.repair(), 25)
        self.assertEqual(self.ws.register_path.read_bytes(), raw)
        self.assertEqual(self.ws.repair(), 0)

    def test_a_held_lock_blocks_and_says_how_to_clear_it(self):
        os.environ["PCPOS_LOCK_WAIT"] = "0.2"
        lock = self.ws._lock()
        e = self.refused(self.z.note, "x", code=base.ENVIRONMENT)
        self.assertIn("unlock", e.hint)
        lock.unlink()
        self.z.note("x")
        self.assertFalse((self.ws.local / "lock").exists())

    def test_concurrent_writers_lose_nothing(self):
        os.environ["PCPOS_LOCK_WAIT"] = "60"
        before = len(self.ws.events()[0])
        ctx = multiprocessing.get_context("spawn")
        with ctx.Pool(4) as pool:
            done = pool.map(_appender, [(str(self.ws.root), "w%d" % i, 12) for i in range(4)])
        events, _ = self.ws.events()
        self.assertEqual(len(events), before + sum(done))
        self.assertEqual(len({e["body"]["text"] for e in events if e["kind"] == "NOTE"}), 48)

    def test_an_oversized_event_is_refused_whole(self):
        n = len(self.ws.events()[0])
        self.refused(self.ws.append, "NOTE", {"text": "x" * 70000}, "P", code=base.USAGE)
        self.assertEqual(len(self.ws.events()[0]), n)

    def test_a_refused_event_writes_nothing(self):
        raw = self.ws.register_path.read_bytes()
        self.refused(self.z.gate, "NOPE", "G1", code=base.NOT_IN_USE)
        self.assertEqual(self.ws.register_path.read_bytes(), raw)

    def test_an_event_kind_from_another_version_is_not_guessed(self):
        self.ws.append("FUTURE_KIND", {"a": 1}, "P")
        self.refused(self.z.state, code=base.INTEGRITY)

    def test_a_declared_extension_is_carried_and_not_interpreted(self):
        n = len(self.ws.events()[0])
        self.ws.append("X_LOCAL", {"a": 1}, "P")
        self.assertEqual(self.z.state().seq, n)
        self.assertTrue(self.z.verify()["ok"])

    def test_altered_object_is_caught(self):
        _, d = self.z.add_bytes(b"content", "c1")
        self.ws.object_path(d).write_bytes(b"c0ntent")
        self.refused(self.ws.get, d, code=base.INTEGRITY)
        self.assertEqual(self.z.verify()["altered_objects"], [d])

    def test_a_workspace_is_never_created_inside_existing_material(self):
        (self.tmp / "full").mkdir(); (self.tmp / "full" / "x").write_text("x")
        self.refused(Workspace.create, self.tmp / "full", code=base.USAGE)
        self.refused(Workspace.create, self.ws.root, code=base.USAGE)


class Replay(Case):
    def _forge(self, kind, body, actor="Z"):
        """Write an event with a correct chain but without the guards, as an editor outside the program could."""
        events, head = self.ws.events()
        ev = {"seq": len(events), "prev": head, "ts": "2026-01-01T00:00:00Z", "actor": actor, "auth": "DECLARED", "kind": kind, "body": body}
        with open(self.ws.register_path, "ab") as f:
            f.write(base.canon(ev) + b"\n")

    def test_a_correctly_chained_event_that_no_guard_would_pass_is_reported(self):
        self.review("R"); self.z.gate("R", "G1")
        self._forge("FINDING_RECORDED", {"id": "F1", "review": "R", "claim": "A", "disposition": "VERIFIED_TRUE", "basis": [], "contrary": [],
                                         "conditions": [], "estimate": None, "left_open": None, "grounding": "EXTERNAL_INSPECTED",
                                         "grounding_observed": "EXTERNAL_INSPECTED", "execution": "EXECUTED", "noncritical": [],
                                         "relation": {"label": "INDEPENDENT"}, "supersedes": None, "session": 1})
        rep = self.z.verify()
        self.assertFalse(rep["ok"])
        self.assertEqual(rep["guard_problems"][0][1], "FINDING_RECORDED")

    def test_computed_fields_cannot_be_edited_upward(self):
        self.review("R"); self.z.gate("R", "G1")
        self.z.finding("R", "A", "UNRESOLVED", grounding="RECALL_ONLY")
        raw = self.ws.register_path.read_bytes()
        events, _ = self.ws.events()
        last = dict(events[-1]); last["body"] = dict(last["body"], execution="EXECUTED")
        self.ws.register_path.write_bytes(b"\n".join(raw.split(b"\n")[:-2] + [base.canon(last)]) + b"\n")
        rep = self.z.verify()
        self.assertFalse(rep["ok"])
        self.assertIn("execution", rep["guard_problems"][0][2])

    def test_a_clean_record_replays_clean(self):
        self.review("R"); self.z.gate("R", "G1")
        self.z.finding("R", "A", "CONDITIONAL", basis=["tree:t-R/a.txt=all of it"], conditions=["the premise holds"])
        rep = self.z.verify()
        self.assertTrue(rep["ok"], rep)


class Bundles(Case):
    def _work(self):
        self.review("R"); self.z.gate("R", "G1")
        self.z.add_bytes(b"proof text\n", "proof")
        self.z.finding("R", "A", "CONDITIONAL", basis=["proof=all of it"], conditions=["premise p"])

    def test_export_is_reproducible_and_passes_an_independent_bagit_validator(self):
        self._work()
        os.environ["PCPOS_FIXED_TIME"] = "2026-11-12T00:00:00Z"
        p1, d1, n, head = bag.export(self.z, self.tmp / "one.zip")
        p2, d2, _, _ = bag.export(self.z, self.tmp / "two.zip")
        self.assertEqual(d1, d2)
        rep = bag.Bundle(p1).verify()
        self.assertEqual((rep["events"], rep["head"], rep["guard_problems"]), (n, head, []))
        try:
            import bagit
        except ImportError:
            self.skipTest("the bagit library is not installed; the bundle was checked by this program only")
        out = self.tmp / "x"
        zipfile.ZipFile(p1).extractall(out)
        self.assertTrue(bagit.Bag(str(next(out.iterdir()))).validate())

    def test_every_tampering_of_a_bundle_is_refused(self):
        self._work()
        p, _, _, _ = bag.export(self.z, self.tmp / "b.zip")
        top = zipfile.ZipFile(p).namelist()[0].split("/")[0]

        def variant(name, edit):
            q = self.tmp / (name + ".zip")
            with zipfile.ZipFile(p) as zin, zipfile.ZipFile(q, "w") as zout:
                for i in zin.infolist():
                    data = zin.read(i)
                    res = edit(i.filename, data)
                    if res is not None:
                        zout.writestr(i.filename, res)
                for extra_name, extra in getattr(edit, "extra", {}).items():
                    zout.writestr(extra_name, extra)
            return q

        def flip_object(n, d): return (d[:-1] + b"X") if "/data/objects/" in n and b"proof" in d else d
        def flip_register(n, d): return d.replace(b"premise p", b"premise q") if n.endswith("register.jsonl") else d
        def drop_object(n, d): return None if "/data/objects/" in n and b"proof" in d else d
        def add_unlisted(n, d): return d
        add_unlisted.extra = {top + "/data/extra.txt": b"unlisted"}
        def escape(n, d): return d
        escape.extra = {top + "/../evil.txt": b"x"}
        def second_top(n, d): return d
        second_top.extra = {"other/file.txt": b"x"}
        for name, edit in (("object", flip_object), ("register", flip_register), ("dropped", drop_object), ("unlisted", add_unlisted),
                           ("escape", escape), ("second-top", second_top)):
            q = variant(name, edit)
            with self.assertRaises(PcposError, msg=name) as cm:
                bag.Bundle(q).verify()
            self.assertEqual(cm.exception.code, base.INTEGRITY, name)

    def test_admission_takes_material_and_never_the_other_party_s_findings(self):
        self._work()
        p, digest, n, head = bag.export(self.z, self.tmp / "z.zip")
        other, _ = Ops.init(self.tmp / "ws2", "t2", "P")
        other.participant_add("A", lineage="family-two")
        a = Ops(other.ws, "A")
        ev, rep, compat, taken = bag.admit(a, p, "Z1")
        self.assertEqual((compat, taken), ("SAME_METHOD", 1))
        st = a.state()
        self.assertEqual(st.findings, {})
        self.assertEqual(st.submissions["Z1"]["head"], head)
        art = st.artifacts[base.sha_bytes(b"proof text\n")]
        self.assertEqual((art["origin"], art["via_submission"], art["origin_there"]), ("SUPPLIED", "Z1", "PRODUCED"))
        self.refused(bag.admit, a, p, "Z1")
        self.assertTrue(a.verify()["ok"])

    def test_a_submission_under_another_release_is_marked_and_not_silently_equated(self):
        self.z.method_declare("a" * 64, "b" * 64, "an earlier packet")
        p, _, _, _ = bag.export(self.z, self.tmp / "old.zip")
        other, _ = Ops.init(self.tmp / "ws3", "t3", "P")
        _, _, compat, _ = bag.admit(other, p, "OLD")
        self.assertEqual(compat, "DIFFERENT_METHOD_UNKNOWN")
        p2, _, _, _ = bag.export(self.z, self.tmp / "old2.zip")
        _, _, compat, _ = bag.admit(other, p2, "OLD2", "read both; only section 2.1 differs in wording")
        self.assertEqual(compat, "DIFFERENT_METHOD_ASSESSED")

    def test_anchor_detects_rewritten_and_shortened_history(self):
        self.z.note("one")
        anchor = self.z.anchor()
        self.assertEqual(self.z.verify(anchor=anchor)["anchor"], "EQUAL")
        self.z.note("two")
        self.assertEqual(self.z.verify(anchor=anchor)["anchor"], "ANCESTOR")
        later = self.z.anchor()
        # rewrite the whole register consistently: same length, different early content
        events, _ = self.ws.events()
        prev, out = base.ZERO, []
        for ev in events:
            ev = dict(ev, prev=prev)
            if ev["kind"] == "NOTE" and ev["body"]["text"] == "one":
                ev["body"] = {"text": "0ne"}
            line = base.canon(ev); out.append(line); prev = base.sha_bytes(line)
        self.ws.register_path.write_bytes(b"\n".join(out) + b"\n")
        self.ws.events()                                             # the chain itself is consistent
        rep = self.z.verify(anchor=anchor)
        self.assertEqual(rep["anchor"], "HISTORY_DIFFERS"); self.assertFalse(rep["ok"])
        self.ws.register_path.write_bytes(b"\n".join(out[:3]) + b"\n")
        self.assertEqual(self.z.verify(anchor=later)["anchor"], "REGISTER_IS_SHORTER")


if __name__ == "__main__":
    unittest.main()
