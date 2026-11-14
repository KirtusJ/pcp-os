"""Working with material: trees, checks, retrieval, the environment report and the command line."""
import http.server, json, os, socket, sys, threading, unittest, zipfile
from pathlib import Path
from common import Case, ROOT, PcposError, Ops
from pcpos import base, doctor, ops as ops_mod, view


class Trees(Case):
    def test_pinning_records_the_exact_state_without_copying_or_touching_it(self):
        d = self.tree("t")
        before = sorted((p.relative_to(d).as_posix(), p.read_bytes()) for p in d.rglob("*") if p.is_file())
        st = self.z.state()
        t = st.trees["t"]
        self.assertEqual((t["files"], t["stored"], t["container"]), (4, False, "DIRECTORY"))
        self.assertEqual(self.z.tree_diff("t", d), ([], [], []))
        self.assertEqual(before, sorted((p.relative_to(d).as_posix(), p.read_bytes()) for p in d.rglob("*") if p.is_file()))
        (d / "a.txt").write_bytes(b"changed"); (d / "new.txt").write_bytes(b"n"); (d / "sub" / "b.txt").unlink()
        self.assertEqual(self.z.tree_diff("t", d), (["sub/b.txt"], ["a.txt"], ["new.txt"]))

    def test_interpreter_by_products_are_left_out_and_counted(self):
        d = self.tmp / "p"; (d / "__pycache__").mkdir(parents=True)
        (d / "m.py").write_text("x=1"); (d / "__pycache__" / "m.cpython-312.pyc").write_bytes(b"\0"); (d / "old.pyc").write_bytes(b"\0")
        ev, _ = self.z.pin(d, "p")
        self.assertEqual((ev["body"]["files"], ev["body"]["excluded_byproducts"]), (1, 1))

    def test_a_workspace_inside_the_material_is_refused(self):
        d = self.tmp / "material"; d.mkdir(); (d / "f").write_text("x")
        inner, _ = Ops.init(d / "ws", "inner", "P")
        e = self.refused(inner.pin, d, "m")
        self.assertIn("outside", e.hint)

    def test_names_that_fail_on_another_system_are_reported_at_pinning(self):
        z = self.tmp / "odd.zip"
        with zipfile.ZipFile(z, "w") as f:
            for name in ("ok/a.txt", "ok/A.TXT", "aux.txt", "dir/trailing.", "x" * 200 + "/" + "y" * 60 + ".txt"):
                f.writestr(name, b"x")
        ev, issues = self.z.pin(z, "odd")
        p = ev["body"]["portability"]
        self.assertEqual((ev["body"]["container"], p["case_collisions"]), ("ZIP", 1))
        self.assertGreaterEqual(p["issues"], 3)
        self.assertEqual(p["longest_relative_path"], 265)
        self.assertEqual(p["windows_root_budget"], 0)

    def test_an_archive_is_read_in_place_and_hostile_member_names_reach_no_disk(self):
        z = self.tmp / "hostile.zip"
        with zipfile.ZipFile(z, "w") as f:
            f.writestr("../../escape.txt", b"x"); f.writestr("/abs.txt", b"y"); f.writestr("fine.txt", b"z")
        ev, issues = self.z.pin(z, "hostile")
        self.assertEqual(ev["body"]["files"], 3)
        self.assertFalse((self.tmp.parent / "escape.txt").exists())
        self.assertTrue(any("'..'" in i for i in issues))

    def test_a_tree_member_is_shown_only_if_it_is_the_pinned_bytes(self):
        d = self.tree("t")
        data, span = self.z.read("tree:t/a.txt", tree_dir=d)
        self.assertEqual(data, b"alpha\n")
        (d / "a.txt").write_bytes(b"other")
        self.refused(self.z.read, "tree:t/a.txt", tree_dir=d, code=base.INTEGRITY)
        self.refused(self.z.read, "tree:t/a.txt", code=base.USAGE)                  # by reference, and no directory given
        d2 = self.tmp / "s"; d2.mkdir(); (d2 / "k.txt").write_bytes(b"kept")
        self.z.pin(d2, "kept", store=True)
        (d2 / "k.txt").unlink()
        self.assertEqual(self.z.read("tree:kept/k.txt")[0], b"kept")


