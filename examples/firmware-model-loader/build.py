"""Build the read-only x86-64 model-media EFI loader from separately pinned assembly inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import struct
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
FILES = ("KERNEL.ELF", "TAILOR.ELF", "WORKER.ELF", "model.manifest")
FIXTURE = b"THIS KERNEL IS BUILT WITH model-memory-fixture"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def geometry(data):
    if len(data) < 52 or data[:7] != b"\x7fELF\x01\x01\x01":
        raise ValueError("expected the kernel's little-endian ELF32 Multiboot image")
    kind, machine, version, entry, offset = struct.unpack_from("<HHIII", data, 16)
    width, count = struct.unpack_from("<HH", data, 42)
    if (kind, machine, version) != (2, 3, 1) or width != 32 or count > 64 or offset + count*width > len(data):
        raise ValueError("unexpected ELF header or program directory")
    segments = []
    for i in range(count):
        typ, pos, virt, phys, size, memory, flags, _ = struct.unpack_from("<8I", data, offset+i*width)
        if typ != 1 or not memory:
            continue
        if phys != virt or phys < 1 << 20 or phys + memory >= 1 << 30 or size > memory or pos+size > len(data):
            raise ValueError("invalid kernel load segment")
        segments.append((phys, memory, pos, size, flags))
    segments.sort()
    if (not segments or len(segments) > 32 or segments[0][0] % 4096
            or any(a[0]+a[1] > b[0] for a, b in zip(segments, segments[1:]))
            or not any(flags & 1 and phys <= entry < phys+size for phys, _, _, size, flags in segments)):
        raise ValueError("overlapping kernel segments or missing entry")
    return entry, segments


def header(directory, receipt, receipt_pin):
    raw = receipt.read_bytes()
    if sha(raw) != receipt_pin:
        raise ValueError("assembly receipt differs from its independent pin")
    r = json.loads(raw)
    if r.get("format") != "pcpos-preloaded-inputs/1":
        raise ValueError("unrecognized preloaded input receipt")
    features = set(r["kernel_features"])
    if not {"model-memory", "proposals"} <= features or "model-memory-fixture" in features:
        raise ValueError("expected normal model-memory and proposals without a fixture kernel")
    if set(r["files"]) != set(FILES):
        raise ValueError("expected exactly the four pinned preloaded inputs")
    buffers = {}
    for name in FILES:
        p = directory/name
        maximum = 56+4096*32 if name == "model.manifest" else 64 << 20
        if not p.is_file() or not 0 < p.stat().st_size <= maximum:
            raise ValueError(f"missing or oversized input: {name}")
        buffers[name] = p.read_bytes()
        if sha(buffers[name]) != r["files"][name]:
            raise ValueError(f"input SHA-256 differs: {name}")
    if FIXTURE in buffers["KERNEL.ELF"]:
        raise ValueError("fixture banner in kernel")
    pin = r["files"]["model.manifest"]
    if r.get("worker_manifest_sha256") != pin or pin.encode("ascii") not in buffers["WORKER.ELF"]:
        raise ValueError("worker build manifest pin differs")
    for name in ("TAILOR.ELF", "WORKER.ELF"):
        if buffers[name][:7] != b"\x7fELF\x02\x01\x01" or buffers[name][18:20] != b"\x3e\x00":
            raise ValueError(f"expected an x86-64 guest ELF: {name}")
    m = buffers["model.manifest"]
    if len(m) < 56 or m[:8] != b"PCPMDL01":
        raise ValueError("not a PCPMDL01 model manifest")
    total, shard, count = struct.unpack_from("<QII", m, 8)
    if (not 0 < total <= 64 << 30 or not 1024 <= shard <= 64 << 20 or shard & (shard-1)
            or count != (total+shard-1)//shard or not 1 <= count <= 4096 or len(m) != 56+32*count):
        raise ValueError("invalid manifest limits, count or length")
    entry, segments = geometry(buffers["KERNEL.ELF"])
    measure = runpy.run_path(str(HERE.parents[1]/"PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"))["measure"]
    measurement = measure(buffers["KERNEL.ELF"])
    h = [f"#define MODEL_BYTES {total}ULL", f"#define SHARD_BYTES {shard}ULL",
         f"#define SHARD_COUNT {count}U", f'#define PIN_TEXT "{pin}"',
         f'#define MEASUREMENT_TEXT "{measurement}"', f'#define UI_TEXT "{r["files"]["TAILOR.ELF"]}"', f'#define WORKER_TEXT "{r["files"]["WORKER.ELF"]}"',
         f"#define KERNEL_ENTRY {entry}U", f"#define KERNEL_BASE {segments[0][0]}U",
         f"#define KERNEL_END {max((s[0]+s[1]+4095)&~4095 for s in segments)}U"]
    for i, name in enumerate(FILES):
        h.append(f"#define INPUT_{i}_SIZE {len(buffers[name])}U")
    h.append("static const UINT8 pins[4][32] = {")
    for name in FILES:
        h.append(" {" + ",".join(map(str, bytes.fromhex(r["files"][name]))) + "},")
    h.append("};\nstatic const struct segment segments[] = {")
    h += [f" {{{addr}U,{pos}U,{size}U}}," for addr, _, pos, size, _ in segments]
    return "\n".join(h+["};", ""])


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("parts", type=Path, help="directory containing the four files that go under PCPOS")
    p.add_argument("receipt", type=Path)
    p.add_argument("--receipt-sha256", required=True)
    p.add_argument("--gnu-efi", type=Path, default=Path("/usr"), help="GNU-EFI prefix with include/efi and lib")
    p.add_argument("output", type=Path)
    args = p.parse_args()
    pins = header(args.parts, args.receipt, args.receipt_sha256)
    prefix = args.gnu_efi.resolve()
    lib = prefix/"lib"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.output.parent, prefix=".model-efi-") as temp:
        work = Path(temp).resolve()
        (work/"config.h").write_text(pins, encoding="ascii")
        env = dict(os.environ, CARGO_TARGET_DIR=str(work/"target"), RUSTFLAGS="-C relocation-model=pic -C no-redzone=yes")
        def run(*cmd):
            subprocess.run(cmd, cwd=work, env=env, check=True)
        run("cargo", "build", "--locked", "--manifest-path", HERE/"hash/Cargo.toml", "--release", "--target", "x86_64-unknown-none")
        run("cc", "-O2", "-Wall", "-Wextra", "-Werror", "-fpic", "-fshort-wchar", "-ffreestanding", "-fno-stack-protector",
            "-mno-red-zone", "-DGNU_EFI_USE_MS_ABI", "-I"+str(prefix/"include/efi"), "-I"+str(prefix/"include/efi/x86_64"),
            "-I"+str(work), "-c", HERE/"loader.c", "-o", "loader.o")
        run("cc", "-c", HERE.parent/"firmware-owned-uefi/handoff.S", "-o", "handoff.o")
        run("ld", "-nostdlib", "-znocombreloc", "-T", lib/"elf_x86_64_efi.lds", "-shared", "-Bsymbolic", "--gc-sections",
            "--no-undefined", lib/"crt0-efi-x86_64.o", "loader.o", "handoff.o",
            work/"target/x86_64-unknown-none/release/libpcpos_efi_sha256.a", "-L"+str(lib), "-lefi", "-lgnuefi", "-o", "loader.so")
        run("objcopy", *sum((["-j", section] for section in (".text",".rodata",".sdata",".data",".dynamic",".dynsym",
            ".rel",".rela",".reloc")), []), "--target=efi-app-x86_64", "loader.so", "BOOTX64.EFI")
        os.link(work/"BOOTX64.EFI", args.output)
    print(sha(args.output.read_bytes()), args.output)


if __name__ == "__main__":
    main()
