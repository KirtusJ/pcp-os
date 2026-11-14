"""This implementation against the native one, case by case.

The native implementation in Rust is the authority. It writes, under `vectors/` of the Rust workspace, a
register, a file of (state, offered event) cases with its answer to each, and every state machine's table.
These tests require this implementation to give the same answers. Set PCPOS_VECTORS to the directory, or
keep the Rust workspace beside this folder. Each comparison requires its own vector files.
"""
import copy, json, os, unittest
from pathlib import Path
from pcpos import model, machines
from pcpos.base import PcposError, canon, sha_bytes, strict_loads

HERE = Path(__file__).resolve().parent.parent
# Prefer the complete generated set if both workspace layouts are present.
VECTOR_DIRS = (HERE.parent / "PCP_OS_Rust" / "vectors", HERE.parent / "os-rust" / "vectors")
VECTORS = Path(os.environ.get("PCPOS_VECTORS") or next(
    (d for filename in ("guard_cases.jsonl", "machines.tsv", "register_scenario.jsonl")
     for d in VECTOR_DIRS if (d / filename).is_file()), VECTOR_DIRS[0]))
LONG = "A" * 9000


def expand(v):
    if v == "@LONG":
        return LONG
    if isinstance(v, list):
        return [expand(x) for x in v]
    if isinstance(v, dict):
        return {k: expand(x) for k, x in v.items()}
    return v


def needs_vectors(*names):
    missing = [name for name in names if not (VECTORS / name).is_file()]
    return unittest.skipIf(missing, "missing native vectors: " + ", ".join(missing))


class Native(unittest.TestCase):
    @needs_vectors("machines.tsv")
    def test_machine_tables_equal_the_native_tables(self):
        self.assertEqual(machines.export(), (VECTORS / "machines.tsv").read_text())

    @needs_vectors("register_scenario.jsonl", "register_scenario.expected.json")
    def test_the_native_register_replays_without_a_problem(self):
        lines = (VECTORS / "register_scenario.jsonl").read_bytes().split(b"\n")[:-1]
        events = [strict_loads(l) for l in lines]
        self.assertEqual([canon(e) for e in events], lines)
        state, problems = model.fold(events, check=True)
        self.assertEqual(problems, [])
        want = json.loads((VECTORS / "register_scenario.expected.json").read_text())
        self.assertEqual((len(events), sha_bytes(lines[-1])), (want["events"], want["head"]))

    @needs_vectors("register_scenario.jsonl", "guard_cases.jsonl")
    def test_the_guards_answer_as_the_native_guards_do(self):
        events = [strict_loads(l) for l in (VECTORS / "register_scenario.jsonl").read_bytes().split(b"\n")[:-1]]
        states = [model.fold(events[:n]) for n in range(len(events) + 1)]
        total, verdict_only, differences = 0, 0, []
        for line in (VECTORS / "guard_cases.jsonl").read_bytes().split(b"\n")[:-1]:
            case = strict_loads(line)
            if "e" in case:
                ev = events[case["e"]]
                kind, actor, body = ev["kind"], ev["actor"], ev["body"]
            else:
                kind, actor, body = case["kind"], case["actor"], expand(case["body"])
            total += 1
            try:
                got = model.guard(copy.deepcopy(states[case["n"]]), kind, copy.deepcopy(body), actor)
                mine = ("ACCEPT", canon(got))
            except PcposError as e:
                mine = ("REFUSE", e.code, e.clause)
            if case["verdict"] == "ACCEPT":
                theirs = ("ACCEPT", canon(expand(case.get("out", body))))
            else:
                theirs = ("REFUSE", case["code"], case["clause"])
            if mine[0] == theirs[0] == "REFUSE" and "FORM" in (mine[2], theirs[2]):
                verdict_only += 1          # a field of the wrong type: the verdict is compared, the clause is not
            elif mine != theirs:
                differences.append((case["n"], kind, mine[:1] + mine[1:][:2], theirs[:1]))
        self.assertGreater(total, 16000)
        self.assertEqual(differences[:10], [], "%d differences" % len(differences))


if __name__ == "__main__":
    unittest.main()