class Checks(Case):
    def test_a_check_records_what_ran_and_what_happened(self):
        d = self.tree("t")
        ev = self.z.check(["python", "-c", "import sys; print('out'); print('err', file=sys.stderr); sys.exit(2)"], name="demo", expect_exit=2)
        b = ev["body"]
        self.assertEqual((b["exit"], b["expect_exit"], b["timeout"], b["argv"][0]), (2, 2, False, "python"))
        self.assertEqual(self.ws.get(b["stdout"]).strip(), b"out")
        self.assertEqual(self.ws.get(b["stderr"]).strip(), b"err")
        self.assertEqual(b["interpreter"], "HOST_INTERPRETER")

    def test_python_means_the_interpreter_in_use_on_every_system(self):
        ev = self.z.check(["python3", "-c", "import sys; print(sys.executable)"])
        self.assertEqual(Path(self.ws.get(ev["body"]["stdout"]).decode().strip()).resolve(), Path(sys.executable).resolve())

    def test_a_check_never_adds_by_products_to_the_material(self):
        d = self.tmp / "m"; d.mkdir()
        (d / "lib.py").write_text("X = 1\n"); (d / "main.py").write_text("import lib\nprint(lib.X)\n")
        self.z.pin(d, "m")
        ev = self.z.check(["python", "main.py"], tree="m", tree_dir=str(d))
        self.assertEqual(ev["body"]["tree"], {"name": "m", "matched_before": True, "matched_after": True})
        self.assertFalse((d / "__pycache__").exists())

    def test_a_check_on_drifted_material_is_refused_and_a_check_that_changes_it_is_flagged(self):
        d = self.tmp / "m"; d.mkdir(); (d / "w.py").write_text("open('made.txt','w').write('x')\n")
        self.z.pin(d, "m")
        ev = self.z.check(["python", "w.py"], tree="m", tree_dir=str(d))
        self.assertEqual((ev["body"]["tree"]["matched_before"], ev["body"]["tree"]["matched_after"]), (True, False))
        n = len(self.ws.events()[0])
        e = self.refused(self.z.check, ["python", "w.py"], tree="m", tree_dir=str(d), clause="2.1")
        self.assertEqual(len(self.ws.events()[0]), n)
        ev = self.z.check(["python", "w.py"], tree="m", tree_dir=str(d), allow_drift=True)
        self.assertFalse(ev["body"]["tree"]["matched_before"])

    def test_timeout_and_missing_program(self):
        ev = self.z.check(["python", "-c", "import time; time.sleep(30)"], timeout=1)
        self.assertEqual((ev["body"]["timeout"], ev["body"]["exit"]), (True, None))
        n = len(self.ws.events()[0])
        e = self.refused(self.z.check, ["no-such-program-xyz"], code=base.ENVIRONMENT)
        self.assertEqual(len(self.ws.events()[0]), n)
        self.assertFalse([p for p in self.ws.local.iterdir() if p.name.startswith(("out.", "err."))])

    def test_large_output_is_cut_and_marked(self):
        old = ops_mod.MAX_OUTPUT; ops_mod.MAX_OUTPUT = 1000
        self.addCleanup(setattr, ops_mod, "MAX_OUTPUT", old)
        ev = self.z.check(["python", "-c", "print('x' * 5000)"])
        self.assertTrue(ev["body"]["truncated"])
        self.assertEqual(len(self.ws.get(ev["body"]["stdout"])), 1000)


class _Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path == "/paper":
            body = b"Theorem 1.1. The statement.\n"
            self.send_response(200); self.send_header("Content-Type", "text/plain"); self.send_header("ETag", '"v1"')
            self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
        elif self.path == "/moved":
            self.send_response(302); self.send_header("Location", "/paper"); self.end_headers()
        elif self.path == "/big":
            self.send_response(200); self.end_headers(); self.wfile.write(b"x" * 5000)
        else:
            self.send_response(404); self.end_headers()


