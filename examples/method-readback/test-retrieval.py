"""Exercise Build 22 assessor commands (synthetic answers only) against both released adapters."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

PREPARE = Path(__file__).resolve().with_name("prepare-retrieval.py")


class Retrieval(unittest.TestCase):
    def setUp(self):
        workspace = tempfile.TemporaryDirectory()
        self.addCleanup(workspace.cleanup)
        self.work = Path(workspace.name)
        self.lookup = self.text / "TOOLS/acr.py"

    def prepare(self, method, output, lookup=None):
        return subprocess.run([sys.executable, "-I", "-B", PREPARE, "--method", method,
                               "--lookup", lookup or self.lookup, "--out", output],
                              capture_output=True, text=True)

    def test_both_adapters(self):
        prompts = []
        for name, method in (("text", self.text), ("os", self.os_method)):
            with self.subTest(adapter=name):
                kit = self.work / name
                run = self.prepare(method, kit)
                self.assertEqual(run.returncode, 0, run.stderr)
                command = [sys.executable, "-I", "-B", "TOOLS/calibrate.py"]
                subprocess.run([*command, "items", "--seed", "19", "--out", "items.json",
                                "--key", "key.json"], cwd=kit, check=True)
                prompts.append(subprocess.check_output([*command, "prompt", "items.json"], cwd=kit))
                items = json.loads((kit / "items.json").read_bytes())["items"]
                self.assertEqual(len(items), 22)
                self.assertTrue(all(set(item) == {"id", "ask"} for item in items))
                for item in items:
                    self.assertIn(item["ask"].encode(), prompts[-1])
                key = json.loads((kit / "key.json").read_bytes())["key"]
                answers = []
                for item in items:
                    row = key[item["id"]]
                    if row["kind"] == "PROVISION":
                        self.assertNotIn(row["text"].encode(), prompts[-1])
                        answer = {"kind": "TEXT", "text": row["text"]}
                    else:
                        answer = {"kind": "DEFINED" if row["kind"] == "TERM" else "NOT_DEFINED"}
                    answers.append(dict(answer, id=item["id"]))

                def score(extent="full"):
                    (kit / "answers.json").write_text(json.dumps(answers))
                    return json.loads(subprocess.check_output(
                        [*command, "score", "items.json", "key.json", "answers.json", "--extent", extent],
                        cwd=kit))

                scored = score()
                self.assertEqual((scored["for"], scored["against"], scored["unscored"]), (22, 0, 0))
                # Characterize the immutable Build 22 scorer (issue #187).
                restricted = score("none")
                self.assertEqual(restricted["by_category"]["EXACT"], 10)
                print(f"{name}: Build 22 credited 10 exact provision answers with extent none", flush=True)
                next(answer for answer in answers if answer["kind"] == "TEXT")["text"] = "Synthetic wrong answer."
                scored = score()
                self.assertEqual((scored["for"], scored["against"], scored["by_category"]["INEXACT"]),
                                 (21, 1, 1))
                again = self.prepare(method, kit)
                self.assertNotEqual(again.returncode, 0)
                self.assertEqual(score(), scored)
        self.assertEqual(prompts[0], prompts[1])

    def test_mismatched_inputs(self):
        staged = self.work / "method"
        shutil.copytree(self.os_method, staged)
        output = self.work / "kit"
        for rel in ("MANIFEST.json", "REFERENCE/Defined_Terms.csv", "MODULES/01_Core/SOURCE/Specification.md"):
            with self.subTest(path=rel):
                target = staged / rel
                original = target.read_bytes()
                target.write_bytes(original + b" ")
                run = self.prepare(staged, output)
                self.assertNotEqual(run.returncode, 0)
                self.assertIn("manifest" if rel == "MANIFEST.json" else rel, run.stderr)
                self.assertFalse(output.exists())
                target.write_bytes(original)
        wrong = self.work / "wrong.py"
        wrong.write_text("raise RuntimeError('substitute lookup executed')\n")
        run = self.prepare(staged, output, wrong)
        self.assertNotEqual(run.returncode, 0)
        self.assertIn("Build 22 digest mismatch for TOOLS/acr.py", run.stderr)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--text-method", type=Path, required=True)
    parser.add_argument("--os-method", type=Path, required=True)
    args = parser.parse_args()
    Retrieval.text = args.text_method.resolve()
    Retrieval.os_method = args.os_method.resolve()
    unittest.main(argv=[sys.argv[0]], verbosity=2)
