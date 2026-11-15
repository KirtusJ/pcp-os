"""Rebuild the native Doom guest with the Store release's cross toolchain."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import zlib


RELEASE_SOURCE = (r"D:\PCP-Replays\Doom\base-engine\doomgeneric-"
                  r"e257a09527062bfb986b6e5e0780839b6cbd5b71\doomgeneric")
APK_SHA256 = "2ca496dd68c0b340b50cf9940bc2f3312eac8b56ec2d3284a621be6f0c09ffb1"


def extract_runtime(apk, work, objcopy):
    compressed = apk.read_bytes()
    if hashlib.sha256(compressed).hexdigest() != APK_SHA256:
        raise ValueError("Expected Alpine x86_64 musl-dev 1.2.5-r13")
    members = {"usr/lib/rcrt1.o": "rcrt1.o", "usr/lib/libc.a": "libc.a"}
    # Alpine APK v2 concatenates signature, metadata and package-data gzip streams.
    while compressed:
        stream = zlib.decompressobj(16 + zlib.MAX_WBITS)
        body = stream.decompress(compressed)
        compressed = stream.unused_data
        with tarfile.open(fileobj=io.BytesIO(body), mode="r:") as archive:
            for name in members.keys() & set(archive.getnames()):
                raw = work / ("input-" + members[name])
                raw.write_bytes(archive.extractfile(name).read())
                subprocess.run([objcopy, "--strip-debug", str(raw), str(work / members[name])], check=True)
    return work / "rcrt1.o", work / "libc.a"


def build(args):
    src = args.source.resolve() / "engine/doomgeneric"
    choc = args.source.resolve() / "chocolate-doom"
    port = args.source.resolve() / "pcpos-port"
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(output)

    files = re.search(r'^FILES="([^"]+)"', (port / "build.sh").read_text(), re.MULTILINE)[1].split()
    for name in ("i_oplmusic.c", "midifile.c", "midifile.h"):
        shutil.copyfile(choc / "src" / name, work / name)
    shutil.copyfile(choc / "opl/opl_queue.c", work / "opl_queue.c")
    opl = work / "i_oplmusic.c"
    body = opl.read_text()
    declaration = "const music_module_t music_opl_module ="
    if body.count(declaration) != 1:
        raise ValueError("Unexpected Chocolate Doom music module declaration")
    opl.write_text(body.replace(declaration, "music_module_t music_opl_module ="))

    flags = ["-O2", "-fPIE", "-fno-strict-aliasing", "-DNORMALUNIX", "-DLINUX",
             "-D_DEFAULT_SOURCE", "-DFEATURE_SOUND", "-I" + str(port), "-I" + str(src), "-w"]
    music = ["-DPCP_OPL_MUSIC", "-I" + str(choc / "opl")]
    compat = music + ["-include", str(port / "choc_compat.h")]
    sources = [(src / (name + ".c"), []) for name in files]
    sources.append((port / "doomgeneric_pcpos.c", []))
    sources.extend((port / (name + ".c"), music) for name in ("i_pcpsound", "opl_pcpos"))
    sources.extend([(opl, compat + ["-Dmusic_opl_module=DG_music_module"]),
                    (work / "midifile.c", compat), (work / "opl_queue.c", music)])

    def compile_file(item):
        source, extra = item
        obj = work / (source.stem + ".o")
        # Two assertion diagnostics retain __FILE__. Map them to their exact release spelling.
        macro = []
        if source.name in ("r_data.c", "w_wad.c"):
            macro.append("-fmacro-prefix-map=" + str(source) + "=" + RELEASE_SOURCE + "\\" + source.name)
        subprocess.run([args.zig, "cc", "-target", "x86_64-linux-musl", *flags,
                        *macro, *extra, "-c", str(source), "-o", str(obj)], check=True)
        return obj

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        # Preserve the release link order across hosts: underscore sorts before the extension dot.
        objects = sorted(pool.map(compile_file, sources), key=lambda p: p.name.replace("_", "!"))
    crt, libc = extract_runtime(args.musl_apk, work, args.objcopy)
    subprocess.run([args.lld, "-static", "-pie", "--no-dynamic-linker", "-z", "text", "-s",
                    "-o", str(output), str(crt), *map(str, objects), str(libc)], check=True)
    print(hashlib.sha256(output.read_bytes()).hexdigest(), output)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path, help="extracted doom-source directory")
    p.add_argument("musl_apk", type=Path, help="Alpine x86_64 musl-dev-1.2.5-r13.apk")
    p.add_argument("work", type=Path, help="a new build directory")
    p.add_argument("output", type=Path, help="new output ELF")
    p.add_argument("--zig", default="zig")
    p.add_argument("--lld", default="ld.lld")
    p.add_argument("--objcopy", default="llvm-objcopy")
    p.add_argument("--jobs", type=int, default=2)
    args = p.parse_args()
    if args.jobs < 1:
        p.error("--jobs must be positive")
    build(args)