class Retrieval(Case):
    def setUp(self):
        super().setUp()
        try:
            self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
        except OSError as e:
            self.skipTest("no local socket: %s" % e)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.addCleanup(self.srv.server_close); self.addCleanup(self.srv.shutdown)
        self.base = "http://127.0.0.1:%d" % self.srv.server_address[1]
        for k in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
            os.environ.pop(k, None)
        os.environ["no_proxy"] = "127.0.0.1"

    def test_retrieval_records_bytes_address_and_time_and_is_not_yet_inspection(self):
        ev, digest, src, n = self.z.fetch(self.base + "/moved", "paper")
        self.assertEqual((src["status"], src["final_url"], n), (200, self.base + "/paper", 28))
        a = self.z.state().artifacts[digest]
        self.assertEqual((a["origin"], a["source"]["headers"]["etag"]), ("RETRIEVED", '"v1"'))
        self.review("R"); self.z.gate("R", "G1")
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["paper"], clause="4.3")
        ev, _ = self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=["paper=Theorem 1.1, in full"])
        self.assertEqual(ev["body"]["grounding"], "EXTERNAL_INSPECTED")

    def test_a_failed_retrieval_records_nothing(self):
        n = len(self.ws.events()[0])
        self.refused(self.z.fetch, self.base + "/absent", code=base.NEGATIVE)
        self.refused(self.z.fetch, self.base + "/big", max_bytes=1000, code=base.REFUSED)
        self.refused(self.z.fetch, "file:///etc/passwd", code=base.USAGE)
        self.refused(self.z.fetch, "ftp://example.org/x", code=base.USAGE)
        s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
        e = self.refused(self.z.fetch, "http://127.0.0.1:%d/x" % port, timeout=3, code=base.ENVIRONMENT)
        self.assertIn("Offline work continues", e.hint)
        self.assertEqual(len(self.ws.events()[0]), n)


class Doctor(Case):
    need_method = False

    def test_requirements_are_found_by_reading_imports(self):
        d = self.tmp / "prog"; d.mkdir()
        (d / "a.py").write_text("import os, json\nimport numpy as np\nfrom no_such_pkg_xyz import thing\nimport helper\nimport fitz\n")
        (d / "helper.py").write_text("x = 1\n"); (d / "broken.py").write_text("def (:\n")
        found, unreadable = doctor.imports_of(d)
        self.assertEqual((sorted(found), unreadable), (["fitz", "no_such_pkg_xyz", "numpy"], 1))
        rows = doctor.requirements(d)
        text = doctor.render(rows)
        self.assertIn("module no_such_pkg_xyz: MISSING", text)
        self.assertIn("PyMuPDF", text) if "module fitz: MISSING" in text else None
        self.assertIn("env create", text)

    def test_tree_audit_gives_the_folder_budget_for_windows(self):
        d = self.tmp / "t"; (d / ("p" * 100)).mkdir(parents=True); (d / ("p" * 100) / ("f" * 90 + ".txt")).write_text("x")
        rows = {r["key"]: r for r in doctor.tree(d)}
        self.assertIn("195 characters", rows["longest relative path"]["detail"])
        self.assertIn("at most 63 characters", rows["longest relative path"]["detail"])

    def test_basics_report_runs_everywhere(self):
        rows = doctor.basics()
        self.assertEqual(rows[0]["level"], "ok")
        self.assertTrue(any(r["key"] == "pip" for r in rows))


