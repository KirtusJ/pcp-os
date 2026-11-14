#!/usr/bin/env python3
"""The measurement the PCP OS kernel takes of itself, computed from the image file without running it.

    python3 tools/measure.py <kernel image: the ELF file run.sh leaves at target/pcpos-kernel.elf32, or the 64-bit one>

The kernel measures its code and read-only data as they are in memory: the sections .text and .rodata, each padded
to a whole page, one after the other (linker.ld). Those bytes are in the file. Their SHA-256 is printed; give it to
the kernel as `image=<digest>` on the loader's command line and the kernel compares its own measurement with it,
verifies only if they are equal, and starts no program otherwise (requirement K-12).

This is a value the kernel did not compute. It is not a signature: whoever writes the command line chooses it.
"""
import hashlib, struct, sys

def measure(raw):
    if raw[:4] != b"\x7fELF":
        raise SystemExit("not an ELF file")
    wide = raw[4] == 2
    if wide:
        shoff, = struct.unpack_from("<Q", raw, 0x28); shentsize, shnum, shstrndx = struct.unpack_from("<HHH", raw, 0x3A)
        def section(i):
            name, _type, _flags, addr, off, size = struct.unpack_from("<IIQQQQ", raw, shoff + i * shentsize)
            return name, addr, off, size
    else:
        shoff, = struct.unpack_from("<I", raw, 0x20); shentsize, shnum, shstrndx = struct.unpack_from("<HHH", raw, 0x2E)
        def section(i):
            name, _type, _flags, addr, off, size = struct.unpack_from("<IIIIII", raw, shoff + i * shentsize)
            return name, addr, off, size
    _, _, stroff, strsize = section(shstrndx)
    names = raw[stroff:stroff + strsize]
    found = {}
    for i in range(shnum):
        name, addr, off, size = section(i)
        found[names[name:names.index(b"\0", name)].decode()] = (addr, off, size)
    text, ro = found.get(".text"), found.get(".rodata")
    if not text or not ro:
        raise SystemExit("the image has no .text or no .rodata section")
    if text[0] % 4096 or ro[0] != text[0] + text[2] or (ro[0] + ro[2]) % 4096:
        raise SystemExit("the sections are not laid out as linker.ld lays them out: .text at a page, .rodata straight after it, each a whole number of pages")
    return hashlib.sha256(raw[text[1]:text[1] + text[2]] + raw[ro[1]:ro[1] + ro[2]]).hexdigest()

if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    print(measure(open(sys.argv[1], "rb").read()))
