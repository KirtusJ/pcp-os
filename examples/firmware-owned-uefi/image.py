"""Build the OVMF optical or removable-media fixture with its full-heap kernel and three pinned files."""
import hashlib
from pathlib import Path
import re
import struct
import subprocess

BASE = 32 << 20
PAGE = 4096
MEMBERS = (("surf.elf", "surf", "b87f1185f86961ff8afdb21315c811f0c071c474d09737613146be46cbb98b34"),
           ("clip.ppm", "picture", "f6a1d9a0702f472f5b02cf0a528586e6d4447acbad2bd92eb0fc6d70eea4df4d"),
           ("clip.wav", "audio", "70ca6a81bc6f94db7d14a61630ff02c517d20a65c31273b16ff02e233018e244"))
PUBLIC_ISO = "93ed3b599ad7be0e1e88a64813b1f2b5299d51507df2edca691aaf6189d10d15"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def round_page(n):
    return (n + PAGE - 1) // PAGE * PAGE


def geometry(elf):
    raw = elf.read_bytes()
    if raw[:7] != b'\x7fELF\x01\x01\x01' or struct.unpack_from('<HH', raw, 16) != (2, 3):
        raise ValueError("Expected an ELF32/i386 executable")
    entry, phoff = struct.unpack_from("<II", raw, 24)
    stride, count = struct.unpack_from("<HH", raw, 42)
    if stride != 32 or not 0 < count <= 32 or phoff + count * stride > len(raw):
        raise ValueError("Invalid program header table")
    segments = []
    for i in range(count):
        kind, offset, virtual, physical, file_size, mem_size, flags, align = struct.unpack_from(
            "<8I", raw, phoff + i * stride)
        if kind != 1 or mem_size == 0:
            continue
        if (physical != virtual or physical < BASE or physical + mem_size > BASE + (32 << 20)
                or file_size > mem_size or offset + file_size > len(raw)
                or (align > 1 and (align & (align - 1) or physical % align != offset % align))):
            raise ValueError("Invalid loadable segment")
        segments.append((physical, mem_size, offset, file_size, flags))
    segments.sort()
    if (not segments or segments[0][0] != BASE
            or any(a[0] + a[1] > b[0] for a, b in zip(segments, segments[1:]))
            or not any(lo <= entry < lo + size and flags & 1 for lo, _, _, size, flags in segments)):
        raise ValueError("Unexpected entry or overlapping segments")
    end = round_page(max(lo + mem for lo, mem, _, _, _ in segments))
    backed = max(lo + size for lo, _, _, size, _ in segments if size) - BASE
    flat = bytearray(backed)
    for lo, _, offset, size, _ in segments:
        if size:
            flat[lo - BASE:lo - BASE + size] = raw[offset:offset + size]
    syms = subprocess.check_output(["nm", "-S", elf], text=True)
    heap = [(int(addr, 16), int(size, 16)) for addr, size in re.findall(
        r"^([0-9a-f]+) ([0-9a-f]+) [bB] \S*4arch4HEAP$", syms, re.M)]
    if len(heap) != 1 or heap[0][1] != 16 << 20 or not any(lo <= heap[0][0] and heap[0][0] + heap[0][1] <= lo + mem
                                                           for lo, mem, _, _, _ in segments):
        raise ValueError("Missing full static heap in load footprint")
    return entry, end, flat, heap[0], segments