class CommandLine(Case):
    def test_exit_codes_distinguish_refusal_not_in_use_usage_and_integrity(self):
        self.tree("t")
        self.assertEqual(self.cli("commission", "--as", "Z", "R", "--target", "t", "--claim", "c")[0], base.NOT_IN_USE)
        self.assertEqual(self.cli("adopt", "--by", "P", "--basis", "use ACR")[0], 0)
        self.assertEqual(self.cli("commission", "--as", "Z", "R", "--target", "t", "--claim", "c")[0], 0)
        self.assertEqual(self.cli("finding", "--as", "Z", "R", "A", "MAYBE")[0], base.REFUSED)
        self.assertEqual(self.cli("nonsense")[0], base.USAGE)
        self.assertEqual(self.cli("status")[0], 0)
        with open(self.ws.register_path, "ab") as f:
            f.write(b"{")
        self.assertEqual(self.cli("status")[0], base.INTEGRITY)
        self.assertEqual(self.cli("repair")[0], 0)
        self.assertEqual(self.cli("verify")[0], 0)

    def test_a_refusal_names_the_rule_and_the_next_step_in_text_and_in_json(self):
        self.adopt(); self.tree("t")
        self.cli("commission", "--as", "Z", "R", "--target", "t", "--claim", "c", "--criterion", "k")
        code, out, err = self.cli("finding", "--as", "Z", "R", "A", "SUPPORTED_IN_SCOPE")
        self.assertEqual(code, base.REFUSED)
        self.assertIn("[Core 6.1]", err); self.assertIn("next:", err)
        code, out, err = self.cli("finding", "--as", "Z", "--json", "R", "A", "SUPPORTED_IN_SCOPE")
        rec = json.loads(out)
        self.assertEqual((rec["ok"], rec["code"], rec["clause"]), (False, base.REFUSED, "Core 6.1"))

    def test_text_can_come_from_a_file_in_the_encodings_shells_produce(self):
        self.adopt(); self.tree("t")
        text = "Is the bound \u2264 3\u03c0 for every n \u2265 2?"
        for name, raw in (("utf8.txt", text.encode("utf-8")), ("bom.txt", b"\xef\xbb\xbf" + text.encode("utf-8")),
                          ("utf16.txt", text.encode("utf-16")), ("crlf.txt", (text + "\r\n").encode("utf-8"))):
            p = self.tmp / name; p.write_bytes(raw)
            label = "R-" + name.split(".")[0]
            self.assertEqual(self.cli("commission", "--as", "Z", label, "--target", "t", "--claim", "@" + str(p))[0], 0, name)
            self.assertEqual(self.z.state().reviews[label]["claims"]["A"]["text"], text, name)

    def test_the_whole_path_from_the_command_line(self):
        d = self.tree("t")
        steps = [("adopt", "--by", "P", "--basis", "Review the tree with ACR."),
                 ("commission", "--as", "Z", "R", "--target", "t", "--claim", "ok.py exits 0", "--kind", "MECHANICAL_CHECK", "--criterion", "exit status 0",
                  "--module", "qa", "--author", "G"),
                 ("gate", "--as", "Z", "R", "G1"),
                 ("check", "--as", "Z", "--name", "run-ok", "--tree", "t", "--at", str(d), "--", "python", "ok.py"),
                 ("finding", "--as", "Z", "R", "A", "SUPPORTED_IN_SCOPE", "--basis", "K1"),
                 ("objection", "--as", "G", "raise", "R", "--text", "exit status alone says little", "--claim", "A"),
                 ("objection", "--as", "Z", "resolve", "O1", "--how", "accepted; the claim is only about the exit status"),
                 ("gate", "--as", "Z", "R", "G5", "--basis", "K1"), ("gate", "--as", "Z", "R", "G6", "--note", "reported"),
                 ("close", "--as", "Z", "R", "--lifecycle", "CLOSED_COMPLETE", "--extent", "TOTAL"),
                 ("condition", "--as", "Z", "add", "--text", "Do not publish.", "--origin", "principal"),
                 ("obligation", "--as", "Z", "open", "--text", "independent check outstanding"),
                 ("verify",), ("brief", "--as", "Z", "R"), ("fidelity", "R"), ("list", "findings"), ("log",), ("anchor",)]
        for s in steps:
            code, out, err = self.cli(*s)
            self.assertEqual(code, 0, (s, err))
        code, out, _ = self.cli("list", "findings")
        self.assertIn("SEPARATE_ACTOR_DIFFERENT_LINEAGE", out)
        out_zip = self.tmp / "b.zip"
        self.assertEqual(self.cli("export", "--out", str(out_zip))[0], 0)
        self.assertEqual(self.cli("bundle", "verify", str(out_zip), ws=False)[0], 0)

    def test_halt_from_the_command_line_needs_no_argument_when_one_person_directs(self):
        self.review("R")
        code, out, _ = self.cli("halt")
        self.assertEqual(code, 0)
        self.assertIn("ACR use has stopped", out); self.assertIn("R", out)
        self.assertEqual(self.cli("gate", "--as", "Z", "R", "G1")[0], base.NOT_IN_USE)

    def test_the_product_label_is_presentation_only(self):
        lab = self.tmp / "label.json"; lab.write_text(json.dumps({"label": "Some Product OS"}))
        code, plain, _ = self.cli("version", ws=False)
        code, branded, _ = self.cli("version", "--label-file", str(lab), ws=False)
        self.assertTrue(plain.startswith("PCP OS 1.0")); self.assertTrue(branded.startswith("Some Product OS 1.0"))
        self.assertEqual(plain.splitlines()[1:], branded.splitlines()[1:])
        os.environ["PCPOS_LABEL_FILE"] = str(lab)
        raw = self.ws.register_path.read_bytes()
        self.z.note("x")
        self.assertNotIn(b"Some Product", self.ws.register_path.read_bytes())
        self.assertNotIn("Some Product", self.cli("status")[1])

    def test_an_actor_is_never_guessed_among_several(self):
        self.assertEqual(self.cli("note", "x")[0], base.USAGE)
        self.assertEqual(self.cli("config", "actor", "G")[0], 0)
        self.assertEqual(self.cli("note", "x")[0], 0)
        self.assertEqual(self.ws.events()[0][-1]["actor"], "G")


if __name__ == "__main__":
    unittest.main()
