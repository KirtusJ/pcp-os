"""Replay Director 3's installed pending-edit evidence against the public #197 checker."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import sys
import tempfile
from uuid import UUID


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checker", type=Path, help="os-docs/tailorbird-stop at b3c7cb67")
    parser.add_argument("--factory", type=Path, help="optional pinned factory raw disk for full-image replay")
    args = parser.parse_args()
    sys.path.insert(0, str(args.checker.resolve()))
    import check

    evidence = Path(__file__).resolve().parent
    result = json.loads((evidence / "result.json").read_bytes())
    require(result["passed"] is True, "incomplete qualification")
    factory = result["factory"]
    require(factory["disk_bytes"] == 1073741824 and factory["factory_sha256"] ==
            "2229b91baeb20892748915443f704b64dcf53869a54d5ccef957bcfd1e050563",
            "unexpected factory disk")
    require(factory["source"] == "6cc994667c4a9826a70c11fb2c4d35f593d4342e"
            and factory["manifest_sha256"] == "c116fdf529feffe30cb004a3f9adae860a0e3c27881a456095e2b72c901b7455",
            "unexpected source or manifest")
    first, cold = result["boots"]
    require(first["name"] == "first" and cold["name"] == "cold", "unexpected boot order")
    require(UUID(first["uuid"]) != UUID(cold["uuid"]), "duplicate boot identity")
    require(first["kill_requested"] is True and first["exit_before_cleanup"] == first["exit_code"] == -9,
            "missing intentional interruption")
    require(cold["kill_requested"] is False and cold["exit_before_cleanup"] == cold["exit_code"] == 33,
            "cold guest did not finish normally")
    launch = b"starting Tailorbird (177232 bytes, SHA-256 b5acb971d222184970b647d13e79cedf60bba681151cc40e0ae12cfb9503c465)"
    traces = [[check.STAGED, check.SAVED, check.STAGED],
              [check.STAGED, check.ACCEPTED, check.STOPPED, check.RAN, check.STAGED, check.SAVED, check.RAN]]
    for count, (boot, trace) in enumerate(zip((first, cold), traces)):
        path = evidence / (boot["name"] + ".serial")
        raw = path.read_bytes()
        require(digest(raw) == boot["serial_sha256"] and boot["qmp_uuid_verified"] is True,
                "serial or QMP receipt mismatch")
        markers = [b"PCP OS EFI amd64 entry, native 64-bit handoff",
                   b"070ab9e608cde750c3873fb39b2af607d6349e31ef8db8c0f7ff8b5c669dd19f (measured; equal to the value the loader named)",
                   f"Persistent application store mounted: {count} file(s)".encode(),
                   b"PCP OS launcher ready:", launch]
        cursor = 0
        for marker in markers:
            at = raw.find(marker, cursor)
            require(at >= cursor, f"missing or out-of-order marker {marker!r}")
            cursor = at + len(marker)
        require(check.desktop_startup(raw, launch) is not None, "incomplete desktop handshake")
        check.check_trace(path, trace)
    cold_lines = (evidence / "cold.serial").read_bytes().splitlines()
    require(any(b"Tailorbird ended DONE: exit 0" in l for l in cold_lines)
            and any(b"ALL SELF-TESTS PASS" in l for l in cold_lines), "missing normal guest exit")
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "capture"
        for label, generation, source in [
            ("factory", 1, None), ("approved", 2, b"print original"),
            ("after_interrupt", 2, b"print original"), ("late", 2, b"print original"),
            ("replacement", 3, b"print replacement"),
        ]:
            raw = gzip.decompress((evidence / (label + ".store.gz")).read_bytes())
            require(len(raw) == (1 + 2 * check.SLOT_SECTORS) * 512, "store length")
            path.write_bytes(raw)
            state = check.checkpoint(path, 0, source)
            require(state == result["stores"][label] and state["generation"] == generation,
                    f"unexpected {label} store")
        stores = result["stores"]
        require(stores["approved"] == stores["after_interrupt"] == stores["late"],
                "committed store changed before fresh approval")
        for expected in ("original", "replacement"):
            frame = result["frames"][expected]
            ppm = gzip.decompress((evidence / (expected + ".ppm.gz")).read_bytes())
            require(digest(ppm) == frame["sha256"], "capture digest")
            path.write_bytes(ppm)
            pixels = check.screen.read_frame(path)
            row = check.screen.matching_row(pixels, expected)
            require(row is not None and row == frame["row_y"], f"missing visible {expected} field")
    baseline = gzip.decompress((evidence / "factory.store.gz").read_bytes())
    committed = gzip.decompress((evidence / "replacement.store.gz").read_bytes())
    disk = json.loads((evidence / "disk-comparison.json").read_bytes())
    start = disk["application_partition_lbas"][0] * 512
    differences = [i for i, pair in enumerate(zip(baseline, committed)) if pair[0] != pair[1]]
    sectors = sorted({(start + i) // 512 for i in differences})
    require(len(differences) == disk["differing_bytes"] and sectors == disk["differing_lbas"],
            "captured store differs from the whole-disk comparison")
    if args.factory:
        before, after = hashlib.sha256(), hashlib.sha256()
        size = 0
        with args.factory.open("rb") as f:
            while chunk := f.read(1024 * 1024):
                before.update(chunk)
                lo = max(size, start)
                hi = min(size + len(chunk), start + len(baseline))
                if lo < hi:
                    require(chunk[lo-size:hi-size] == baseline[lo-start:hi-start],
                            "factory store differs from the capture")
                    chunk = chunk[:lo-size] + committed[lo-start:hi-start] + chunk[hi-size:]
                after.update(chunk)
                size += len(chunk)
        require(size == factory["disk_bytes"] == disk["compared_bytes"] and
                before.hexdigest() == factory["factory_sha256"] == disk["factory_sha256"] and
                after.hexdigest() == disk["working_sha256"], "full-image replay differs")
        print("Reconstructed and verified the whole final disk digest from the factory disk and saved store.")
    print("Verified two installed boot receipts, five store captures, both control traces and both full output fields.")


if __name__ == "__main__":
    main()
