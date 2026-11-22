#!/usr/bin/env python3
"""Independent, bounded ELF64 x86_64 source-build layout audit.

This does not execute the guest, fix the loader, claim ABI conformance or
measure network reconnect. It compares the generated guest's bytes with the
kernel loader's explicit ET_EXEC/ET_DYN + PT_LOAD + no-interpreter policy.
ELF section relocation counts describe metadata, not proof that an entry
will execute correctly: some images relocate themselves at their own start.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ELF_HEADER = "<16sHHIQQQIHHHHHH"
PROGRAM_HEADER = "<IIQQQQQQ"
SECTION_HEADER = "<IIQQQQIIQQ"


def parse(path):
    b = path.read_bytes()
    if len(b) < 64:
        raise ValueError("incomplete ELF64 header")
    header = struct.unpack_from(ELF_HEADER, b)
    ident, kind, machine, version, entry, phoff, shoff, flags, ehsize, phentsize, phnum, shentsize, shnum, shstrndx = header
    if ident[:4] != b"\x7fELF" or ident[4] != 2 or ident[5] != 1:
        raise ValueError("not little-endian ELF64")
    if machine != 62 or kind not in (2, 3):
        raise ValueError("not an ELF64 x86_64 executable/dynamic image")
    if ehsize != 64 or phentsize != 56 or not 0 < phnum <= 64:
        raise ValueError("unexpected ELF/program-header size")
    if phoff + phnum * phentsize > len(b):
        raise ValueError("program headers truncated")
    programs = []
    for i in range(phnum):
        t, fl, off, va, pa, filesz, memsz, align = struct.unpack_from(
            PROGRAM_HEADER, b, phoff + i * phentsize
        )
        if t == 1 and (filesz > memsz or off + filesz > len(b)):
            raise ValueError("PT_LOAD not contained within file")
        programs.append({
            "type": t, "flags": fl, "file_offset": off, "virtual_address": va,
            "file_size": filesz, "memory_size": memsz, "alignment": align,
        })
    sections = []
    if shoff and shnum and shentsize == 64 and shoff + shentsize * shnum <= len(b):
        for i in range(shnum):
            name, t, fl, va, off, size, link, info, align, entsize = struct.unpack_from(
                SECTION_HEADER, b, shoff + i * shentsize
            )
            sections.append({
                "index": i, "name_offset": name, "type": t, "address": va,
                "offset": off, "size": size, "entries": (size // entsize if entsize else None),
                "entry_size": entsize,
            })
        if shstrndx < len(sections):
            st = sections[shstrndx]
            names = b[st["offset"]:st["offset"] + st["size"]]
            for section in sections:
                pos = section["name_offset"]
                if pos < len(names):
                    section["name"] = names[pos:].split(b"\0", 1)[0].decode("ascii", "replace")
    relocations = [
        {"section": x.get("name", "(unnamed)"), "entries": x["entries"],
         "type": x["type"]}
        for x in sections if x["type"] in (4, 9)  # SHT_RELA / SHT_REL
    ]
    # Do not assume section headers are even retained by an executable.
    return {
        "file": path.name, "bytes": len(b),
        "sha256": hashlib.sha256(b).hexdigest(),
        "kind": "ET_EXEC" if kind == 2 else "ET_DYN",
        "entry_vaddr": entry,
        "phnum": phnum,
        "programs": programs,
        "has_pt_interp": any(p["type"] == 3 for p in programs),
        "has_pt_dynamic": any(p["type"] == 2 for p in programs),
        "executable_program_headers": sum(p["type"] == 1 and p["flags"] & 1 != 0 for p in programs),
        "relocation_sections": relocations,
        "has_unreadable_reloc_sections": any(x["entries"] is None for x in relocations),
        "loader_policy": "ET_EXEC/ET_DYN, static no PT_INTERP, PT_LOAD W^X; entry must be executable",
        "loader_accepted_by_inspection_only": (
            not any(p["type"] == 3 for p in programs)
            and all(not (p["type"] == 1 and p["flags"] & 3 == 3) for p in programs)
            and any(p["type"] == 1 and p["flags"] & 1 != 0 for p in programs)
        ),
        "executed": False,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--ethernet", type=Path, required=True)
    p.add_argument("--serial", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    result = {
        "scope": "observational ELF-file structure, not running guest",
        "negative_control": "a readable header cannot establish runtime success",
        "ethernet": parse(args.ethernet),
        "serial": parse(args.serial),
        "result": "LAYOUT_DESCRIBED_NOT_QUALIFIED",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({
        k: {"kind": result[k]["kind"], "sha256": result[k]["sha256"],
            "has_pt_interp": result[k]["has_pt_interp"],
            "has_pt_dynamic": result[k]["has_pt_dynamic"],
            "relocation_sections": result[k]["relocation_sections"]}
        for k in ("ethernet", "serial")
    }, sort_keys=True))


if __name__ == "__main__":
    main()
