"""Stage a publisher-pinned model and its separately built Tailorbird worker."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import struct
import tempfile

from basic import archive, sha
from worker import sha256_arg


def worker_record(parts, receipt_pin, manifest_pin):
    raw = (parts / "inputs.json").read_bytes()
    if hashlib.sha256(raw).hexdigest() != receipt_pin:
        raise ValueError("worker receipt digest differs")
    record = json.loads(raw)
    if (record["format"] != "pcpos-model-worker-parts/1"
            or record["target"] != "x86_64-unknown-none"
            or record["worker_features"] != ["model-worker"]
            or record["model_manifest_sha256"] != manifest_pin):
        raise ValueError("worker record does not bind this model and target")
    for name in ("tailorbird_head", "tailorbird_tree", "guest_os_head"):
        if not re.fullmatch("[0-9a-f]{40}", record[name]):
            raise ValueError("worker receipt lacks full " + name)
    if set(record["files"]) != {"model-worker.elf"}:
        raise ValueError("expected a single worker artifact")
    return record


def readback(folder, model_pin, manifest_pin):
    manifest = (folder / "model.manifest").read_bytes()
    if hashlib.sha256(manifest).hexdigest() != manifest_pin:
        raise ValueError("packed model manifest differs from the independent pin")
    # The Rust packer has already validated this manifest with Manifest::parse.
    # The fixed header is followed by one SHA-256 per shard in the packed order.
    total, shard_bytes, count = struct.unpack_from("<QII", manifest, 8)
    if len(manifest) != 56 + count * 32 or manifest[24:56].hex() != model_pin:
        raise ValueError("manifest does not bind the supplied model digest")
    shards = sorted(folder.glob("shard-*.bin"))
    if len(shards) != count:
        raise ValueError("shard count differs from the manifest")
    whole = hashlib.sha256()
    files = {"PCPOS/model.manifest": manifest_pin}
    for ordinal, shard in enumerate(shards):
        if shard.name != f"shard-{ordinal:05}.bin":
            raise ValueError("missing or misnumbered model shard")
        part = hashlib.sha256()
        with shard.open("rb") as stream:
            while data := stream.read(1 << 20):
                whole.update(data)
                part.update(data)
        length = min(shard_bytes, total - ordinal * shard_bytes)
        offset = 56 + ordinal * 32
        if shard.stat().st_size != length or part.digest() != manifest[offset:offset + 32]:
            raise ValueError("model shard readback differs at " + shard.name)
        files["PCPOS/" + shard.name] = part.hexdigest()
    if not shards or whole.hexdigest() != model_pin:
        raise ValueError("model shard readback differs from the publisher's digest")
    return files


def stage(args):
    record = worker_record(args.worker_parts, args.worker_receipt_sha256, args.manifest_sha256)
    args.parts.mkdir()
    try:
        payload = args.parts / "PCPOS"
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            archive(args.os, record["guest_os_head"], ["PCP_OS_Rust"], work)
            subprocess.run(["cargo", "run", "--locked", "--release", "-p", "pcpos-model-media",
                            "--example", "pack", "--", args.model.resolve(), args.model_sha256,
                            payload.resolve()], cwd=work / "PCP_OS_Rust", check=True,
                           env=dict(os.environ, CARGO_TARGET_DIR=str(work / "target")))
        files = readback(payload, args.model_sha256, args.manifest_sha256)
        worker = payload / "WORKER.ELF"
        shutil.copyfile(args.worker_parts / "model-worker.elf", worker)
        files["PCPOS/WORKER.ELF"] = sha(worker)
        if files["PCPOS/WORKER.ELF"] != record["files"]["model-worker.elf"]:
            raise ValueError("copied worker ELF differs from its pinned receipt")
        receipt = {"format": "pcpos-preloaded-model-payload/1", "files": files,
                   "model_sha256": args.model_sha256, "model_manifest_sha256": args.manifest_sha256,
                   "packer_os_head": record["guest_os_head"],
                   "worker_receipt_sha256": args.worker_receipt_sha256}
        lock = args.parts / "inputs.json"
        lock.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"{sha(lock)}  {lock}")
    except BaseException:
        shutil.rmtree(args.parts)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("os", "worker-parts", "model"):
        parser.add_argument("--" + name, required=True, type=Path)
    for name in ("worker-receipt-sha256", "model-sha256", "manifest-sha256"):
        parser.add_argument("--" + name, required=True, type=sha256_arg)
    parser.add_argument("parts", type=Path)
    stage(parser.parse_args())


if __name__ == "__main__":
    main()
