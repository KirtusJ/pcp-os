"""Run with python sqlite_replay.py PATH_TO_KEYROOK. Uses the standard SQLite client."""
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile


def main():
    binary = Path(sys.argv[1]).resolve()

    def cli(*args, success=True):
        completed = subprocess.run([str(binary), *map(str, args)], capture_output=True, text=True)
        assert completed.returncode == (0 if success else 2), (args, completed.stderr)
        return completed.stdout if success else completed.stderr

    with tempfile.TemporaryDirectory(prefix="keyrook-") as work:
        root = Path(work)
        session, fresh = root / "first", root / "fresh"
        cli("new", session)
        cli("new", fresh)
        cli("new", session, success=False)
        empty = {"runs": [0, 0], "wins": [0, 0], "work": [0, 0], "next": 0}
        assert json.loads(cli("stats", fresh, 8)) == empty
        assert json.loads(cli("stats", session, 8)) == empty
        results = []
        for seed, depth, strategy in [(0, 0, 0), (42, 2, 1), (31415, 3, 1)]:
            row = json.loads(cli("train", session, 8, seed, depth))
            assert row["strategy"] == strategy and row["verified"] and row["nodes"] == 2**(depth + 1) - 1
            results.append(row)
        assert json.loads(cli("stats", fresh, 8)) == empty
        cli("train", fresh, 8, 0, 0)
        before = json.loads(cli("stats", session, 8))
        assert "already trained" in cli("train", session, 8, 0, 3, success=False)
        assert json.loads(cli("stats", session, 8)) == before
        cli("train", session, 4, 0, 1)
        exported = root / "export.sqlite"
        cli("export-sqlite", session, exported)
        original = exported.read_bytes()
        cli("export-sqlite", fresh, exported, success=False)
        assert exported.read_bytes() == original
        with sqlite3.connect(exported) as sql:
            assert sql.execute("PRAGMA integrity_check").fetchone() == ("ok",)
            assert sql.execute("PRAGMA user_version").fetchone() == (1,)
            assert sql.execute("SELECT count(*) FROM episodes").fetchone() == (4,)
            for bits in (4, 6, 8):
                m = json.loads(cli("stats", session, bits))
                for policy in range(2):
                    expected = tuple(m[k][policy] for k in ("runs", "wins", "work"))
                    assert sql.execute("SELECT runs,wins,work,next FROM stats WHERE bits=? AND strategy=?",
                                       (bits, policy)).fetchone() == (*expected, m["next"])
                    assert sql.execute("SELECT count(*),coalesce(sum(verified),0),coalesce(sum(work),0) "
                                       "FROM episodes WHERE bits=? AND strategy=?", (bits, policy)).fetchone() == expected
            for row in results:
                expected = tuple(row[k] for k in ("strategy", "verified", "work", "nodes", "candidates"))
                assert sql.execute("SELECT strategy,verified,work,nodes,candidates FROM episodes WHERE bits=8 AND seed=?",
                                   (row["seed"],)).fetchone() == expected
        sql.close()
        # A bad legacy row aborts the whole migration. After repair, both old rows survive one import.
        legacy = root / "legacy"
        for bits, seed, decision in [(4, 0, "0 0"), (8, 42, "1 9")]:
            entry = legacy / f"v1-{bits}" / f"episode-{seed}"
            entry.mkdir(parents=True)
            (entry / "decision").write_text(decision)
        assert "legacy strategy" in cli("stats", legacy, 4, success=False)
        (legacy / "v1-8" / "episode-42" / "decision").write_text("1 1")
        assert json.loads(cli("stats", legacy, 4))["runs"] == [1, 0]
        assert json.loads(cli("stats", legacy, 8))["runs"] == [0, 1]
        assert "already trained" in cli("train", legacy, 4, 0, 1, success=False)
        print(json.dumps({"sqlite": sqlite3.sqlite_version, "training": results, "reopened_stats": before}))


if __name__ == "__main__":
    main()
