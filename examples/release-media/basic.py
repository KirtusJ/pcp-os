"""Prepare the pinned inputs for the model-free x86-64 UEFI image."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

from toolchain import build_environment, check_project_wrappers, tool_record


def run(*args, cwd=None, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def archive(repo, head, paths, dest):
    if not re.fullmatch("[0-9a-f]{40}", head) or run("git", "-C", repo, "cat-file", "-t", head).strip() != "commit":
        raise ValueError("supply a complete Git commit ID")
    dest.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryFile() as output:
        subprocess.run(["git", "-C", repo, "archive", head, "--", *paths], stdout=output, check=True)
        output.seek(0)
        with tarfile.open(fileobj=output) as source:
            source.extractall(dest, filter="data")


def cargo_bin(directory, binary, features, env, work):
    check_project_wrappers(directory, work)
    command = [env["CARGO"], "build", "--locked", "--release", "--target", "x86_64-unknown-none",
               "--bin", binary, "--no-default-features", "--message-format=json",
               "--config", f"target.x86_64-unknown-none.rustflags=['--remap-path-prefix={work}=/pcpos-build']"]
    if features:
        command += ["--features", ",".join(features)]
    messages = run(*command, cwd=directory, env=env).splitlines()
    artifacts = [event for line in messages if (event := json.loads(line)).get("reason") == "compiler-artifact"
                 and event["target"]["name"] == binary and event["executable"]]
    if len(artifacts) != 1 or sorted(artifacts[0]["features"]) != sorted(features):
        raise ValueError(f"unexpected resolved Cargo features for {binary}")
    return Path(artifacts[0]["executable"]), artifacts[0]["features"]


def export_guest(os_repo, tailorbird, head, bird, extra_paths=()):
    archive(tailorbird, head, ["Cargo.toml", "Cargo.lock", ".cargo", "guest"], bird)
    pin = run("git", "-C", tailorbird, "ls-tree", head, "pcp-os").split()
    if len(pin) != 4 or pin[:2] != ["160000", "commit"] or pin[3] != "pcp-os":
        raise ValueError("expected the PCP OS submodule pinned by Tailorbird")
    paths = ["store/native/guest", "store/games/flux-rink/guest.ld", "PCP_OS_Rust", *extra_paths]
    archive(os_repo, pin[2], paths, bird / "pcp-os")
    return pin[2]


def prepare(args, stage, work):
    files = stage / "files"
    efi = files / "EFI" / "BOOT"
    payload = files / "PCPOS"
    efi.mkdir(parents=True)
    payload.mkdir()
    env = build_environment(work)
    env["RUSTC_BOOTSTRAP"] = "1"

    if sha(args.assets_iso) != args.assets_sha256:
        raise ValueError("donor ISO SHA-256 differs")
    os_src = work / "os"
    archive(args.os, args.os_head, ["PCP_OS_Rust"], os_src)
    kernel_dir = os_src / "PCP_OS_Rust/crates/pcpos-kernel"
    kernel, features = cargo_bin(kernel_dir, "pcpos-kernel", ["proposals"], env, work)
    packed = payload / "KERNEL.ELF"
    subprocess.run(["objcopy", "-I", "elf64-x86-64", "-O", "elf32-i386", kernel, packed], check=True)
    if b"THIS KERNEL IS BUILT WITH model-memory-fixture" in packed.read_bytes():
        raise ValueError("fixture build marker in kernel")
    measurement = run(sys.executable, kernel_dir / "tools/measure.py", packed).strip()
    if not re.fullmatch("[0-9a-f]{64}", measurement):
        raise ValueError("kernel measurement is missing")

    bird = work / "tailorbird"
    guest_head = export_guest(args.os, args.tailorbird, args.tailorbird_head, bird)
    app, app_features = cargo_bin(bird, "tailorbird", [], env, work)
    shutil.copyfile(app, payload / "TAILOR.ELF")

    for member, dest in [("surf.elf", "SURF.ELF"), ("clip.ppm", "CLIP.PPM"), ("clip.wav", "CLIP.WAV"),
                         ("notices.txt", "ASSET-NOTICES.TXT"), ("license.md", "ASSET-LICENSE.MD")]:
        subprocess.run(["xorriso", "-osirrox", "on", "-indev", args.assets_iso, "-extract",
                        "/" + member, payload / dest], check=True, stdout=subprocess.DEVNULL)
    shutil.copyfile(os_src / "PCP_OS_Rust/NOTICE.md", payload / "PCPOS-NOTICE.MD")
    shutil.copyfile(args.grub_license, payload / "GRUB-LICENSE.TXT")

    # Use the firmware-selected device. Another stick may hold a different PCP OS.
    early = work / "early.cfg"
    early.write_text('configfile "$cmdpath/grub.cfg"\n', encoding="ascii")
    config = (f'multiboot /PCPOS/KERNEL.ELF image={measurement} propose_image={sha(app)}\n'
              'module /PCPOS/TAILOR.ELF tailorbird\n'
              'module /PCPOS/SURF.ELF surf.elf\n'
              'module /PCPOS/CLIP.PPM clip.ppm\n'
              'module /PCPOS/CLIP.WAV clip.wav\nboot\n')
    (efi / "grub.cfg").write_text(config, encoding="ascii")
    subprocess.run(["grub-mkimage", "-O", "x86_64-efi", "-d", args.grub_modules,
                    "-p", "/EFI/BOOT", "-c", early, "-o", efi / "BOOTX64.EFI",
                    "normal", "configfile", "fat", "iso9660", "part_gpt",
                    "part_msdos", "multiboot", "boot", "efi_gop", "all_video"], env=env, check=True)
    return {"format": "pcpos-release-parts/1", "profile": "model-free",
            "os_head": args.os_head, "tailorbird_head": args.tailorbird_head, "guest_os_head": guest_head,
            "kernel_features": features, "tailorbird_features": app_features, "measurement": measurement,
            "asset_iso_sha256": args.assets_sha256, "rustc": run(env["RUSTC"], "--version", env=env).strip(),
            "rust_tools": tool_record(env),
            "grub": run("grub-mkimage", "--version").strip(),
            "files": {p.relative_to(files).as_posix(): sha(p) for p in sorted(files.rglob("*")) if p.is_file()}}


def write_parts(args, builder):
    args.parts = args.parts.absolute()
    # mkdir reserves the new output for this invocation; incomplete builds never get a lock file.
    args.parts.mkdir()
    try:
        work = args.work.resolve()
        work.mkdir(mode=0o700)
        try:
            receipt = builder(args, args.parts, work)
        finally:
            shutil.rmtree(work)
        lock = args.parts / "inputs.json"
        lock.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"{sha(lock)}  {lock}")
    except BaseException:
        shutil.rmtree(args.parts)
        raise


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("os", "tailorbird", "assets-iso", "grub-modules", "grub-license"):
        p.add_argument("--" + name, type=Path, required=True)
    for name in ("os-head", "tailorbird-head", "assets-sha256"):
        p.add_argument("--" + name, required=True)
    p.add_argument("--work", type=Path, default=Path(tempfile.gettempdir()) / f"pcpos-release-basic-{os.getuid()}",
                   help="fixed, initially absent build path; use the same path for repeat builds")
    p.add_argument("parts", type=Path)
    args = p.parse_args()
    write_parts(args, prepare)


if __name__ == "__main__":
    main()