def make(iso, elf, measure, work, source, *, usb_vm=False, details=False):
    if digest(iso) != PUBLIC_ISO:
        raise ValueError("Unexpected demo source ISO")
    entry, top, flat, heap, segments = geometry(elf)
    pin = subprocess.check_output(["python3", measure, elf], text=True).strip()
    if not re.fullmatch(r"[0-9a-f]{64}", pin):
        raise ValueError("Missing kernel measurement")
    (work / "kernel.bin").write_bytes(flat)
    definitions = {"IMAGE_BASE": BASE, "KERNEL_ENTRY": entry, "KERNEL_FILE_BYTES": len(flat)}
    names = [("kernel", "kernel.bin")]
    modules = {}
    for filename, symbol, expected in MEMBERS:
        dest = work / filename
        subprocess.run(["xorriso", "-osirrox", "on", "-indev", iso,
                       "-extract", "/" + filename, dest], check=True)
        if digest(dest) != expected:
            raise ValueError("Module digest mismatch")
        size = dest.stat().st_size
        definitions.update({symbol.upper() + "_BASE": top, symbol.upper() + "_SIZE": size})
        modules[filename] = [top, top + size]
        names.append((symbol, filename))
        top = round_page(top + size)
    definitions.update(INFO_BASE=top, INFO_SIZE=32 * 1024, ARENA_END=top + 32 * 1024 + 16 * 1024)
    # The original optical register check reads 32 numbered rows at 1024×768.
    command = f"image={pin}" + (" desktop=1024x768" if details else "")
    (work / "image.h").write_text("".join(f"#define {k} {v}ULL\n" for k, v in definitions.items())
                                  + f'#define GRANT "{command}"\n')
    (work / "payload.S").write_text('.section .rodata\n' + ''.join(
        f'.global {s}_bytes\n{s}_bytes:\n.incbin "{file}"\n' for s, file in names) + '.section .note.GNU-stack,"",@progbits\n')

    def run(*cmd):
        subprocess.run(cmd, cwd=work, check=True)
    run("gcc", "-Wall", "-Wextra", "-Werror", "-DEFI_FUNCTION_WRAPPER", "-fpic", "-fshort-wchar", "-fno-stack-protector",
        "-mno-red-zone", *(["-DPCP_USB_VM"] if usb_vm else []), "-I/usr/include/efi", "-I/usr/include/efi/x86_64", "-I.", "-c", source / "loader.c", "-o", "loader.o")
    run("gcc", "-c", source / "handoff.S", "-o", "handoff.o")
    run("gcc", "-c", "payload.S", "-o", "payload.o")
    run("ld", "-nostdlib", "-znocombreloc", "-T", "/usr/lib/elf_x86_64_efi.lds", "-shared", "-Bsymbolic",
        "/usr/lib/crt0-efi-x86_64.o", "loader.o", "handoff.o", "payload.o", "-L/usr/lib", "-lefi", "-lgnuefi", "-o", "loader.so")
    run("objcopy", "-j", ".text", "-j", ".rodata", "-j", ".sdata", "-j", ".data", "-j", ".dynamic", "-j", ".dynsym", "-j", ".rel",
        "-j", ".rela", "-j", ".reloc", "--target=efi-app-x86_64", "loader.so", "BOOTX64.EFI")
    efi = (work / "BOOTX64.EFI").read_bytes()
    if any((work / name).read_bytes() not in efi for _, name in names):
        raise ValueError("The EFI application did not retain every embedded payload")
    esp = work / "usb.img" if usb_vm else work / "cd" / "efi.img"
    esp.parent.mkdir(exist_ok=True)
    with esp.open("wb") as disk:
        disk.truncate(8 << 20)
    run("mkfs.vfat", esp)
    run("mmd", "-i", esp, "::/EFI", "::/EFI/BOOT")
    run("mcopy", "-i", esp, "BOOTX64.EFI", "::/EFI/BOOT/")
    if not usb_vm:
        run("xorriso", "-as", "mkisofs", "-quiet", "-eltorito-alt-boot",
            "-e", "efi.img", "-no-emul-boot", "-o", "owned.iso", esp.parent)
    receipt = {"elf_sha256": digest(elf), "measurement": pin, "heap": heap, "segments": segments, "modules": modules,
            "arena": [BASE, definitions["ARENA_END"]], "source_iso": PUBLIC_ISO, "efi_sha256": digest(work / "BOOTX64.EFI")}
    if usb_vm:
        receipt["usb_sha256"] = digest(esp)
    else:
        receipt["iso_sha256"] = digest(work / "owned.iso")
    return receipt
