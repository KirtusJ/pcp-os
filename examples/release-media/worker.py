"""Build a standalone Tailorbird qualification worker for one exact model manifest."""
import argparse
import os
from pathlib import Path
import re
import shutil
import tempfile

from toolchain import build_environment, tool_record
from basic import cargo_bin, export_guest, run, sha, write_parts


def sha256_arg(value):
    if not re.fullmatch("[0-9a-f]{64}", value):
        raise argparse.ArgumentTypeError("expected 64 lowercase hexadecimal digits")
    return value


def prepare(args, stage, work):
    if args.manifest is not None and sha(args.manifest) != args.manifest_sha256:
        raise ValueError("model manifest differs from the independently supplied SHA-256")
    env = build_environment(work)
    tools = tool_record(env)
    compiler = tools["rustc"]["version"]
    if "release: 1.97.0" not in compiler.splitlines():
        raise ValueError("this worker qualification uses rustc 1.97.0")
    bird = work / "tailorbird"
    guest_head = export_guest(args.os, args.tailorbird, args.tailorbird_head, bird,
                              ["store/apps/parrots-all-the-way-down/guest/src/heap.rs"])
    env["PCP_MODEL_MANIFEST_SHA256"] = args.manifest_sha256
    worker, features = cargo_bin(bird, "model-worker", ["model-worker"], env, work)
    output = stage / "model-worker.elf"
    shutil.copyfile(worker, output)
    elf_digest = sha(output)
    if args.expect_elf_sha256 and elf_digest != args.expect_elf_sha256:
        raise ValueError("worker ELF differs from expected SHA-256")
    return {
        "format": "pcpos-model-worker-parts/1",
        "tailorbird_head": args.tailorbird_head,
        "tailorbird_tree": run("git", "-C", args.tailorbird, "rev-parse",
                               args.tailorbird_head + "^{tree}").strip(),
        "guest_os_head": guest_head,
        "model_manifest_sha256": args.manifest_sha256,
        "target": "x86_64-unknown-none",
        "rustc": compiler,
        "cargo": tools["cargo"]["version"].splitlines()[0],
        "rust_tools": tools,
        "worker_features": features,
        "files": {output.name: elf_digest},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--os", required=True, type=Path)
    parser.add_argument("--tailorbird", required=True, type=Path)
    parser.add_argument("--tailorbird-head", required=True)
    parser.add_argument("--manifest-sha256", required=True, type=sha256_arg)
    parser.add_argument("--manifest", type=Path,
                        help="verify the actual packed manifest against the independently supplied digest")
    parser.add_argument("--expect-elf-sha256", type=sha256_arg,
                        help="verify a rebuild against an independently recorded worker digest")
    parser.add_argument("--work", type=Path,
                        default=Path(tempfile.gettempdir()) / f"pcpos-release-worker-{os.getuid()}",
                        help="fixed, initially absent build path")
    parser.add_argument("parts", type=Path)
    write_parts(parser.parse_args(), prepare)


if __name__ == "__main__":
    main()
