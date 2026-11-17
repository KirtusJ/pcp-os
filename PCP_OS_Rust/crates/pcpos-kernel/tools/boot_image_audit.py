#!/usr/bin/env python3
"""Read-only evidence check for PCP OS's current Multiboot ELF64 -> ELF32 conversion.

This is a *host-side test harness*, not a kernel component or a security gate.
It cannot establish that QEMU, UEFI, or physical hardware has booted the image.

Usage:
  python3 tools/boot_image_audit.py <linked ELF64 kernel> <QEMU ELF32 wrapper>

For the current run.sh defaults, the files are:
  /tmp/pcpos-kernel-build/crates/pcpos-kernel/target/x86_64-unknown-none/release/pcpos-kernel
  PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32

The test takes both exact output files, prints their SHA-256 identities and refuses
truncated metadata, relocation of load-critical sections, wrong ELF classes,
changed executable section bytes, or a missing/invalid Multiboot 1 header.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

MAX_SIZE = 128 * 1024 * 1024
MAGIC = 0x1BADB002
RELEVANT = (".boot", ".text", ".rodata", ".data", ".bss")


class AuditError(ValueError):
    """An image fails a declared structural check."""


def need(raw, offset, length, label):
    if offset < 0 or length < 0 or offset > len(raw) or length > len(raw) - offset:
        raise AuditError(f"{label}: range outside the image")
    return raw[offset:offset + length]


def number(raw, offset, fmt, label):
    size = struct.calcsize(fmt)
    return struct.unpack(fmt, need(raw, offset, size, label))[0]


def elf(raw, klass, machine):
    """Parse bounded section metadata for the linker/objcopy output used here.

    Extended numbering, ELF compression and nonstandard name tables are refused,
    not guessed. This is a focused layout check, not a general ELF parser.
    """
    if len(raw) < 64 or raw[:4] != b"\x7fELF":
        raise AuditError("not a complete ELF header")
    if raw[4] != klass or raw[5] != 1 or raw[6] != 1:
        raise AuditError("unexpected ELF class, endianness, or version")
    if number(raw, 18, "<H", "e_machine") != machine:
        raise AuditError(f"wrong e_machine (expected {machine})")
    if number(raw, 16, "<H", "e_type") != 2:
        raise AuditError("kernel is not ET_EXEC")
    if klass == 2:
        entry = number(raw, 24, "<Q", "e_entry")
        shoff = number(raw, 40, "<Q", "e_shoff")
        entsize, count, strindex = struct.unpack("<HHH", need(raw, 58, 6, "section counts"))
        wanted = 64
    else:
        entry = number(raw, 24, "<I", "e_entry")
        shoff = number(raw, 32, "<I", "e_shoff")
        entsize, count, strindex = struct.unpack("<HHH", need(raw, 46, 6, "section counts"))
        wanted = 40
    if entsize != wanted or not (2 <= count <= 256) or not (0 < strindex < count):
        raise AuditError("unsupported or malformed section table")
    need(raw, shoff, entsize * count, "section table")
    sections = []
    for i in range(count):
        p = shoff + i * entsize
        if klass == 2:
            name, typ = struct.unpack("<II", need(raw, p, 8, "section fields"))
            addr, offset, size = struct.unpack("<QQQ", need(raw, p + 16, 24, "section bounds"))
        else:
            name, typ, _flags, addr, offset, size = struct.unpack(
                "<IIIIII", need(raw, p, 24, "section fields")
            )
        if size > MAX_SIZE:
            raise AuditError("unreasonable section size")
        if typ != 8:  # SHT_NOBITS (.bss) has no file bytes
            need(raw, offset, size, "section data")
        sections.append((name, typ, addr, offset, size))
    _, typ, _, strings_off, strings_size = sections[strindex]
    if typ != 3:  # SHT_STRTAB
        raise AuditError("section-name table is not a string table")
    names = need(raw, strings_off, strings_size, "section-name strings")
    result = {}
    for nameindex, typ, addr, offset, size in sections:
        if nameindex >= len(names):
            raise AuditError("section name offset out of bounds")
        end = names.find(b"\0", nameindex)
        if end < 0:
            raise AuditError("unterminated section name")
        try:
            name = names[nameindex:end].decode("ascii")
        except UnicodeDecodeError as e:
            raise AuditError("non-ASCII section name") from e
        if name in result:
            raise AuditError(f"duplicate section {name!r}")
        data = None if typ == 8 else raw[offset:offset + size]
        result[name] = (addr, size, typ, data)
    for name in RELEVANT:
        if name not in result:
            raise AuditError(f"missing {name} section")
    # The firmware/Multiboot loader acts on PT_LOAD headers, not the section table.
    # Compare loaded bytes, addresses, lengths and permissions as well as sections.
    if klass == 2:
        phoff, phentsize, phnum = (number(raw, 32, "<Q", "e_phoff"),
                                 number(raw, 54, "<H", "e_phentsize"),
                                 number(raw, 56, "<H", "e_phnum"))
        width = 56
    else:
        phoff, phentsize, phnum = (number(raw, 28, "<I", "e_phoff"),
                                 number(raw, 42, "<H", "e_phentsize"),
                                 number(raw, 44, "<H", "e_phnum"))
        width = 32
    if phentsize != width or not (1 <= phnum <= 64):
        raise AuditError("unsupported or malformed program header table")
    need(raw, phoff, width * phnum, "program headers")
    loads = []
    for i in range(phnum):
        p = phoff + width * i
        if klass == 2:
            typ, flags, off, virt, phys, filesz, memsz, align = struct.unpack(
                "<IIQQQQQQ", need(raw, p, 56, "ELF64 program header"))
        else:
            typ, off, virt, phys, filesz, memsz, flags, align = struct.unpack(
                "<IIIIIIII", need(raw, p, 32, "ELF32 program header"))
        if typ != 1:  # PT_LOAD
            continue
        if (filesz > memsz or memsz == 0 or phys >= (1 << 32)
                or phys + memsz > (1 << 32) or virt >= (1 << 32)
                or virt + memsz > (1 << 32)):
            raise AuditError("PT_LOAD size or address incompatible with legacy low-memory boot")
        if align > 1 and (align & (align - 1) or (virt - off) % align):
            raise AuditError("PT_LOAD alignment invalid")
        data = need(raw, off, filesz, "PT_LOAD file bytes")
        loads.append((virt, phys, filesz, memsz, flags, data))
    if not loads:
        raise AuditError("image has no loadable segments")
    if not any(v <= entry < v + m and (f & 1) for v, _, _, m, f, _ in loads):
        raise AuditError("entry not covered by an executable PT_LOAD")
    if entry > 0xFFFFFFFF:
        raise AuditError("Multiboot trampoline entry exceeds 32 bits")
    addr, length, _, _ = result[".boot"]
    if not (addr <= entry < addr + length) or addr >= (1 << 30):
        raise AuditError("entry outside the low-address .boot trampoline")
    return entry, result, loads


def multiboot(raw):
    """Check the Multiboot 1 first-8192-byte header contract.

    The primary source is the GNU Multiboot Specification 0.6.96, section 3.1.
    It does not specify the EFI boot path.
    """
    matches = []
    end = min(len(raw), 8192)
    for off in range(0, end - 11, 4):
        magic, flags, check = struct.unpack("<III", raw[off:off + 12])
        if magic != MAGIC:
            continue
        if (magic + flags + check) & 0xFFFFFFFF:
            raise AuditError("Multiboot header checksum mismatch")
        if flags != 3:  # current boot.s contract; flags 16 and 2 are not supported
            raise AuditError("Multiboot flags depart from boot.s")
        matches.append(off)
    if len(matches) != 1:
        raise AuditError(f"expected one Multiboot 1 header in the first 8192 bytes, found {len(matches)}")
    return matches[0]


def audit(linked, wrapper):
    if len(linked) > MAX_SIZE or len(wrapper) > MAX_SIZE:
        raise AuditError("image exceeds 128 MiB read-only audit limit")
    source_entry, source_sections, source_loads = elf(linked, 2, 62)
    target_entry, target_sections, target_loads = elf(wrapper, 1, 3)
    if source_entry != target_entry:
        raise AuditError("the ELF32 conversion changed the entry point")
    for name in RELEVANT:
        a, b = source_sections[name], target_sections[name]
        if a != b:
            raise AuditError(f"the ELF32 conversion changed {name} address, size, type or content")
    if source_loads != target_loads:
        raise AuditError("the ELF32 conversion changed PT_LOAD mapping, permissions or bytes")
    header = multiboot(wrapper)
    return {
        "result": "PASS: STRUCTURAL ONLY (not a boot, signature or authorization)",
        "linked_elf64_sha256": hashlib.sha256(linked).hexdigest(),
        "wrapper_elf32_sha256": hashlib.sha256(wrapper).hexdigest(),
        "entry": f"0x{source_entry:x}",
        "multiboot_header_file_offset": header,
        "compared_sections": list(RELEVANT),
        "compared_load_segments": len(source_loads),
    }


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("linked_elf64", type=Path)
    p.add_argument("wrapper_elf32", type=Path)
    args = p.parse_args(argv)
    try:
        for path in (args.linked_elf64, args.wrapper_elf32):
            if path.stat().st_size > MAX_SIZE:
                raise AuditError(f"{path}: too large")
        finding = audit(args.linked_elf64.read_bytes(), args.wrapper_elf32.read_bytes())
    except (AuditError, OSError) as e:
        print(f"FAIL: {e}", file=sys.stderr)
        return 1
    print(json.dumps(finding, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
