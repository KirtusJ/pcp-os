#!/usr/bin/env python3
"""Build a PCP OS BIOS tester CD from this checkout, and say exactly what went into it.

    python3 examples/tester-cd/build.py <limine binary release directory> <new output directory>

Everything on the CD except the loader is made here from the checked-out source: the kernel (default
features, so no guest Ethernet), the Surfbird guest, and the demo clip. The loader is the pinned Limine
binary release, checked by digest. The image is assembled twice and the two must be the same bytes.

Needs: the Rust toolchain named in TOOLCHAIN with its source and the x86_64-unknown-none target, ld.lld,
objcopy, xorriso and Python 3. Nothing is fetched. This program publishes nothing: it writes an ISO and a
receipt into the output directory, and whether that image is given to anyone is a separate decision.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import fcntl
import shutil
import struct
import subprocess
import sys

REPO = Path(__file__).resolve().parents[2]
KERNEL = REPO / "PCP_OS_Rust" / "crates" / "pcpos-kernel"
GUEST = REPO / "apps" / "surfbird" / "guest"
TOOLCHAIN = "1.97.0"
# Limine 10.8.5 binary release, commit 7a9013f305de0bea1f6310e76e7baba30499fef0. The same three digests are
# checked by examples/optical-disk-check/build.py; this is the release whose BIOS path does not write to an
# attached disk (see that directory's README).
LOADER = {
    "limine-bios.sys": "34dd9fb6e706d78e920f9e3a403addcab66647ebe78d282d6c11eb6f1ffa5788",
    "limine-bios-cd.bin": "913b56f2a70679049ea2f18c5b5e5902ce488131d76a036ca363c7beb4d8f657",
    "LICENSE": "f174b8ff78f6f8982a1fc992668f8486eb8cf598f6938923c22e9bb50f8338a8",
}
# One fixed time for every date in the image, so that two builds of one source are the same bytes.
EPOCH = 1794614400  # 2026-11-13T12:00:00Z, the date the documents builder also uses


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run(*args, cwd=None, env=None):
    done = subprocess.run([str(a) for a in args], cwd=cwd, env=env, text=True, capture_output=True)
    if done.returncode:
        sys.exit("%s failed with status %d:\n%s%s" % (" ".join(str(a) for a in args[:3]), done.returncode, done.stdout[-2000:], done.stderr[-2000:]))
    return done.stdout


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def third_party(manifest, features, env):
    """Every external crate compiled into the bare-metal program, with the license files it ships. A crate
    counts if the program reaches it through ordinary or build dependencies with the features it is built
    with; a crate used only by the tests (a dev-dependency) is not compiled into the program and is left out."""
    meta = json.loads(run("cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", "x86_64-unknown-none",
                          "--manifest-path", manifest, *(["--features", features] if features else []), env=env))
    nodes = {node["id"]: node for node in meta["resolve"]["nodes"]}
    reached, queue = set(), [meta["resolve"]["root"]]
    while queue:
        current = queue.pop()
        if current in reached:
            continue
        reached.add(current)
        for dep in nodes[current]["deps"]:
            if any(kind["kind"] in (None, "build") for kind in dep["dep_kinds"]):
                queue.append(dep["pkg"])
    found = {}
    for package in meta["packages"]:
        if package["id"] not in reached or package["source"] is None:
            continue  # not compiled into this program, or part of this repository
        folder = Path(package["manifest_path"]).parent
        texts = sorted(p for p in folder.iterdir() if p.is_file() and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")))
        found[(package["name"], package["version"])] = (package.get("license") or "see its files", [(p.name, p.read_text(errors="replace")) for p in texts])
    return found


def notices(crates, queue_desk=False):
    programs = "kernel, Surfbird and queue desk" if queue_desk else "kernel and Surfbird program"
    lines = ["Third-party components compiled into this CD's " + programs, "",
             "The loader is Limine; its license is limine.txt. PCP OS itself is proprietary; see notice.md.", ""]
    for (name, version), (license, _) in sorted(crates.items()):
        lines.append("%s %s | %s" % (name, version, license))
    for (name, version), (_, texts) in sorted(crates.items()):
        if not texts:
            lines += ["", "==== %s %s: no license file in the published crate; its declared license is listed above ====" % (name, version)]
        for filename, text in texts:
            lines += ["", "==== %s %s: %s ====" % (name, version, filename), "", text.rstrip()]
    return ("\n".join(lines) + "\n").encode()


def read_back(image):
    """The files in the root directory of an ISO 9660 image, by their Rock Ridge names, as the bytes on the disc."""
    volume = image[16 * 2048:17 * 2048]
    if volume[1:6] != b"CD001":
        sys.exit("the assembled file is not an ISO 9660 image")
    start, size = struct.unpack("<I", volume[158:162])[0], struct.unpack("<I", volume[166:170])[0]
    directory, files, at = image[start * 2048:start * 2048 + size], {}, 0
    while at < len(directory):
        length = directory[at]
        if length == 0:
            at = (at // 2048 + 1) * 2048
            continue
        entry = directory[at:at + length]
        at += length
        extent, extent_size, flags, name_length = struct.unpack("<I", entry[2:6])[0], struct.unpack("<I", entry[10:14])[0], entry[25], entry[32]
        if flags & 2:
            continue  # the directory itself and its parent
        use, name, i = entry[33 + name_length + (1 - name_length % 2):], None, 0
        while i + 4 <= len(use) and use[i + 2]:
            if use[i:i + 2] == b"NM":
                name = use[i + 5:i + use[i + 2]].decode()
            i += use[i + 2]
        files[name or entry[33:33 + name_length].decode().split(";")[0]] = image[extent * 2048:extent * 2048 + extent_size]
    return files


def assemble(tree, iso, env):
    for path in sorted(tree.iterdir()):
        os.utime(path, (EPOCH, EPOCH))
    os.utime(tree, (EPOCH, EPOCH))
    # The conventional El Torito layout for Limine's BIOS CD loader. xorriso pads the end of the image by
    # 300 KiB in this mode, which the loader needs to read the last file on some drives.
    run("xorriso", "-as", "mkisofs", "-R", "-V", "PCP_OS", "-b", "limine-bios-cd.bin", "-c", "boot.cat", "-no-emul-boot",
        "-boot-load-size", "4", "-boot-info-table", "-o", iso, tree, env=env)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("limine", type=Path, help="checkout of the pinned Limine binary release")
    parser.add_argument("output", type=Path, help="new directory for the ISO and its receipt")
    parser.add_argument("--queue-desk", action="store_true", help="also build the native queue desk as the second launcher application")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    env = {**os.environ, "RUSTUP_TOOLCHAIN": TOOLCHAIN, "SOURCE_DATE_EPOCH": str(EPOCH), "TZ": "UTC", "LC_ALL": "C"}

    commit = run("git", "rev-parse", "HEAD", cwd=REPO).strip()
    # The tree is the content itself: it is the same for any commit with these files, so it identifies the
    # source even where the commit named above is one that a hosting service made for a test and later drops.
    source_tree = run("git", "rev-parse", "HEAD^{tree}", cwd=REPO).strip()
    changed = run("git", "status", "--porcelain", "--untracked-files=no", cwd=REPO).strip()
    rustc = run("rustc", "-vV", env=env)

    loader = {}
    for name, expected in LOADER.items():
        data = (args.limine / name).read_bytes()
        if sha(data) != expected:
            sys.exit("%s is not the pinned Limine file" % name)
        loader[name] = data

    # Where a crate is compiled is part of what the compiler produces (run.sh explains why), so both
    # programs are built in one fixed folder whatever the checkout or the output directory is called.
    # The folder is removed and made again, and only if this program made it: it carries a marker file.
    # One build at a time: a lock beside the folder is held until this program ends, so a second build
    # started meanwhile stops instead of removing the folder this one is working in.
    work = Path(os.environ.get("PCPOS_TESTER_CD_STAGE", "/tmp/pcpos-tester-cd-build")).resolve()
    lock = open(str(work) + ".lock", "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        sys.exit("another build is using %s; wait for it, or set PCPOS_TESTER_CD_STAGE to another folder" % work)
    marker = work / ".pcpos-tester-cd-stage"
    if work.exists():
        if not marker.is_file():
            sys.exit("%s exists and was not made by this program; remove it yourself or set PCPOS_TESTER_CD_STAGE" % work)
        shutil.rmtree(work)
    work.mkdir(parents=True)
    marker.write_text("made by examples/tester-cd/build.py\n")

    # The kernel, by its own build script, default features. The script leaves its result beside itself, so
    # it is run from a private copy of the Rust workspace under the locked folder: nothing is written into
    # the checkout, and two builds with different staging folders cannot hand each other a kernel.
    rust = work / "source" / "PCP_OS_Rust"
    shutil.copytree(REPO / "PCP_OS_Rust", rust, ignore=shutil.ignore_patterns("target", "programs", "guard_cases.jsonl", "__pycache__"))
    private = rust / "crates" / "pcpos-kernel"
    run("sh", private / "run.sh", "build", env={**env, "PCPOS_KERNEL_STAGE": str(work / "kernel"), "PCPOS_KERNEL_FEATURES": ""})
    kernel = (private / "target" / "pcpos-kernel.elf32").read_bytes()
    measurement = run(sys.executable, private / "tools" / "measure.py", private / "target" / "pcpos-kernel.elf32").split()[-1]
    if len(measurement) != 64:
        sys.exit("unexpected output from measure.py")

    # The Surfbird guest that plays files handed over by the loader.
    # It is copied with the three things it reaches by relative path: the shared guest library, the linker
    # script and the kernel's font.
    guest = work / GUEST.relative_to(REPO)
    skip = shutil.ignore_patterns("target", "evidence", "__pycache__")
    shutil.copytree(GUEST, guest, ignore=skip)
    shutil.copytree(REPO / "store" / "native" / "guest", work / "store" / "native" / "guest", ignore=skip)
    (work / "store" / "games" / "flux-rink").mkdir(parents=True)
    shutil.copy(REPO / "store" / "games" / "flux-rink" / "guest.ld", work / "store" / "games" / "flux-rink" / "guest.ld")
    font = Path("PCP_OS_Rust") / "crates" / "pcpos-kernel" / "src" / "font.rs"
    (work / font).parent.mkdir(parents=True)
    shutil.copy(REPO / font, work / font)
    run("cargo", "build", "--locked", "--release", "--target", "x86_64-unknown-none", "--features", "local-media",
        "--config", "target.x86_64-unknown-none.rustflags=['--remap-path-prefix=%s=pcpos']" % work, cwd=guest, env=env)
    surf = (guest / "target" / "x86_64-unknown-none" / "release" / "surfcable").read_bytes()

    # The demo clip, from the same generator the media replay test uses.
    media = work / "media"
    media.mkdir()
    load(GUEST / "replay_local_media.py", "replay_local_media").make_media(media, 0)

    crates = {**third_party(KERNEL / "Cargo.toml", "", env), **third_party(GUEST / "Cargo.toml", "local-media", env)}
    if args.queue_desk:
        # The desk's core uses pcpos-core from the private Rust workspace.
        root = rust.parent
        app = Path("store/apps/parrots-all-the-way-down")
        for part in (app / "core", app / "guest", Path("store/native/guest")):
            shutil.copytree(REPO / part, root / part, ignore=skip)
        desk = root / app / "guest"
        run("cargo", "build", "--locked", "--release", "--target", "x86_64-unknown-none",
            "--config", "target.x86_64-unknown-none.rustflags=['--remap-path-prefix=%s=pcpos']" % work,
            cwd=desk, env=env)
        desk_elf = (desk / "target/x86_64-unknown-none/release/rdesk").read_bytes()
        crates.update(third_party(desk / "Cargo.toml", "", env))
    config = ("timeout: 0\n\n/PCP OS\n    protocol: multiboot1\n    path: boot():/kernel.elf32\n    cmdline: image=%s\n"
              "    module_path: boot():/surf.elf\n    module_string: surf.elf\n"
              "    module_path: boot():/clip.ppm\n    module_string: clip.ppm\n"
              "    module_path: boot():/clip.wav\n    module_string: clip.wav\n" % measurement).encode()
    readme = ("PCP OS tester CD | x86-64, legacy BIOS | built from source\n"
              "Source: KirtusJ/pcp-os commit %s, tree %s%s\n"
              "Kernel features: default (no guest Ethernet). Running-image measurement: %s\n"
              "Boot it as a virtual CD with no disk attached. Enter starts the Surfbird demo, Space plays, Ctrl+Alt+Delete\n"
              "returns to the launcher, P powers off. Nothing is kept after power off.\n"
              "Loader: Limine v10.8.5 (limine.txt). Third-party notices: notices.txt. Rights: notice.md.\n"
              % (commit, source_tree, " with uncommitted changes" if changed else "", measurement)).encode()
    files = {"kernel.elf32": kernel, "surf.elf": surf, "clip.ppm": (media / "clip.ppm").read_bytes(), "clip.wav": (media / "clip.wav").read_bytes(),
             "limine-bios.sys": loader["limine-bios.sys"], "limine-bios-cd.bin": loader["limine-bios-cd.bin"], "limine.txt": loader["LICENSE"],
             "limine.conf": config, "readme.txt": readme, "notices.txt": notices(crates, args.queue_desk),
             "notice.md": (REPO / "PCP_OS_Rust" / "NOTICE.md").read_bytes()}

    if args.queue_desk:
        files["rdesk.elf"] = desk_elf
        files["limine.conf"] += b"    module_path: boot():/rdesk.elf\n    module_string: rdesk.elf\n"
        files["readme.txt"] += (b"Down selects the queue desk. Enter opens it. N creates a request. Type a label and press Enter.\n"
                                b"Esc returns to the launcher. Reopening the desk restores this boot's queue.\n")

    images = []
    for attempt in ("first", "second"):
        tree = out / ("tree-" + attempt)
        tree.mkdir()
        for name, data in files.items():
            (tree / name).write_bytes(data)
        iso = out / ("%s.iso" % attempt)
        assemble(tree, iso, env)
        images.append(iso.read_bytes())
        shutil.rmtree(tree)
    if images[0] != images[1]:
        sys.exit("two assemblies of the same files gave different images")
    # What is on the disc, read back from the image. Every file must be the bytes that went in, except the
    # two the mastering writes: the boot catalog, which it creates, and the loader's CD boot file, into which
    # it writes the boot information table (bytes 8 to 63) that the loader reads to find itself.
    on_disc = read_back(images[0])
    boot = "limine-bios-cd.bin"
    if set(on_disc) != set(files) | {"boot.cat"}:
        sys.exit("the image does not hold exactly the files that went in and a boot catalog: %s" % sorted(set(on_disc) ^ set(files)))
    for name, data in files.items():
        same = on_disc[name] == data if name != boot else on_disc[name][:8] + on_disc[name][64:] == data[:8] + data[64:]
        if not same:
            sys.exit("%s on the disc is not the file that went in" % name)
    (out / "second.iso").unlink()
    (out / "first.iso").rename(out / "pcp-os-tester.iso")
    shutil.rmtree(work)

    receipt = {"format": "pcp-os-tester-cd/1", "source_commit": commit, "source_tree": source_tree, "uncommitted_changes": bool(changed),
               "toolchain": TOOLCHAIN, "rustc": dict(line.split(": ", 1) for line in rustc.splitlines() if ": " in line),
               "kernel_features": "default", "guest_features": "local-media",
               "kernel_measurement": measurement, "iso_bytes": len(images[0]), "iso_sha256": sha(images[0]),
               "inputs": {name: {"bytes": len(data), "sha256": sha(data)} for name, data in sorted(files.items())},
               "on_disc": {name: {"bytes": len(data), "sha256": sha(data)} for name, data in sorted(on_disc.items())},
               "written_by_mastering": ["boot.cat", boot],
               "third_party_crates_compiled_in": ["%s %s" % key for key in sorted(crates)]}
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({k: receipt[k] for k in ("source_commit", "source_tree", "uncommitted_changes", "kernel_measurement", "iso_bytes", "iso_sha256")}, indent=2))


if __name__ == "__main__":
    main()
